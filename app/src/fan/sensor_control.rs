//! Sensor-based fan speed: the temperature picks an RPM band one degree wide and the
//! target creeps toward the band's edge a few RPM per call, with hysteresis so brief
//! temperature flickers do not reverse the fan.

use super::mode::Mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tendency {
    Unknown,
    Increase,
    Decrease,
    DoNothing,
}

/// Seconds in DoNothing at an unchanged temperature before nudging again.
const IDLE_NUDGE_SECS: i64 = 60;
/// A temperature reversal this soon after the current tendency began is ignored.
const REVERSAL_HOLD_SECS: i64 = 11;
/// An unchanged target is sent again after this long.
const RESEND_SECS: i64 = 31;

#[derive(Clone, Debug)]
pub struct SensorControl {
    last_temp: Option<i32>,
    rpm: i32,
    last_set_rpm: i32,
    last_set_time: Option<i64>,
    tendency: Tendency,
    double: bool,
    half: bool,
    do_nothing_since: i64,
    increase_since: i64,
    decrease_since: i64,
    /// Fan name, used as the log prefix.
    name: String,
}

impl SensorControl {
    /// `now` is wall-clock seconds since the epoch, as in every other method here.
    pub fn new(name: &str, now: i64) -> SensorControl {
        SensorControl {
            last_temp: None,
            rpm: -1,
            last_set_rpm: -1,
            last_set_time: None,
            tendency: Tendency::Unknown,
            double: false,
            half: false,
            do_nothing_since: now,
            increase_since: now,
            decrease_since: now,
            name: name.to_string(),
        }
    }

    /// Forgets the current target; tendency and its timers are kept.
    pub fn reset(&mut self) {
        self.last_temp = None;
        self.rpm = -1;
        self.last_set_rpm = -1;
        self.last_set_time = None;
    }

