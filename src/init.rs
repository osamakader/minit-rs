use nix::unistd::Pid;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::load_config;
use crate::deps::resolve_start_order;
use crate::log::debug_logs_enabled;
use crate::service::spawn_service;
use crate::signals::{register_shutdown_flag, signal_services};
use crate::supervisor::{next_restart_delay, reap_children, should_restart};
use nix::sys::signal::Signal;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE_SLEEP: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
struct ServiceRuntime {
    last_start: Option<Instant>,
    restart_count: u32,
    next_restart_at: Option<Instant>,
}

impl ServiceRuntime {
    fn new() -> Self {
        Self {
            last_start: None,
            restart_count: 0,
            next_restart_at: None,
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_requested = register_shutdown_flag()?;

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/minit.json".to_string());
    let config = load_config(Path::new(&config_path))?;
    if debug_logs_enabled() {
        println!(
            "loaded {} services from {}",
            config.services.len(),
            config_path
        );
    }

    let mut running = HashMap::<Pid, usize>::new();
    let start_order = resolve_start_order(&config.services)?;
    let mut runtimes = vec![ServiceRuntime::new(); config.services.len()];
    for &idx in &start_order {
        let service = &config.services[idx];
        let pid = spawn_service(service)?;
        runtimes[idx].last_start = Some(Instant::now());
        running.insert(pid, idx);
    }

    let mut shutdown_signal_sent = false;
    let mut shutdown_deadline = None;
    while !running.is_empty() || has_pending_restarts(&runtimes) {
        if shutdown_requested.load(Ordering::Relaxed) && !shutdown_signal_sent {
            println!("shutdown signal received, stopping services");
            let ordered = ordered_running_pids(&running, &start_order);
            signal_services(&ordered, Signal::SIGTERM);
            shutdown_signal_sent = true;
            shutdown_deadline = Some(Instant::now() + SHUTDOWN_TIMEOUT);
        }

        match reap_children()? {
            Some((pid, status)) => {
                let Some(service_idx) = running.remove(&pid) else {
                    println!("reaped unmanaged pid {} with status {:?}", pid, status);
                    continue;
                };
                let service = &config.services[service_idx];
                if debug_logs_enabled() {
                    println!(
                        "reaped service '{}' pid {} with status {:?}",
                        service.name, pid, status
                    );
                }
                runtimes[service_idx].next_restart_at = None;

                if !shutdown_requested.load(Ordering::Relaxed)
                    && should_restart(status, service.restart)
                {
                    let Some(last_start) = runtimes[service_idx].last_start else {
                        return Err(format!(
                            "service '{}' exited without recorded start time",
                            service.name
                        )
                        .into());
                    };

                    match next_restart_delay(
                        Instant::now(),
                        last_start,
                        &mut runtimes[service_idx].restart_count,
                        service.respawn_delay_secs,
                        service.respawn_window_secs,
                        service.respawn_max,
                    ) {
                        Some(delay) => {
                            println!(
                                "restarting service '{}' in {}s",
                                service.name,
                                delay.as_secs()
                            );
                            runtimes[service_idx].next_restart_at = Some(Instant::now() + delay);
                        }
                        None => {
                            println!(
                                "service '{}' reached respawn limit and will stay stopped",
                                service.name
                            );
                        }
                    }
                }
            }
            None => {
                restart_ready_services(&config.services, &mut runtimes, &mut running)?;
                if let Some(deadline) = shutdown_deadline {
                    if Instant::now() >= deadline && !running.is_empty() {
                        let ordered = ordered_running_pids(&running, &start_order);
                        signal_services(&ordered, Signal::SIGKILL);
                        shutdown_deadline = None;
                    }
                }
                thread::sleep(IDLE_SLEEP);
            }
        }
    }

    if debug_logs_enabled() {
        println!("all services exited");
    }

    Ok(())
}

fn ordered_running_pids(running: &HashMap<Pid, usize>, start_order: &[usize]) -> Vec<Pid> {
    let mut ordered = Vec::new();
    for &idx in start_order.iter().rev() {
        if let Some((pid, _)) = running.iter().find(|(_, service_idx)| **service_idx == idx) {
            ordered.push(*pid);
        }
    }
    ordered
}

fn restart_ready_services(
    services: &[crate::service::ServiceConfig],
    runtimes: &mut [ServiceRuntime],
    running: &mut HashMap<Pid, usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = Instant::now();
    for (idx, service) in services.iter().enumerate() {
        let Some(restart_at) = runtimes[idx].next_restart_at else {
            continue;
        };
        if now < restart_at {
            continue;
        }
        if !dependencies_ready(idx, services, running) {
            continue;
        }

        let pid = spawn_service(service)?;
        runtimes[idx].last_start = Some(now);
        runtimes[idx].next_restart_at = None;
        running.insert(pid, idx);
    }

    Ok(())
}

fn has_pending_restarts(runtimes: &[ServiceRuntime]) -> bool {
    runtimes
        .iter()
        .any(|runtime| runtime.next_restart_at.is_some())
}

fn dependencies_ready(
    service_idx: usize,
    services: &[crate::service::ServiceConfig],
    running: &HashMap<Pid, usize>,
) -> bool {
    services[service_idx].depends.iter().all(|dep| {
        let Some(provider_idx) = services
            .iter()
            .position(|service| service.name == *dep || service.provides.iter().any(|p| p == dep))
        else {
            return false;
        };

        provider_idx == service_idx || running.values().any(|idx| *idx == provider_idx)
    })
}
