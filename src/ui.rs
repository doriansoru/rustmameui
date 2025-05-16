use dioxus::prelude::*;
use crate::rustmameuiconfig::Config;
use crate::game::Game;
use rfd::FileDialog; // Used for native file/folder dialogs
use rust_i18n::t; // Macro for internationalization (i18n)
use std::path::PathBuf;
use tokio::task; // Import tokio::task

// Define external assets
const TABS_CSS: Asset = asset!("/assets/tabs.css");
const ABOUT: Asset = asset!("/assets/about.svg");

// Application name constant
const APP_NAME: &str = "MAMEUI";
// Batch size for processing ROMs (e.g., verification)
const BATCH_SIZE: usize = 100;

/// Enum to represent the different tabs in the application UI.
#[derive(PartialEq, Eq, Clone)]
enum Tab {
    Games,
    Favourites,
    Settings,
    About,
}

/// The main entry point for drawing and launching the application UI.
pub fn draw() {
    #[cfg(feature = "desktop")]
    fn launch_app() {
        let window = dioxus::desktop::tao::window::WindowBuilder::new()
            .with_resizable(true)
            .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(1024.0, 860.0))
            .with_title::<&str>(APP_NAME);

        dioxus::LaunchBuilder::new()
            .with_cfg(dioxus::desktop::Config::new().with_window(window).with_menu(None))
            .launch(App);
    }

    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        dioxus::launch(App);
    }

    launch_app();
}

/// Checks if a native file dialog utility (zenity or kdialog) is installed.
fn check_dialog_utility() -> Result<(), String> {
    let mut found = false;
    if which::which("zenity").is_ok() {
        found = true;
    }
    if which::which("kdialog").is_ok() {
        found = true;
    }
    if !found {
        return Err(t!("error_no_dialog_utility").to_string());
    }
    Ok(())
}

