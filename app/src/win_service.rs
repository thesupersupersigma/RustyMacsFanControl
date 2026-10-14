//! Service Control Manager helpers for the kernel drivers the SMC providers rely on.

use std::ptr::{null, null_mut};
use std::thread::sleep;
use std::time::Duration;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, CreateServiceW, DeleteService,
    OpenSCManagerW,
    OpenServiceW,
    QueryServiceConfigW, QueryServiceStatus, StartServiceW, QUERY_SERVICE_CONFIGW, SC_HANDLE,
    SC_MANAGER_ALL_ACCESS, SERVICES_ACTIVE_DATABASEW, SERVICE_ALL_ACCESS, SERVICE_AUTO_START,
    SERVICE_ERROR_NORMAL, SERVICE_KERNEL_DRIVER,
    SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_NO_CHANGE,
    SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STOPPED, SERVICE_STOP_PENDING,
};

use crate::registry::wide;

pub struct WinService {
    manager: SC_HANDLE,
    handle: SC_HANDLE,
    name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartResult {
    /// With the Windows error code.
    Failed(u32),
    StopPending,
    TimedOut,
    Started,
    AlreadyRunning,
}

impl StartResult {
    pub fn is_running(self) -> bool {
        matches!(self, StartResult::Started | StartResult::AlreadyRunning)
    }
}

impl WinService {
    pub fn open(name: &str) -> Option<WinService> {
        let manager =
            unsafe { OpenSCManagerW(null(), SERVICES_ACTIVE_DATABASEW, SC_MANAGER_ALL_ACCESS) };
        if manager.is_null() {
            crate::error!("OpenSCManager failed with error {}", unsafe { GetLastError() });
            return None;
        }
        let handle = unsafe { OpenServiceW(manager, wide(name).as_ptr(), SERVICE_ALL_ACCESS) };
        if handle.is_null() {
            crate::error!("OpenService ({name}) failed with error {}", unsafe { GetLastError() });
            unsafe { CloseServiceHandle(manager) };
            return None;
        }
        Some(WinService { manager, handle, name: name.to_string() })
    }

    /// Registers a demand-start kernel driver service; returns the Windows error on failure.
    pub fn create_kernel_driver(name: &str, display_name: &str, path: &str) -> Result<(), u32> {
        unsafe {
            let manager = OpenSCManagerW(null(), null(), SC_MANAGER_ALL_ACCESS);
            if manager.is_null() {
                return Err(GetLastError());
            }
            let handle = CreateServiceW(
                manager,
                wide(name).as_ptr(),
                wide(display_name).as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_KERNEL_DRIVER,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_NORMAL,
                wide(path).as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
            );
            let result = if handle.is_null() { Err(GetLastError()) } else { Ok(()) };
            if !handle.is_null() {
                CloseServiceHandle(handle);
            }
            CloseServiceHandle(manager);
            result
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn state(&self) -> Option<u32> {
        let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
        (unsafe { QueryServiceStatus(self.handle, &mut status) } != 0)
            .then_some(status.dwCurrentState)
    }

    /// Start type and binary path as registered.
    fn config(&self) -> Option<(u32, String)> {
        let mut needed = 0u32;
        unsafe { QueryServiceConfigW(self.handle, null_mut(), 0, &mut needed) };
        if needed == 0 {
            return None;
        }
        // u64 storage keeps the buffer aligned for QUERY_SERVICE_CONFIGW.
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        let config = buf.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        if unsafe { QueryServiceConfigW(self.handle, config, needed, &mut needed) } == 0 {
            return None;
        }
        unsafe {
            let path_ptr = (*config).lpBinaryPathName;
            let mut len = 0;
            while !path_ptr.is_null() && *path_ptr.add(len) != 0 {
                len += 1;
            }
            let path = if path_ptr.is_null() {
                String::new()
            } else {
                String::from_utf16_lossy(std::slice::from_raw_parts(path_ptr, len))
            };
            Some(((*config).dwStartType, path))
        }
    }

    /// A service counts as enabled unless it is auto-start or disabled; a failed query
    /// counts as not enabled.
    pub fn is_enabled(&self) -> bool {
        self.config().is_some_and(|(start_type, _)| {
            start_type != SERVICE_AUTO_START && start_type != SERVICE_DISABLED
        })
    }

    /// The driver file the service loads, without the `\??\` prefix the SCM adds.
    pub fn binary_path(&self) -> Option<String> {
        self.config().map(|(_, path)| path.trim_start_matches(r"\??\").to_string())
    }

    pub fn set_binary_path(&self, path: &str) -> bool {
        unsafe {
            ChangeServiceConfigW(
                self.handle,
                SERVICE_NO_CHANGE,
                SERVICE_NO_CHANGE,
                SERVICE_NO_CHANGE,
                wide(path).as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
                null(),
            ) != 0
        }
    }

    /// Marks the service for deletion; it goes once stopped and every handle is closed.
    pub fn delete(&self) -> bool {
        unsafe { DeleteService(self.handle) != 0 }
    }

    pub fn is_started(&self) -> bool {
        self.state() == Some(SERVICE_RUNNING)
    }

    pub fn set_demand_start(&self) -> bool {
        unsafe {
            ChangeServiceConfigW(
                self.handle,
                SERVICE_NO_CHANGE,
                SERVICE_DEMAND_START,
                SERVICE_NO_CHANGE,
                null(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
                null(),
            ) != 0
        }
    }

    fn wait_for_state(&self, state: u32, mut timeout_ms: i32) -> bool {
        loop {
            if self.state() == Some(state) {
                return true;
            }
            if timeout_ms < 0 {
                return false;
            }
            sleep(Duration::from_millis(100));
            timeout_ms -= 100;
        }
    }

    pub fn start(&self, timeout_ms: i32) -> StartResult {
        match self.state() {
            Some(SERVICE_STOP_PENDING) => return StartResult::StopPending,
            Some(SERVICE_RUNNING) => return StartResult::AlreadyRunning,
            _ => {}
        }
        if unsafe { StartServiceW(self.handle, 0, null()) } == 0 {
            let error = unsafe { GetLastError() };
            crate::error!("StartService failed with error {error}");
            return StartResult::Failed(error);
        }
        if timeout_ms == 0 {
            return StartResult::Failed(0);
        }
        if self.wait_for_state(SERVICE_RUNNING, timeout_ms) {
            StartResult::Started
        } else {
            StartResult::TimedOut
        }
    }

    pub fn stop(&self) {
        crate::info!("Trying to stop service {}", self.name);
        match self.state() {
            Some(SERVICE_STOP_PENDING) | Some(SERVICE_STOPPED) => {}
            _ => {
                let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
                unsafe { ControlService(self.handle, SERVICE_CONTROL_STOP, &mut status) };
            }
        }
    }
}

impl Drop for WinService {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.handle);
            CloseServiceHandle(self.manager);
        }
    }
}
