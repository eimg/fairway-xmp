//! Operator opt-in only: creates a new Keychain item and cluster in a fresh profile.
//! Never run against a personal data directory. Leaves its cluster/secret intact.
use fairway_core::{
    config::Layout,
    postgresql::{managed_contract::*, Managed},
};
use std::{fs, net::TcpListener, path::PathBuf};
fn main() -> Result<(), String> {
    if fairway_core::postgresql::managed_health_worker() {
        return Ok(());
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || args[0] != "--confirm-managed-cluster-init" {
        return Err("Usage: postgresql_managed_smoke --confirm-managed-cluster-init NEW_PROFILE RUNTIME_ROOT; run only in a disposable macOS account. This creates a Keychain item and retains the new cluster.".into());
    }
    let root = PathBuf::from(&args[1]);
    if !root.is_absolute()
        || root.parent().and_then(|p| p.canonicalize().ok()).as_deref() != root.parent()
    {
        return Err("Use a canonical absolute new profile with an existing parent".into());
    }
    fs::create_dir(&root)
        .map_err(|_| "Profile already exists or cannot be created; existing data kept")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect new profile")?;
    }
    let runtime = PathBuf::from(&args[2])
        .canonicalize()
        .map_err(|_| "Runtime root unavailable")?;
    // Layout::initialize and integration::refresh are deliberately not called.
    let state = Managed::new(Some(Layout::at(root.join("home"), root, runtime)));
    let confirmed = || Confirmation { confirmed: true };
    let result = (|| -> Result<(), String> {
        state.provision(confirmed()).map_err(|e| e.message)?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "No free loopback port")?;
        let port = listener
            .local_addr()
            .map_err(|_| "No free loopback port")?
            .port();
        drop(listener);
        let initialized = state
            .initialize(Initialize {
                confirmed: true,
                port,
            })
            .map_err(|e| e.message)?;
        assert!(!initialized.running);
        for restart in [false, true] {
            let snapshot = if restart {
                state.restart(confirmed())
            } else {
                state.start(confirmed())
            };
            let snapshot = snapshot.map_err(|e| e.message)?;
            if snapshot.authentication != "authenticated" || snapshot.responsiveness != "accepting"
            {
                return Err(
                    "Server access was not authenticated; inspect vault/signing qualification"
                        .into(),
                );
            }
        }
        Ok(())
    })();
    // Always request smart shutdown, including a failed authenticated-health check.
    let stopped = state.stop(confirmed()).map_err(|e| e.message);
    result.and(stopped.map(|_| ()))?;
    println!("PASS: native Keychain round-trip, initdb, authenticated start/restart and smart stop. New profile, cluster and Keychain item retained; no deletion performed.");
    Ok(())
}
