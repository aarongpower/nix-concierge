use std::fs;
use std::fs::{read_to_string, write, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::{DateTime, Local, TimeZone};
// use colored::*;
use eyre::{eyre, ContextCompat, OptionExt, Result, WrapErr};
// use git2::TreeBuilder;
use log::debug;
use os_version::OsVersion;

use crate::hash::hash_file;
use crate::settings::Settings;

/// Deploy configuration from source to target using rsync
/// then use platform appropriate tools to build and apply configuration
/// using nix
pub fn deploy_nix_configuration(settings: Settings, hostname: String) -> Result<()> {
    // We will assume source git repo state is valid, that stuff is handled elsewhere
    // Confirm that source at least has a flake.nix
    // Use rsync to copy from source to destination
    // Use platform appropriate tool to build and apply config
    //   - in particular, check if nix-darwin is installed on macOS and bootstrap it if not

    debug!("Deploying Nix configuration with settings: {:?}", settings);
    let os = os_version::detect().map_err(|e| eyre!("Failed to detect OS: {:?}", e))?;

    let deployment_time = Local::now();

    // check that source directory has a flake.nix
    if !settings.flake_file().exists() {
        return Err(eyre!(format!(
            "Config source dir {:?} does not contain flake.nix",
            settings.config_path
        )));
    }

    if settings.force_evaluation {
        backup_file(settings.flake_file(), Local::now())
            .wrap_err_with(|| "Failed to backup flake.nix before tagging")?;
        tag_file_content(settings.flake_file(), deployment_time).wrap_err_with(|| {
            format!(
                "Failed to tag file to force evaluation: {}",
                settings.flake_file().to_string_lossy()
            )
        })?;
    };

    // tag files named `docker-compose.nix` to force pulling latest docker images during update
    if settings.update {
        for file in search_files_with_name(&settings.config_path, "docker-compose.yml")? {
            tag_file_content(file, deployment_time.clone())?;
        }
    }

    if let Some(name) = settings.update_input {
        println!("Updating input {}", &name);
        realtime_command_in_dir(
            "nix",
            settings.config_path.clone(),
            vec!["flake", "update", name.as_str()],
            format!("Error updating unput {}", name).as_str(),
        )?;
    }

    // rsync from config to install dir
    rsync(
        settings.config_path.clone(),
        settings.install_path.clone(),
        settings.sync_exclusions.clone(),
        vec!["-ahi".to_string(), "--delete".to_string()],
        true,
    )
    .wrap_err_with(|| "Failed rsync")?;

    let mut update_command: Vec<&str> = vec![];

    if let OsVersion::MacOS(_) = os {
    } else {
        update_command.push("sudo");
    }

    // run flake update if update option is true
    update_command.extend(vec!["nix", "flake", "update", "--flake"]);

    if settings.show_trace {
        update_command.push("-vv");
    };

    if settings.fallback {
        update_command.push("--fallback");
    }

    if settings.show_trace {
        update_command.push("--show-trace");
    }

    update_command.push(
        settings
            .install_path
            .as_os_str()
            .to_str()
            .ok_or_eyre("Failed to reslove installation path for deployment command")?,
    );

    if settings.update {
        realtime_command_vec(
            update_command,
            "Failed syncing configuration to installation location",
        )?;
    };

    match os {
        OsVersion::Linux(l) if l.distro == "nixos" => realtime_command(
            "sudo",
            vec!["nixos-rebuild", "switch"],
            "Failed to bulid and apply Nix configuration",
        )?,
        OsVersion::MacOS(_) => realtime_command(
            "darwin-rebuild",
            vec![
                "switch",
                "--flake",
                settings
                    .install_path
                    .as_os_str()
                    .to_str()
                    .wrap_err_with(|| {
                        format!(
                            "Failed to convert install path to string: {:?}",
                            settings.install_path
                        )
                    })?,
            ],
            "Failed to build and apply nix configuration",
        )?,
        _ => return Err(eyre!("Unsupported OS")),
    }

    // pull back any changed flake.lock files
    rsync(
        settings.install_path,
        settings.config_path,
        vec!["*"],
        vec!["-aim", "--include='*.lock'", "--include='*/'"],
        true,
    )
    .wrap_err_with(|| "Failed syncing updated .lock files back to config dir")?;

    Ok(())
}

fn rsync<P: AsRef<Path>, S: AsRef<str>>(
    source: P,
    destination: P,
    exclusions: Vec<S>,
    params: Vec<S>,
    sudo: bool,
) -> Result<()> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    let exclusions: Vec<&str> = exclusions.iter().map(|s| s.as_ref()).collect();
    let params: Vec<&str> = params.iter().map(|s| s.as_ref()).collect();

    let source_str = source
        .to_str()
        .wrap_err_with(|| format!("Failed to get source path string {:?}", source))?;
    let destination_str = destination
        .to_str()
        .wrap_err_with(|| format!("Failed to get destination path string {:?}", destination))?;

    let exclusions_string = exclusions.iter().fold(String::new(), |mut acc, item| {
        acc.push_str(" --exclude=");
        acc.push_str(format!("'{}'", item).as_str());
        acc
    });

    let extra_params_string = params.iter().fold(String::new(), |mut acc, item| {
        acc.push_str(" ");
        acc.push_str(item);
        acc
    });

    let mut rsync_command = format!(
        "rsync {} {} {}/ {}/",
        extra_params_string, exclusions_string, source_str, destination_str
    );

    if sudo {
        rsync_command = format! {"{} {}", "sudo", rsync_command};
    }

    debug!("Running rsync with command {}", &rsync_command);

    realtime_command(
        "nix-shell",
        vec!["-p", "rsync", "--run", &rsync_command],
        "Failed executing rsync",
    )
}

