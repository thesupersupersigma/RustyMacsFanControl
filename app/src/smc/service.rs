//! Fan and temperature access on top of a key transport.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use super::platform::{key, key_text, Key, KeyInfo, Provider};
use crate::model::{Family, Model};

/// Which RPM value of a fan a key addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanKey {
    Actual,
    Minimum,
    Maximum,
    Target,
    Safe,
}

impl FanKey {
    fn suffix(self) -> &'static str {
        match self {
            FanKey::Actual => "Ac",
            FanKey::Minimum => "Mn",
            FanKey::Maximum => "Mx",
            FanKey::Target => "Tg",
            FanKey::Safe => "Sf",
        }
    }
}

fn fan_key(kind: FanKey, fan: usize) -> Key {
    key(&format!("F{fan}{}", kind.suffix()))
}

/// How the SMC switches fans between automatic and forced control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModeKey {
    /// One `FS! ` bitmask for all fans.
    Bitmask,
    /// One `F{n}Md` byte per fan.
    PerFan,
}

const FAN_COUNT_UNREAD: u8 = 0xFF;
/// MacPro CPU keys that report distance to the limit rather than a temperature.
const MAC_PRO_INVERTED: [&str; 6] = ["TCAC", "TCBC", "TC0C", "TC1C", "TC2C", "TC3C"];

pub struct Service {
    provider: Mutex<Box<dyn Provider>>,
    fan_count: AtomicU8,
    temperature_keys: OnceLock<(Vec<Key>, u8)>,
    fan_key_size: u8,
    mode_key: ModeKey,
    /// iMac Pro and Mac Studio: two synchronized fans presented as one.
    dual_fan: bool,
    is_xserve: bool,
    is_mac_pro: bool,
}

/// One row of the key table walked while locating temperature keys.
#[derive(Clone, Copy, Default)]
struct Entry {
    key: Key,
    info: KeyInfo,
    valid: bool,
}

