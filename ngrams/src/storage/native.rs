//! Native filesystem storage using buffered CBOR files.
//!
//! Keys are joined to the base directory as paths, without filename validation.

use super::{
    Storage,
    StorageError,
};
use serde::{
    de::DeserializeOwned,
    Serialize,
};
use std::{
    fs::{
        self,
        File,
    },
    io::{
        BufReader,
        BufWriter,
    },
    path::{
        absolute,
        PathBuf,
    },
};

lazy_static::lazy_static! {
    /// Base directory for storage on native platforms
    pub(crate) static ref STORAGE_DIR: PathBuf = absolute(PathBuf::from_iter([".", "test", "cache"])).unwrap();
}

/// Native filesystem-based storage
#[derive(Debug, Clone)]
pub(crate) struct NativeStorage {
    /// Directory against which storage keys are resolved.
    base_dir: PathBuf,
}

impl Default for NativeStorage {
    /// Use the same process-relative cache directory as `new`.
    fn default() -> Self {
        Self::new()
    }
}

impl NativeStorage {
    /// Create a new native storage with the default base directory
    pub(crate) fn new() -> Self {
        Self {
            base_dir: STORAGE_DIR.clone(),
        }
    }

    /// Create a new native storage with a custom base directory
    pub(crate) fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the full path for a key
    fn key_to_path(
        &self,
        key: &str,
    ) -> PathBuf {
        self.base_dir.join(key)
    }
}

impl Storage for NativeStorage {
    /// Replace the target file with CBOR data, creating parent directories.
    ///
    /// Replacement is not atomic: an existing file is removed before writing.
    fn store<T: Serialize>(
        &self,
        key: &str,
        data: &T,
    ) -> Result<(), StorageError> {
        let path = self.key_to_path(key);

        eprintln!("Write to storage: {}", path.display());

        // Remove existing file if present
        if path.exists() {
            fs::remove_file(&path)?;
        }

        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write data using ciborium (CBOR format)
        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        ciborium::into_writer(data, writer)?;

        Ok(())
    }

    /// Read and deserialize a CBOR file, reporting a missing key separately.
    fn load<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<T, StorageError> {
        let path = self.key_to_path(key);

        eprintln!("Read from storage: {}", path.display());

        if !path.exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let data = ciborium::from_reader(reader)?;

        Ok(data)
    }

    /// Check path existence without attempting to deserialize its contents.
    fn exists(
        &self,
        key: &str,
    ) -> bool {
        self.key_to_path(key).exists()
    }

    /// Remove the file if present; absence is treated as success.
    fn remove(
        &self,
        key: &str,
    ) -> Result<(), StorageError> {
        let path = self.key_to_path(key);

        if path.exists() {
            fs::remove_file(&path)?;
        }

        Ok(())
    }
}
