use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub fn register_shutdown_flag() -> Result<Arc<AtomicBool>, Box<dyn std::error::Error>> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&shutdown_requested))?;
    flag::register(SIGINT, Arc::clone(&shutdown_requested))?;
    Ok(shutdown_requested)
}

pub fn terminate_running_services(running: &HashMap<Pid, usize>) {
    println!("shutdown signal received, stopping services");
    for pid in running.keys() {
        if let Err(err) = kill(*pid, Signal::SIGTERM) {
            eprintln!("failed to SIGTERM pid {}: {}", pid, err);
        }
    }
}