impl Service {
    pub fn new(provider: Box<dyn Provider>, model: &Model) -> Service {
        let fan_key_size = match provider.key_info(&key("F0Mn")) {
            Some(info) => {
                crate::info!(
                    "detectSmcFanKeySize KeyType {} KeyLen {}",
                    key_text(&info.kind),
                    info.size
                );
                info.size
            }
            None => {
                crate::error!("Unexpected failure at getKeyInfo, using 2, no fans?");
                2
            }
        };
        let mode_key = if provider.key_info(&key("FS! ")).is_some() {
            ModeKey::Bitmask
        } else {
            crate::error!(
                "Unexpected failure at getKeyInfo for SMC_KEY_FANS_CONTROL_MODE, using 2"
            );
            ModeKey::PerFan
        };
        Service {
            provider: Mutex::new(provider),
            fan_count: AtomicU8::new(FAN_COUNT_UNREAD),
            temperature_keys: OnceLock::new(),
            fan_key_size,
            mode_key,
            dual_fan: matches!(model.family, Family::IMacPro | Family::MacStudio),
            is_xserve: model.family == Family::Xserve,
            is_mac_pro: model.family == Family::MacPro,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.lock().unwrap().name()
    }

    pub fn key_count(&self) -> u32 {
        let mut raw = [0u8; 4];
        if !self.provider.lock().unwrap().read_key(&key("#KEY"), &mut raw) {
            return 0;
        }
        let count = u32::from_be_bytes(raw);
        if count > 0xFFFF {
            crate::error!("getSMCKeysCount failed");
            return 0;
        }
        count
    }

    pub fn fan_count(&self) -> usize {
        let cached = self.fan_count.load(Ordering::Relaxed);
        if cached != FAN_COUNT_UNREAD {
            return cached as usize;
        }
        let mut raw = [0u8; 1];
        let count = if !self.provider.lock().unwrap().read_key(&key("FNum"), &mut raw) {
            crate::error!("Unable to read number of fans");
            0
        } else if self.dual_fan && raw[0] == 2 {
            crate::info!("Dual fan: the mac has a dual-fan cooler with two syncronized fans.");
            1
        } else if self.is_xserve {
            crate::info!("XServe 10 fans limit");
            10
        } else {
            raw[0]
        };
        self.fan_count.store(count, Ordering::Relaxed);
        count as usize
    }

    /// The SMC's own label for a fan, or `Fan N` (0-based) when it has none.
    pub fn fan_name(&self, fan: usize) -> String {
        let fan_id = key(&format!("F{fan}ID"));
        let mut raw = [0u8; 16];
        let name = if self.provider.lock().unwrap().read_key(&fan_id, &mut raw) {
            let text = &raw[4..];
            let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
            String::from_utf8_lossy(&text[..end]).trim().to_string()
        } else {
            crate::error!("Unable to read fan {fan} name. smc key {}", key_text(&fan_id));
            String::new()
        };
        if name.is_empty() {
            format!("Fan {fan}")
        } else {
            name
        }
    }

    pub fn fan_rpm(&self, fan: usize, kind: FanKey) -> Option<u16> {
        let fan_key = fan_key(kind, fan);
        let mut raw = [0u8; 4];
        let size = (self.fan_key_size as usize).min(4);
        if !self.provider.lock().unwrap().read_key(&fan_key, &mut raw[..size]) {
            crate::error!("Unable to read fan rpm, smc key {}", key_text(&fan_key));
            return None;
        }
        Some(match self.fan_key_size {
            // fpe2: big-endian 14.2 fixed point, fraction dropped
            2 => ((raw[0] as u16) << 6) | ((raw[1] as u16) >> 2),
            4 => f32::from_le_bytes(raw) as u16,
            _ => 0,
        })
    }

    fn write_fan_rpm(&self, fan: usize, kind: FanKey, rpm: u16) -> bool {
        let fan_key = fan_key(kind, fan);
        let data: Vec<u8> = match self.fan_key_size {
            2 => vec![(rpm >> 6) as u8, (rpm << 2) as u8],
            4 => (rpm as f32).to_le_bytes().to_vec(),
            _ => {
                crate::error!("Unknown m_smcFanKeySize");
                return false;
            }
        };
        let ok = self.provider.lock().unwrap().write_key(&fan_key, &data);
        if ok {
            crate::info!("setFanRPM {rpm} for {}", key_text(&fan_key));
        } else {
            crate::error!("Unable to set fan {} RPM {rpm}", key_text(&fan_key));
        }
        ok
    }

    pub fn set_fan_rpm(&self, fan: usize, kind: FanKey, rpm: u16) -> bool {
        let mut ok = self.write_fan_rpm(fan, kind, rpm);
        if self.dual_fan {
            if fan == 0 {
                ok = self.write_fan_rpm(1, kind, rpm);
            } else {
                crate::error!("Dual fan: unexpected error, fan index is supposed to be 0");
            }
        }
        ok
    }

    /// One flag per fan, `true` meaning the fan is under forced (manual) control.
    pub fn fan_control_modes(&self) -> Option<Vec<bool>> {
        let count = self.fan_count();
        let provider = self.provider.lock().unwrap();
        match self.mode_key {
            ModeKey::Bitmask => {
                let mut raw = [0u8; 2];
                if !provider.read_key(&key("FS! "), &mut raw) {
                    crate::error!("Unable to read fans control mode, key FS! ");
                    return None;
                }
                Some((0..count).map(|i| raw[1] & (1u8 << i) != 0).collect())
            }
            ModeKey::PerFan => {
                let mut modes = Vec::new();
                for fan in 0..count {
                    let mut raw = [0u8; 1];
                    if !provider.read_key(&key(&format!("F{fan}Md")), &mut raw) {
                        crate::error!("Unable to query fan mode {fan}");
                        break;
                    }
                    modes.push(raw[0] != 0);
                }
                (!modes.is_empty()).then_some(modes)
            }
        }
    }

    pub fn set_fan_control_modes(&self, modes: &[bool]) -> bool {
        if modes.len() != self.fan_count() {
            crate::error!("setFansControlModes: fanCount != vModes.size()");
            return false;
        }
        let provider = self.provider.lock().unwrap();
        match self.mode_key {
            ModeKey::Bitmask => {
                let mask = modes
                    .iter()
                    .enumerate()
                    .fold(0u8, |mask, (i, &forced)| mask | ((forced as u8) << i));
                let ok = provider.write_key(&key("FS! "), &[0, mask]);
                if ok {
                    crate::info!("Set fans mode {mask}");
                } else {
                    crate::error!("Unable to set fan control modes");
                }
                ok
            }
            ModeKey::PerFan => {
                let mut modes = modes.to_vec();
                if self.dual_fan {
                    if modes.len() == 1 {
                        modes.push(modes[0]);
                    } else {
                        crate::error!("Dual fan: unexpected error, must be only one fan");
                    }
                }
                let mut ok = false;
                for (fan, &forced) in modes.iter().enumerate() {
                    ok = provider.write_key(&key(&format!("F{fan}Md")), &[forced as u8]);
                    if !ok {
                        crate::error!("Unable to set fan {fan} control mode");
                        break;
                    }
                    crate::info!("Set fan {fan} mode {}", forced as u8);
                }
                ok
            }
        }
    }

    fn entry(&self, cache: &mut [Entry], index: usize) -> Option<Entry> {
        if !cache[index].valid {
            let provider = self.provider.lock().unwrap();
            let found = provider
                .key_by_index(index as u32)
                .and_then(|key| provider.key_info(&key).map(|info| (key, info)));
            let Some((key, info)) = found else {
                crate::error!("applesmc_get_entry_by_index failed");
                return None;
            };
            cache[index] = Entry { key, info, valid: true };
        }
        Some(cache[index])
    }

    /// Index of the first key not ordered before `target`; the SMC lists keys sorted.
    fn lower_bound(&self, cache: &mut [Entry], target: &[u8]) -> usize {
        let (mut begin, mut end) = (0, cache.len());
        while begin < end {
            let middle = begin + (end - begin) / 2;
            let Some(entry) = self.entry(cache, middle) else {
                return 0;
            };
            let text_end = entry.key.iter().position(|&b| b == 0).unwrap_or(4);
            if &entry.key[..text_end] < target {
                begin = middle + 1;
            } else {
                end = middle;
            }
        }
        begin
    }

    fn discover_temperature_keys(&self) -> Option<(Vec<Key>, u8)> {
        let count = self.key_count() as usize;
        if count == 0 {
            return None;
        }
        let mut cache = vec![Entry::default(); count];
        let low = self.lower_bound(&mut cache, b"T");
        let high = self.lower_bound(&mut cache, b"U");
        crate::info!("Locating temperature SMC keys");
        if low == 0 || high == 0 || high <= low || count <= high {
            crate::error!("one of applesmc_get_lower_bound failed {low}, {high}, {count}");
            return None;
        }
        let mut keys = Vec::new();
        let mut size = 0u8;
        for index in low..high {
            let Some(entry) = self.entry(&mut cache, index) else {
                continue;
            };
            let is_float = &entry.info.kind == b"flt ";
            if (&entry.info.kind == b"sp78" || is_float) && entry.key[0] == b'T' {
                // One value size for every temperature key, taken from the first match.
                if keys.is_empty() {
                    size = if is_float { 4 } else { 2 };
                }
                keys.push(entry.key);
            }
        }
        crate::info!("{} keys found", keys.len());
        Some((keys, size))
    }

    /// Every temperature key the SMC exposes. Discovery runs once and is cached.
    pub fn temperature_keys(&self) -> Vec<Key> {
        if let Some((keys, _)) = self.temperature_keys.get() {
            return keys.clone();
        }
        match self.discover_temperature_keys() {
            Some(found) => self.temperature_keys.get_or_init(|| found).0.clone(),
            None => Vec::new(),
        }
    }

    /// Temperature in degrees Celsius, or `None` when unreadable or outside (0, 127).
    pub fn temperature(&self, temperature_key: &Key) -> Option<f64> {
        let size = self.temperature_keys.get().map(|(_, size)| *size).unwrap_or(0);
        let mut raw = [0u8; 4];
        if !self
            .provider
            .lock()
            .unwrap()
            .read_key(temperature_key, &mut raw[..(size as usize).min(4)])
        {
            return None;
        }
        let value = match size {
            // sp78 with both bytes taken unsigned; negative readings land above 127
            2 => raw[0] as f64 + raw[1] as f64 * 0.00390625,
            4 => f32::from_le_bytes(raw) as f64,
            _ => 0.0,
        };
        if value <= 0.0 || value >= 127.0 {
            crate::warn!(
                "getTemperature - failed reading sensor {}, its value is {value}",
                key_text(temperature_key)
            );
            return None;
        }
        let name = key_text(temperature_key);
        if self.is_mac_pro && MAC_PRO_INVERTED.contains(&name.as_str()) {
            return Some(100.0 - value);
        }
        Some(value)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub use crate::smc::fake::FakeSmc;

    /// A T2 laptop: float fan keys, per-fan mode keys, float temperatures.
    pub fn t2_laptop() -> (FakeSmc, Service) {
        let smc = FakeSmc::default();
        smc.set("FNum", "ui8 ", &[2]);
        for fan in 0..2 {
            smc.set(&format!("F{fan}Ac"), "flt ", &1850.5f32.to_le_bytes());
            smc.set(&format!("F{fan}Mn"), "flt ", &1836f32.to_le_bytes());
            smc.set(&format!("F{fan}Mx"), "flt ", &5927f32.to_le_bytes());
            smc.set(&format!("F{fan}Tg"), "flt ", &1836f32.to_le_bytes());
            smc.set(&format!("F{fan}Md"), "ui8 ", &[0]);
        }
        smc.set("TC0P", "flt ", &48.25f32.to_le_bytes());
        smc.set("TB0T", "flt ", &0f32.to_le_bytes());
        smc.set("TVMD", "ui8 ", &[1]);
        smc.set("AAAA", "ui8 ", &[1]);
        smc.set("VP0R", "flt ", &12f32.to_le_bytes());
        smc.finish();
        let service = Service::new(Box::new(smc.clone()), &Model::parse("MacBookPro16,1"));
        (smc, service)
    }

    #[test]
    fn t2_fans_use_float_keys_and_per_fan_modes() {
        let (smc, service) = t2_laptop();
        assert_eq!(service.fan_count(), 2);
        assert_eq!(service.fan_name(0), "Fan 0");
        assert_eq!(service.fan_rpm(0, FanKey::Actual), Some(1850));
        assert_eq!(service.fan_rpm(1, FanKey::Maximum), Some(5927));
        assert_eq!(service.fan_control_modes(), Some(vec![false, false]));

        assert!(service.set_fan_rpm(1, FanKey::Target, 3000));
        assert_eq!(smc.get("F1Tg"), 3000f32.to_le_bytes());
        assert!(service.set_fan_control_modes(&[false, true]));
        assert_eq!((smc.get("F0Md"), smc.get("F1Md")), (vec![0], vec![1]));
        assert_eq!(service.fan_control_modes(), Some(vec![false, true]));
        assert!(!service.set_fan_control_modes(&[true]));
    }

    #[test]
    fn temperature_keys_are_filtered_by_range_type_and_prefix() {
        let (_, service) = t2_laptop();
        assert_eq!(service.temperature_keys(), vec![key("TB0T"), key("TC0P")]);
        assert_eq!(service.temperature(&key("TC0P")), Some(48.25));
        assert_eq!(service.temperature(&key("TB0T")), None);
    }

    #[test]
    fn older_macs_use_fpe2_and_the_fs_bitmask() {
        let smc = FakeSmc::default();
        smc.set("FNum", "ui8 ", &[2]);
        // 2000 rpm as fpe2 is 2000 << 2 = 0x1F40
        smc.set("F0Ac", "fpe2", &[0x1F, 0x40]);
        smc.set("F0Mn", "fpe2", &[0x1F, 0x40]);
        smc.set("F1Tg", "fpe2", &[0, 0]);
        smc.set("F0ID", "{fds", b"\0\0\0\0  Exhaust \0\0\0\0");
        smc.set("FS! ", "ui16", &[0, 0]);
        smc.set("TC0P", "sp78", &[45, 0x80]);
        smc.set("TCAC", "sp78", &[30, 0]);
        smc.set("AAAA", "ui8 ", &[1]);
        smc.set("VP0R", "ui8 ", &[1]);
        smc.finish();
        let service = Service::new(Box::new(smc.clone()), &Model::parse("MacPro5,1"));

        assert_eq!(service.fan_rpm(0, FanKey::Actual), Some(2000));
        assert_eq!(service.fan_name(0), "Exhaust");
        assert!(service.set_fan_rpm(1, FanKey::Target, 2500));
        assert_eq!(smc.get("F1Tg"), vec![(2500u16 >> 6) as u8, ((2500u16 << 2) & 0xFF) as u8]);
        assert!(service.set_fan_control_modes(&[false, true]));
        assert_eq!(smc.get("FS! "), vec![0, 0b10]);
        assert_eq!(service.fan_control_modes(), Some(vec![false, true]));

        assert_eq!(service.temperature_keys(), vec![key("TC0P"), key("TCAC")]);
        assert_eq!(service.temperature(&key("TC0P")), Some(45.5));
        // MacPro reports these as distance to the limit
        assert_eq!(service.temperature(&key("TCAC")), Some(70.0));
    }

    #[test]
    fn imac_pro_presents_its_two_fans_as_one() {
        let smc = FakeSmc::default();
        smc.set("FNum", "ui8 ", &[2]);
        for fan in 0..2 {
            smc.set(&format!("F{fan}Mn"), "flt ", &1100f32.to_le_bytes());
            smc.set(&format!("F{fan}Tg"), "flt ", &0f32.to_le_bytes());
            smc.set(&format!("F{fan}Md"), "ui8 ", &[0]);
        }
        smc.finish();
        let service = Service::new(Box::new(smc.clone()), &Model::parse("iMacPro1,1"));
        assert_eq!(service.fan_count(), 1);
        assert!(service.set_fan_rpm(0, FanKey::Target, 2000));
        assert_eq!(smc.get("F1Tg"), 2000f32.to_le_bytes());
        assert!(service.set_fan_control_modes(&[true]));
        assert_eq!(smc.get("F1Md"), vec![1]);
    }
}
