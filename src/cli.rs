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
        let gitme_config = config::Config::new()?;

        // Initialise octocrab
        let token = gitme_config.api_key.as_ref().cloned();
        let config = octocrab::OctocrabBuilder::new()
            .user_access_token(token.unwrap_or_default())
            .build()?;
        octocrab::initialise(config);

        match cli.command {
            Some(_) => todo!(),
            None => tui::run(gitme_config).await?,
        };

        Ok(())
    }
}