    /// Advances one step and returns the RPM to write, or `None` when nothing needs
    /// writing. `current`, `min` and `max` are the fan's measured and limit RPMs.
    pub fn update(
        &mut self,
        current: i32,
        min: i32,
        max: i32,
        temperature: f64,
        start: i32,
        max_temp: i32,
        now: i64,
    ) -> Option<i32> {
        let temp = temperature as i32;
        if temp == 0 {
            return None;
        }
        let range = max_temp - start;
        // RPM per degree. The dialog keeps max_temp above start; a zero range would
        // otherwise divide by zero, so it is given a flat band.
        let step = if range == 0 { 0 } else { ((max - min) as f64 / range as f64) as i32 };
        let band = |t: i32| {
            let low = ((t - start - 1) * step + min).max(min).min(max - step);
            (low, (low + step).min(max))
        };
        let mut accept = true;
        let name = self.name.clone();

        if temp < start {
            if self.rpm != min {
                self.reset();
                self.rpm = min;
            }
            return self.finish(now);
        }

        match self.last_temp {
            None => {
                let (low, high) = band(temp);
                if low < current && current < high {
                    self.rpm = current;
                    self.tendency = Tendency::DoNothing;
                } else {
                    self.rpm = low;
                    self.tendency = Tendency::Increase;
                    self.half = true;
                    crate::info!("{name}: HALF INCREASE ON");
                }
                self.increase_since = now;
                crate::info!(
                    "{name}: Tendency is set to {:?}, Temperature {temp}, RPM={current}",
                    self.tendency
                );
            }
            Some(last) if last == temp => {
                if self.tendency == Tendency::DoNothing
                    && now - self.do_nothing_since >= IDLE_NUDGE_SECS
                {
                    self.half = true;
                    if range / 2 + start < temp {
                        self.tendency = Tendency::Increase;
                        crate::info!("{name}: HALF INCREASE ON");
                    } else {
                        self.tendency = Tendency::Decrease;
                        crate::info!("{name}: HALF DECREASE ON");
                    }
                }
            }
            Some(last) => {
                let was_half = self.half;
                self.double = false;
                self.half = false;
                if temp < last {
                    crate::info!("{name}: Temperature DECREASED to {temp}");
                    match self.tendency {
                        Tendency::Unknown => crate::warn!("TendencyUnknown is unexpected! (1)"),
                        Tendency::Increase => {
                            if now - self.increase_since < REVERSAL_HOLD_SECS {
                                accept = false;
                                crate::info!("{name}: Ignoring temperature change");
                            } else {
                                self.tendency = Tendency::DoNothing;
                                self.do_nothing_since = now;
                            }
                        }
                        Tendency::Decrease => self.decrease_since = now,
                        Tendency::DoNothing => {
                            self.decrease_since = now;
                            self.tendency = Tendency::Decrease;
                        }
                    }
                } else {
                    crate::info!("{name}: Temperature INCREASED to {temp}");
                    match self.tendency {
                        Tendency::Unknown => crate::warn!("TendencyUnknown is unexpected! (2)"),
                        Tendency::Increase => {
                            if was_half {
                                crate::info!("{name}: HALF INCREASE OFF");
                            } else {
                                self.double = true;
                                crate::info!("{name}: DOUBLE INCREASE ON");
                            }
                        }
                        Tendency::Decrease | Tendency::DoNothing => {
                            let since = if self.tendency == Tendency::Decrease {
                                self.decrease_since
                            } else {
                                self.do_nothing_since
                            };
                            if now - since < REVERSAL_HOLD_SECS {
                                accept = false;
                                crate::info!("{name}: Ignoring temperature change");
                            } else {
                                self.increase_since = now;
                                self.tendency = Tendency::Increase;
                            }
                        }
                    }
                }
                crate::info!("{name}: Tendency is set to {:?}", self.tendency);
            }
        }

        if max_temp < temp {
            self.rpm = max;
        } else {
            let (low, high) = band(if accept { temp } else { self.last_temp.unwrap_or(temp) });
            match self.tendency {
                Tendency::Increase => {
                    if 2 * step < low - self.rpm {
                        self.rpm += step / 3;
                        crate::info!(
                            "{name}: Adding extra {} from current rpm {}",
                            step / 3,
                            self.rpm
                        );
                    }
                    let next = if self.double {
                        self.rpm + 4
                    } else if self.half {
                        self.rpm + 1
                    } else {
                        self.rpm + 2
                    };
                    self.rpm = next.min(high);
                    if self.rpm == high {
                        self.tendency = Tendency::DoNothing;
                        self.do_nothing_since = now;
                        crate::info!(
                            "{name}: PEEK MAX RPM '{high}' for current temperature, do nothing now"
                        );
                    }
                }
                Tendency::Decrease => {
                    if step < self.rpm - high {
                        self.rpm -= step / 3;
                        crate::info!(
                            "{name}: Subtracting extra {} from current rpm {}",
                            step / 3,
                            self.rpm
                        );
                    }
                    let next = if self.half { self.rpm - 1 } else { self.rpm - 2 };
                    self.rpm = next.max(low);
                    if self.rpm == low {
                        self.tendency = Tendency::DoNothing;
                        self.do_nothing_since = now;
                        crate::info!(
                            "{name}: PEEK MIN RPM '{low}' for current temperature, do nothing now"
                        );
                    }
                }
                Tendency::DoNothing | Tendency::Unknown => {}
            }
        }
        if accept {
            self.last_temp = Some(temp);
        }
        self.finish(now)
    }

    /// Convenience wrapper taking the sensor-based mode's temperatures.
    pub fn update_for_mode(
        &mut self,
        current: i32,
        min: i32,
        max: i32,
        temperature: f64,
        mode: &Mode,
        now: i64,
    ) -> Option<i32> {
        match mode {
            Mode::Sensor { start, max: max_temp, .. } => {
                self.update(current, min, max, temperature, *start, *max_temp, now)
            }
            _ => None,
        }
    }

