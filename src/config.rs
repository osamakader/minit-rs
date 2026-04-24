use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::service::ServiceConfig;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub services: Vec<ServiceConfig>,
}

pub fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    if config.services.is_empty() {
        return Err("config has no services".into());
    }
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut seen_names = HashSet::new();

    for service in &config.services {
        if service.name.trim().is_empty() {
            return Err("service name cannot be empty".into());
        }
        if !seen_names.insert(service.name.as_str()) {
            return Err(format!("duplicate service name '{}'", service.name).into());
        }
        if service.command.is_empty() {
            return Err(format!("service '{}' has empty command", service.name).into());
        }
        if service.respawn_window_secs == 0 {
            return Err(format!(
                "service '{}' must have respawn_window_secs > 0",
                service.name
            )
            .into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Config, validate_config};
    use crate::service::{RestartPolicy, ServiceConfig};

    fn service(name: &str) -> ServiceConfig {
        ServiceConfig {
            name: name.to_string(),
            command: vec!["/bin/demo".to_string()],
            depends: Vec::new(),
            provides: Vec::new(),
            restart: RestartPolicy::OnFailure,
            respawn_delay_secs: 1,
            respawn_max: 0,
            respawn_window_secs: 60,
        }
    }

    #[test]
    fn reject_duplicate_service_names() {
        let config = Config {
            services: vec![service("demo"), service("demo")],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn reject_empty_commands() {
        let mut svc = service("demo");
        svc.command.clear();
        let config = Config {
            services: vec![svc],
        };

        assert!(validate_config(&config).is_err());
    }
}
