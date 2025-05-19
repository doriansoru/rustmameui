//! This module defines the `Game` struct, representing a single game entry,
//! and provides methods for managing game details, retrieving snapshots,
//! and launching the game.

use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use base64::{Engine as _};
use thiserror::Error; // Add this import

/// Custom error type for operations within the `game` module.
#[derive(Error, Debug)]
pub enum GameError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zip file error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Base64 decoding error: {0}")]
    Base64Decode(#[from] base64::DecodeError), // Although we are encoding, it's good practice to include decode if you might decode elsewhere
    #[error("Error executing command: {0}")]
    CommandExecution(std::io::Error),
    #[error("Snapshot file not found in zip: {0}")]
    SnapshotNotFoundInZip(String),
}

/// Represents a single game entry with its details.
///
/// This struct stores the game's ROM name, description, and whether a snapshot
/// image is available. It supports serialization/deserialization (for config/data files),
/// cloning, debugging, and equality comparisons.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Game {
    /// The name of the game's ROM file (without extension).
    rom: String,
    /// A human-readable description or title for the game.
    description: String,
    /// Indicates whether a snapshot image is available for this game.
    snap: bool,
}

impl Game {
    /// Creates a new `Game` instance.
    ///
    /// # Arguments
    ///
    /// * `rom` - The ROM name of the game.
    /// * `description` - The description or title of the game.
    /// * `snap` - A boolean indicating if a snapshot is available.
    ///
    /// # Returns
    ///
    /// A new `Game` instance.
    pub fn new(rom: &String, description: &String, snap: bool) -> Self {
        Self {
            rom: rom.into(),
            description: description.into(),
            snap
        }
    }

    /// Returns the description of the game.
    ///
    /// # Returns
    ///
    /// A `String` containing the game's description.
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// Retrieves the snapshot image for the game from a specified zip file,
    /// encodes it into Base64, and returns it as a data URL.
    ///
    /// If `self.snap()` is false, an empty string is returned.
    /// The snapshot is expected to be a PNG file named `rom_name.png` within the zip archive.
    ///
    /// # Arguments
    ///
    /// * `snap_file` - The path to the zip file containing snapshot images.
    ///
    /// # Returns
    ///
    /// A `Result` containing a `String` with the Base64-encoded PNG data as a data URL
    /// (`data:image/png;base64,...`) on success.
    /// Returns an empty string if `snap` is false.
    /// Returns a `GameError` if the zip file cannot be opened,
    /// the snapshot file is not found within the zip, or an I/O error occurs during reading or encoding.
    pub fn get_snap(&self, snap_file: &String) -> Result<String, GameError> { // Updated return type
        if !self.snap() {
            return Ok("".into());
        }

        let file = std::fs::File::open(snap_file)?;
        let reader = std::io::BufReader::new(file);
        let mut archive = zip::ZipArchive::new(reader)?;

        // Get the PNG filename to look for in the zip file
        let png_name = format!("{}.png", self.rom());
        // Search and read from the zipfile
        let mut zip_file = archive.by_name(&png_name).map_err(|_| GameError::SnapshotNotFoundInZip(png_name))?; // Map zip error to custom error
        // Read the content of the png
        let mut png_data = Vec::new();
        std::io::Read::read_to_end(&mut zip_file, &mut png_data)?;

        // Convert the content to base64
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);
        // Create the data URL for the html src attribute
        let data_url = format!("data:image/png;base64,{}", base64_data);
        Ok(data_url)
    }

    /// Launches the game using the specified MAME executable and ROMs path.
    ///
    /// Constructs the full path to the game's ROM zip file and executes the MAME
    /// process as a separate command.
    ///
    /// # Arguments
    ///
    /// * `mame_executable` - The path to the MAME executable.
    /// * `roms_path` - The path to the directory containing the ROM files.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the command is successfully spawned.
    /// Returns a `GameError` if an error occurs while trying to execute
    /// the MAME process.
    pub fn launch(&self, mame_executable: &PathBuf, roms_path: &Path) -> Result<(), GameError> { // Updated return type
        std::process::Command::new(mame_executable)
            .arg("-rp")
            .arg(roms_path.display().to_string())
            .arg(self.rom())
            .spawn()
            .map(|_| ()) // Discard the Child process struct, return Ok(())
            .map_err(GameError::CommandExecution)?; // Map io::Error to custom error
        Ok(())
    }

    /// Returns whether a snapshot is available for this game.
    ///
    /// # Returns
    ///
    /// `true` if a snapshot is available, `false` otherwise.
    pub fn snap(&self) -> bool {
        self.snap
    }

    /// Returns the ROM name of the game.
    ///
    /// # Returns
    ///
    /// A `String` containing the game's ROM name.
    pub fn rom(&self) -> String {
        self.rom.clone()
    }
}