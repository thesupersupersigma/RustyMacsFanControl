//! The fan controller: fan state, the periodic sensor/RPM refresh, and the control loop
//! that writes fan speeds according to the active preset.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::fan::mode::Mode;
use crate::fan::names;
use crate::fan::profile::{Predefined, Profile, ProfileList};
use crate::fan::sensor_control::SensorControl;
use crate::model::Model;
use crate::sensors::Provider;
use crate::settings::Store;
use crate::smc::service::{FanKey, Service};

/// Refresh and control loop period.
const LOOP_PERIOD: Duration = Duration::from_millis(1500);
/// A constant-speed fan is rewritten when it drifts further than this from its target.
const RPM_TOLERANCE: i32 = 30;
/// The forced-mode mask is re-asserted at most this often while speeds are written.
const MODES_REAPPLY: Duration = Duration::from_millis(3000);
const WAKE_WORKAROUND_INTERVAL: Duration = Duration::from_millis(10000);
/// RPM value meaning "never read".
pub const NO_RPM: u16 = 0xFFFF;

pub struct Fan {
    pub name: String,
    pub current: u16,
    pub min: u16,
    pub max: u16,
    control: SensorControl,
}

fn wall_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Everything the loops and the UI share, guarded by one lock.
pub struct Core {
    pub model: Model,
    service: Arc<Service>,
    pub fans: Vec<Fan>,
    pub sensors: Provider,
    pub profiles: ProfileList,
    pub sleep_mode: bool,
    pub user_logged_out: bool,
    last_modes_apply: Option<Instant>,
    last_wake: Option<Instant>,
}

impl Core {
    pub fn load(service: Arc<Service>, model: Model, store: &dyn Store) -> Core {
        let mut fans = Vec::new();
        if !model.is_fanless() {
            let count = service.fan_count();
            crate::info!("Fans count - {count}");
            if count == 0 {
                crate::error!("No fans detected!");
            }
            let table = names::for_model(&model);
            let now = wall_seconds();
            for index in 0..count {
                let name = match table.get(index) {
                    Some(name) if !name.is_empty() => name.clone(),
                    _ => service.fan_name(index),
                };
                crate::info!("Fan {index}: {name}");
                let read = |kind| service.fan_rpm(index, kind).unwrap_or(NO_RPM);
                fans.push(Fan {
                    control: SensorControl::new(&name, now),
                    name,
                    current: read(FanKey::Actual),
                    min: read(FanKey::Minimum),
                    max: read(FanKey::Maximum),
                });
            }
        }
        let max_rpms: Vec<u16> = fans.iter().map(|fan| fan.max).collect();
        let mut core = Core {
            sensors: Provider::load(&service, &model),
            profiles: ProfileList::new(&max_rpms),
            model,
            service,
            fans,
            sleep_mode: false,
            user_logged_out: false,
            last_modes_apply: None,
            last_wake: None,
        };
        if core.fans.is_empty() {
            crate::warn!("initialLoadThread - no fans");
            return core;
        }
        core.release_fans_forced_elsewhere();
        let sensors = &core.sensors;
        core.profiles.load(store, &|key| sensors.find(key).is_some());
        let profile = core.profiles.current_profile();
        core.apply_profile(&profile, false);
        core
    }

    /// Fans left in forced mode by another program are handed back to the SMC.
    fn release_fans_forced_elsewhere(&mut self) {
        match self.service.fan_control_modes() {
            None => crate::error!("Unable to query fans control mode!"),
            Some(modes) => {
                for (fan, _) in modes.iter().enumerate().filter(|(_, forced)| **forced) {
                    crate::warn!("Fan {fan} mode is set by another program");
                }
                if modes.contains(&true) {
                    let automatic = self.profiles.predefined(Predefined::Automatic).clone();
                    self.apply_profile(&automatic, false);
                }
            }
        }
    }