/// Component for rendering a list of games or favourites.
#[component]
fn GameListTab(
    /// The list of games or favourites to display.
    items: Signal<Vec<Game>>,
    /// The full list of games (needed for adding to favourites in Games tab).
    all_games: Signal<Vec<Game>>,
    /// The list of favourites (needed for adding/removing).
    favourites: Signal<Vec<Game>>,
    /// Path to the MAME executable.
    mame_executable: Signal<PathBuf>,
    /// Path to the ROMs directory.
    roms_path: Signal<PathBuf>,
    /// Path to the snapshots zip file.
    snap_file: Signal<PathBuf>,
    /// Signal to control context menu visibility.
    show_context_menu: Signal<bool>,
    /// Text for the main header (e.g., "all_games", "favourite_games").
    header_text: String,
    /// Text for the filter input label.
    filter_label_text: String,
    /// Flag indicating if this is the favourites list tab.
    is_favourite_list: bool,
) -> Element {
    let mut menu_x = use_signal(|| 0_i32);
    let mut menu_y = use_signal(|| 0_i32);

    let mut item_filter = use_signal(String::new);

    let filtered_items = use_memo(move || {
        let filter = item_filter();
        let current_items = items(); // Use the 'items' prop here
        current_items
            .iter()
            .filter(|item| item.description().to_lowercase().contains(&filter.to_lowercase()))
            .cloned()
            .collect::<Vec<Game>>()
    });

    let handle_context_menu = move |event: Event<MouseData>| {
        event.stop_propagation();
        event.prevent_default();
        let (x, y) = (event.coordinates().client().x as i32, event.coordinates().client().y as i32);
        menu_x.set(x);
        menu_y.set(y);
        show_context_menu.set(true);
    };

    let mut selected_item_rom = use_signal(String::new);
    let mut snap_data = use_signal(|| "".to_string());

    // Memo to determine the context menu label and action based on selected item and tab type
    let context_menu_action = use_memo(move || {
        let rom = selected_item_rom();
        let is_favourite = favourites().iter().any(|fav| fav.rom() == rom);

        if is_favourite_list {
            // If it's the favourites list, the option is always "Remove from favourites"
            (t!("remove_from_favourites").to_string(), ContextAction::RemoveFavourite)
        } else {
            // If it's the games list, the option is "Add/Remove from favourites" based on its favourite status
            if is_favourite {
                (t!("remove_from_favourites").to_string(), ContextAction::RemoveFavourite)
            } else {
                (t!("add_to_favourites").to_string(), ContextAction::AddFavourite)
            }
        }
    });

    // Enum to represent the context menu actions
    #[derive(PartialEq, Clone, Copy)]
    enum ContextAction {
        AddFavourite,
        RemoveFavourite,
    }


    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {header_text} }
            p {
                margin_bottom: "1.5em",
                {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
            }
            // Content Layout
            div {
                class: "row",
                // Left Column: List and Filter
                div {
                    class: "column",
                    // List
                    select {
                        style: "max-width: 500px;",
                        resize: "both",
                        autocomplete: "on",
                        size: 20,
                        oncontextmenu: handle_context_menu,
                        onchange: move |event| async move { // Made onchange async move
                            let rom = event.value().clone();
                            selected_item_rom.set(rom.clone());

                            let selected_game_option = items().into_iter() // Search within the *current* list (games or favourites)
                                .find(|item| item.rom().clone() == rom);

                            if let Some(selected_game) = selected_game_option {
                                // Use tokio::task::spawn_blocking for get_snap as it might read from disk
                                let snap_path_clone = snap_file().display().to_string(); // Read signal before moving
                                let selected_game_clone = selected_game.clone();
                                let snap_handle: tokio::task::JoinHandle<Result<String, Box<dyn std::error::Error + Send + Sync + 'static>>> = task::spawn_blocking(move || {
                                    selected_game_clone.get_snap(&snap_path_clone)
                                });

                                // Await the result of the blocking task
                                match snap_handle.await {
                                    Ok(snap_result) => {
                                        match snap_result {
                                            Ok(snap_base64) => {
                                                snap_data.set(snap_base64);
                                            },
                                            Err(e) => {
                                                // Handle potential error from the blocking task
                                                eprintln!("{}", t!("error_fetching_snap.error", error = e.to_string()));
                                                snap_data.set("".to_string()); // Clear snap on error
                                            }
                                        }
                                    },
                                    Err(_) => {
                                        snap_data.set("".to_string());
                                    }
                                }
                            } else {
                                snap_data.set("".to_string()); // Clear snap if game not found (shouldn't happen)
                            }
                        },
                        ondoubleclick: move |_| {
                            let rom = selected_item_rom();
                            if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                match game.launch(&mame_executable(), &roms_path()) {
                                    Ok(_) => {},
                                    Err(e) => {
                                        // Handle potential error from the blocking task
                                        eprintln!("{}", t!("error_launching_game.error", error = e.to_string()));
                                    }
                                }
                            }
                        },
                        onkeypress: move |ev: Event<KeyboardData>| {
                            if ev.key() == Key::Enter {
                                let rom = selected_item_rom();
                                if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                    match game.launch(&mame_executable(), &roms_path()) {
                                        Ok(_) => {},
                                        Err(e) => {
                                            // Handle potential error from the blocking task
                                            eprintln!("{}", t!("error_launching_game.error", error = e.to_string()));
                                        }
                                    }
                                }
                            }
                        },
                        for item in filtered_items() {
                            option {
                                value: item.rom().clone(),
                                {item.description().clone()}
                            }
                        }
                    }
                    // Context Menu
                    {
                        if show_context_menu() {
                            rsx! {
                                div {
                                    class: "popup",
                                    style: "left: {menu_x()}px; top: {menu_y()}px;",
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| {
                                            let rom = selected_item_rom();
                                             if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                                match game.launch(&mame_executable(), &roms_path()) {
                                                    Ok(_) => {},
                                                    Err(e) => {
                                                        // Handle potential error from the blocking task
                                                        eprintln!("{}", t!("error_launching_game.error", error = e.to_string()));
                                                    }
                                                }
                                            }
                                            show_context_menu.set(false);
                                        },
                                        {t!("launch_game")}
                                    }
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| {
                                            let rom = selected_item_rom();
                                            let action = context_menu_action().1; // Get the ContextAction enum value
                                            let config = match Config::new() {
                                                Ok(c) => c,
                                                Err(e) => {
                                                    eprintln!("{}", t!("error_loading_configuration.error", error = e.to_string()));
                                                    Config::default()
                                                }
                                            };

                                            let mut current_favourites = favourites();

                                            // Find the game in the *full* games list if adding, or create a dummy for removing
                                            match action {
                                                ContextAction::AddFavourite => {
                                                     if let Some(selected_game) = all_games.read().iter().find(|game| game.rom() == rom) {
                                                        let new_favourites = crate::games::add_favourite(&config, &mut current_favourites, selected_game);
                                                        favourites.set(new_favourites);
                                                    }
                                                },
                                                ContextAction::RemoveFavourite => {
                                                     // Need to find the *specific* favourite to remove from the favourites list
                                                     // Pass a dummy game with just ROM for comparison/removal
                                                     let dummy_game = Game::new(&rom, &String::from(""), false); // Corrected: Use String::from("")
                                                     let new_favourites = crate::games::remove_favourite(&config, &mut current_favourites, &dummy_game); // Pass dummy for comparison
                                                     favourites.set(new_favourites);
                                                },
                                            }


                                            // After modifying favourites, potentially re-filter the *current* list if it's the favourites tab
                                            if is_favourite_list {
                                                 items.set(favourites()); // Update the items list signal for the favourites tab
                                            }
                                            show_context_menu.set(false);
                                        },
                                        {context_menu_action().0} // Use the calculated label
                                    }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                    // Filter Input
                    div {
                        div {
                            class: "column",
                            input {
                                r#type: "text",
                                size: 50,
                                oninput: move |evt| {
                                    item_filter.set(evt.value());
                                }
                            }
                            label {
                                {filter_label_text}
                            }
                        }
                    }
                }
                // Right Column: Snapshot
                div {
                    class: "snap",
                    img {
                        src: "{snap_data()}",
                        width: 500,
                    }
                }
            }
        }
    }
}


