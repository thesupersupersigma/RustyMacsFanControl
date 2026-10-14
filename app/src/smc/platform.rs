//! Transports that carry SMC key requests to the hardware.

use std::path::PathBuf;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::model::Model;
use crate::registry::wide;
use crate::win_service::{StartResult, WinService};

const ORIGINAL_DOWNLOAD: &str = "https://crystalidea.com/macs-fan-control";

/// Four ASCII characters in string order, exactly as they go on the wire.
pub type Key = [u8; 4];

pub fn key(text: &str) -> Key {
    let mut out = [0u8; 4];
    for (slot, byte) in out.iter_mut().zip(text.bytes()) {
        *slot = byte;
    }
    out
}

pub fn key_text(key: &Key) -> String {
    let end = key.iter().position(|&b| b == 0).unwrap_or(4);
    String::from_utf8_lossy(&key[..end]).into_owned()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyInfo {
    pub size: u8,
    pub kind: [u8; 4],
    pub attributes: u8,
}

pub trait Provider: Send {
    fn key_by_index(&self, index: u32) -> Option<Key>;
    fn key_info(&self, key: &Key) -> Option<KeyInfo>;
    /// Fills `out` (zero-padded) with the key's value.
    fn read_key(&self, key: &Key, out: &mut [u8]) -> bool;
    fn write_key(&self, key: &Key, data: &[u8]) -> bool;
    fn name(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// `/applesmc`: skip the Boot Camp driver.
    pub force_apple_smc: bool,
}

struct Device(HANDLE);

// The handle is only used for DeviceIoControl, which is safe to call from any thread.
unsafe impl Send for Device {}

impl Device {
    fn open(path: &str, access: u32, share: u32) -> Option<Device> {
        let handle = unsafe {
            CreateFileW(
                wide(path).as_ptr(),
                access,
                share,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        (handle != INVALID_HANDLE_VALUE).then_some(Device(handle))
    }

    /// Returns the number of bytes the driver wrote to `output`.
    fn ioctl(&self, code: u32, input: &[u8], output: &mut [u8]) -> Option<u32> {
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                code,
                if input.is_empty() { null() } else { input.as_ptr().cast() },
                input.len() as u32,
                if output.is_empty() { null_mut() } else { output.as_mut_ptr().cast() },
                output.len() as u32,
                &mut returned,
                null_mut(),
            )
        };
        (ok != 0).then_some(returned)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

const APPLESMC_SERVICE: &str = "AppleSMC";
const IOCTL_APPLESMC_READ_KEY: u32 = 0x220000;
const IOCTL_APPLESMC_WRITE_KEY: u32 = 0x220004;
const IOCTL_APPLESMC_GET_KEY_BY_INDEX: u32 = 0x220008;
const IOCTL_APPLESMC_GET_KEY_INFO: u32 = 0x22000C;
const IOCTL_APPLESMC_GET_PROTOCOL: u32 = 0x220020;
const MAX_KEY_DATA: usize = 32;

/// The app's own `applesmc.sys` kernel driver.
pub struct AppleSmc {
    device: Device,
}

impl AppleSmc {
    pub fn open() -> Result<AppleSmc, String> {
        crate::info!("Connecting using our driver...");
        let service = WinService::open(APPLESMC_SERVICE)
            .ok_or_else(|| "The AppleSMC driver service could not be opened.".to_string())?;
        match service.start(500) {
            result if result.is_running() => {}
            result => {
                let error = match result {
                    StartResult::Failed(error) => error,
                    _ => 0,
                };
                crate::error!("Failed to start service {}, error {error}", service.name());
                service.stop();
                return Err(start_failure_message(error));
            }
        }
        let path = r"\\.\APPLESMC";
        let Some(device) =
            Device::open(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE)
        else {
            let error = unsafe { GetLastError() };
            crate::error!("Create file failed to open {path}, error {error}");
            service.stop();
            return Err(format!("The AppleSMC driver is running but could not be opened (Windows error {error})."));
        };
        let mut protocol = [0xFFu8];
        match device.ioctl(IOCTL_APPLESMC_GET_PROTOCOL, &[], &mut protocol) {
            Some(_) => crate::info!("Connected, protocol is {}", protocol[0]),
            None => crate::warn!("GetProtocol failed, error {}", unsafe { GetLastError() }),
        }
        Ok(AppleSmc { device })
    }

    fn log_failure(&self) {
        crate::error!("Error in DeviceIoControl {}", unsafe { GetLastError() });
    }
}

impl Provider for AppleSmc {
    fn key_by_index(&self, index: u32) -> Option<Key> {
        let mut out = [0u8; 4];
        match self.device.ioctl(IOCTL_APPLESMC_GET_KEY_BY_INDEX, &index.to_le_bytes(), &mut out) {
            Some(4) => Some(out),
            Some(_) => None,
            None => {
                self.log_failure();
                None
            }
        }
    }

    fn key_info(&self, key: &Key) -> Option<KeyInfo> {
        let mut out = [0u8; 6];
        match self.device.ioctl(IOCTL_APPLESMC_GET_KEY_INFO, key, &mut out) {
            Some(6) => Some(KeyInfo {
                size: out[0],
                kind: [out[1], out[2], out[3], out[4]],
                attributes: out[5],
            }),
            Some(n) => {
                crate::error!("IOCTL_APPLESMC_GET_SMC_KEY_INFO returned {n} bytes");
                None
            }
            None => {
                self.log_failure();
                None
            }
        }
    }

    fn read_key(&self, key: &Key, out: &mut [u8]) -> bool {
        let mut input = [0u8; 5];
        input[..4].copy_from_slice(key);
        input[4] = out.len() as u8;
        let mut buf = [0u8; MAX_KEY_DATA];
        match self.device.ioctl(IOCTL_APPLESMC_READ_KEY, &input, &mut buf) {
            Some(returned) => {
                let n = (returned as usize).min(out.len());
                out[..n].copy_from_slice(&buf[..n]);
                true
            }
            None => {
                self.log_failure();
                false
            }
        }
    }

    fn write_key(&self, key: &Key, data: &[u8]) -> bool {
        if data.len() > MAX_KEY_DATA {
            return false;
        }
        let mut input = [0u8; 5 + MAX_KEY_DATA];
        input[..4].copy_from_slice(key);
        input[4] = data.len() as u8;
        input[5..5 + data.len()].copy_from_slice(data);
        let mut out = [0u8; 1];
        match self.device.ioctl(IOCTL_APPLESMC_WRITE_KEY, &input[..5 + data.len()], &mut out) {
            Some(1) => true,
            Some(_) => false,
            None => {
                self.log_failure();
                false
            }
        }
    }

    fn name(&self) -> &'static str {
        "AppleSMC"
    }
}

impl Drop for AppleSmc {
    fn drop(&mut self) {
        // The driver is stopped whenever the app lets go of it.
        if let Some(service) = WinService::open(APPLESMC_SERVICE) {
            service.stop();
        }
    }
}

const BOOTCAMP_SERVICE: &str = "MacHALDriver";
const IOCTL_HAL_READ_KEY: u32 = 0x9C402454;
const IOCTL_HAL_WRITE_KEY: u32 = 0x9C402458;
const IOCTL_HAL_GET_KEY_BY_INDEX: u32 = 0x9C40245C;
const IOCTL_HAL_GET_KEY_INFO: u32 = 0x9C402460;

/// Apple's Boot Camp `MacHALDriver`.
pub struct BootCamp {
    device: Device,
}

impl BootCamp {
    pub fn open() -> Option<BootCamp> {
        let Some(device) = Device::open(r"\\.\MacHALDriver", GENERIC_READ, 0) else {
            crate::error!("Failed to open standard port X (error {}), will try inpout32", unsafe {
                GetLastError()
            });
            return None;
        };
        Some(BootCamp { device })
    }

    fn request(key: &Key) -> [u8; 5] {
        [key[0], key[1], key[2], key[3], 0]
    }
}

impl Provider for BootCamp {
    fn key_by_index(&self, index: u32) -> Option<Key> {
        let mut out = [0u8; 5];
        if self.device.ioctl(IOCTL_HAL_GET_KEY_BY_INDEX, &index.to_le_bytes(), &mut out).is_none()
        {
            crate::error!("getKeyByIndex: ioctl failed, GetLastError() = {}", unsafe {
                GetLastError()
            });
            return None;
        }
        Some([out[0], out[1], out[2], out[3]])
    }

    fn key_info(&self, key: &Key) -> Option<KeyInfo> {
        let mut out = [0u8; 12];
        if self.device.ioctl(IOCTL_HAL_GET_KEY_INFO, &Self::request(key), &mut out).is_none() {
            crate::error!("GetKeyInfo: ioctl failed, GetLastError() = {}", unsafe {
                GetLastError()
            });
            return None;
        }
        (out[0] != 0).then_some(KeyInfo {
            size: out[0],
            kind: [out[4], out[5], out[6], out[7]],
            attributes: out[8],
        })
    }

    fn read_key(&self, key: &Key, out: &mut [u8]) -> bool {
        // This driver takes the length from the output buffer size alone.
        let ok = self.device.ioctl(IOCTL_HAL_READ_KEY, &Self::request(key), out).is_some();
        if !ok {
            crate::error!("readKey: ioctl failed, GetLastError() = {}", unsafe { GetLastError() });
        }
        ok
    }

    fn write_key(&self, key: &Key, data: &[u8]) -> bool {
        let mut input = Self::request(key).to_vec();
        input.extend_from_slice(data);
        let ok = self.device.ioctl(IOCTL_HAL_WRITE_KEY, &input, &mut []).is_some();
        if !ok {
            crate::error!("writeKey: ioctl failed, GetLastError() = {}", unsafe {
                GetLastError()
            });
        }
        ok
    }

    fn name(&self) -> &'static str {
        "BootCamp"
    }
}

/// Where the original app's installer puts `applesmc.sys`, plus next to this exe.
fn driver_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) {
        candidates.push(dir.join("applesmc.sys"));
    }
    for program_files in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(dir) = std::env::var(program_files) {
            candidates.push(PathBuf::from(dir).join("Macs Fan Control").join("applesmc.sys"));
        }
    }
    candidates
}

