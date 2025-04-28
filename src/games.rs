//! This module handles interactions with the MAME executable, specifically
//! extracting game lists from its XML output, verifying the presence of
//! ROM and snapshot files, and managing persistent lists of games and
//! favourite games via JSON files.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use quick_xml::Reader;
use quick_xml::events::Event;
use crate::rustmameuiconfig::Config;
use crate::game::Game;
use std::io::BufReader;
use rust_i18n::t;

/// Executes the MAME executable with the `-lx` argument to get the list of
/// supported games in XML format.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the path
///   to the MAME executable.
///
/// # Returns
///
/// A `Result` containing the XML output as a `String` on success,
/// or a `Box<dyn std::error::Error>` if the command execution fails or
/// the output is not valid UTF-8.
pub fn get_xml_roms(config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(&config.mame_executable)
        .arg("-lx")
        .output()?;
    let xml_string = String::from_utf8(output.stdout)?;
    Ok(xml_string)
}

/// Parses the XML output from `mame -lx` to extract ROM names and descriptions.
///
/// It iterates through the XML, specifically looking for `<machine>` and `<description>`
/// tags. It filters machines based on the "good" emulation status of their driver
/// and checks if the corresponding ROM zip file exists in the configured ROMs path.
/// BIOS machines are ignored.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the ROMs path.
/// * `xml_string` - The XML output from `mame -lx` as a string.
///
/// # Returns
///
/// A `Result` containing a tuple `(Vec<String>, Vec<String>)` on success,
/// where the first vector contains the ROM names and the second contains
/// the corresponding descriptions. Both vectors are sorted alphabetically by ROM name.
/// Returns a `Box<dyn std::error::Error>` if an XML parsing error occurs.
pub fn get_all_roms(config: &Config, xml_string: &str) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    let roms_path = &config.roms_path;
    let mut reader = Reader::from_str(xml_string);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = false;

    let mut buf = Vec::new();

    let mut roms = Vec::new();
    let mut descriptions = Vec::new();

    let mut current_machine: Option<String> = None;
    let mut current_description: Option<String> = None;
    let mut is_bios = false;

    let mut good_driver_count = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"machine" => {
                current_machine = None;
                is_bios = false;

                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"name" {
                        if let Ok(name) = String::from_utf8(attr.value.to_vec()) {
                            current_machine = Some(name);
                        }
                    } else if attr.key.as_ref() == b"isbios" {
                        if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                            is_bios = val == "yes";
                        }
                    }
                }
            },
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"description" => {
                current_description = Some(reader.read_text(e.name())?.to_string());
            },
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"driver" => {
                process_driver(e, &current_machine, &current_description, is_bios, roms_path, &mut roms, &mut descriptions, &mut good_driver_count)?;
            },
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"driver" => {
                process_driver(e, &current_machine, &current_description, is_bios, roms_path, &mut roms, &mut descriptions, &mut good_driver_count)?;
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(e.into());
            },
            _ => (),
        }
        buf.clear();
    }

    roms.sort();
    Ok((roms, descriptions))
}

/// Verifies the presence and integrity of a batch of ROM files using `mame -verifyroms`.
///
/// Executes the MAME executable with the `-verifyroms` argument followed by the
/// list of ROM names.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the path
///   to the MAME executable.
/// * `roms` - A slice of `String` containing the ROM names to verify.
///
/// # Returns
///
/// A `Vec<bool>` where each boolean corresponds to a ROM in the input slice,
/// indicating `true` if MAME verified it successfully (i.e., the ROM name
/// was NOT found in the standard error output), and `false` otherwise.
/// If the `mame -verifyroms` command fails to execute, an error is printed,
/// and a vector of `false` values is returned for all ROMs.
pub fn verify_batch_roms(config: &Config, roms: &[String]) -> Vec<bool> {
    let mut command = Command::new(&config.mame_executable);
    command.arg("-verifyroms");
    for rom in roms {
        command.arg(rom);
    }

    let output = Command::new(&config.mame_executable)
        .args(command.get_args())
        .output()
        .inspect_err(|e| {
            eprintln!("{}",
                t!("error_while_executing.mame_executable.error", mame_executable = config.mame_executable.display().to_string(), error = e.to_string())
                //"Error while executing '{} -verifyroms': {}", config.mame_executable.display(), e
            );
        })
        .unwrap_or_else(|_| std::process::Output {
            stdout: vec![],
            stderr: vec![],
            status: Default::default(),
        });

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut batch_result = Vec::with_capacity(roms.len());

    for rom in roms {
        // MAME prints the rom name to stderr if it's NOT OK
        let is_valid = !stderr.contains(rom);
        batch_result.push(is_valid);
    }

    batch_result
}

