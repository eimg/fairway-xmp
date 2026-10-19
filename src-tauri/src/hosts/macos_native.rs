//! Experimental SMAppService registration. Approval belongs to macOS, not Fairway.
use crate::config::Result;
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSError, NSString};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

const LABEL: &str = "dev.fairway.environment.hosts-helper";
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

fn service() -> Retained<AnyObject> {
    let plist = NSString::from_str(&format!("{LABEL}.plist"));
    unsafe { msg_send![class!(SMAppService), daemonServiceWithPlistName: &*plist] }
}

fn helper() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let contents = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Cannot locate app bundle.")?;
    let helper = contents.join("Library/HelperTools").join(LABEL);
    if !helper.is_file()
        || !contents
            .join("Library/LaunchDaemons")
            .join(format!("{LABEL}.plist"))
            .is_file()
    {
        return Err("This prototype needs its bundled signed address helper.".into());
    }
    Ok(helper)
}

// A fresh service can report NotFound before its first registration (observed
// on macOS 27). The bundle payload is checked separately; this status must not
// prevent registration. Enabled/RequiresApproval must retain the user's choice.
fn register_if_needed(service: &AnyObject) -> Result<isize> {
    let mut status: isize = unsafe { msg_send![service, status] };
    if status == 0 || status == 3 {
        let mut error: Option<Retained<NSError>> = None;
        let success: bool = unsafe { msg_send![service, registerAndReturnError: &mut error] };
        // register can return EPERM while successfully recording a daemon
        // that awaits approval. Consult the resulting status even on failure.
        status = unsafe { msg_send![service, status] };
        if !success && status != 2 {
            let detail = error.map_or_else(
                || "No system error detail.".to_owned(),
                |error| {
                    format!(
                        "{} / {}: {} ({})",
                        error.domain(),
                        error.code(),
                        error.localizedDescription(),
                        error
                            .localizedFailureReason()
                            .map_or_else(String::new, |s| s.to_string())
                    )
                },
            );
            return Err(format!(
                "Cannot register address helper (status {status}): {detail}"
            ));
        }
    }
    Ok(status)
}

/// Fresh, signed/notarized app smoke check. Never approves the daemon, opens
/// Settings, connects to XPC or writes hosts. Refuses an existing registration.
pub fn test_registration() -> Result<()> {
    helper()?;
    let service = service();
    let before: isize = unsafe { msg_send![&service, status] };
    if before != 0 && before != 3 {
        return Err(format!(
            "Registration smoke check requires a fresh service (status {before})."
        ));
    }
    let outcome = register_if_needed(&service).and_then(|status| {
        println!("Address helper registration: {before} -> {status}");
        if status == 1 || status == 2 {
            Ok(())
        } else {
            Err(format!(
                "Address helper registration returned unexpected status {status}."
            ))
        }
    });
    // Even a failed status check must clean up the registration it attempted.
    let mut error: Option<Retained<NSError>> = None;
    let removed: bool = unsafe { msg_send![&service, unregisterAndReturnError: &mut error] };
    if !removed {
        return Err(format!(
            "Registration smoke cleanup failed: {error:?}; result: {outcome:?}"
        ));
    }
    outcome
}

pub fn unregister() -> Result<()> {
    let service = service();
    let status: isize = unsafe { msg_send![&service, status] };
    if status == 0 || status == 3 {
        return Ok(());
    }
    let mut error: Option<Retained<NSError>> = None;
    let success: bool = unsafe { msg_send![&service, unregisterAndReturnError: &mut error] };
    if success {
        Ok(())
    } else {
        Err(format!("Cannot remove address helper: {error:?}"))
    }
}

pub fn ensure(names: &[String]) -> Result<()> {
    if names.len() > 1024
        || names
            .iter()
            .any(|name| !crate::config::valid_project_name(name))
    {
        return Err("Invalid site domains or too many sites (maximum 1024).".into());
    }
    if super::current(names)? {
        if names.is_empty() {
            unregister()?;
        }
        return Ok(());
    }
    let helper = helper()?;
    let service = service();
    let status = register_if_needed(&service)?;
    if status == 2 {
        crate::desktop::authorization::open_login_items()?;
        return Err("Allow Fairway XMP in System Settings → General → Login Items & Extensions, then try the address change again.".into());
    }
    if status != 1 {
        return Err(format!(
            "macOS could not enable the address helper (status {status}). Try Set up addresses again."
        ));
    }
    let mut child = Command::new(helper)
        .arg("--client")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("Cannot send addresses to helper.")?
        .write_all(&serde_json::to_vec(names).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Address change failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !super::current(names)? {
        return Err("The address helper did not update the hosts file.".into());
    }
    if names.is_empty() {
        unregister()?;
    }
    Ok(())
}
