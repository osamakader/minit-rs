use nix::unistd::Pid;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::config::load_config;
use crate::deps::resolve_start_order;
use crate::service::spawn_service;
use crate::signals::{register_shutdown_flag, terminate_running_services};
use crate::supervisor::{reap_children, should_restart};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_requested = register_shutdown_flag()?;

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/minit.json".to_string());
    let config = load_config(Path::new(&config_path))?;
    println!("loaded {} services from {}", config.services.len(), config_path);

    let mut running = HashMap::<Pid, usize>::new();
    let start_order = resolve_start_order(&config.services)?;
    for idx in start_order {
        let service = &config.services[idx];
        let pid = spawn_service(service)?;
        running.insert(pid, idx);
    }

    let mut shutdown_signal_sent = false;
    while !running.is_empty() {
        if shutdown_requested.load(Ordering::Relaxed) && !shutdown_signal_sent {
            terminate_running_services(&running);
            shutdown_signal_sent = true;
        }

        match reap_children()? {
            Some((pid, status)) => {
                let Some(service_idx) = running.remove(&pid) else {
                    println!("reaped unmanaged pid {} with status {:?}", pid, status);
                    continue;
                };
                let service = &config.services[service_idx];
                println!(
                    "reaped service '{}' pid {} with status {:?}",
                    service.name, pid, status
                );

                if !shutdown_requested.load(Ordering::Relaxed)
                    && should_restart(status, service.restart)
                {
                    println!("restarting service '{}'", service.name);
                    let new_pid = spawn_service(service)?;
                    running.insert(new_pid, service_idx);
                }
            }
            None => {
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    println!("all services exited");
    Ok(())
}