/// Verifies the presence of snapshot images for a batch of ROMs within the
/// configured snapshot zip file.
///
/// Checks if a PNG file named `rom_name.png` exists inside the zip archive
/// specified by `config.snap_file` for each ROM in the input slice.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the path
///   to the snapshot zip file.
/// * `roms` - A slice of `String` containing the ROM names to check for snapshots.
///
/// # Returns
///
/// A `Vec<bool>` where each boolean corresponds to a ROM in the input slice,
/// indicating `true` if the corresponding `.png` file was found in the zip file,
/// and `false` otherwise. If the snapshot file cannot be opened or is not a valid
/// zip archive, an error is printed, and a vector of `false` values is returned
/// for all ROMs.
pub fn verify_batch_snaps(config: &Config, roms: &[String]) -> Vec<bool> {
    let snap_file_path = config.snap_file.clone();
    let roms_to_check = roms.to_vec();
    let mut local_found = Vec::new();
    match std::fs::File::open(snap_file_path) {
        Ok(f) => {
            let reader = BufReader::new(f);
            // Note: .unwrap() here means a zip error will panic.
            let mut archive = zip::ZipArchive::new(reader).unwrap();

            for rom in roms_to_check.iter() {
                match archive.by_name(format!("{}.png", rom).as_str()) {
                    Ok(_) => {
                        local_found.push(true);
                    },
                    Err(..) => {
                        local_found.push(false);
                    }
                };
            }

        },
        Err(_) => {
            eprintln!("{}",
                t!("cannot_open_the_snaps_file")
                //"Cannot open the snaps file."
            );
            return vec![false; roms.len()];
        }
    }
    local_found
}

/// Saves a vector of `Game` structs to a JSON file in the application's
/// configuration directory.
///
/// The file is named `games.json` within the directory specified by
/// `config.project_config_dir`. The data is serialized with pretty printing.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the project
///   configuration directory path.
/// * `games` - A reference to a vector of `Game` structs to save.
///
/// # Returns
///
/// A `std::io::Result<()>` indicating success or failure. Returns an error
/// if the file cannot be created, the data cannot be serialized, or the data
/// cannot be written to the file.
pub fn save(config: &Config, games: &Vec<Game>) -> std::io::Result<()> {
    let mut games_file = PathBuf::from(&config.project_config_dir);
    games_file.push("games");
    games_file.set_extension("json");
    let mut file = std::fs::File::create(games_file)?;
    let json_string = serde_json::to_string_pretty(games)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}

/// Loads a vector of `Game` structs from the `games.json` file in the
/// application's configuration directory.
///
/// If the `games.json` file does not exist, an empty vector is returned
/// without an error.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the project
///   configuration directory path.
///
/// # Returns
///
/// A `Result` containing a `Vec<Game>` on success. Returns an empty vector
/// if the file doesn't exist. Returns a `Box<dyn std::error::Error>` if the
/// file exists but cannot be read or the content cannot be deserialized into
/// a `Vec<Game>`. The loaded games are sorted by description before being returned.
pub fn load(config: &Config) -> Result<Vec<Game>, Box<dyn std::error::Error>> {
    let mut games_file = PathBuf::from(&config.project_config_dir);
    games_file.push("games");
    games_file.set_extension("json");
    if !games_file.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(games_file)?;
    let mut games: Vec<Game> = serde_json::from_str(&contents)?;
    sort(&mut games);
    Ok(games)
}

/// Loads a vector of favourite `Game` structs from the `favourites.json` file
/// in the application's configuration directory.
///
/// If the `favourites.json` file does not exist, an empty vector is returned
/// without an error.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the project
///   configuration directory path.
///
/// # Returns
///
/// A `Result` containing a `Vec<Game>` on success. Returns an empty vector
/// if the file doesn't exist. Returns a `Box<dyn std::error::Error>` if the
/// file exists but cannot be read or the content cannot be deserialized into
/// a `Vec<Game>`. The loaded favourites are sorted by description before being returned.
pub fn load_favourites(config: &Config) -> Result<Vec<Game>, Box<dyn std::error::Error>> {
    let mut favourites_file = PathBuf::from(&config.project_config_dir);
    favourites_file.push("favourites");
    favourites_file.set_extension("json");
    if !favourites_file.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(favourites_file)?;
    let mut favourites: Vec<Game> = serde_json::from_str(&contents)?;
    sort(&mut favourites);
    Ok(favourites)
}

/// Adds a `Game` to the vector of favourites and saves the updated list
/// to the `favourites.json` file.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the project
///   configuration directory path.
/// * `favourites` - A mutable reference to the current vector of favourite `Game`s.
///   The game will be added to this vector.
/// * `game` - A reference to the `Game` to add to favourites.
///
/// # Returns
///
/// A clone of the updated `favourites` vector after adding the game and attempting to save.
/// Errors during file creation or writing are printed to `eprintln!`, and the function
/// still returns the (potentially unsaved) updated vector. Serialization errors are also
/// printed.
pub fn add_favourite(config: &Config, favourites: &mut Vec<Game>, game: &Game) -> Vec<Game> {
    favourites.push(game.clone());
    let mut favourites_file = PathBuf::from(&config.project_config_dir);
    favourites_file.push("favourites");
    favourites_file.set_extension("json");

    let mut file = match std::fs::File::create(favourites_file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}",
                t!("error_while_creating_the_favourites_file.error", error = e.to_string())
                //"Error while creating the favourites file: {}", e
            );
            return favourites.clone();
        }
    };
    match serde_json::to_string_pretty(favourites) {
        Ok(json_string) => {
            if let Err(e) = file.write_all(json_string.as_bytes()) {
                eprintln!("{}",
                    t!("error_while_writing_to_the_favourites_file.error", error = e.to_string())
                    //"Error while writing to the favourites file: {}", e
                );
            }
        }
        Err(e) => {
            eprintln!("{}",
                t!("error_while_serializing_the_favourites.error", error = e.to_string())
                //"Error while serializing the favourites: {}", e
            );
        }
    }
    sort(favourites);
    favourites.clone()
}

