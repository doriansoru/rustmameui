use std::path::PathBuf;
use rust_i18n::t;

macro_rules! box_err {
    ($msg:expr) => {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, $msg)))
    };
    // A version thich accepts a format similar to format!
    ($fmt:expr, $($arg:tt)*) => {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))))
    };
}

#[derive(Clone)]
pub struct Config {
    pub project_config_dir: PathBuf,
    pub mame_executable: PathBuf,
    pub roms_path: PathBuf,
    pub snap_file: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Get the package name
        let package_name = env!("CARGO_PKG_NAME");
        let config_dir = dirs::config_dir().expect(
            t!("cannot_get_the_system_configuration_directory").to_string().as_str()
            //"Cannot get the system configuration directory."
        );

        // Create the path of the configuration directory for this app
        let project_config_dir = config_dir.join(package_name);

        // Create this app configuration directory if it does not exist
        if !project_config_dir.exists() {
            // Create the project configuration directory
            std::fs::create_dir_all(&project_config_dir).expect(
                t!("cannot_create_the_app_configuration_directory").to_string().as_str()
                //"Cannot create the app configuration directory."
            );
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
