//! One-shot AppKit handoffs; never target or poll a system security process.
use crate::config::Result;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
#[cfg(feature = "native-macos-hosts")]
use objc2_foundation::NSString;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, OnceLock,
};

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static PROMPTS: AtomicUsize = AtomicUsize::new(0);

pub(super) fn initialize(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
}

pub(super) fn in_progress() -> bool {
    PROMPTS.load(Ordering::SeqCst) != 0
}

/// Wait for the handoff before starting a blocking authorization operation. AppKit
/// always runs on the main thread; the authorization call stays on its worker.
fn on_main(action: impl FnOnce(MainThreadMarker) + Send + 'static) -> Result<()> {
    let Some(app) = APP.get() else {
        // Non-GUI maintenance commands and non-mutating native unit tests.
        return Ok(());
    };
    if let Some(mtm) = MainThreadMarker::new() {
        action(mtm);
        return Ok(());
    }
    let (send, receive) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        action(MainThreadMarker::new().expect("AppKit handoff must run on the main thread"));
        let _ = send.send(());
    })
    .map_err(|e| format!("Cannot hand control to macOS: {e}"))?;
    receive
        .recv()
        .map_err(|e| format!("Cannot complete the macOS focus handoff: {e}"))
}

#[must_use]
pub(crate) struct Authorization<'a>(&'a AtomicUsize);

impl<'a> Authorization<'a> {
    fn acquire(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count)
    }
}

impl Drop for Authorization<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
        // Never reactivate Fairway on success, cancellation or failure. The
        // user may still be interacting with Settings or another application.
    }
}

/// macOS owns its authorization UI and doesn't expose an application to
/// yield to. Relinquish Fairway's active appearance once, before asking for it.
/// Keep explicit Fairway focus requests suppressed until the native call ends.
pub(crate) fn begin() -> Result<Authorization<'static>> {
    let guard = Authorization::acquire(&PROMPTS);
    on_main(|mtm| {
        let app = NSApplication::sharedApplication(mtm);
        if app.isActive() {
            app.deactivate();
        }
    })?;
    Ok(guard)
}

/// Settings is a known application: use cooperative activation before the
/// ServiceManagement API opens its Login Items page. No extra launch or prompt.
#[cfg(feature = "native-macos-hosts")]
pub(crate) fn open_login_items() -> Result<()> {
    on_main(|mtm| {
        NSApplication::sharedApplication(mtm).yieldActivationToApplicationWithBundleIdentifier(
            &NSString::from_str("com.apple.systempreferences"),
        );
        unsafe {
            let _: () = objc2::msg_send![objc2::class!(SMAppService), openSystemSettingsLoginItems];
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_suppression_survives_overlapping_requests_and_releases_on_error() {
        let count = AtomicUsize::new(0);
        let first = Authorization::acquire(&count);
        let failed = || -> Result<()> {
            let _second = Authorization::acquire(&count);
            assert_eq!(count.load(Ordering::SeqCst), 2);
            Err("cancelled".into())
        };
        assert!(failed().is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        drop(first);
        assert_eq!(count.load(Ordering::SeqCst), 0);
        drop(Authorization::acquire(&count));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
