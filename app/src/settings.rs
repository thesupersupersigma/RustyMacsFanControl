//! Persistent settings. The registry key is the one the original app uses, so both
//! share preferences and presets.

use crate::registry::Key;

pub const REGISTRY_PATH: &str = r"Software\CrystalIdea Software\Macs Fan Control";

/// The value types the app stores.
pub trait Store {
    fn get_string(&self, name: &str) -> Option<String>;
    fn set_string(&self, name: &str, value: &str);
    fn get_strings(&self, name: &str) -> Vec<String>;
    fn set_strings(&self, name: &str, values: &[String]);
    fn get_bool(&self, name: &str, default: bool) -> bool;
    fn set_bool(&self, name: &str, value: bool);
}

impl Store for Key {
    fn get_string(&self, name: &str) -> Option<String> {
        Key::get_string(self, name)
    }

    fn set_string(&self, name: &str, value: &str) {
        Key::set_string(self, name, value);
    }

    fn get_strings(&self, name: &str) -> Vec<String> {
        Key::get_strings(self, name)
    }

    fn set_strings(&self, name: &str, values: &[String]) {
        Key::set_strings(self, name, values);
    }

    fn get_bool(&self, name: &str, default: bool) -> bool {
        Key::get_bool(self, name, default)
    }

    fn set_bool(&self, name: &str, value: bool) {
        Key::set_bool(self, name, value);
    }
}

pub fn open() -> Option<Key> {
    Key::open_user(REGISTRY_PATH)
}

/// In-memory store for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    values: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>>,
}

#[cfg(test)]
impl Store for MemoryStore {
    fn get_string(&self, name: &str) -> Option<String> {
        self.values.borrow().get(name).and_then(|v| v.first().cloned())
    }

    fn set_string(&self, name: &str, value: &str) {
        self.values.borrow_mut().insert(name.to_string(), vec![value.to_string()]);
    }

    fn get_strings(&self, name: &str) -> Vec<String> {
        self.values.borrow().get(name).cloned().unwrap_or_default()
    }

    fn set_strings(&self, name: &str, values: &[String]) {
        self.values.borrow_mut().insert(name.to_string(), values.to_vec());
    }

    fn get_bool(&self, name: &str, default: bool) -> bool {
        self.get_string(name).map_or(default, |v| v == "1")
    }

    fn set_bool(&self, name: &str, value: bool) {
        self.set_string(name, if value { "1" } else { "0" });
    }
}
