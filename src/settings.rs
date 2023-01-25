use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use actix_web::error::{Error, ErrorBadRequest};
use anyhow::{anyhow, bail, Context, Result};
use log::{info, warn};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Webhook {
    pub name: String,
    pub command: String,
    pub cwd: PathBuf,
    #[serde(default = "default_pueue_group")]
    pub pueue_group: String,
}

fn default_pueue_group() -> String {
    "webhook".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub domain: String,
    pub port: i32,
    #[serde(default = "Default::default")]
    pub secret: Option<String>,
    #[serde(default = "Default::default")]
    pub ssl_private_key: Option<String>,
    #[serde(default = "Default::default")]
    pub ssl_cert_chain: Option<String>,
    #[serde(default = "Default::default")]
    pub basic_auth_user: Option<String>,
    #[serde(default = "Default::default")]
    pub basic_auth_password: Option<String>,
    #[serde(default = "Default::default")]
    pub basic_auth_and_secret: bool,
    #[serde(default = "Default::default")]
    pub webhooks: Vec<Webhook>,
}

impl Settings {
    pub fn new() -> Result<Self> {
        info!("Init settings file");
        let settings = parse_config()?;

        if settings.basic_auth_password.is_some() || settings.basic_auth_user.is_some() {
            settings
                .basic_auth_user
                .as_ref()
                .ok_or_else(|| anyhow!("Can't find basic_auth_user in config"))?;
            settings
                .basic_auth_password
                .as_ref()
                .ok_or_else(|| anyhow!("Can't find basic_auth_password in config"))?;
        }

        // Verify that everything is in place, if `basic_auth_and_secret` is activated
        if settings.basic_auth_and_secret {
            settings
                .secret
                .as_ref()
                .ok_or_else(|| anyhow!("Can't find secret in config"))?;
            settings
                .basic_auth_user
                .as_ref()
                .ok_or_else(|| anyhow!("Can't find basic_auth_user in config"))?;
            settings
                .basic_auth_password
                .as_ref()
                .ok_or_else(|| anyhow!("Can't find basic_auth_password in config"))?;
        }

        Ok(settings)
    }

    /// Get settings for a specific webhook
    pub fn get_webhook_by_name(&self, name: &str) -> Result<Webhook, Error> {
        for webhook in self.webhooks.iter() {
            if webhook.name == name {
                return Ok(webhook.clone());
            }
        }

        let error = format!("Can't find webhook with name: {name}");
        warn!("{}", error);
        Err(ErrorBadRequest(error))
    }
}

fn parse_config() -> Result<Settings> {
    info!("Parsing config files");
    let config_paths = get_config_paths()?;

    for path in config_paths.into_iter() {
        info!("Checking path: {:?}", &path);
        if path.exists() {
            info!("Using config file at: {:?}", path);
            let file = File::open(path).context("Failed to open file.")?;
            let reader = BufReader::new(file);

            return serde_yaml::from_reader(reader).context("Failed to deserialize settings");
        }
    }

    bail!("Can't find suitable settings file")
}

#[cfg(target_os = "linux")]
fn get_config_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Can't resolve home dir"))?;
    paths.push(Path::new("/etc/webhook_server.yml").to_path_buf());
    paths.push(home_dir.join(".config/webhook_server.yml"));
    paths.push(Path::new("./webhook_server.yml").to_path_buf());

    Ok(paths)
}

#[cfg(target_os = "windows")]
fn get_config_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Can't resolve home dir"))?;
    paths.push(home_dir.join("AppData\\Roaming\\webhook_server\\webhook_server.yml"));
    paths.push(Path::new(".\\webhook_server.yml").to_path_buf());

    Ok(paths)
}

#[cfg(target_os = "macos")]
fn get_config_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Can't resolve home dir"))?;
    paths.push(home_dir.join("Library/Application Support/webhook_server.yml"));
    paths.push(home_dir.join("Library/Preferences/webhook_server.yml"));
    paths.push(Path::new("./webhook_server.yml").to_path_buf());

    Ok(paths)
}
