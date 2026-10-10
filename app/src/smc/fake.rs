//! In-memory SMC used by the tests and by `--demo` in debug builds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex as StdMutex};

use super::platform::{key, Key, KeyInfo, Provider};

/// In-memory SMC: keys kept sorted, as the real one lists them.
#[derive(Clone, Default)]
pub struct FakeSmc {
    pub keys: Arc<StdMutex<BTreeMap<Key, ([u8; 4], Vec<u8>)>>>,
}

impl FakeSmc {
    pub fn set(&self, name: &str, kind: &str, data: &[u8]) {
        self.keys.lock().unwrap().insert(key(name), (key(kind), data.to_vec()));
    }

    pub fn get(&self, name: &str) -> Vec<u8> {
        self.keys.lock().unwrap()[&key(name)].1.clone()
    }

    /// Adds `#KEY` so that it matches the number of keys present.
    pub fn finish(&self) {
        let count = self.keys.lock().unwrap().len() as u32 + 1;
        self.set("#KEY", "ui32", &count.to_be_bytes());
    }
}

impl Provider for FakeSmc {
    fn key_by_index(&self, index: u32) -> Option<Key> {
        self.keys.lock().unwrap().keys().nth(index as usize).copied()
    }

    fn key_info(&self, key: &Key) -> Option<KeyInfo> {
        let keys = self.keys.lock().unwrap();
        let (kind, data) = keys.get(key)?;
        Some(KeyInfo { size: data.len() as u8, kind: *kind, attributes: 0 })
    }

    fn read_key(&self, key: &Key, out: &mut [u8]) -> bool {
        let keys = self.keys.lock().unwrap();
        let Some((_, data)) = keys.get(key) else {
            return false;
        };
        let n = out.len().min(data.len());
        out[..n].copy_from_slice(&data[..n]);
        true
    }

    fn write_key(&self, key: &Key, data: &[u8]) -> bool {
        let mut keys = self.keys.lock().unwrap();
        let Some(entry) = keys.get_mut(key) else {
            return false;
        };
        entry.1 = data.to_vec();
        // A fan follows its target, so the demo shows speeds changing.
        if key[0] == b'F' && &key[2..] == b"Tg" {
            if let Some(actual) = keys.get_mut(&[b'F', key[1], b'A', b'c']) {
                actual.1 = data.to_vec();
            }
        }
        true
    }

    fn name(&self) -> &'static str {
        "Fake"
    }
}

/// A T2 laptop with two fans and a handful of sensors, for trying the UI without the
/// driver. Writing a fan target moves that fan's reported speed.
pub fn demo() -> FakeSmc {
    let smc = FakeSmc::default();
    smc.set("FNum", "ui8 ", &[2]);
    for (fan, min, max) in [(0, 1836f32, 5616f32), (1, 1700f32, 5200f32)] {
        smc.set(&format!("F{fan}Ac"), "flt ", &(min + 150.0).to_le_bytes());
        smc.set(&format!("F{fan}Mn"), "flt ", &min.to_le_bytes());
        smc.set(&format!("F{fan}Mx"), "flt ", &max.to_le_bytes());
        smc.set(&format!("F{fan}Tg"), "flt ", &min.to_le_bytes());
        smc.set(&format!("F{fan}Md"), "ui8 ", &[0]);
    }
    for (name, value) in [
        ("TA0V", 23.8f32), ("TB0T", 34.9), ("TB1T", 33.2), ("TB2T", 34.9), ("TC0P", 64.0),
        ("TC1C", 68.0), ("TC2C", 69.0), ("TC3C", 71.0), ("TC4C", 68.0), ("TCGC", 67.0),
        ("TCXC", 71.0), ("TG0P", 63.8), ("TGDD", 70.0), ("TH0a", 35.3), ("TM0P", 60.0),
        ("TPCD", 61.0), ("TW0P", 60.9), ("Th1H", 65.5), ("Ts0P", 35.0),
    ] {
        smc.set(name, "flt ", &value.to_le_bytes());
    }
    smc.set("AAAA", "ui8 ", &[1]);
    smc.set("VP0R", "ui8 ", &[1]);
    smc.finish();
    smc
}
