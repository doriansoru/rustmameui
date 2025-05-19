//! This module handles the application's configuration, including loading from a file and environment
//! variables, saving the configuration, and managing the application's configuration directory.

use std::path::PathBuf;
use thiserror::Error; // Add this import
use config::{ConfigError, Environment, File}; // Import necessary items from config crate
use std::collections::HashMap; // Import HashMap


/// Custom error type for operations within the `rustmameuiconfig` module.
#[derive(Error, Debug)]
pub enum ConfigErrorWithThisError { // Renamed to avoid clash with config::ConfigError
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Cannot get the system configuration directory.")]
    CannotGetConfigDir,
    #[error("Cannot create the application configuration directory: {path}")]
    CannotCreateAppConfigDir { path: PathBuf },
    #[error("Missing required configuration key: {key}")]
    MissingConfigKey { key: String },
    #[error("Error finding MAME executable: {0}")]
    WhichError(#[from] which::Error),
}


/// Represents the application's configuration settings.
///
/// This struct holds paths to important directories and files used by the application,
/// such as the project configuration directory, MAME executable path, ROMs path,
/// and snapshot file path. It is designed to be clonable.
#[derive(Clone)]
pub struct Config {
    /// The base directory for the application's configuration files.
    pub project_config_dir: PathBuf,
    /// The path to the MAME executable.
    pub mame_executable: PathBuf,
    /// The path to the directory containing MAME ROM files.
    pub roms_path: PathBuf,
    /// The path to the snapshot file used by the application.
    pub snap_file: PathBuf,
}

// Implement the Default trait
impl Default for Config {
    fn default() -> Self {
        Self {
            project_config_dir: PathBuf::new(), // Empty path
            mame_executable: PathBuf::new(),    // Empty path
            roms_path: PathBuf::new(),          // Empty path
            snap_file: PathBuf::new(),          // Empty path
        }
    }
}

impl Config {
    /// Creates a new `Config` instance by loading settings from the configuration file
    /// and environment variables.
    ///
    /// It first determines the application's configuration directory based on the package name,
    /// creates it if it doesn't exist, and finds the path to the configuration file (e.g., `app_name.toml`).
    ///
    /// If the configuration file does not exist, it attempts to find the MAME executable
    /// and saves a default configuration file with initial values (potentially empty PathBufs).
    ///
    /// It then loads configuration settings from the created/existing file and overrides
    /// them with environment variables prefixed by the package name.
    ///
    /// Finally, it extracts and validates the required paths (`mame_executable`, `roms_path`, `snap_file`)
    /// from the loaded settings.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the populated `Config` struct on success,
    /// or a `ConfigErrorWithThisError` if any step fails (e.g., cannot find config directory,
    /// cannot create directory, cannot read file, missing required configuration keys).
    pub fn new() -> Result<Self, ConfigErrorWithThisError> { // Updated return type
        // Get the package name
        let package_name = env!("CARGO_PKG_NAME");
        let config_dir = dirs::config_dir().ok_or(ConfigErrorWithThisError::CannotGetConfigDir)?; // Replace panic!

        // Create the path of the configuration directory for this app
        let project_config_dir = config_dir.join(package_name);
        // Create this app configuration directory if it does not exist
        if !project_config_dir.exists() {
            // Create the project configuration directory
            std::fs::create_dir_all(&project_config_dir)
                .map_err(|_| ConfigErrorWithThisError::CannotCreateAppConfigDir { path: project_config_dir.clone() })?; // Replace panic!
        }

        let mut config_file = project_config_dir.join(package_name);
        config_file.set_extension("toml");
        if !config_file.exists() {
            let mame_executable = which::which("mame").unwrap_or_else(|_| PathBuf::new()); // which::which returns Result, unwrap_or_else handles Err

            let config = Self {
                project_config_dir: project_config_dir.clone(),
                mame_executable,
                roms_path: PathBuf::new(),
                snap_file: PathBuf::new(),
            };
            // Handle save error, perhaps logging it or propagating if save also returned Result
            // For now, keeping the original behavior of unwrap() for the initial save
             config.save().unwrap_or_else(|e| {
                eprintln!("Error saving initial config: {}", e);
            });
        }

        let settings = config::Config::builder()
           .add_source(File::with_name(&config_file.display().to_string()))
            // Add in settings from the environment (with a prefix of APP)
            // Eg.. `PACKAGE_NAME_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(Environment::with_prefix(package_name))
            .build()? // From ConfigError
            .try_deserialize::<HashMap<String, String>>()?; // From ConfigError

        let mame_executable = settings.get("mame_executable")
            .ok_or(ConfigErrorWithThisError::MissingConfigKey { key: "mame_executable".to_string() })?; // Replace io::Error

         let roms_path = settings.get("roms_path")
            .ok_or(ConfigErrorWithThisError::MissingConfigKey { key: "roms_path".to_string() })?; // Replace io::Error

        let snap_file = settings.get("snap_file")
            .ok_or(ConfigErrorWithThisError::MissingConfigKey { key: "snap_file".to_string() })?; // Replace io::Error


        Ok(Self {
            project_config_dir,
            mame_executable: PathBuf::from(mame_executable),
            roms_path: PathBuf::from(roms_path),
            snap_file: PathBuf::from(snap_file),
        })
    }

    /// Saves the current configuration settings to the application's configuration file.
    ///
    /// The configuration is written to the TOML file located within the `project_config_dir`.
    /// The file name is derived from the package name with a `.toml` extension.
    ///
    /// The method serializes the `mame_executable`, `roms_path`, and `snap_file` fields
    /// into a simple TOML format.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a `ConfigErrorWithThisError` if an error occurs
    /// during file creation or writing.
    pub fn save(&self) -> Result<(), ConfigErrorWithThisError> { // Updated return type
        use std::io::Write;
        use std::fs::File;

        // Create the config file path
        let package_name = env!("CARGO_PKG_NAME");
        let mut config_file = self.project_config_dir.join(package_name);
        config_file.set_extension("toml");

        // Create the config file TOML content
        let content = format!(
            "mame_executable = \"{}\"\n\
             roms_path = \"{}\"\n\
             snap_file = \"{}\"\n",
            self.mame_executable.display().to_string().replace("\\", "\\\\"),
            self.roms_path.display().to_string().replace("\\", "\\\\"),
            self.snap_file.display().to_string().replace("\\", "\\\\")
        );

        // Write the file
        let mut file = File::create(&config_file)?; // From Io
        file.write_all(content.as_bytes())?; // From Io
        Ok(())
    }
}