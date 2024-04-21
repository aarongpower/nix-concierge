use std::fs;
use std::path::Path;

pub fn is_directory_empty<P: AsRef<Path>>(path: P) -> std::io::Result<bool> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}
