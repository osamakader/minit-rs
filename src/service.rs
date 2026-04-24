use crate::log::debug_logs_enabled;
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

fn default_respawn_delay_secs() -> u64 {
    1
}

fn default_respawn_window_secs() -> u64 {
    60
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
    #[serde(default = "default_respawn_delay_secs")]
    pub respawn_delay_secs: u64,
    #[serde(default)]
    pub respawn_max: u32,
    #[serde(default = "default_respawn_window_secs")]
    pub respawn_window_secs: u64,
}

pub fn spawn_service(service: &ServiceConfig) -> Result<Pid, Box<dyn std::error::Error>> {
    if service.command.is_empty() {
        return Err(format!("service '{}' has empty command", service.name).into());
    }

    let mut child = Command::new(&service.command[0]);
    child.args(&service.command[1..]);
    let child = child.spawn()?;
    let pid = Pid::from_raw(child.id() as i32);
    if debug_logs_enabled() {
        println!("spawned service '{}' with pid {}", service.name, pid);
    }
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::{RestartPolicy, ServiceConfig};

    #[test]
    fn deserialize_service_defaults() {
        let service: ServiceConfig = serde_json::from_str(
            r#"{
                "name": "demo",
                "command": ["/bin/demo"]
            }"#,
        )
        .expect("service should deserialize");

        assert!(matches!(service.restart, RestartPolicy::OnFailure));
        assert_eq!(service.respawn_delay_secs, 1);
        assert_eq!(service.respawn_max, 0);
        assert_eq!(service.respawn_window_secs, 60);
    }
}
