//! Headless private worker for isolated managed-PostgreSQL fixtures. No server start.
fn main() {
    if fairway_core::postgresql::explorer::worker_entrypoint() {
        return;
    }
    if !fairway_core::postgresql::managed_health_worker() {
        std::process::exit(2);
    }
}