/// Component for the Settings tab content.
#[component]
fn SettingsTab(
    mame_executable: Signal<std::path::PathBuf>,
    roms_path: Signal<std::path::PathBuf>,
    snap_file: Signal<std::path::PathBuf>,
    status: Signal<String>,
    games: Signal<Vec<Game>>,
) -> Element {
    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {t!("app_name.settings", app_name = APP_NAME )} }
            // Settings Fields
            // MAME Executable Path
            div {
                class: "setting",
                label { {t!("mame_executable_path")} }
                input {
                    size: 50,
                    value: "{mame_executable().display().to_string()}",
                    onchange: move |evt| {
                        mame_executable.set(std::path::PathBuf::from(evt.value()));
                    }
                }
                button {
                    onclick: move |_| {
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(file) = FileDialog::new()
                                    .add_filter(t!("all_files"), &["*"])
                                    .pick_file() {
                                    mame_executable.set(file);
                                }
                            },
                            Err(e) => {
                                panic!("{}", e); 
                            }
                        }
                    },
                    {t!("browse")}
                }
            }
            // ROMs Directory Path
            div {
                class: "setting",
                label { {t!("roms_directory")} }
                input {
                    size: 50,
                    value: "{roms_path().display().to_string()}",
                    onchange: move |evt| {
                        roms_path.set(std::path::PathBuf::from(evt.value()));
                    }
                }
                button {
                    onclick: move |_| {
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(directory) = FileDialog::new()
                                    .add_filter(t!("all_files"), &["*"])
                                    .pick_folder() {
                                    roms_path.set(directory);
                                }
                            },
                            Err(e) => {
                                panic!("{}", e); 
                            }
                        }
                    },
                    {t!("browse")}
                }
            }
            // Snapshots Zip File Path
            div {
                class: "setting",
                label { {t!("snaps_zip_not_7z_file_path")} }
                input {
                    size: 50,
                    value: "{snap_file().display().to_string()}",
                    onchange: move |evt| {
                        snap_file.set(std::path::PathBuf::from(evt.value()));
                    }
                }
                button {
                    onclick: move |_| {
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(file) = FileDialog::new()
                                    .add_filter(t!("zip_file"), &["*.zip"])
                                    .pick_file() {
                                    snap_file.set(file);
                                }
                            },
                            Err(e) => {
                                panic!("{}", e); 
                            }
                        }
                    },
                    "Browse"
                }
            }
            // Action Buttons
            div {
                div {
                    class: "row",
                    button {
                        onclick: move |_| {
                            let mut config = match Config::new() {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!("{}", t!("error_loading_configuration.error", error = e.to_string()));
                                    Config::default()
                                }
                            };

                            config.mame_executable = mame_executable();
                            config.roms_path = roms_path();
                            config.snap_file = snap_file();
                            match config.save() {
                                Ok(_) => {
                                    status.set(t!("the_settings_have_been_saved_correctly").into());
                                },
                                Err(e) => {
                                    status.set(t!("error_while_saving_settings.error", error = e.to_string()).to_string());
                                }
                            }
                        },
                        "Save settings"
                    }
                    button {
                        class: "action-button",
                        onclick: move |_| async move {
                            let config_third_clone = match Config::new() {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!("{}", t!("error_loading_configuration.error", error = e.to_string()));
                                    Config::default()
                                }
                            };                            
                            let config_fourth_clone = config_third_clone.clone();
                            let config_fifth_clone = config_third_clone.clone();
                            let config_sixth_clone = config_third_clone.clone();

                            status.set(t!("reading_the_roms_list_from_mame").into());
                            let xml_handle: tokio::task::JoinHandle<String> = tokio::task::spawn_blocking(
                                move || {
                                    // Executed in a blocking thread
                                    crate::games::get_xml_roms(&config_third_clone)
                                        .unwrap_or_else(|_| { panic!("{}",  t!("cannot_get_xml_data_from_mame").to_string() )}) 
                                }
                            );
                            let xml_string_result = xml_handle.await;
                            let xml_string: String = match xml_string_result {
                                Ok(s) => s, // Ok, the task has finished
                                Err(_) => {
                                    panic!("{}", t!("cannot_get_xml_data_from_mame").to_string()); 
                                }
                            };

                            status.set(t!("parsing_all_roms_and_their_descriptions").into());
                            // Create a new blocking task for parsing the XML string.
                            // The closure takes ownership of 'config_clone_for_parsing' and 'xml_string' (by value).
                            let parse_handle: tokio::task::JoinHandle<(Vec<String>, Vec<String>)> = tokio::task::spawn_blocking(
                                move || {
                                    // This closure executes in a dedicated blocking thread pool.
                                    // It performs the potentially heavy XML parsing.
                                    crate::games::get_all_roms(&config_fourth_clone, &xml_string) // Use the cloned config and the moved xml_string
                                    .unwrap_or_else(|_| {
                                            panic!("{}", t!("cannot_read_all_roms_and_their_descriptions").to_string()); 
                                        })
                                }
                            );
                            // Await the result from the parsing task.
                            let parse_result: Result<(Vec<String>, Vec<String>), tokio::task::JoinError> = parse_handle.await;
                            // Extract the result and handle potential errors that occurred within the blocking task itself
                            let (roms, descriptions) = match parse_result {
                                Ok(tuple) => {
                                    // The blocking task completed successfully and returned the tuple
                                    tuple
                                }
                                Err(_) => {
                                    panic!("{}", t!("cannot_read_all_roms_and_their_descriptions").to_string()); 
                                }
                            };

                            let mut all_games: Vec<Game> = Vec::new();
                            status.set(t!("verifying_working_roms").into());
                            let total_roms = roms.len();
                            let mut working_roms = Vec::with_capacity(total_roms);
                            let mut working_snaps = Vec::with_capacity(total_roms);
                            let num_batches = total_roms.div_ceil(BATCH_SIZE);

                            for batch_index in 0..num_batches {
                                let start_index = batch_index * BATCH_SIZE;
                                let end_index = usize::min((batch_index + 1) * BATCH_SIZE, total_roms);
                                status.set(t!("verifying_roms.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into());
                                let current_batch = &roms[start_index..end_index];
                                let current_batch_owned: Vec<String> = current_batch.to_vec();
                                let config_clone_for_rom_verify = config_fifth_clone.clone();
                                let parse_handle: tokio::task::JoinHandle<Vec<bool>> = tokio::task::spawn_blocking(
                                    move || {
                                        crate::games::verify_batch_roms(&config_clone_for_rom_verify, &current_batch_owned) // Use the cloned config
                                    }
                                );
                                let parse_result: Result<Vec<bool>, tokio::task::JoinError> = parse_handle.await;

                                // Extract the result and handle potential errors that occurred within the blocking task itself
                                let roms_batch_results = match parse_result {
                                    Ok(batch) => {
                                        batch
                                    }
                                    Err(e) => {
                                        panic!("{}", t!("rom_verification_task_error", error = e)); 
                                    }
                                };
                                working_roms.extend(roms_batch_results);

                                status.set(t!("verifying_snaps.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into());
                                let current_batch_owned: Vec<String> = current_batch.to_vec();
                                let config_clone_for_rom_verify = config_fifth_clone.clone();
                                let parse_handle: tokio::task::JoinHandle<Vec<bool>> = tokio::task::spawn_blocking(
                                    move || {
                                        crate::games::verify_batch_snaps(&config_clone_for_rom_verify, &current_batch_owned) // Use the cloned config
                                    }
                                );
                                let parse_result: Result<Vec<bool>, tokio::task::JoinError> = parse_handle.await;

                                // Extract the result and handle potential errors that occurred within the blocking task itself
                                let snaps_batch_results = match parse_result {
                                    Ok(batch) => {
                                        batch
                                    }
                                    Err(e) => {
                                        panic!("{}", t!("snap_verification_task_error", error = e)); 
                                    }
                                };
                                working_snaps.extend(snaps_batch_results);
                            }

                            if !roms.is_empty() {
                                for (i, rom) in roms.iter().enumerate() {
                                    if working_roms.get(i).copied().unwrap_or(false) {
                                        let game = Game::new(rom, descriptions.get(i).unwrap(), working_snaps.get(i).copied().unwrap_or(false));
                                        all_games.push(game);
                                    }
                                }
                            }

                            status.set(t!("saving_all_games").into());
                            match crate::games::save(&config_sixth_clone, &all_games) {
                                Ok(_) => {},
                                Err(e) => {
                                    panic!("{}", t!("error_while_saving_games_list.error", error = e.to_string()).to_string()); 
                                }
                            };
                            status.set(t!("complete").into());
                            games.set(all_games); // Update the games signal in App
                        },
                        {t!("refresh_games_list")}
                    }
                }
                div { {status()} }
            }
        }
    }
}