    /// Sets which fans are forced. Constant speeds are written here only on request;
    /// normally the control loop writes them.
    pub fn apply_profile(&mut self, profile: &Profile, write_constants: bool) {
        crate::info!("Apply profile: {}", profile.name);
        let forced: Vec<bool> = (0..self.fans.len())
            .map(|fan| profile.modes.get(fan).is_none_or(|mode| *mode != Mode::Automatic))
            .collect();
        self.service.set_fan_control_modes(&forced);
        if write_constants {
            for (fan, mode) in profile.modes.iter().enumerate() {
                if let Mode::Constant(rpm) = mode {
                    self.service.set_fan_rpm(fan, FanKey::Target, clamp_rpm(*rpm));
                }
            }
        }
    }

    fn set_fan_speed(&mut self, fan: usize, rpm: i32, now: Instant) {
        if self.last_modes_apply.is_none_or(|last| now.duration_since(last) > MODES_REAPPLY) {
            let profile = self.profiles.current_profile();
            self.apply_profile(&profile, false);
            self.last_modes_apply = Some(now);
        }
        self.service.set_fan_rpm(fan, FanKey::Target, clamp_rpm(rpm));
    }

    fn mode_of(&self, fan: usize) -> Mode {
        let profile = self.profiles.current_profile();
        if profile.is_valid(self.fans.len()) {
            profile.modes[fan].clone()
        } else {
            Mode::Invalid
        }
    }

    /// One pass of the control loop over all fans.
    pub fn control_pass(&mut self, store: &dyn Store, now: Instant, wall: i64) {
        for fan in 0..self.fans.len() {
            if self.sleep_mode {
                break;
            }
            let mode = self.mode_of(fan);
            let current = self.fans[fan].current as i32;
            match &mode {
                Mode::Constant(target) => {
                    let changed = self.profiles.changed;
                    if (current - target).abs() > RPM_TOLERANCE || changed {
                        self.set_fan_speed(fan, *target, now);
                        if changed {
                            self.profiles.changed = false;
                        }
                    }
                }
                Mode::Sensor { key, .. } => match self.sensors.find(key) {
                    None => {
                        // The sensor went away: hand this fan back to the SMC.
                        self.fans[fan].control.reset();
                        crate::info!(
                            "ControlFanSpeedThread: reset fan mode for {}",
                            self.fans[fan].name
                        );
                        let mut profile = self.profiles.current_profile();
                        profile.modes[fan] = Mode::Automatic;
                        self.set_profile(profile, store);
                    }
                    Some(sensor) => {
                        let temperature = self.sensors.sensors()[sensor].value;
                        if temperature != 0.0 {
                            let (min, max) = (self.fans[fan].min as i32, self.fans[fan].max as i32);
                            let rpm = self.fans[fan]
                                .control
                                .update_for_mode(current, min, max, temperature, &mode, wall);
                            if let Some(rpm) = rpm {
                                self.set_fan_speed(fan, rpm, now);
                            }
                        }
                    }
                },
                Mode::Automatic | Mode::Invalid => {}
            }
        }
    }

    /// Makes `profile` the active preset and applies it.
    pub fn set_profile(&mut self, profile: Profile, store: &dyn Store) {
        self.profiles.set_current(profile, store);
        let profile = self.profiles.current_profile();
        self.apply_profile(&profile, false);
    }

    /// Changes one fan's mode, optionally saving the result into the preset `save_as`.
    pub fn set_fan_mode(&mut self, fan: usize, mode: Mode, save_as: Option<&str>, store: &dyn Store) {
        let mut profile = self.profiles.current_profile();
        profile.modes[fan] = mode;
        self.profiles.set_current(profile, store);
        if let Some(name) = save_as {
            self.profiles.save_current_as(name, store);
        }
        let profile = self.profiles.current_profile();
        self.apply_profile(&profile, false);
    }

