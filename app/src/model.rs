//! Mac model identification from the SMBIOS product name (e.g. `MacBookPro16,1`).

use crate::registry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    MacBook,
    MacBookAir,
    MacBookNeo,
    MacBookPro,
    IMac,
    IMacPro,
    MacMini,
    MacStudio,
    MacPro,
    Xserve,
    Adp,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Model {
    /// Full identifier as reported by the firmware.
    pub name: String,
    pub family: Family,
    /// The identifier's digits run together: `MacBookPro15,1` is 151.
    pub number: u32,
}

/// Prefixes in the order they are tested; the first match wins.
const PREFIXES: [(&str, Family); 11] = [
    ("Macmini", Family::MacMini),
    ("MacPro", Family::MacPro),
    ("MacBookPro", Family::MacBookPro),
    ("MacBookNeo", Family::MacBookNeo),
    ("MacBookAir", Family::MacBookAir),
    ("MacBook", Family::MacBook),
    ("iMacPro", Family::IMacPro),
    ("iMac", Family::IMac),
    ("Xserve", Family::Xserve),
    ("ADP", Family::Adp),
    ("MacStudio", Family::MacStudio),
];

impl Model {
    pub fn parse(name: &str) -> Model {
        let family = PREFIXES
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix))
            .map(|&(_, family)| family)
            .unwrap_or(Family::Unknown);
        let digits: String = name.chars().filter(char::is_ascii_digit).collect();
        Model { name: name.to_string(), family, number: digits.parse().unwrap_or(0) }
    }

    pub fn detect() -> Model {
        let name = registry::read_machine_string(
            r"HARDWARE\DESCRIPTION\System\BIOS",
            "SystemProductName",
        )
        .unwrap_or_default();
        Model::parse(name.trim())
    }

    /// Whether a `models` attribute from the embedded XML tables covers this machine.
    ///
    /// `all` matches everything. Otherwise the attribute is a `;`-separated list where a
    /// plain entry must equal the identifier exactly and an entry containing `+` matches
    /// the same family from that number upwards (`MacBookPro18,1+` is any MacBookPro
    /// whose digits read 181 or more).
    pub fn matches(&self, models: &str) -> bool {
        if models == "all" {
            return true;
        }
        models.split(';').any(|entry| {
            if entry.contains('+') {
                let wanted = Model::parse(entry);
                wanted.family != Family::Unknown
                    && wanted.number != 0
                    && wanted.family == self.family
                    && self.number >= wanted.number
            } else {
                entry == self.name
            }
        })
    }

    /// Passively cooled models: no fans are loaded for these at all.
    pub fn is_fanless(&self) -> bool {
        match self.family {
            Family::MacBookAir => self.number >= 101,
            Family::MacBookNeo => true,
            Family::MacBook => matches!(self.number, 81 | 91 | 101),
            _ => false,
        }
    }

    /// Macs with a T2 chip, whose SMC is only reachable through the applesmc driver.
    pub fn is_t2(&self) -> bool {
        match self.family {
            Family::MacMini | Family::MacBookAir => self.number > 80,
            Family::MacPro => self.number >= 71,
            Family::MacBookPro => self.number > 150,
            Family::IMacPro => true,
            Family::IMac => self.number >= 201,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_family_and_number() {
        let m = Model::parse("MacBookPro16,1");
        assert_eq!((m.family, m.number), (Family::MacBookPro, 161));
        assert!(m.is_t2());
        let m = Model::parse("MacBookPro15,1");
        assert_eq!(m.number, 151);
        assert!(m.is_t2());
        assert!(!Model::parse("MacBookPro14,3").is_t2());
        let m = Model::parse("iMac20,1");
        assert_eq!((m.family, m.number), (Family::IMac, 201));
        assert!(m.is_t2());
        assert!(!Model::parse("iMac19,1").is_t2());
        assert_eq!(Model::parse("iMacPro1,1").family, Family::IMacPro);
        assert_eq!(Model::parse("MacPro7,1").number, 71);
        assert!(Model::parse("MacPro7,1").is_t2());
        assert!(!Model::parse("MacPro6,1").is_t2());
        assert_eq!(Model::parse("Macmini8,1").family, Family::MacMini);
        assert!(Model::parse("Macmini8,1").is_t2());
        assert_eq!(Model::parse("MacBook10,1").family, Family::MacBook);
        assert_eq!(Model::parse("Latitude 7490").family, Family::Unknown);
    }

    #[test]
    fn matches_model_lists() {
        let m = Model::parse("MacBookPro16,1");
        assert!(m.matches("all"));
        assert!(m.matches("MacBookPro15,1;MacBookPro16,1;MacBookPro18,1+"));
        assert!(m.matches("MacBookPro13,2+"));
        assert!(!m.matches("MacBookPro18,1+"));
        assert!(!m.matches("MacBookAir8,1+"));
        assert!(!m.matches("macbookpro16,1"));
        assert!(Model::parse("Macmini10,1").matches("MacBookAir9,1;ADP3,2;Macmini8,1+"));
        assert!(Model::parse("MacBookAir10,1").is_fanless());
        assert!(Model::parse("MacBook9,1").is_fanless());
        assert!(!m.is_fanless());
    }
}
