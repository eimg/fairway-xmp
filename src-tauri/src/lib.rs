pub mod config;
pub mod database;
pub mod hosts;
pub mod integration;
pub mod manager;
pub mod process;
pub mod tls;

#[cfg(all(feature = "cli", unix))]
pub mod cli;

#[cfg(any(feature = "desktop", test))]
mod lifecycle;

#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
pub mod desktop;
