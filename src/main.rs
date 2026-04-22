use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use serde::Deserialize;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct Config {
    services: Vec<ServiceConfig>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}

#[derive(Debug, Deserialize, Clone)]
struct ServiceConfig {
    name: String,
    command: Vec<String>,
    #[serde(default = "default_restart_policy")]
    restart: RestartPolicy,
}

fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    if config.services.is_empty() {
        return Err("config has no services".into());
    }
    Ok(config)
}

fn spawn_service(service: &ServiceConfig) -> Result<Pid, Box<dyn std::error::Error>> {
    if service.command.is_empty() {
        return Err(format!("service '{}' has empty command", service.name).into());
    }
    let mut child = Command::new(&service.command[0]);
    child.args(&service.command[1..]);
    let child = child.spawn()?;
    let pid = Pid::from_raw(child.id() as i32);
    println!("spawned service '{}' with pid {}", service.name, pid);
    Ok(pid)
}

fn should_restart(status: WaitStatus, policy: RestartPolicy) -> bool {
    match policy {
        RestartPolicy::Always => true,
        RestartPolicy::Never => false,
        RestartPolicy::OnFailure => match status {
            WaitStatus::Exited(_, code) => code != 0,
            WaitStatus::Signaled(_, _, _) => true,
            _ => false,
        },
    }
}

fn reap_children() -> Result<Option<(Pid, WaitStatus)>, nix::Error> {
    match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::StillAlive) => Ok(None),
        Ok(status @ WaitStatus::Exited(pid, _))
        | Ok(status @ WaitStatus::Signaled(pid, _, _))
        | Ok(status @ WaitStatus::Stopped(pid, _))
        | Ok(status @ WaitStatus::Continued(pid))
        | Ok(status @ WaitStatus::PtraceEvent(pid, _, _))
        | Ok(status @ WaitStatus::PtraceSyscall(pid)) => Ok(Some((pid, status))),
        Err(Errno::ECHILD) => Ok(None),
        Err(err) => Err(err),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&shutdown_requested))?;
    flag::register(SIGINT, Arc::clone(&shutdown_requested))?;

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/minit.json".to_string());
    let config = load_config(Path::new(&config_path))?;
    println!("loaded {} services from {}", config.services.len(), config_path);

    let mut running = HashMap::<Pid, usize>::new();
    for (idx, service) in config.services.iter().enumerate() {
        let pid = spawn_service(service)?;
        running.insert(pid, idx);
    }

    let mut shutdown_signal_sent = false;
    while !running.is_empty() {
        if shutdown_requested.load(Ordering::Relaxed) && !shutdown_signal_sent {
            println!("shutdown signal received, stopping services");
            for pid in running.keys() {
                if let Err(err) = kill(*pid, Signal::SIGTERM) {
                    eprintln!("failed to SIGTERM pid {}: {}", pid, err);
                }
            }
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
