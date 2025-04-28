//! This module handles the application's configuration, including loading from a file and environment
//! variables, saving the configuration, and managing the application's configuration directory.

use std::path::PathBuf;
use rust_i18n::t;

/// A macro to create a boxed `std::io::Error` with a custom message.
///
/// This provides a convenient way to return a generic `Box<dyn std::error::Error>`
/// containing an `io::Error` with `ErrorKind::Other`.
///
/// It supports two forms:
/// - `box_err!($msg:expr)`: Takes a single string message.
/// - `box_err!($fmt:expr, $($arg:tt)*)`: Takes a format string and arguments,
///   similar to `format!`.
macro_rules! box_err {
    ($msg:expr) => {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, $msg)))
    };
    // A version thich accepts a format similar to format!
    ($fmt:expr, $($arg:tt)*) => {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))))
    };
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
    /// or a `Box<dyn std::error::Error>` if any step fails (e.g., cannot find config directory,
    /// cannot create directory, cannot read file, missing required configuration keys).
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Get the package name
        let package_name = env!("CARGO_PKG_NAME");
        let config_dir = dirs::config_dir().unwrap_or_else(|| { panic!("{}", t!("cannot_get_the_system_configuration_directory").to_string()) });

        // Create the path of the configuration directory for this app
        let project_config_dir = config_dir.join(package_name);

        // Create this app configuration directory if it does not exist
        if !project_config_dir.exists() {
            // Create the project configuration directory
            std::fs::create_dir_all(&project_config_dir).unwrap_or_else(|_| { panic!("{}", t!("cannot_create_the_app_configuration_directory").to_string()) });
        }

        let mut config_file = project_config_dir.join(package_name);
        config_file.set_extension("toml");

        if !config_file.exists() {
            let mame_executable = match which::which("mame") {
                Ok(mame) => mame,
                Err(_) => PathBuf::new()
            };

            let config = Self {
                project_config_dir: project_config_dir.clone(),
                mame_executable,
                roms_path: PathBuf::new(),
                snap_file: PathBuf::new(),
            };

            config.save().unwrap();
        }

        let settings = config::Config::builder()
           .add_source(config::File::with_name(&config_file.display().to_string()))
            // Add in settings from the environment (with a prefix of APP)
            // Eg.. `PACKAGE_NAME_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(config::Environment::with_prefix(package_name))
            .build()?.try_deserialize::<std::collections::HashMap<String, String>>()?;


        let mame_executable = match settings.get("mame_executable") {
            Some(v) => v,
            None => {
                return box_err!(
                    t!("cannot_read_the_mame_executable_path_from_the_configuration_file")
                    //"Cannot read the mame executable path from the configuration file"
                );
            }
         };

        let roms_path = match settings.get("roms_path") {
            Some(v) => v,
            None => {
                return box_err!(
                    t!("cannot_read_the_roms_path_from_the_configuration_file")
                    //"Cannot read the roms path from the configuration file"
                );
            }
        };

        let snap_file = match settings.get("snap_file") {
            Some(v) => v,
            None => {
                return box_err!(
                    t!("cannot_read_the_snaps_file_path_from_the_configuration_file")
                    //"Cannot read the snap file path from the configuration file"
                );
            }
        };

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
    /// Returns `Ok(())` on success, or a `Box<dyn std::error::Error>` if an error occurs
    /// during file creation or writing.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
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
            self.mame_executable.display(),
            self.roms_path.display(),
            self.snap_file.display()
        );

        // Write the file
        let mut file = File::create(&config_file)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}