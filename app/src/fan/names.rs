//! Per-model fan names from the embedded `fanInfo.xml` table.

use crate::model::Model;

const FAN_INFO_XML: &str = include_str!("../../assets/xml/fanInfo.xml");

fn names_from(xml: &str, model: &Model) -> Vec<String> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    // The first matching block wins.
    let Some(block) = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("modelMatch"))
        .find(|n| n.attribute("models").is_some_and(|models| model.matches(models)))
    else {
        return Vec::new();
    };
    // A missing fanN is skipped, shifting later names down.
    (0..4)
        .filter_map(|i| block.children().find(|n| n.has_tag_name(format!("fan{i}").as_str())))
        .map(|n| n.text().unwrap_or("").to_string())
        .collect()
}

/// Names for fans 0.. on this model; fans beyond the list use the SMC's own name.
pub fn for_model(model: &Model) -> Vec<String> {
    names_from(FAN_INFO_XML, model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_table_names_this_models_fans() {
        assert_eq!(for_model(&Model::parse("MacBookPro16,1")), ["Left side", "Right side"]);
        assert_eq!(for_model(&Model::parse("MacBookPro19,1")), ["Left side", "Right side"]);
        assert_eq!(for_model(&Model::parse("Macmini9,1")), ["Exhaust"]);
        assert_eq!(for_model(&Model::parse("MacPro7,1")).len(), 4);
        assert!(for_model(&Model::parse("MacBookPro11,3")).is_empty());
    }
}
