//! Temperature sensors shown in the sensor list and usable for sensor-based fan control.

pub mod defs;

use std::time::Instant;

use crate::model::Model;
use crate::smc::platform::{key, key_text};
use crate::smc::service::Service;
use defs::{smc_key_match, Group, Kind, SensorDef};

#[derive(Clone, Debug)]
enum Source {
    Smc,
    /// Mean of the sensors at these positions in the provider's list.
    Average(Vec<usize>),
}

#[derive(Clone, Debug)]
pub struct Sensor {
    /// Identifier stored in presets and settings: the SMC key as the SMC spells it, or
    /// a group key for averages.
    pub key: String,
    pub name: String,
    pub kind: Kind,
    /// Not listed, but still readable (members of a hiding group, covered sensors).
    pub hidden: bool,
    /// Degrees Celsius; 0 until first read.
    pub value: f64,
    pub last_update: Option<Instant>,
    group: Option<Group>,
    source: Source,
}

impl Sensor {
    /// Default "start increasing at" temperature offered for sensor-based control.
    pub fn suggested_min_temp(&self) -> i32 {
        (self.value.round() as i32 - 5).max(20)
    }

    /// Default maximum temperature offered for sensor-based control.
    pub fn suggested_max_temp(&self) -> i32 {
        if self.kind == Kind::Cpu {
            90
        } else {
            (self.value.round() as i32 + 5).min(100)
        }
    }

    /// Whether a stored identifier refers to this sensor.
    pub fn matches_key(&self, wanted: &str) -> bool {
        match self.source {
            Source::Smc => smc_key_match(&self.key, wanted),
            Source::Average(_) => self.key == wanted,
        }
    }
}

/// CPU keys where the uppercase spelling wins when the SMC lists both cases.
const PREFER_UPPERCASE: [&str; 5] = ["TH0A", "TH1A", "TH1B", "TG0D", "TG1D"];

/// Of two keys differing only in the case of the last character, whether to keep `a`.
fn keep_first(a: &str, b: &str) -> bool {
    let last = |key: &str| key.as_bytes()[3];
    if PREFER_UPPERCASE.iter().any(|special| a.eq_ignore_ascii_case(special)) {
        if last(a).is_ascii_uppercase() {
            return true;
        }
        if last(b).is_ascii_uppercase() {
            return false;
        }
    }
    last(a).is_ascii_lowercase()
}

/// Sort key: display name, disks after everything else, and two-digit performance
/// cores after single-digit ones.
fn sort_name(sensor: &Sensor) -> Vec<u16> {
    let mut name = sensor.name.clone();
    if sensor.kind.is_disk() {
        name.insert_str(0, "zzz");
    }
    if sensor.kind == Kind::Cpu && name.contains("Performance") {
        let chars: Vec<char> = name.chars().collect();
        if chars.len() >= 2 {
            let tail: String = chars[chars.len() - 2..].iter().collect();
            if tail.trim().parse::<i32>().is_ok_and(|n| n >= 10) {
                let head: String = chars[..chars.len() - 2].iter().collect();
                name = format!("{head}9{tail}");
            }
        }
    }
    name.encode_utf16().collect()
}

pub struct Provider {
    sensors: Vec<Sensor>,
}

impl Provider {
    pub fn load(service: &Service, model: &Model) -> Provider {
        let mut provider = Provider { sensors: load_smc_sensors(service, &defs::load(model)) };
        provider.build_averages();
        crate::info!(
            "InitialLoadThread: number of supported SMC sensors: {}",
            provider.sensors.len()
        );
        provider
    }

    fn build_averages(&mut self) {
        let mut groups: Vec<(Group, Vec<usize>)> = Vec::new();
        for (index, sensor) in self.sensors.iter_mut().enumerate() {
            let Some(group) = sensor.group.clone() else {
                continue;
            };
            if group.hide_members {
                sensor.hidden = true;
            }
            match groups.iter_mut().find(|(existing, _)| existing.key == group.key) {
                Some((_, members)) => members.push(index),
                None => groups.push((group, vec![index])),
            }
        }
        for (group, members) in groups {
            self.sensors.push(Sensor {
                key: group.key,
                name: group.name,
                kind: self.sensors[members[0]].kind,
                hidden: false,
                value: 0.0,
                last_update: None,
                group: None,
                source: Source::Average(members),
            });
        }
    }

    pub fn sensors(&self) -> &[Sensor] {
        &self.sensors
    }

    /// Positions of the listed sensors in display order.
    pub fn visible(&self) -> Vec<usize> {
        let mut order: Vec<usize> =
            (0..self.sensors.len()).filter(|&i| !self.sensors[i].hidden).collect();
        order.sort_by_cached_key(|&i| sort_name(&self.sensors[i]));
        order
    }

    /// The listed sensor a stored identifier refers to.
    pub fn find(&self, wanted: &str) -> Option<usize> {
        self.visible().into_iter().find(|&i| self.sensors[i].matches_key(wanted))
    }

