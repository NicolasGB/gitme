use color_eyre::{
    Result,
    eyre::{Context, ContextCompat, bail},
};
use inquire::{Confirm, Text, required};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub command_args: Vec<String>,
    #[serde(default)]
    pub repositories: Vec<Repository>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub system_path: Option<String>,
}

impl Config {
    pub(crate) fn new() -> Result<Self> {
        match Self::read_config()? {
            Some(conf) => Ok(conf),
            None => {
                //If no config found create it
                let default_config = Self::prompt_new_config()?;
                default_config.write_config()?;
                Ok(default_config)
            }
        }
    }

    // Reads the config if exists.
    fn read_config() -> Result<Option<Self>> {
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

        match config_file.exists() {
            true => {
                let contents =
                    std::fs::read_to_string(&config_file).wrap_err("Failed to read config file")?;
                toml::from_str(&contents).wrap_err("Failed to parse config file")
            }
            false => Ok(None),
        }
    }

    // Writes the given config
    fn write_config(&self) -> Result<()> {
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
        std::fs::write(
            &config_file,
            toml::to_string(self).wrap_err("Failed to marshall config file")?,
        )
        .wrap_err("Failed to write config file")?;

        Ok(())
    }

    fn prompt_new_config() -> Result<Self> {
        println!("Welcome to Gitme!");
        println!("No configuration file found, we're going to create one");
        println!();

        let username = Text::new("Whats your github username?")
            .with_validator(required!())
            .prompt()
            .wrap_err("Could not prompt username")?
            .trim()
            .to_string();

        let api_key = Text::new("Insert your github token:")
            .with_validator(required!())
            .prompt()
            .wrap_err("Could not prompt api_key")?
            .trim()
            .to_string();

        let command = Text::new("What command do you want to use for reviews?")
            .with_validator(required!())
            .prompt()
            .wrap_err("Could not prompt the command")?
            .trim()
            .to_string();

        let mut command_args = vec![];
        println!("If needed add argument(s) to your command, leave empty otherwise:");
        let mut arg_counter = 1;
        loop {
            let arg = Text::new(&format!("Argument {}:", arg_counter))
                .prompt()
                .wrap_err("could not prompt for command argument")?;
            let arg = arg.trim();
            if arg.is_empty() {
                break;
            }
            command_args.push(arg.to_string());

            // Increment the argument counter
            arg_counter += 1;
        }

        let mut repositories = vec![];

        // While the user want's to add a repository
        while Confirm::new("Do you wish to add a repository ?")
            .with_default(true)
            .prompt()?
        {
            repositories.push(Self::prompt_repository_input()?);
            println!();
        }

        Ok(Self {
            api_key: Some(api_key),
            username: Some(username),
            command: Some(command),
            command_args,
            repositories,
        })
    }

    fn prompt_repository_input() -> Result<Repository> {
        let owner = Text::new("Repository owner:")
            .with_validator(required!())
            .prompt()?
            .trim()
            .to_string();

        let name = Text::new("Repository name:")
            .with_validator(required!())
            .prompt()?
            .trim()
            .to_string();

        let system_path = if Confirm::new(
            "Do you wish to add a path to the local repository for review operations?",
        )
        .with_default(true)
        .prompt()?
        {
            let path = Text::new("Absolute path to local repository (~ is allowed):")
                .with_validator(required!())
                .prompt()?
                .trim()
                .to_string();
            Some(path)
        } else {
            None
        };

        Ok(Repository {
            owner,
            name,
            system_path,
        })
    }

    pub fn add_repository(&mut self) -> Result<()> {
        let new_repo = Self::prompt_repository_input()?;

        let exists = self
            .repositories
            .iter()
            .any(|r| r.owner == new_repo.owner && r.name == new_repo.name);

        // Return early if already exists
        if exists {
            bail!(format!(
                "The repository {}/{} already exists in the config",
                new_repo.owner, new_repo.name
            ))
        }

        // Otherwise append it to the config
        self.repositories.push(new_repo);

        // Write the config
        self.write_config()?;

        Ok(())
    }

    pub fn remove_repository(&mut self) -> Result<()> {
        Ok(())
    }
}
