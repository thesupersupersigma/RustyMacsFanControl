//! Sensor names, categories and groups from the embedded `smcSensors.xml` table.

use crate::model::Model;

const SMC_SENSORS_XML: &str = include_str!("../../assets/xml/smcSensors.xml");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Optical,
    Cpu,
    Hdd,
    Ssd,
    Nvme,
    External,
    Airport,
    Psu,
    Gpu,
    Other,
}

impl Kind {
    pub fn is_disk(self) -> bool {
        matches!(self, Kind::Hdd | Kind::Ssd | Kind::Nvme | Kind::External)
    }

    fn from_attribute(text: Option<&str>) -> Kind {
        match text {
            Some("cpu") => Kind::Cpu,
            Some("gpu") => Kind::Gpu,
            Some("odd") => Kind::Optical,
            Some("psu") => Kind::Psu,
            Some("airport") => Kind::Airport,
            Some("ssd") => Kind::Ssd,
            _ => Kind::Other,
        }
    }
}

/// An averaging group a sensor belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    pub name: String,
    /// Members are hidden and only the average is listed.
    pub hide_members: bool,
}

#[derive(Clone, Debug)]
pub struct SensorDef {
    pub key: String,
    pub name: String,
    pub kind: Kind,
    pub group: Option<Group>,
}

/// SMC keys naming the same sensor: first three characters equal, the fourth compared
/// without regard to case.
pub fn smc_key_match(a: &str, b: &str) -> bool {
    a.len() == 4 && b.len() == 4 && a.as_bytes()[..3] == b.as_bytes()[..3] && a.eq_ignore_ascii_case(b)
}

fn parse_block(node: roxmltree::Node, group: Option<&Group>, defs: &mut Vec<SensorDef>) {
    for child in node.children().filter(|n| n.is_element()) {
        let name = child.attribute("name").unwrap_or("");
        if child.tag_name().name() == "group" {
            let key = match child.attribute("key") {
                Some(key) if !key.is_empty() => key.to_string(),
                _ => name.replace(' ', "_").to_lowercase(),
            };
            let hide_members = match child.attribute("hideSensors") {
                None => true,
                Some(value) => value == "true" || value == "1",
            };
            let group = Group { key, name: name.to_string(), hide_members };
            parse_block(child, Some(&group), defs);
            continue;
        }
        // A later definition replaces any earlier one for the same key, and `exclude`
        // (whatever its value) removes the key altogether.
        defs.retain(|def| !smc_key_match(&def.key, name));
        if child.attribute("exclude").is_some() {
            continue;
        }
        let text = child.text().unwrap_or("");
        defs.push(SensorDef {
            key: name.to_string(),
            name: text.split(';').next().unwrap_or("Unknown").to_string(),
            kind: Kind::from_attribute(child.attribute("type")),
            group: group.cloned(),
        });
    }
}

fn load_from(xml: &str, model: &Model) -> Vec<SensorDef> {
    let mut defs = Vec::new();
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return defs;
    };
    for block in doc.root_element().children().filter(|n| n.has_tag_name("modelMatch")) {
        if block.attribute("models").is_some_and(|models| model.matches(models)) {
            parse_block(block, None, &mut defs);
        }
    }
    defs
}

/// Definitions that apply to `model`, later matching blocks overriding earlier ones.
pub fn load(model: &Model) -> Vec<SensorDef> {
    load_from(SMC_SENSORS_XML, model)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<smcSensors>
      <modelMatch models="all">
        <smc name="TA0P">Ambient</smc>
        <group name="CPU Core (average)" key="cpucoreavg" hideSensors="0">
          <smc name="TC1c" type="cpu">CPU Core 2</smc>
        </group>
        <group name="Memory Bank">
          <smc name="TM0P">Memory Proximity</smc>
        </group>
        <smc name="TG0P" type="gpu">GPU Proximity</smc>
      </modelMatch>
      <modelMatch models="MacBookPro13,2+">
        <smc name="TC1C" type="cpu">CPU Core 1</smc>
        <smc name="TG0P" exclude="1"/>
      </modelMatch>
      <modelMatch models="iMac20,1">
        <smc name="TA0P">Never</smc>
      </modelMatch>
    </smcSensors>"#;

    #[test]
    fn later_blocks_override_and_exclude() {
        let defs = load_from(XML, &Model::parse("MacBookPro16,1"));
        let keys: Vec<&str> = defs.iter().map(|d| d.key.as_str()).collect();
        assert_eq!(keys, ["TA0P", "TM0P", "TC1C"]);
        let core = &defs[2];
        assert_eq!((core.name.as_str(), core.kind), ("CPU Core 1", Kind::Cpu));
        // redefinition outside the group drops membership
        assert_eq!(core.group, None);
        // a group without key or hideSensors derives its key and hides its members
        assert_eq!(
            defs[1].group,
            Some(Group { key: "memory_bank".into(), name: "Memory Bank".into(), hide_members: true })
        );
    }

    #[test]
    fn group_attributes_are_kept_when_not_overridden() {
        let defs = load_from(XML, &Model::parse("MacBookPro11,3"));
        let core = defs.iter().find(|d| d.key == "TC1c").unwrap();
        let group = core.group.as_ref().unwrap();
        assert_eq!((group.key.as_str(), group.hide_members), ("cpucoreavg", false));
        assert!(defs.iter().any(|d| d.key == "TG0P" && d.kind == Kind::Gpu));
    }

    #[test]
    fn fourth_character_is_case_insensitive() {
        assert!(smc_key_match("TC0c", "TC0C"));
        assert!(!smc_key_match("Tc0C", "TC0C"));
        assert!(!smc_key_match("TC0", "TC0"));
    }

    #[test]
    fn shipped_table_loads_for_this_family() {
        let defs = load(&Model::parse("MacBookPro16,1"));
        assert!(defs.iter().any(|d| d.key == "TB0T" && d.name == "Battery Max"));
        assert!(defs.len() > 20);
    }
}
