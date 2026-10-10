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
}
