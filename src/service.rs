use nix::unistd::Pid;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default = "default_restart_policy")]
    pub restart: RestartPolicy,
}

pub fn spawn_service(service: &ServiceConfig) -> Result<Pid, Box<dyn std::error::Error>> {
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
