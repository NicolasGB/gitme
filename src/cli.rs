use clap::{Parser, Subcommand};
use color_eyre::Result;

use crate::{config, tui};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct GitMe {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(alias = "ar")]
    AddRepo,
    #[command(alias = "dr")]
    DeleteRepo,
}

impl GitMe {
    pub async fn run() -> Result<()> {
        let cli = Self::parse();
        // Get gitme config
        let mut gitme_config = config::Config::new()?;

        // Initialise octocrab
        let token = gitme_config.api_key.as_ref().cloned();
        let config = octocrab::OctocrabBuilder::new()
            .user_access_token(token.unwrap_or_default())
            .build()?;
        octocrab::initialise(config);

        match cli.command {
            Some(a) => match a {
                Command::AddRepo => gitme_config.add_repository()?,
                Command::DeleteRepo => gitme_config.remove_repository()?,
            },
            None => tui::run(gitme_config).await?,
        };

        Ok(())
    }
}