// fn run_compose2nix<P: AsRef<Path>, S: AsRef<str>>(dir: P, name: S, failure_msg: S) -> Result<()> {
//     realtime_command_in_dir(
//         "compose2nix",
//         dir,
//         vec!["-project", name.as_ref()],
//         failure_msg.as_ref(),
//     )
// }

// Recursively searches directory tree from specified root for files with a specified name
// Returns a `Vec` of `PathBuf`
fn search_files_with_name<P: AsRef<Path>, S: AsRef<str>>(root: P, name: S) -> Result<Vec<PathBuf>> {
    let root = root.as_ref();
    let name = name.as_ref();

    let mut files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(search_files_with_name(path, name)?);
        } else if path.is_file() {
            if let Some(filename) = path.file_name() {
                if filename == name {
                    files.push(path);
                }
            }
        }
    }

    Ok(files)
}

fn realtime_command_vec<S: AsRef<str>>(cmd_args: Vec<S>, failure_msg: S) -> Result<()> {
    let mut cmd_args: Vec<&str> = cmd_args.iter().map(|s| s.as_ref()).collect();
    let cmd = cmd_args.remove(0);

    realtime_command(cmd, cmd_args, failure_msg.as_ref())
}

fn realtime_command_in_dir<P: AsRef<Path>, S: AsRef<str>>(
    command: S,
    dir: P,
    args: Vec<S>,
    failure_msg: S,
) -> Result<()> {
    println!("realtime_command_in_dir called on dir {:?}", dir.as_ref());

    let command = command.as_ref();
    let args: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
    let failure_msg = failure_msg.as_ref();
    let dir = dir.as_ref();

    println!(
        "Running command {} in realtime in dir {} with args {:?}",
        command,
        dir.to_string_lossy(),
        &args,
    );

    let mut child = Command::new(command)
        .args(&args)
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .wrap_err_with(|| {
            format!(
                "Error spawning process {} with args {:?}: {failure_msg}",
                command, args
            )
        })?;

    let output = child.wait().wrap_err_with(|| {
        format!(
            "Failed getting exit status for process {} with args {:?}",
            &command, &args
        )
    })?;

    match output.code() {
        Some(c) if c == 0 => return Ok(()),
        Some(c) => {
            return Err(eyre!(
                "Process {} with args {:?} failed with return code {}",
                &command,
                &args,
                c
            ))
        }
        None => {
            return Err(eyre!(
                "Process {} with args {:?} was terminated by signal",
                &command,
                &args
            ))
        }
    }
}