/// Component for the About tab content.
#[component]
fn AboutTab() -> Element {
    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {t!("about.app_name", app_name = APP_NAME)} }
            // Content
            div {
                class: "row",
                div {
                    img {
                        src: "{ABOUT}",
                        width: 300,
                        style: "margin-right: 1em",
                    }
                }
                div {
                    p {
                        {t!("app_name.version.by", app_name = APP_NAME, version = env!("CARGO_PKG_VERSION"))}
                        a {
                            href: "mailto:doriansoru@gmail.com",
                            "Dorian Soru"
                        },
                        ", 2025."
                    }
                    p {
                        {t!("released_under_the")}
                        a {
                            href: "https://www.gnu.org/licenses/gpl-3.0.html",
                            "GNU General Public License 3.0"
                        },
                        "."
                    }
                }
            }
        }
    }
}

/// The main application component.
#[component]
fn App() -> Element {
    let mut selected_tab = use_signal(|| Tab::Games);

    let mut config_load_error = use_signal(|| None::<String>); // Error message in loading config
    let mut games_load_error = use_signal(|| None::<String>); // Error message in loading games
    let mut fav_load_error = use_signal(|| None::<String>); // Error message in loading favourites
    
    let config = match crate::rustmameuiconfig::Config::new() { // Usa il percorso completo al modulo Config
        Ok(c) => {
            config_load_error.set(None); // Conferma nessun errore di configurazione
            c
        },
        Err(e) => {
            let error_msg = t!("error_loading_configuration.error", error = e.to_string()).to_string();
            config_load_error.set(Some(error_msg.clone())); // Imposta l'errore di configurazione
            eprintln!("{}", error_msg); // Logga l'errore per debugging
            crate::rustmameuiconfig::Config::default() // Usa l'implementazione Default
        }
    };
    
    let config_clone = config.clone();
    let mame_executable = use_signal(|| config_clone.mame_executable);
    let snap_file = use_signal(|| config_clone.snap_file);
    let roms_path = use_signal(|| config_clone.roms_path);

    // Handle error loading games
    let games = use_signal(|| {
        if config_load_error().is_none() { // Load games only if the configuration is valid
            match crate::games::load(&config) {
                Ok(g) => {
                    games_load_error.set(None); // Confirm no error
                    g
                },
                Err(e) => {
                    let error_msg = t!("error_loading_games.error", error = e.to_string()).to_string();
                    eprintln!("{}", error_msg);
                    games_load_error.set(Some(error_msg)); // Set the error message
                    Vec::new() // Returns an empty Vec
                }
            }
        } else {
            Vec::new() // No game with invalid configuration
        }
    });

    // Handle error loading favourites
    let favourites = use_signal(|| {
        if config_load_error().is_none() { // Load favourites only if the configuration is valid
           match crate::games::load_favourites(&config) {
               Ok(f) => {
                   fav_load_error.set(None); // Confirm no error
                   f
               },
               Err(e) => {
                    let error_msg = t!("error_loading_favourites.error", error = e.to_string()).to_string();
                   eprintln!("{}", error_msg);
                   fav_load_error.set(Some(error_msg)); // Set the error message
                   Vec::new() // Returns an empty Vec
               }
           }
       } else {
           Vec::new() // No favourite with invalid configuration
       }
    });

    let status = use_signal(|| String::from(""));
    let mut show_context_menu = use_signal(|| false);

    let hide_menu = move |_| {
        show_context_menu.set(false);
    };

    rsx! {
        document::Link { rel: "stylesheet", href: TABS_CSS }
        div {
            class: "container",
            if let Some(error_msg) = config_load_error() {
                div {
                    class: "error-message",
                    "{error_msg}"
                }
            } else {
                // Tabs Header
                div {
                    class: "tabs-header",
                    button {
                        class: if selected_tab() == Tab::Games { "tab-button active" } else { "tab-button" },
                        onclick: move |_| selected_tab.set(Tab::Games),
                        {t!("games")}
                    }
                    button {
                        class: if selected_tab() == Tab::Favourites { "tab-button active" } else { "tab-button" },
                        onclick: move |_| selected_tab.set(Tab::Favourites),
                        {t!("favourites")}
                    }
                    button {
                        class: if selected_tab() == Tab::Settings { "tab-button active" } else { "tab-button" },
                        onclick: move |_| selected_tab.set(Tab::Settings),
                        {t!("settings")}
                    }
                    button {
                        class: if selected_tab() == Tab::About { "tab-button active" } else { "tab-button" },
                        onclick: move |_| selected_tab.set(Tab::About),
                        {t!("about")}
                    }
                }

                // Show error on loading games and favourites
                if let Some(error_msg) = games_load_error() {
                    div { class: "error-message", "{error_msg}" }
                }
                if let Some(error_msg) = fav_load_error() {
                     div { class: "error-message", "{error_msg}" }
                }
                // Tab Content
                div {
                    onclick: hide_menu,
                    class: "tab-content",
                    if selected_tab() == Tab::Games {
                        GameListTab {
                            items: games, // Pass the main games list
                            all_games: games, // Pass the main games list (needed for add_favourite logic)
                            favourites: favourites, // Pass favourites for context menu logic
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            show_context_menu: show_context_menu,
                            header_text: t!("all_games").to_string(),
                            filter_label_text: t!("type_the_name_of_a_game_to_search_for_it").to_string(),
                            is_favourite_list: false, // Flag for Games tab
                        }
                    }
                    if selected_tab() == Tab::Favourites {
                        GameListTab {
                            items: favourites, // Pass the favourites list
                            all_games: games, // Still need access to all_games to get full Game struct if needed (e.g., add_favourite from games tab logic might implicitly be here, though less likely). It's safer to pass it.
                            favourites: favourites, // Pass favourites
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            show_context_menu: show_context_menu,
                            header_text: t!("favourite_games").to_string(),
                            filter_label_text: t!("type_the_name_of_a_game_to_search_for_it").to_string(),
                            is_favourite_list: true, // Flag for Favourites tab
                        }
                    }
                    if selected_tab() == Tab::Settings {
                        SettingsTab {
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            status: status,
                            games: games, // Pass games for the refresh logic in SettingsTab
                        }
                    }
                    if selected_tab() == Tab::About {
                        AboutTab {}
                    }
                }
            }
        }
    }
}