/// Registers the AppleSMC driver service from an existing `applesmc.sys`, as the
/// original's installer does.
fn install_driver_service() -> Result<(), String> {
    let Some(driver) = driver_candidates().into_iter().find(|path| path.is_file()) else {
        return Err(format!(
            "The AppleSMC driver is not installed. This app uses the signed driver that comes \
             with the original Macs Fan Control, so install that first ({ORIGINAL_DOWNLOAD}). \
             It does not need to be running."
        ));
    };
    let path = driver.to_string_lossy().into_owned();
    crate::info!("Registering the {APPLESMC_SERVICE} service for {path}");
    WinService::create_kernel_driver(APPLESMC_SERVICE, "Apple SMC service", &path).map_err(|error| {
        crate::error!("CreateService failed with error {error}");
        format!("The AppleSMC driver service could not be registered (Windows error {error}).")
    })
}

fn start_failure_message(error: u32) -> String {
    match error {
        // ERROR_INVALID_IMAGE_HASH, ERROR_DRIVER_BLOCKED
        577 | 1275 => format!(
            "Windows refused to load the AppleSMC driver (Windows error {error}). This usually \
             means Memory Integrity is on: Windows Security > Device security > Core isolation."
        ),
        0 => "The AppleSMC driver did not start in time.".to_string(),
        _ => format!("The AppleSMC driver could not be started (Windows error {error})."),
    }
}

