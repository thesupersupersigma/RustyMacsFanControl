//! Interface strings loaded from the `languages/*.xml` files, looked up by
//! `Section/key` path with the embedded English file as fallback.

use std::collections::HashMap;
use std::path::Path;

const ORIGINAL_NAME: &str = "Macs Fan Control";
const EMBEDDED_ENGLISH: &str = include_str!("../assets/xml/English.xml");

pub struct Language {
    strings: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

fn parse(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return out;
    };
    fn walk(node: roxmltree::Node, path: &str, out: &mut HashMap<String, String>) {
        for child in node.children().filter(|n| n.is_element()) {
            let child_path = if path.is_empty() {
                child.tag_name().name().to_string()
            } else {
                format!("{path}/{}", child.tag_name().name())
            };
            if child.children().any(|n| n.is_element()) {
                walk(child, &child_path, out);
            } else {
                // The files spell line breaks as a literal backslash-n, and name the
                // original product, which this app is not.
                let text = child
                    .text()
                    .unwrap_or("")
                    .replace("\\n", "\n")
                    .replace(ORIGINAL_NAME, crate::ui::text::APP_NAME);
                out.insert(child_path, text);
            }
        }
    }
    walk(doc.root_element(), "", &mut out);
    out
}

impl Language {
    pub fn english() -> Self {
        Language { strings: HashMap::new(), fallback: parse(EMBEDDED_ENGLISH) }
    }

    pub fn load(file: &Path) -> Self {
        let mut lang = Self::english();
        if let Ok(xml) = std::fs::read_to_string(file) {
            lang.strings = parse(&xml);
        }
        lang
    }

    pub fn get(&self, path: &str) -> &str {
        self.strings
            .get(path)
            .filter(|s| !s.is_empty())
            .or_else(|| self.fallback.get(path))
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Looks up `path` and substitutes `%1`, `%2`, ... with `args`.
    pub fn format(&self, path: &str, args: &[&str]) -> String {
        let mut text = self.get(path).to_string();
        for (i, arg) in args.iter().enumerate().rev() {
            text = text.replace(&format!("%{}", i + 1), arg);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_lookup_and_substitution() {
        let lang = Language::english();
        assert_eq!(lang.get("MainWindow/listColumnFan"), "Fan");
        assert_eq!(lang.get("core/QMessageBoxEx/buttonYes"), "Yes");
        assert_eq!(lang.format("MainWindow/fanControlConstant", &["2000"]), "Constant value of 2000");
        assert!(lang.get("Presets/presetCreateNewMessage").contains("system. \n\nPlease"));
        assert_eq!(lang.get("No/such/key"), "");
        assert_eq!(lang.get("TrayMenu/itemShow"), "Show Rusty Macs Fan Control");
        assert_eq!(lang.get("AboutDialog/title"), "About Rusty Macs Fan Control");
    }
}