fn realtime_command<S: AsRef<str>>(command: S, args: Vec<S>, failure_msg: S) -> Result<()> {
    let command = command.as_ref();
    let args: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
    let failure_msg = failure_msg.as_ref();

    debug!(
        "Running command {} in realtime with args {:?}",
        command, &args,
    );

    let mut child = Command::new(command)
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .wrap_err_with(|| {
            format!(
                "Error spawning process {} with args {:?}: {failure_msg}",
                command, args
            )
        })?;

    let output = child.wait().wrap_err_with(|| {
        format!(
            "Failed getting exit status for process {} with args {:?}",
            &command, &args
        )
    })?;

    match output.code() {
        Some(c) if c == 0 => return Ok(()),
        Some(c) => {
            return Err(eyre!(
                "Process {} with args {:?} failed with return code {}",
                &command,
                &args,
                c
            ))
        }
        None => {
            return Err(eyre!(
                "Process {} with args {:?} was terminated by signal",
                &command,
                &args
            ))
        }
    }
}

fn tag_file_content<P: AsRef<Path>, Tz: TimeZone>(path: P, timestamp: DateTime<Tz>) -> Result<()> {
    let path = path.as_ref();

    if !path_is_file(path)? {
        return Err(eyre!(
            "Failed to write re-evaluation timestamp, path is not a file: {:?}",
            path
        ));
    }

    // let file = OpenOptions::new().read(true).write(true).open(path);

    let lines: Vec<String> = read_to_string(path)
        .wrap_err_with(|| format!("Failed to read file contents {:?}", path))?
        .lines()
        .map(String::from)
        .collect();

    // filter out any lines that currently contain the force-reevaluation prefix
    let mut filtered_lines: Vec<String> = lines
        .into_iter()
        .filter(|s| !s.starts_with("# TAGGED:"))
        .collect();

    // timestamp forced reevaluation
    filtered_lines.push(format!("# TAGGED: {}", timestamp.to_rfc3339()));

    let mut new_file =
        File::create(path).wrap_err_with(|| format!("Failed to create file: {:?}", path))?;

    let output_content = filtered_lines.join("\n");

    new_file
        .write_all(output_content.as_bytes())
        .wrap_err_with(|| {
            format!(
                "Failed to write content to file {:?}:\n{}\n",
                path, &output_content
            )
        })?;

    new_file
        .flush()
        .wrap_err_with(|| format!("Failed to flush file: {:?}", path))?;

    Ok(())
}

/// Backs up the given file into a `.concierge-backup` directory with a timestamped filename.
fn backup_file<P: AsRef<Path>, Tz: TimeZone>(file_path: P, dt: DateTime<Tz>) -> Result<PathBuf> {
    let file_path = file_path.as_ref();
    let parent_dir = file_path
        .parent()
        .wrap_err_with(|| format!("Failed to get parent dir: {:?}", file_path))?;

    let backup_dir = parent_dir.join(".concierge-backup");
    std::fs::create_dir_all(&backup_dir)
        .wrap_err_with(|| format!("Failed to create backup dir: {:?}", &backup_dir))?;

    // let date_time = Local::now().to_rfc3339();
    let filename = file_path
        .file_name()
        .wrap_err_with(|| format!("Failed to get filename: {:?}", file_path))?;
    let backup_file_name = format!("{}-{}", filename.to_string_lossy(), dt.to_rfc3339());
    let backup_file_path = backup_dir.join(backup_file_name);

    std::fs::copy(file_path, &backup_file_path).wrap_err_with(|| {
        format!(
            "Failed to copy file {:?} to {:?}",
            file_path, &backup_file_path
        )
    })?;

    assert!(file_path.exists());
    // println!(
    //     "Copied file from {} to {}",
    //     file_path.to_string_lossy(),
    //     backup_file_path.to_string_lossy()
    // );

    Ok(backup_file_path)
}

