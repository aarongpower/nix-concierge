use clap::Parser;
use eyre::{eyre, Context, Result};
use log::debug;
use nix::install_nix;
use settings::Settings;

use crate::deploy::deploy_nix_configuration;

mod config;
pub mod deploy;
mod error;
pub mod fs;
pub mod git;
mod nix;
pub mod settings;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Force re-evaluation by tagging flake.nix
    #[arg(short, long)]
    force_eval: bool,

    /// Update packages to latest versions
    #[arg(short, long)]
    update: bool,
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();
    // Install Nix if not currently installed.
    debug!("Checking nix installation");
    install_nix().wrap_err_with(|| "Error installing Nix.")?;

    // now that we know there is a config in the expected loaction, let's deploy ita
    debug!("Initialising settings");
    let mut settings = Settings::new().wrap_err_with(|| "Failed creating settings")?;
    debug!("Settings initialised:\n{:?}", settings);

    if args.force_eval {
        settings.force_evaluation();
    }

    if args.update {
        settings.update()
    }

    // Check that configuration is present
    debug!("Checking if flake.nix exists in config dir");
    if !settings.flake_file().exists() {
        return Err(eyre!(
            "flake.nix not found in expected location: {}",
            &settings.flake_file().to_string_lossy()
        ));
    } else {
        debug!(
            "flake.nix exists at {}",
            settings.flake_file().to_string_lossy()
        );
    }

    debug!("Deploying nix configuration");
    deploy_nix_configuration(settings)
        .wrap_err_with(|| "Failed to deploy and build nix configuration")?;

    Ok(())
}
