//! OS login registration is independent of the service launch preferences.
use crate::config::Result;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub registered: bool,
    pub state: &'static str,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSAppleEventManager, NSError};

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    fn service() -> Retained<AnyObject> {
        unsafe { msg_send![class!(SMAppService), mainAppService] }
    }

    pub fn status() -> Result<Status> {
        let state: isize = unsafe { msg_send![&*service(), status] };
        Ok(match state {
            0 => Status {
                registered: false,
                state: "notRegistered",
            },
            1 => Status {
                registered: true,
                state: "enabled",
            },
            2 => Status {
                registered: true,
                state: "requiresApproval",
            },
            3 => Status {
                registered: false,
                state: "notFound",
            },
            _ => {
                return Err(format!(
                    "macOS returned an unknown Fairway XMP login item status ({state})."
                ))
            }
        })
    }

    pub fn set(enabled: bool) -> Result<Status> {
        let before = status()?;
        if enabled == before.registered {
            return Ok(before);
        }
        // Register only on explicit user action; never repair a user-disabled item at launch.
        let service = service();
        let mut error: Option<Retained<NSError>> = None;
        let success: bool = unsafe {
            if enabled {
                msg_send![&*service, registerAndReturnError: &mut error]
            } else {
                msg_send![&*service, unregisterAndReturnError: &mut error]
            }
        };
        let after = status()?;
        if !success && !(enabled && after.registered) {
            return Err(format!(
                "Cannot change Fairway XMP launch at login: {}",
                error.map_or_else(
                    || "No system error detail.".into(),
                    |error| error.localizedDescription().to_string()
                )
            ));
        }
        Ok(after)
    }

    pub fn open_settings() -> Result<()> {
        crate::desktop::authorization::open_login_items()
    }

    pub fn login_launch() -> bool {
        // Launch Apple Event: keyAEPropData == keyAELaunchedAsLogInItem.
        let Some(event) = NSAppleEventManager::sharedAppleEventManager().currentAppleEvent() else {
            return false;
        };
        // These CoreServices scalar selectors are bridged directly, without
        // adding the entire CoreServices dependency to Foundation.
        unsafe {
            let event_id: u32 = msg_send![&*event, eventID];
            if event_id != u32::from_be_bytes(*b"oapp") {
                return false;
            }
            let value: Option<Retained<AnyObject>> =
                msg_send![&*event, paramDescriptorForKeyword: u32::from_be_bytes(*b"prdt")];
            value.is_some_and(|value| {
                let code: u32 = msg_send![&*value, enumCodeValue];
                code == u32::from_be_bytes(*b"lgit")
            })
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
        RegKey,
    };

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Fairway XMP";

    fn command() -> Result<String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        Ok(format!("\"{}\" --startup", executable.to_string_lossy()))
    }

    pub fn status() -> Result<Status> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        let key = match user.open_subkey_with_flags(RUN, KEY_READ) {
            Ok(key) => key,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Status {
                    registered: false,
                    state: "notRegistered",
                })
            }
            Err(error) => return Err(error.to_string()),
        };
        let value: String = match key.get_value(NAME) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Status {
                    registered: false,
                    state: "notRegistered",
                })
            }
            Err(error) => return Err(error.to_string()),
        };
        // Registration is not Windows approval. Settings/Task Manager can disable
        // a Run entry independently; do not write the private StartupApproved keys.
        let registered = value == command()?;
        Ok(Status {
            registered,
            state: if registered {
                "registered"
            } else {
                "otherInstallation"
            },
        })
    }

    pub fn set(enabled: bool) -> Result<Status> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        if enabled {
            let (key, _) = user.create_subkey(RUN).map_err(|error| error.to_string())?;
            key.set_value(NAME, &command()?)
                .map_err(|error| error.to_string())?;
        } else {
            match user.open_subkey_with_flags(RUN, KEY_READ | KEY_WRITE) {
                Ok(key) => match key.get_value::<String, _>(NAME) {
                    Ok(value) if value != command()? => return status(),
                    Ok(_) => key.delete_value(NAME).map_err(|error| error.to_string())?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                    Err(error) => return Err(error.to_string()),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.to_string()),
            }
        }
        status()
    }

    pub fn open_settings() -> Result<()> {
        // Explorer can return a nonzero exit status after a successful shell
        // handoff, so only process creation determines success here.
        let mut child = std::process::Command::new("explorer.exe")
            .arg("ms-settings:startupapps")
            .spawn()
            .map_err(|error| error.to_string())?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    pub fn login_launch() -> bool {
        std::env::args().any(|arg| arg == "--startup")
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::*;
    pub fn status() -> Result<Status> {
        Err("Login items are supported on macOS and Windows.".into())
    }
    pub fn set(_: bool) -> Result<Status> {
        status()
    }
    pub fn open_settings() -> Result<()> {
        Err("Login items are supported on macOS and Windows.".into())
    }
    pub fn login_launch() -> bool {
        false
    }
}

pub use platform::{login_launch, open_settings, set, status};