    pub fn delete_profile(&mut self, profile: &Profile, store: &dyn Store) {
        let before = self.profiles.current_profile();
        self.profiles.delete(profile, store);
        let after = self.profiles.current_profile();
        if before != after {
            self.apply_profile(&after, false);
        }
    }

    /// What to run while the machine sleeps: sensor-driven fans (every fan on some
    /// iMacs) are pinned to their minimum, the rest go back to the SMC.
    fn sleep_profile(&self) -> Profile {
        let mut profile = self.profiles.predefined(Predefined::Automatic).clone();
        if self.model.skips_sleep_profile() {
            return profile;
        }
        let force_minimum = self.model.is_legacy_imac();
        for fan in 0..self.fans.len() {
            if matches!(self.mode_of(fan), Mode::Sensor { .. }) || force_minimum {
                crate::info!(
                    "getProfileToApplyForSleepMode: set min rpm for the {} fan",
                    self.fans[fan].name
                );
                profile.modes[fan] = Mode::Constant(self.fans[fan].min as i32);
            }
        }
        profile
    }

    /// Applies the sleep profile; returns the profile to restore on wake, if it differs.
    fn enter_sleep(&mut self) -> Option<Profile> {
        crate::info!("ControlFanSpeedThread: sleep mode");
        for fan in &mut self.fans {
            fan.control.reset();
        }
        let current = self.profiles.current_profile();
        let sleep = self.sleep_profile();
        let changed = current != sleep;
        crate::info!("ControlFanSpeedThread: changed = {changed}");
        if changed {
            self.apply_profile(&sleep, true);
            Some(current)
        } else {
            None
        }
    }

    /// Returns every fan to automatic control, as on exit.
    pub fn restore_default_control(&mut self) {
        if self.fans.is_empty() {
            return;
        }
        if self.user_logged_out && self.model.is_legacy_imac() {
            crate::info!("restore default control? no");
            return;
        }
        crate::info!("restore default control? yes");
        let automatic = self.profiles.predefined(Predefined::Automatic).clone();
        self.apply_profile(&automatic, false);
    }

    fn wake_smc(&mut self, now: Instant) {
        if self.last_wake.is_some_and(|last| now.duration_since(last) <= WAKE_WORKAROUND_INTERVAL) {
            return;
        }
        crate::info!("Waking SMC system");
        self.service.set_fan_control_modes(&vec![true; self.fans.len()]);
        thread::sleep(Duration::from_millis(500));
        self.service.set_fan_control_modes(&vec![false; self.fans.len()]);
        self.last_wake = Some(now);
    }

    /// One pass of the refresh loop: current fan speeds, then temperatures. With the
    /// window hidden only the tray sensor and sensors driving fans are read.
    pub fn refresh(&mut self, window_visible: bool, tray_sensor: &str, on_battery: bool) {
        let mut needed: Vec<String> = Vec::new();
        for fan in 0..self.fans.len() {
            if let Some(rpm) = self.service.fan_rpm(fan, FanKey::Actual) {
                self.fans[fan].current = rpm;
            }
            if !window_visible {
                if let Mode::Sensor { key, .. } = self.mode_of(fan) {
                    needed.push(key);
                }
            }
        }
        if self.model.needs_wake_workaround() && self.profiles.is_automatic() && on_battery {
            self.wake_smc(Instant::now());
        }
        for index in self.sensors.visible() {
            let key = &self.sensors.sensors()[index].key;
            if window_visible || key == tray_sensor || needed.contains(key) {
                self.sensors.update(index, &self.service);
            }
        }
    }
}

fn clamp_rpm(rpm: i32) -> u16 {
    rpm.clamp(0, u16::MAX as i32) as u16
}

/// A flag threads can wait on with a timeout.
#[derive(Default)]
struct Event {
    set: Mutex<bool>,
    signal: Condvar,
}

impl Event {
    fn set(&self) {
        *self.set.lock().unwrap() = true;
        self.signal.notify_all();
    }

    fn reset(&self) {
        *self.set.lock().unwrap() = false;
    }