    /// Reads one sensor; a failed read keeps the previous value.
    pub fn update(&mut self, index: usize, service: &Service) {
        match self.sensors[index].source.clone() {
            Source::Smc => {
                if let Some(value) = service.temperature(&key(&self.sensors[index].key)) {
                    self.sensors[index].value = value;
                }
            }
            Source::Average(members) => {
                // Members that failed to read still count, with their stale value.
                for &member in &members {
                    self.update(member, service);
                }
                let sum: f64 = members.iter().map(|&m| self.sensors[m].value).sum();
                self.sensors[index].value = sum / members.len() as f64;
            }
        }
        self.sensors[index].last_update = Some(Instant::now());
    }
}

fn load_smc_sensors(service: &Service, defs: &[SensorDef]) -> Vec<Sensor> {
    let keys: Vec<String> = service.temperature_keys().iter().map(key_text).collect();
    // Where the SMC lists both case variants of a key, only one becomes a sensor.
    let mut excluded: Vec<&str> = Vec::new();
    for a in &keys {
        for b in &keys {
            if a != b
                && smc_key_match(a, b)
                && !excluded.contains(&a.as_str())
                && !excluded.contains(&b.as_str())
            {
                excluded.push(if keep_first(a, b) { b } else { a });
                break;
            }
        }
    }
    let mut sensors = Vec::new();
    for smc_key in &keys {
        if excluded.contains(&smc_key.as_str()) {
            continue;
        }
        // Keys the table does not name are not shown.
        let Some(def) = defs.iter().find(|def| smc_key_match(&def.key, smc_key)) else {
            continue;
        };
        if service.temperature(&key(smc_key)).is_none() {
            crate::info!("FilterSupportedTempSensors - skipping sensor {smc_key}");
            continue;
        }
        sensors.push(Sensor {
            key: smc_key.clone(),
            name: def.name.clone(),
            kind: def.kind,
            hidden: false,
            value: 0.0,
            last_update: None,
            group: def.group.clone(),
            source: Source::Smc,
        });
    }
    sensors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smc::service::tests::FakeSmc;

    fn def(key: &str, name: &str, kind: Kind, group: Option<(&str, bool)>) -> SensorDef {
        SensorDef {
            key: key.into(),
            name: name.into(),
            kind,
            group: group.map(|(key, hide)| Group {
                key: key.into(),
                name: "CPU Core (average)".into(),
                hide_members: hide,
            }),
        }
    }

    fn provider(hide_members: bool) -> (Service, Provider) {
        let smc = FakeSmc::default();
        smc.set("F0Mn", "flt ", &1000f32.to_le_bytes());
        smc.set("AAAA", "ui8 ", &[1]);
        for (name, value) in [
            ("TA0P", 30.0f32),
            ("TC0C", 60.0),
            ("TC0c", 61.0),
            ("TC1c", 70.0),
            ("TH0A", 40.0),
            ("TH0a", 41.0),
            ("TXXX", 50.0),
            ("TB0T", 0.0),
        ] {
            smc.set(name, "flt ", &value.to_le_bytes());
        }
        smc.set("VP0R", "ui8 ", &[1]);
        smc.finish();
        let service = Service::new(Box::new(smc), &Model::parse("MacBookPro16,1"));
        let group = Some(("cpucoreavg", hide_members));
        let defs = [
            def("TA0P", "Ambient", Kind::Other, None),
            def("TB0T", "Battery Max", Kind::Other, None),
            def("TC0c", "CPU Core 1", Kind::Cpu, group),
            def("TC1c", "CPU Core 2", Kind::Cpu, group),
            def("TH0A", "Apple SSD", Kind::Ssd, None),
        ];
        let mut provider = Provider { sensors: load_smc_sensors(&service, &defs) };
        provider.build_averages();
        (service, provider)
    }

    #[test]
    fn loads_known_readable_keys_and_prefers_the_right_case() {
        let (_, provider) = provider(false);
        let keys: Vec<&str> = provider.sensors().iter().map(|s| s.key.as_str()).collect();
        // TXXX is not in the table, TB0T reads 0, TC0C and TH0a lose to their case twins
        assert_eq!(keys, ["TA0P", "TC0c", "TC1c", "TH0A", "cpucoreavg"]);
    }

    #[test]
    fn average_is_the_mean_of_its_members_and_disks_sort_last() {
        let (service, mut provider) = provider(false);
        let order = provider.visible();
        let names: Vec<&str> = order.iter().map(|&i| provider.sensors()[i].name.as_str()).collect();
        assert_eq!(names, ["Ambient", "CPU Core (average)", "CPU Core 1", "CPU Core 2", "Apple SSD"]);
        let average = provider.find("cpucoreavg").unwrap();
        provider.update(average, &service);
        assert_eq!(provider.sensors()[average].value, 65.5);
        // a preset written with the other case still finds the sensor
        assert_eq!(provider.find("TC0C"), provider.find("TC0c"));
        assert!(provider.find("TXXX").is_none());
    }

    #[test]
    fn hiding_groups_list_only_the_average() {
        let (_, provider) = provider(true);
        let order = provider.visible();
        let names: Vec<&str> = order.iter().map(|&i| provider.sensors()[i].name.as_str()).collect();
        assert_eq!(names, ["Ambient", "CPU Core (average)", "Apple SSD"]);
        assert!(provider.find("TC0c").is_none());
    }
}
