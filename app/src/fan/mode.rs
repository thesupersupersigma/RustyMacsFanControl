//! How a single fan is controlled, and its settings-string form.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Unparseable or unset; a profile containing one is not usable.
    Invalid,
    /// The SMC controls the fan.
    Automatic,
    /// Fixed RPM.
    Constant(i32),
    /// RPM follows a temperature sensor between two temperatures (degrees Celsius).
    Sensor { key: String, start: i32, max: i32 },
}

/// Integer parse that yields 0 on anything unparseable, surrounding whitespace ignored.
fn to_int(text: &str) -> i32 {
    text.trim().parse().unwrap_or(0)
}

impl Mode {
    pub fn to_setting(&self) -> String {
        match self {
            Mode::Invalid => "INVALID".to_string(),
            Mode::Automatic => "0".to_string(),
            Mode::Constant(rpm) => format!("1,{rpm}"),
            Mode::Sensor { key, start, max } => format!("2,{key},{start},{max}"),
        }
    }

    /// Parses a settings string. A sensor-based mode whose sensor `sensor_exists`
    /// rejects is invalid, so presets referring to vanished sensors do not load.
    pub fn from_setting(text: &str, sensor_exists: &dyn Fn(&str) -> bool) -> Mode {
        // Any single character means automatic; the character itself is not checked.
        if text.encode_utf16().count() == 1 {
            return Mode::Automatic;
        }
        let fields: Vec<&str> = text.split(',').collect();
        if text.starts_with('1') {
            // A constant mode without its RPM field is treated as invalid.
            return fields.get(1).map_or(Mode::Invalid, |rpm| Mode::Constant(to_int(rpm)));
        }
        if text.starts_with('2') {
            if fields.len() != 4 {
                crate::warn!("lst.size() is not 4");
                return Mode::Invalid;
            }
            if !sensor_exists(fields[1]) {
                crate::warn!("Failed to find {} sensor", fields[1]);
                return Mode::Invalid;
            }
            return Mode::Sensor {
                key: fields[1].to_string(),
                start: to_int(fields[2]),
                max: to_int(fields[3]),
            };
        }
        Mode::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Mode {
        Mode::from_setting(text, &|key| key == "TC0P")
    }

    #[test]
    fn settings_strings_round_trip() {
        let sensor = Mode::Sensor { key: "TC0P".into(), start: 50, max: 80 };
        assert_eq!(Mode::Automatic.to_setting(), "0");
        assert_eq!(Mode::Constant(2400).to_setting(), "1,2400");
        assert_eq!(sensor.to_setting(), "2,TC0P,50,80");
        assert_eq!(Mode::Invalid.to_setting(), "INVALID");
        assert_eq!(parse("0"), Mode::Automatic);
        assert_eq!(parse("1,2400"), Mode::Constant(2400));
        assert_eq!(parse("2,TC0P,50,80"), sensor);
    }

    #[test]
    fn malformed_strings() {
        assert_eq!(parse("x"), Mode::Automatic);
        assert_eq!(parse(""), Mode::Invalid);
        assert_eq!(parse("1,abc"), Mode::Constant(0));
        assert_eq!(parse("12"), Mode::Invalid);
        assert_eq!(parse("2,TC0P,50"), Mode::Invalid);
        assert_eq!(parse("2,TXXX,50,80"), Mode::Invalid);
        assert_eq!(parse("INVALID"), Mode::Invalid);
    }
}
