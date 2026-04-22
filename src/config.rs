use std::fs;
use std::path::Path;
use serde::Deserialize;

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
    Ok(config)
}
