use std::path::PathBuf;

use eyre::{eyre, Result};
use os_version::OsVersion;

#[derive(Debug)]
pub struct Settings {
    pub force_evaluation: bool,
    pub update: bool,
    pub show_trace: bool,
    pub config_path: PathBuf,
    pub install_path: PathBuf,
    pub sync_exclusions: Vec<String>,
}

impl Settings {
    pub fn new() -> Result<Settings> {
        let config_path = PathBuf::from(shellexpand::tilde("~/.config/nix").into_owned());
        let os = os_version::detect().map_err(|e| eyre!("Failed to detect OS version: {:?}", e))?;
        println!("Current OS {:?}", os);
        let install_path = match os {
            OsVersion::Linux(l) if l.distro == "nixos" => PathBuf::from("/etc/nixos"),
            _ => PathBuf::from("/etc/nix-config"),
        };
        Ok(Settings {
            force_evaluation: false,
            update: false,
            show_trace: false,
            config_path,
            install_path,
            sync_exclusions: vec![".gitignore", ".stfolder", ".git", ".concierge-backup"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
    }

    pub fn force_evaluation(&mut self) {
        self.force_evaluation = true;
    }

    pub fn update(&mut self) {
        self.update = true;
    }

    pub fn show_trace(&mut self) {
        self.show_trace = true;
    }

    pub fn flake_file(&self) -> PathBuf {
        self.config_path.join("flake.nix")
    }

    pub fn push_exclusion<S: AsRef<str>>(&mut self, exclusion: S) {
        let exclusion = exclusion.as_ref();
        self.sync_exclusions.push(exclusion.to_string());
    }

    pub fn install_path_string(&self) -> String {
        self.install_path.to_string_lossy().into_owned()
    }

    pub fn config_path_string(&self) -> String {
        self.config_path.to_string_lossy().into_owned()
    }
}
