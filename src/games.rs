use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use quick_xml::Reader;
use quick_xml::events::Event;
use crate::rustmameuiconfig::Config;
use crate::game::Game;
use std::io::BufReader;

pub fn get_xml_roms(config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(&config.mame_executable)
        .arg("-lx")
        .output()?;
    let xml_string = String::from_utf8(output.stdout)?;
    Ok(xml_string)
}

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

pub fn verify_batch_roms(config: &Config, roms: &[String]) -> Vec<bool> {
    let mut command = Command::new(&config.mame_executable);
    command.arg("-verifyroms");
    for rom in roms {
        command.arg(rom);
    }

    let output = Command::new(&config.mame_executable)
        .args(command.get_args())
        .output()
        .map_err(|e| {
            eprintln!("Error executing '{} -verifyroms': {}", config.mame_executable.display(), e);
            e
        })
        .unwrap_or_else(|_| std::process::Output {
            stdout: vec![],
            stderr: vec![],
            status: Default::default(),
        });

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut batch_result = Vec::with_capacity(roms.len());

    for rom in roms {
        let is_valid = !stderr.contains(rom);
        batch_result.push(is_valid);
    }

    batch_result
}

pub fn verify_batch_snaps(config: &Config, roms: &[String]) -> Vec<bool> {
    let snap_file_path = config.snap_file.clone();
    let roms_to_check = roms.to_vec();
    let mut local_found = Vec::new();
    match std::fs::File::open(snap_file_path) {
        Ok(f) => {
            let reader = BufReader::new(f);
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
            eprintln!("Cannot open the snaps file.");
            return vec![false; roms.len()];
        }
    }
    local_found
}

pub fn save(config: &Config, games: &Vec<Game>) -> std::io::Result<()> {
    let mut games_file = PathBuf::from(&config.project_config_dir);
    games_file.push("games");
    games_file.set_extension("json");
    let mut file = std::fs::File::create(games_file)?;
    let json_string = serde_json::to_string_pretty(games)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}

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

pub fn add_favourite(config: &Config, favourites: &mut Vec<Game>, game: &Game) -> Vec<Game> {
    favourites.push(game.clone());
    let mut favourites_file = PathBuf::from(&config.project_config_dir);
    favourites_file.push("favourites");
    favourites_file.set_extension("json");  

    let mut file = match std::fs::File::create(favourites_file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error creating favourites file: {}", e);
            return favourites.clone();
        }
    };
    match serde_json::to_string_pretty(favourites) {
        Ok(json_string) => {
            if let Err(e) = file.write_all(json_string.as_bytes()) {
                eprintln!("Error writing to favourites file: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error serializing favourites: {}", e);
        }
    }
    sort(favourites);
    favourites.clone()
}

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
            eprintln!("Error creating favourites file: {}", e);
            return favourites.clone();
        }
    };
    match serde_json::to_string_pretty(favourites) {
        Ok(json_string) => {
            if let Err(e) = file.write_all(json_string.as_bytes()) {
                eprintln!("Error writing to favourites file: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error serializing favourites: {}", e);
        }
    }
    sort(favourites);
    favourites.clone()
}

fn sort(games: &mut [Game]) {
    games.sort_by_key(|x| x.description().clone());
}

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