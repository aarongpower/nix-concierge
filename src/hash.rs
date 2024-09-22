use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

/// Computes the SHA-256 hash of a file.
pub fn hash_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let path = path.as_ref();
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_hash_file() {
        // Create a temporary file for testing
        let file_path = "test_file.txt";
        let mut test_file = File::create(file_path).expect("Failed to create test file");

        // Write some content to the file
        let content = b"Hello, Rust!";
        test_file
            .write_all(content)
            .expect("Failed to write to test file");

        // Compute the expected hash manually (SHA-256 of "Hello, Rust!")
        let expected_hash = "12a967da1e8654e129d41e3c016f14e81e751e073feb383125bf82080256ca19";

        // Call the hash_file function
        let computed_hash = hash_file(file_path).expect("Failed to compute file hash");

        // Ensure the hash matches the expected value
        assert_eq!(computed_hash, expected_hash);

        // Clean up: remove the test file
        std::fs::remove_file(file_path).expect("Failed to delete test file");
    }
}
