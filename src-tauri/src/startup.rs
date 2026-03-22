/// Windows startup task management via Task Scheduler.
///
/// Uses `schtasks.exe` to create/query/delete a scheduled task that launches
/// the app at user logon with HighestAvailable (admin) run level.

use log::{info, warn};
use std::env;
use std::fs;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const TASK_NAME: &str = "TuringSmartScreenStartup";

const TASK_XML_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>{AUTHOR}</Author>
    <Description>Start Turing Smart Screen at startup</Description>
    <URI>\TuringSmartScreenStartup</URI>
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
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>true</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <Priority>0</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{EXE_PATH}</Command>
    </Exec>
  </Actions>
</Task>"#;

/// Check if the startup task exists in Windows Task Scheduler.
pub fn get_run_on_startup() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        if let Ok(out) = output {
            out.status.success()
        } else {
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Enable or disable startup task.
pub fn set_run_on_startup(enable: bool) {
    if enable {
        create_startup_task();
    } else {
        delete_startup_task();
    }
}

fn create_startup_task() {
    #[cfg(target_os = "windows")]
    {
        let exe_path = env::current_exe().unwrap_or_default();
        let exe_str = exe_path.to_string_lossy();
        let author = env::var("USERNAME").unwrap_or_else(|_| "User".to_string());

        let xml_content = TASK_XML_TEMPLATE
            .replace("{AUTHOR}", &author)
            .replace("{EXE_PATH}", &exe_str);

        let temp_xml_path = env::temp_dir().join("turing_screen_startup.xml");

        // Write UTF-16 LE with BOM (schtasks requires this format)
        let mut utf16_bom: Vec<u8> = vec![0xFF, 0xFE];
        for c in xml_content.encode_utf16() {
            utf16_bom.push((c & 0xFF) as u8);
            utf16_bom.push((c >> 8) as u8);
        }

        if let Err(e) = fs::write(&temp_xml_path, utf16_bom) {
            warn!("Failed to write startup task XML: {}", e);
            return;
        }

        let path_str = temp_xml_path.to_string_lossy().to_string();

        // App already runs as admin, so schtasks should succeed directly
        let output = Command::new("schtasks")
            .args(["/Create", "/TN", TASK_NAME, "/XML", &path_str, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                info!("Startup task created successfully");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("schtasks create failed: {}", stderr);
                // Fallback: try elevated via PowerShell
                elevated_schtasks(&format!(
                    "/Create /TN \"{}\" /XML \"{}\" /F",
                    TASK_NAME, path_str
                ));
            }
            Err(e) => {
                warn!("Failed to run schtasks: {}", e);
            }
        }

        let _ = fs::remove_file(temp_xml_path);
    }
}

fn delete_startup_task() {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                info!("Startup task deleted successfully");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("schtasks delete failed: {}", stderr);
                elevated_schtasks(&format!("/Delete /TN \"{}\" /F", TASK_NAME));
            }
            Err(e) => {
                warn!("Failed to run schtasks: {}", e);
            }
        }
    }
}

/// Fallback: run schtasks via elevated PowerShell.
#[cfg(target_os = "windows")]
fn elevated_schtasks(args: &str) {
    let _ = Command::new("powershell")
        .args([
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process schtasks -ArgumentList '{}' -WindowStyle Hidden -Verb RunAs -Wait",
                args
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}