fn path_is_file<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();
    Ok(std::fs::metadata(path)
        .wrap_err_with(|| format!("Failed to get metadata of file {:?}", path))?
        .is_file())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use assert_cmd::prelude::*;
    use chrono::Utc;
    use predicates::prelude::*;
    use tempfile::{tempdir, NamedTempFile};

    use super::*;

    fn dt() -> DateTime<Local> {
        Local.with_ymd_and_hms(2023, 06, 16, 11, 12, 00).unwrap()
    }

    fn temp_file() -> NamedTempFile {
        NamedTempFile::new().expect("Failed to create temporary file.")
    }

    fn test_text() -> String {
        String::from_str(lipsum::LIBER_PRIMUS)
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .take(10)
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn test_tag_file_content() {
        let dt: DateTime<Utc> = Utc.with_ymd_and_hms(2023, 06, 16, 11, 12, 00).unwrap();

        let file = NamedTempFile::new().expect("Failed to create temporary file.");

        let res = tag_file_content(file.path(), dt);

        println!("{:?}", res);
        assert!(res.is_ok());

        let expected_line = format!("# TAGGED: {}", dt.to_rfc3339());

        let contents = read_to_string(file).expect("Failed reading file to string.");

        println!("File contents:\n{}\n", &contents);

        assert!(contents.contains(&expected_line))
    }

    #[test]
    fn should_retain_tagged_file_content() {
        let dt = dt();
        let mut file = temp_file();
        let text = test_text();

        // Write some stuff to the file
        file.write_all(text.as_bytes())
            .expect("Failed to write liber primus to file.");

        // flush contents to file to be safe
        file.flush().expect("Failed to flush file.");

        // tag the file
        tag_file_content(file.path(), dt).expect("Failed to tag file.");

        // confirm the file = liber_primus + tag
        let expected = format!("{}\n# TAGGED: {}", text, dt.to_rfc3339());
        let actual = read_to_string(file).expect("Failed to read file.");
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_backup_file() {
        let dt = Local::now();
        let dir = tempdir().unwrap();
        // let mut file = NamedTempFile::new_in(&dir).unwrap();
        let test_file_path = dir.path().join("testing123");
        let mut test_file = File::create(&test_file_path).expect("Create file");

        test_file
            .write_all(test_text().as_bytes())
            .expect("Write test text");
        test_file.flush().expect("Flush file");

        println!("Created test file at {}", test_file_path.to_string_lossy());

        assert!(&test_file_path.exists());

        backup_file(&test_file_path, dt).expect("Backup file");

        // let path = file.path();
        let name = test_file_path.file_name().unwrap();
        let parent_dir = test_file_path.parent().expect("Parent path");
        let expected_path = parent_dir.join(".concierge-backup/").join(format!(
            "{}-{}",
            name.to_string_lossy(),
            dt.to_rfc3339()
        ));

        println!(
            "Expecting backup file at {}",
            expected_path.to_string_lossy()
        );

        assert_eq!(
            read_to_string(expected_path).expect("Read expected path to string"),
            read_to_string(test_file_path).expect("Read orignal path to string")
        );
    }

    #[test]
    fn should_find_files_with_name() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create subdirectories
        let subdir1 = temp_path.join("subdir1");
        let subdir2 = temp_path.join("subdir2");
        fs::create_dir_all(&subdir1).unwrap();
        fs::create_dir_all(&subdir2).unwrap();

        // Create files with the same name in different subdirectories
        let mut file1 = File::create(subdir1.join("target_file.txt")).unwrap();
        let mut file2 = File::create(subdir2.join("target_file.txt")).unwrap();
        let _file3 = File::create(temp_path.join("other_file.txt")).unwrap();

        writeln!(file1, "content for file1").unwrap();
        writeln!(file2, "content for file2").unwrap();

        // Call the function to search for files named "target_file.txt"
        let result = search_files_with_name(temp_path, "target_file.txt").unwrap();

        // Expected result
        let expected: Vec<PathBuf> = vec![
            subdir1.join("target_file.txt"),
            subdir2.join("target_file.txt"),
        ];

        // Assert that the results match
        assert_eq!(result.len(), expected.len());
        for path in &expected {
            assert!(result.contains(path));
        }
    }
}