    fn is_set(&self) -> bool {
        *self.set.lock().unwrap()
    }

    /// Waits up to `timeout`; returns whether the event is set.
    fn wait(&self, timeout: Duration) -> bool {
        let guard = self.set.lock().unwrap();
        *self.signal.wait_timeout_while(guard, timeout, |set| !*set).unwrap().0
    }
}

/// What the refresh loop needs to know about the UI each pass.
pub trait Host: Send + Sync + 'static {
    fn window_visible(&self) -> bool;
    fn on_battery(&self) -> bool;
    /// Called after each refresh pass, from the refresh thread.
    fn refreshed(&self);
}

pub struct Engine {
    core: Arc<Mutex<Core>>,
    store: Arc<dyn Store + Send + Sync>,
    stop: Arc<Event>,
    wake: Arc<Event>,
    threads: Vec<JoinHandle<()>>,
}

impl Engine {
    pub fn new(core: Core, store: Arc<dyn Store + Send + Sync>) -> Engine {
        Engine {
            core: Arc::new(Mutex::new(core)),
            store,
            stop: Arc::default(),
            wake: Arc::default(),
            threads: Vec::new(),
        }
    }

    pub fn core(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap()
    }

    pub fn store(&self) -> &(dyn Store + Send + Sync) {
        &*self.store
    }

    pub fn start(&mut self, host: Arc<dyn Host>) {
        let (core, store, stop) = (self.core.clone(), self.store.clone(), self.stop.clone());
        self.threads.push(thread::spawn(move || {
            let mut first = true;
            loop {
                let visible = first || host.window_visible();
                let tray_sensor = store.get_string("traySensor").unwrap_or_default();
                core.lock().unwrap().refresh(visible, &tray_sensor, host.on_battery());
                host.refreshed();
                first = false;
                if stop.wait(LOOP_PERIOD) {
                    return;
                }
            }
        }));

        let (core, store, stop, wake) =
            (self.core.clone(), self.store.clone(), self.stop.clone(), self.wake.clone());
        self.threads.push(thread::spawn(move || {
            loop {
                core.lock().unwrap().control_pass(&*store, Instant::now(), wall_seconds());
                if core.lock().unwrap().sleep_mode {
                    let restore = core.lock().unwrap().enter_sleep();
                    while !wake.wait(Duration::from_millis(200)) {
                        if stop.is_set() {
                            break;
                        }
                    }
                    crate::info!("_wakeupEvent triggered");
                    wake.reset();
                    if let Some(profile) = restore {
                        crate::info!("Restore previous profile");
                        core.lock().unwrap().apply_profile(&profile, false);
                    }
                }
                if stop.wait(LOOP_PERIOD) {
                    break;
                }
            }
            core.lock().unwrap().restore_default_control();
        }));
    }

    pub fn entering_sleep(&self) {
        crate::info!("slotEnteringSleepMode");
        self.core().sleep_mode = true;
    }

    pub fn woke_up(&self) {
        crate::info!("slotWakeUp");
        self.core().sleep_mode = false;
        self.wake.set();
    }

    pub fn user_logged_out(&self) {
        crate::info!("slotUserLoggedOut");
        self.core().user_logged_out = true;
    }