    fn finish(&mut self, now: i64) -> Option<i32> {
        if self.last_set_rpm == self.rpm {
            let last = self.last_set_time?;
            if now - last < RESEND_SECS {
                return None;
            }
        } else {
            self.last_set_rpm = self.rpm;
            crate::info!("{}: Setting fan speed to {}", self.name, self.rpm);
        }
        self.last_set_time = Some(now);
        Some(self.rpm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fan 2000..6000 rpm, sensor range 50..90 °C: 100 rpm per degree.
    const MIN: i32 = 2000;
    const MAX: i32 = 6000;

    fn step(control: &mut SensorControl, current: i32, temp: f64, now: i64) -> Option<i32> {
        control.update(current, MIN, MAX, temp, 50, 90, now)
    }

    #[test]
    fn below_start_holds_minimum_and_resends_every_31_seconds() {
        let mut c = SensorControl::new("Left side", 1000);
        assert_eq!(step(&mut c, 2500, 40.9, 1000), Some(2000));
        assert_eq!(step(&mut c, 2500, 41.0, 1001), None);
        assert_eq!(step(&mut c, 2500, 41.0, 1030), None);
        assert_eq!(step(&mut c, 2500, 41.0, 1031), Some(2000));
        assert_eq!(step(&mut c, 2500, 0.4, 2000), None);
    }

    #[test]
    fn above_max_temperature_jumps_to_maximum() {
        let mut c = SensorControl::new("Left side", 0);
        assert_eq!(step(&mut c, 2000, 91.0, 0), Some(6000));
    }

    #[test]
    fn first_sample_starts_at_band_bottom_and_creeps_up_at_half_speed() {
        let mut c = SensorControl::new("Left side", 0);
        // 60 °C: band is (60-50-1)*100+2000 = 2900..3000
        assert_eq!(step(&mut c, 2000, 60.0, 0), Some(2901));
        assert_eq!(c.tendency, Tendency::Increase);
        assert_eq!(step(&mut c, 2000, 60.7, 2), Some(2902));
        // creep stops at the band top and switches to DoNothing
        let mut last = 2902;
        let mut now = 2;
        while c.tendency == Tendency::Increase {
            now += 2;
            last = step(&mut c, 2000, 60.0, now).unwrap();
        }
        assert_eq!(last, 3000);
        assert_eq!(step(&mut c, 3000, 60.0, now + 2), None);
    }

    #[test]
    fn first_sample_inside_band_keeps_the_measured_speed() {
        let mut c = SensorControl::new("Left side", 0);
        assert_eq!(step(&mut c, 2950, 60.0, 0), Some(2950));
        assert_eq!(c.tendency, Tendency::DoNothing);
    }

    #[test]
    fn rising_temperature_doubles_the_step_and_far_targets_get_an_extra_jump() {
        let mut c = SensorControl::new("Left side", 0);
        step(&mut c, 2000, 60.0, 0); // 2901, half
        // 61 °C right after a half step: half is switched off, no double yet
        assert_eq!(step(&mut c, 2000, 61.0, 2), Some(2903));
        // 62 °C: double increase
        assert_eq!(step(&mut c, 2000, 62.0, 4), Some(2907));
        // 70 °C: band 3900..4000 is more than two steps away, so step/3 is added first
        assert_eq!(step(&mut c, 2000, 70.0, 6), Some(2907 + 33 + 4));
    }

    #[test]
    fn quick_reversal_is_ignored_then_honoured_after_the_hold_time() {
        let mut c = SensorControl::new("Left side", 0);
        step(&mut c, 2000, 60.0, 0);
        // drop within 11 s of starting to increase: ignored, keeps creeping in the 60° band
        assert_eq!(step(&mut c, 2000, 59.0, 5), Some(2903));
        assert_eq!(c.tendency, Tendency::Increase);
        assert_eq!(c.last_temp, Some(60));
        // same drop after the hold: stop increasing
        step(&mut c, 2000, 59.0, 20);
        assert_eq!(c.tendency, Tendency::DoNothing);
        assert_eq!(c.last_temp, Some(59));
    }

    #[test]
    fn decrease_creeps_down_to_the_band_bottom() {
        let mut c = SensorControl::new("Left side", 0);
        assert_eq!(step(&mut c, 2950, 60.0, 0), Some(2950)); // DoNothing at 2950
        // cooling to 58 °C well after the hold: band 2700..2800, more than a step above it
        assert_eq!(step(&mut c, 2950, 58.0, 100), Some(2950 - 33 - 2));
        assert_eq!(c.tendency, Tendency::Decrease);
        let mut now = 100;
        let mut last = 0;
        while c.tendency == Tendency::Decrease {
            now += 2;
            last = step(&mut c, 2900, 58.0, now).unwrap();
        }
        assert_eq!(last, 2700);
    }

    #[test]
    fn idle_at_unchanged_temperature_nudges_after_a_minute() {
        let mut c = SensorControl::new("Left side", 0);
        step(&mut c, 2950, 60.0, 0); // DoNothing; timer runs from construction
        assert_eq!(step(&mut c, 2950, 60.0, 20), None);
        // unchanged target is re-sent once 31 s have passed
        assert_eq!(step(&mut c, 2950, 60.0, 59), Some(2950));
        assert_eq!(c.tendency, Tendency::DoNothing);
        // 60 °C is below the midpoint (70), so the nudge is downward at half speed
        assert_eq!(step(&mut c, 2950, 60.0, 60), Some(2949));
        assert_eq!(c.tendency, Tendency::Decrease);
    }
}
