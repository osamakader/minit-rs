use nix::errno::Errno;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct Config {
    services: Vec<ServiceConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct ServiceConfig {
    name: String,
    command: Vec<String>,
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
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/minit.json".to_string());
    let config = load_config(Path::new(&config_path))?;
    println!("loaded {} services from {}", config.services.len(), config_path);

    let mut running = HashMap::<Pid, String>::new();
    for service in &config.services {
        let pid = spawn_service(service)?;
        running.insert(pid, service.name.clone());
    }

    while !running.is_empty() {
        match reap_children()? {
            Some((pid, status)) => {
                let service_name = running
                    .remove(&pid)
                    .unwrap_or_else(|| "<unknown>".to_string());
                println!(
                    "reaped service '{}' pid {} with status {:?}",
                    service_name, pid, status
                );
            }
            None => {
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    println!("all services exited");
    Ok(())
}
