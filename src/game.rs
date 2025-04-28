use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use base64::{Engine as _};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Game {
    rom: String,
    description: String,
    snap: bool,
}

impl Game {
    pub fn new(rom: &String, description: &String, snap: bool) -> Self {
        Self {
            rom: rom.into(),
            description: description.into(),
            snap
        }
    }

    pub fn description(&self) -> String {
        self.description.clone()
    }

    pub fn get_snap(&self, snap_file: &String) -> Result<String, Box<dyn std::error::Error>> {
        if !self.snap() {
            return Ok("".into());
        }

        let file = std::fs::File::open(snap_file)?;
        let reader = std::io::BufReader::new(file);
        let mut archive = zip::ZipArchive::new(reader)?;

        // Get the PNG filename to look for in the zip file 
        let png_name = format!("{}.png", self.rom());

        // Search and read from the zipfile
        let mut zip_file = archive.by_name(&png_name)?;

        // Read the content of the png
        let mut png_data = Vec::new();
        std::io::Read::read_to_end(&mut zip_file, &mut png_data)?;

        // Convert the content to base64
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);

        // Create the data URL for the html src attribute
        let data_url = format!("data:image/png;base64,{}", base64_data);

        Ok(data_url)
    }

    pub fn launch(&self, mame_executable: &PathBuf, roms_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut full_rom = roms_path.to_path_buf();
        full_rom.push(self.rom());
        full_rom.set_extension("zip");
        let _ = std::process::Command::new(mame_executable)
            .arg(full_rom).spawn()?;
        Ok(())
    }

    pub fn snap(&self) -> bool {
        self.snap
    }

    pub fn rom(&self) -> String {
        self.rom.clone()
    }
}