/// Removes a `Game` from the vector of favourites based on its ROM name
/// and saves the updated list to the `favourites.json` file.
///
/// # Arguments
///
/// * `config` - A reference to the application `Config`, containing the project
///   configuration directory path.
/// * `favourites` - A mutable reference to the current vector of favourite `Game`s.
///   The game will be removed from this vector if found.
/// * `game` - A reference to the `Game` to remove from favourites. Comparison is
///   based on the `rom` field.
///
/// # Returns
///
/// A clone of the updated `favourites` vector after attempting to remove the game
/// and save. Errors during file creation or writing are printed to `eprintln!`,
/// and the function still returns the (potentially unsaved) updated vector.
/// Errors during serialization will cause a panic (as per original code).
pub fn remove_favourite(config: &Config, favourites: &mut Vec<Game>, game: &Game) -> Vec<Game> {
    if let Some(index) = favourites.iter().position(|value| *value.rom() == *game.rom()) {
        favourites.swap_remove(index);
    }
    let mut favourites_file = PathBuf::from(&config.project_config_dir);
    favourites_file.push("favourites");
    favourites_file.set_extension("json");

    let mut file = match std::fs::File::create(favourites_file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}",
                t!("error_while_creating_the_favourites_file.error", error = e.to_string())
                //"Error while creating the favourites file: {}", e
            );
            return favourites.clone();
        }
    };
    match serde_json::to_string_pretty(favourites) {
        Ok(json_string) => {
            if let Err(e) = file.write_all(json_string.as_bytes()) {
                eprintln!("{}",
                    t!("error_while_writing_to_the_favourites_file.error", error = e.to_string())
                    //"Error while writing to the favourites file: {}", e
                );
            }
        }
        Err(e) => {
            panic!("{}", // Note: The original code uses panic! here, not eprintln!
                t!("error_while_serializing_the_favourites.error", error = e.to_string()).to_string()
            );
            //"Error while serializing the favourites: {}", e
        }
    }
    sort(favourites);
    favourites.clone()
}

/// Sorts a slice of `Game` structs alphabetically by their description.
/// This is a helper function used internally after loading/modifying game lists.
fn sort(games: &mut [Game]) {
    games.sort_by_key(|x| x.description().clone());
}

/// Processes a `<driver>` tag from the MAME XML output.
///
/// This helper function is used by `get_all_roms` to check the emulation status
/// of a driver. If the driver status is "good" and the corresponding ROM zip file
/// exists in the specified ROMs path, the machine's ROM name and description
/// are added to the provided vectors. BIOS machines are skipped.
///
/// # Arguments
///
/// * `e` - The `BytesStart` event for the `<driver>` tag.
/// * `current_machine` - An `Option` containing the name of the current machine.
/// * `current_description` - An `Option` containing the description of the current machine.
/// * `is_bios` - A boolean indicating if the current machine is a BIOS.
/// * `roms_path` - The path to the directory containing the ROM files.
/// * `roms` - A mutable reference to the vector where ROM names are collected.
/// * `descriptions` - A mutable reference to the vector where descriptions are collected.
/// * `good_driver_count` - A mutable reference to a counter for good drivers (though
///   this counter is updated, its final value isn't used outside this function).
///
/// # Returns
///
/// A `Result` indicating success or failure. Returns a `quick_xml::Error` if
/// attribute reading or parsing fails.
fn process_driver(
    e: &quick_xml::events::BytesStart<'_>,
    current_machine: &Option<String>,
    current_description: &Option<String>,
    is_bios: bool,
    roms_path: &Path,
    roms: &mut Vec<String>,
    descriptions: &mut Vec<String>,
    good_driver_count: &mut i32,
) -> Result<(), quick_xml::Error> {
    if let Some(machine) = current_machine {
        if !is_bios {
            for attr in e.attributes().flatten() {

                if attr.key.as_ref() == b"emulation" {
                    if attr.value.as_ref() == b"good" {
                        if let Some(description) = current_description {
                            let rom_name = machine.clone();
                            let mut rom_path = roms_path.join(&rom_name);
                            rom_path.set_extension("zip");
                            if rom_path.exists() {
                                roms.push(rom_name);
                                descriptions.push(description.clone());
                                *good_driver_count += 1;
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    Ok(())
}