fn try_apple_smc() -> Result<Box<dyn Provider>, String> {
    if WinService::open(APPLESMC_SERVICE).is_none() {
        crate::warn!("AppleSMC service cannot be opened");
        install_driver_service()?;
    }
    let service = WinService::open(APPLESMC_SERVICE)
        .ok_or_else(|| "The AppleSMC driver service could not be opened.".to_string())?;
    if !service.is_enabled() {
        crate::warn!("AppleSMC service is not enabled");
        service.set_demand_start();
    }
    crate::info!("Trying PlatformWinProviderAppleSmc");
    AppleSmc::open().map(|p| Box::new(p) as Box<dyn Provider>)
}

fn try_boot_camp() -> Option<Box<dyn Provider>> {
    let Some(service) = WinService::open(BOOTCAMP_SERVICE) else {
        crate::error!("HAL service cannot be opened");
        return None;
    };
    if !service.is_enabled() {
        crate::error!("HAL service is not enabled");
        return None;
    }
    if !service.is_started() {
        crate::info!("HAL service is not started, trying to fix this");
        service.start(500);
    }
    if !service.is_started() {
        crate::error!("HAL service is not started");
        return None;
    }
    crate::info!("Using HAL");
    BootCamp::open().map(|p| Box::new(p) as Box<dyn Provider>)
}

fn is_windows_11() -> bool {
    crate::registry::read_machine_string(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )
    .and_then(|build| build.trim().parse::<u32>().ok())
    .is_some_and(|build| build > 21999)
}

/// Picks the transport the way the original does: T2 Macs, Windows 11 and `/applesmc`
/// use the applesmc driver alone; everything else prefers Boot Camp's driver. The error
/// explains why no transport could be opened.
pub fn open(model: &Model, options: Options) -> Result<Box<dyn Provider>, String> {
    if is_windows_11() || model.is_t2() || options.force_apple_smc {
        try_apple_smc()
    } else {
        try_boot_camp().ok_or(()).or_else(|_| try_apple_smc())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_packed_in_string_order() {
        assert_eq!(key("TC0P"), [0x54, 0x43, 0x30, 0x50]);
        assert_eq!(u32::from_le_bytes(key("TC0P")), 0x50304354);
        assert_eq!(key("FS! "), *b"FS! ");
        assert_eq!(key_text(&key("F0Ac")), "F0Ac");
    }
}
