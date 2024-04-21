use std::process::{Command, Stdio};

use eyre::{eyre, Result, WrapErr};
use os_version::OsVersion;

pub fn is_nix_installed() -> bool {
    let output = Command::new("sh")
        .arg("-c")
        .arg("nix --version")
        .output()
        .expect("failed to execute process");

    output.status.success()
}

pub fn install_nix() -> Result<()> {
    // Install Nix if it is not already installed.
    if !is_nix_installed() {
        println!("*** Nix is NOT installed.");
        let current_os = os_version::detect()
            .map_err(|e| eyre!(format!("{:?}", e)))
            .wrap_err_with(|| "Failed to detect os version.")?;
        match current_os {
            // OsVersion::Linux(_) => {
            //     println!("*** Current OS is Linux, attempting Linux installation.");
            //     let mut child = Command::new("sh")
            //         .arg("-c")
            //         .arg("curl -L https://nixos.org/nix/install | sh -s -- --daemon")
            //         .stdout(Stdio::inherit())
            //         .spawn()?;
            //     child.wait()?;
            // }
            OsVersion::MacOS(_) | OsVersion::Linux(_) => {
                // We install Nix here, extras like nix-darwin are handled later
                let mut child = Command::new("sh")
                    .arg("-c")
                    .arg("curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install")
                    .stdout(Stdio::inherit())
                    .spawn()?;
                child.wait()?;
            }
            _ => {
                return Err(eyre!(
                    "Unsupported operating system. Currently only macOS and Linux are supported."
                ));
            }
        }
    }
    Ok(())
}
