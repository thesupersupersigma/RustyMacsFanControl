//! Thin wrapper over one registry key under HKEY_CURRENT_USER.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD, REG_MULTI_SZ, REG_OPTION_NON_VOLATILE,
    REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_MULTI_SZ, RRF_RT_REG_SZ,
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Key(HKEY);

// Registry handles may be used from any thread.
unsafe impl Send for Key {}
unsafe impl Sync for Key {}

impl Key {
    /// Opens (creating if needed) `HKCU\<path>`.
    pub fn open_user(path: &str) -> Option<Key> {
        let mut hkey: HKEY = null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                null(),
                &mut hkey,
                null_mut(),
            )
        };
        (status == ERROR_SUCCESS).then_some(Key(hkey))
    }

    fn read_raw(&self, name: &str, flags: u32) -> Option<Vec<u8>> {
        let name = wide(name);
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(self.0, null(), name.as_ptr(), flags, null_mut(), null_mut(), &mut size)
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let status = unsafe {
            RegGetValueW(
                self.0,
                null(),
                name.as_ptr(),
                flags,
                null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        buf.truncate(size as usize);
        (status == ERROR_SUCCESS).then_some(buf)
    }

    fn write_raw(&self, name: &str, kind: u32, data: &[u8]) -> bool {
        let status = unsafe {
            RegSetValueExW(self.0, wide(name).as_ptr(), 0, kind, data.as_ptr(), data.len() as u32)
        };
        status == ERROR_SUCCESS
    }

    pub fn get_u32(&self, name: &str) -> Option<u32> {
        let raw = self.read_raw(name, RRF_RT_REG_DWORD)?;
        Some(u32::from_le_bytes(raw.get(..4)?.try_into().ok()?))
    }

    pub fn set_u32(&self, name: &str, value: u32) -> bool {
        self.write_raw(name, REG_DWORD, &value.to_le_bytes())
    }

    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        self.get_u32(name).map(|v| v != 0).unwrap_or(default)
    }

    pub fn set_bool(&self, name: &str, value: bool) -> bool {
        self.set_u32(name, value as u32)
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        let raw = self.read_raw(name, RRF_RT_REG_SZ)?;
        let units = utf16_units(&raw);
        let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
        Some(String::from_utf16_lossy(&units[..end]))
    }

    pub fn set_string(&self, name: &str, value: &str) -> bool {
        self.write_raw(name, REG_SZ, &utf16_bytes(&wide(value)))
    }

    pub fn get_strings(&self, name: &str) -> Vec<String> {
        let Some(raw) = self.read_raw(name, RRF_RT_REG_MULTI_SZ) else {
            return Vec::new();
        };
        utf16_units(&raw)
            .split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    pub fn set_strings(&self, name: &str, values: &[String]) -> bool {
        let mut units: Vec<u16> = Vec::new();
        for value in values {
            units.extend(value.encode_utf16());
            units.push(0);
        }
        units.push(0);
        self.write_raw(name, REG_MULTI_SZ, &utf16_bytes(&units))
    }

    pub fn delete(&self, name: &str) -> bool {
        unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) == ERROR_SUCCESS }
    }
}

/// Reads a string value from `HKLM\<path>` without opening the key for writing.
pub fn read_machine_string(path: &str, name: &str) -> Option<String> {
    let (path, name) = (wide(path), wide(name));
    let mut size = 0u32;
    let query = |buf: *mut u8, size: &mut u32| unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.cast(),
            size,
        )
    };
    if query(null_mut(), &mut size) != ERROR_SUCCESS {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    if query(buf.as_mut_ptr(), &mut size) != ERROR_SUCCESS {
        return None;
    }
    buf.truncate(size as usize);
    let units = utf16_units(&buf);
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    Some(String::from_utf16_lossy(&units[..end]))
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn utf16_units(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn utf16_bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|u| u.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_each_value_type() {
        let key = Key::open_user(r"Software\RustyMacsFanControl\UnitTest").unwrap();
        assert!(key.set_u32("Number", 42));
        assert_eq!(key.get_u32("Number"), Some(42));
        assert!(key.set_bool("Flag", true));
        assert!(key.get_bool("Flag", false));
        assert!(key.get_bool("Missing", true));
        assert!(key.set_string("Text", "English.xml"));
        assert_eq!(key.get_string("Text").as_deref(), Some("English.xml"));
        let list = vec!["one".to_string(), "two:1,2000".to_string()];
        assert!(key.set_strings("List", &list));
        assert_eq!(key.get_strings("List"), list);
        assert!(key.set_strings("List", &[]));
        assert!(key.get_strings("List").is_empty());
        for name in ["Number", "Flag", "Text", "List"] {
            assert!(key.delete(name));
        }
    }
}