    /// Stops both loops; the control loop returns the fans to automatic on its way out.
    pub fn stop(&mut self) {
        self.stop.set();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fan::profile::Current;
    use crate::settings::MemoryStore;
    use crate::smc::service::tests::t2_laptop;

    fn loaded() -> (crate::smc::service::tests::FakeSmc, Core, MemoryStore) {
        let (smc, service) = t2_laptop();
        let store = MemoryStore::default();
        let core = Core::load(Arc::new(service), Model::parse("MacBookPro16,1"), &store);
        (smc, core, store)
    }

    fn target(smc: &crate::smc::service::tests::FakeSmc, fan: usize) -> f32 {
        f32::from_le_bytes(smc.get(&format!("F{fan}Tg")).try_into().unwrap())
    }

    #[test]
    fn loads_fans_with_table_names_and_limits() {
        let (_, core, store) = loaded();
        let names: Vec<&str> = core.fans.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Left side", "Right side"]);
        assert_eq!((core.fans[0].min, core.fans[0].current, core.fans[0].max), (1836, 1850, 5927));
        assert!(core.profiles.is_automatic());
        assert_eq!(store.get_string("ActivePreset").as_deref(), Some("Predefined:0"));
    }

    #[test]
    fn fans_forced_by_another_program_are_released_at_startup() {
        let (smc, service) = t2_laptop();
        smc.set("F1Md", "ui8 ", &[1]);
        let store = MemoryStore::default();
        Core::load(Arc::new(service), Model::parse("MacBookPro16,1"), &store);
        assert_eq!(smc.get("F1Md"), vec![0]);
    }

    #[test]
    fn constant_mode_forces_the_fan_and_writes_the_target_once() {
        let (smc, mut core, store) = loaded();
        core.set_fan_mode(1, Mode::Constant(3000), None, &store);
        assert_eq!((smc.get("F0Md"), smc.get("F1Md")), (vec![0], vec![1]));
        assert!(matches!(core.profiles.current(), Current::Unsaved(_)));

        let now = Instant::now();
        core.control_pass(&store, now, 0);
        assert_eq!(target(&smc, 1), 3000.0);
        assert_eq!(target(&smc, 0), 1836.0);

        // once the fan is within tolerance nothing more is written
        core.fans[1].current = 2990;
        smc.set("F1Tg", "flt ", &0f32.to_le_bytes());
        core.control_pass(&store, now, 2);
        assert_eq!(target(&smc, 1), 0.0);
        // drifting away triggers a rewrite
        core.fans[1].current = 2500;
        core.control_pass(&store, now, 4);
        assert_eq!(target(&smc, 1), 3000.0);
    }

    #[test]
    fn sensor_mode_follows_the_temperature_and_resets_when_the_sensor_vanishes() {
        let (smc, mut core, store) = loaded();
        let sensor = Mode::Sensor { key: "TC0P".into(), start: 40, max: 80 };
        core.set_fan_mode(0, sensor, None, &store);
        core.refresh(true, "", false);
        assert_eq!(core.sensors.sensors()[core.sensors.find("TC0P").unwrap()].value, 48.25);
        core.control_pass(&store, Instant::now(), 0);
        // step = (5927-1836)/40 = 102; 48 °C band starts at (48-40-1)*102+1836 = 2550
        assert_eq!(target(&smc, 0), 2551.0);
        assert_eq!(smc.get("F0Md"), vec![1]);

        core.set_fan_mode(0, Mode::Sensor { key: "TZZZ".into(), start: 40, max: 80 }, None, &store);
        core.control_pass(&store, Instant::now(), 2);
        assert!(core.profiles.is_automatic());
        assert_eq!(smc.get("F0Md"), vec![0]);
    }

    #[test]
    fn sleep_pins_sensor_fans_to_minimum_and_exit_restores_automatic() {
        let (smc, mut core, store) = loaded();
        core.set_fan_mode(0, Mode::Sensor { key: "TC0P".into(), start: 40, max: 80 }, None, &store);
        core.set_fan_mode(1, Mode::Constant(3000), None, &store);
        let restore = core.enter_sleep().unwrap();
        assert_eq!(target(&smc, 0), 1836.0);
        assert_eq!((smc.get("F0Md"), smc.get("F1Md")), (vec![1], vec![0]));
        core.apply_profile(&restore, false);
        assert_eq!((smc.get("F0Md"), smc.get("F1Md")), (vec![1], vec![1]));

        core.restore_default_control();
        assert_eq!((smc.get("F0Md"), smc.get("F1Md")), (vec![0], vec![0]));
    }
}
