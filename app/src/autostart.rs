//! "Autostart minimized when you log in": a Task Scheduler logon task that starts the
//! app elevated with `/minimized`.

use std::os::windows::process::CommandExt;
use std::process::Command;

/// Kept distinct from the original app's `MacsFanControl` task so both can coexist.
const TASK_NAME: &str = "RustyMacsFanControl";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks(args: &[&str]) -> bool {
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn task_xml(exe: &str) -> String {
    // The time limit is lifted: the scheduler would otherwise stop the app after 3 days.
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Rusty Macs Fan Control</Author>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>/minimized</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        xml_escape(exe)
    )
}

pub fn is_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME])
}

/// Creates or removes the logon task; returns whether the change took effect.
pub fn set_enabled(enabled: bool) -> bool {
    if !enabled {
        return schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let path = std::env::temp_dir().join("RustyMacsFanControl-task.xml");
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(task_xml(&exe.to_string_lossy()).encode_utf16().flat_map(u16::to_le_bytes));
    if std::fs::write(&path, bytes).is_err() {
        return false;
    }
    let created = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    created
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_definition_starts_the_exe_minimized_with_highest_privileges() {
        let xml = task_xml(r"C:\Apps\R&D\app.exe");
        assert!(xml.contains(r"<Command>C:\Apps\R&amp;D\app.exe</Command>"));
        assert!(xml.contains("<Arguments>/minimized</Arguments>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
    }
}
