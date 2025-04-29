use color_eyre::{
    Result,
    eyre::{Context, ContextCompat},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub command_args: Vec<String>,
    #[serde(default)]
    pub repositories: Vec<Repositories>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repositories {
    pub owner: String,
    pub name: String,
    pub system_path: Option<String>,
}

impl Config {
    pub(crate) fn new() -> Result<Self> {
        // Build the config dir path
        let config_dir = dirs::config_dir()
            .wrap_err("Failed to get config directory")?
            .join("gitme");

        // Check if it exists, otherwise creates it
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).wrap_err(format!(
                "Failed to create config directory {}",
                config_dir.display()
            ))?;
        }

        let config_file = config_dir.join("config.toml");

        if config_file.exists() {
            let contents =
                std::fs::read_to_string(&config_file).wrap_err("Failed to read config file")?;
            toml::from_str(&contents).wrap_err("Failed to parse config file")
        } else {
            let default_config = Config {
                api_key: None,
                username: None,
                repositories: Vec::new(),
                command: None,
                command_args: Vec::new(),
            };

            let toml = toml::to_string(&default_config).wrap_err("Failed to serialize config")?;

            std::fs::write(&config_file, toml).wrap_err("Failed to write config file")?;

            Ok(default_config)
        }
    }
}
