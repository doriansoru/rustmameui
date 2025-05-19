use dioxus::prelude::*;
use crate::rustmameuiconfig::{Config, ConfigErrorWithThisError}; // Import the new config error type
use crate::game::{Game, GameError}; // Import the new game error type
use crate::games::{self, GamesError}; // Import the new games error type and the module itself
use rfd::FileDialog; // Used for native file/folder dialogs
use rust_i18n::t; // Macro for internationalization (i18n)
use std::path::PathBuf;
use tokio::task; // Import tokio::task
use thiserror::Error; // Add this import
use which::Error as WhichError; // Import WhichError explicitly

/// Custom error type for operations within the `ui` module.
#[derive(Error, Debug)]
pub enum UiError {
    #[error("Missing a required native file dialog utility (zenity or kdialog). Please install one of them.")]
    MissingDialogUtility,
    #[error("Error loading configuration: {0}")]
    ConfigError(#[from] ConfigErrorWithThisError),
    #[error("Error loading/managing games data: {0}")]
    GamesError(#[from] GamesError),
    #[error("Error performing game operation: {0}")]
    GameError(#[from] GameError),
    #[error("Error during async task join: {0}")]
    TaskJoinError(#[from] tokio::task::JoinError),
    #[error("Error checking dialog utility: {0}")]
    WhichError(#[from] WhichError),
}

// Define external assets
const TABS_CSS: Asset = asset!("/assets/tabs.css");
const ABOUT: Asset = asset!("/assets/about.svg");

// Application name constant
const APP_NAME: &str = "MAMEUI";
// Batch size for processing ROMs (e.e., verification)
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

#[cfg(target_os = "linux")]
/// Checks if a native file dialog utility (zenity or kdialog) is installed.
///
/// # Returns
///
/// Returns `Ok(())` if a utility is found, or `UiError::MissingDialogUtility` otherwise.
pub fn check_dialog_utility() -> Result<(), UiError> {
    let mut found = false;
    if which::which("zenity").is_ok() {
        found = true;
    }
    if which::which("kdialog").is_ok() {
        found = true;
    }
    if !found {
        return Err(UiError::MissingDialogUtility);
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
    /// Signal to report errors from this tab.
    error_message: Signal<Option<String>>,
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
                {t!("double_click_on_a_game_to_launch_right_click_for_more_options").to_string()} // Added .to_string()
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
                            error_message.set(None); // Clear previous error

                            let selected_game_option = items().into_iter() // Search within the *current* list (games or favourites)
                                .find(|item| item.rom().clone() == rom);

                            if let Some(selected_game) = selected_game_option {
                                // Use tokio::task::spawn_blocking for get_snap as it might read from disk
                                let snap_path_clone = snap_file().display().to_string();
                                // Read signal before moving
                                let selected_game_clone = selected_game.clone();
                                let snap_handle: tokio::task::JoinHandle<Result<String, GameError>> = task::spawn_blocking(move || {
                                    selected_game_clone.get_snap(&snap_path_clone)
                                });

                                // Await the result of the blocking task
                                match snap_handle.await {
                                    Ok(inner_result) => { // Changed variable name to avoid shadowing
                                        match inner_result {
                                            Ok(snap_base64) => {
                                                snap_data.set(snap_base64);
                                            },
                                            Err(e) => {
                                                // Handle potential error from the blocking task
                                                let err = UiError::GameError(e); // Wrap the GameError
                                                eprintln!("Error fetching snap: {}", err);
                                                error_message.set(Some(t!("error_fetching_snap.error", error = err.to_string()).to_string())); // Added .to_string()
                                                snap_data.set("".to_string()); // Clear snap on error
                                            }
                                        }
                                    },
                                    Err(e) => {
                                         let err = UiError::TaskJoinError(e); // Wrap the JoinError
                                         eprintln!("Error joining snap task: {}", err);
                                         error_message.set(Some(t!("error_fetching_snap.error", error = err.to_string()).to_string())); // Added .to_string()
                                         snap_data.set("".to_string());
                                    }
                                }
                            } else {
                                snap_data.set("".to_string()); // Clear snap if game not found (shouldn't happen)
                            }
                        },
                        ondoubleclick: move |_| async move // Made ondoubleclick async move
                        {
                            let rom = selected_item_rom();
                            error_message.set(None); // Clear previous error
                            if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                // Use tokio::task::spawn_blocking for launch as it executes an external process
                                let mame_executable_clone = mame_executable().clone();
                                let roms_path_clone = roms_path().clone();
                                let game_clone = game.clone();
                                let launch_handle: tokio::task::JoinHandle<Result<(), GameError>> = task::spawn_blocking(move || {
                                     game_clone.launch(&mame_executable_clone, &roms_path_clone)
                                });

                                match launch_handle.await {
                                    Ok(inner_result) => { // Changed variable name to avoid shadowing
                                         if let Err(e) = inner_result {
                                            let err = UiError::GameError(e); // Wrap the GameError
                                            eprintln!("Error launching game: {}", err);
                                            error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
                                        }
                                    },
                                    Err(e) => {
                                        let err = UiError::TaskJoinError(e); // Wrap the JoinError
                                        eprintln!("Error joining launch task: {}", err);
                                        error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
                                    }
                                }
                            }
                        },
                        onkeypress: move |ev: Event<KeyboardData>| async move // Made onkeypress async move
                        {
                            if ev.key() == Key::Enter {
                                let rom = selected_item_rom();
                                error_message.set(None); // Clear previous error
                                if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                     // Use tokio::task::spawn_blocking for launch
                                     let mame_executable_clone = mame_executable().clone();
                                     let roms_path_clone = roms_path().clone();
                                     let game_clone = game.clone();
                                     let launch_handle: tokio::task::JoinHandle<Result<(), GameError>> = task::spawn_blocking(move || {
                                         game_clone.launch(&mame_executable_clone, &roms_path_clone)
                                     });

                                     match launch_handle.await {
                                        Ok(inner_result) => { // Changed variable name to avoid shadowing
                                             if let Err(e) = inner_result {
                                                let err = UiError::GameError(e); // Wrap the GameError
                                                eprintln!("Error launching game: {}", err);
                                                error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
                                            }
                                        },
                                        Err(e) => {
                                            let err = UiError::TaskJoinError(e); // Wrap the JoinError
                                            eprintln!("Error joining launch task: {}", err);
                                            error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
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
                                        onclick: move |_| async move // Made onclick async move
                                        {
                                            let rom = selected_item_rom();
                                            error_message.set(None); // Clear previous error
                                            if let Some(game) = items.read().iter().find(|item| item.rom() == rom) {
                                                 // Use tokio::task::spawn_blocking for launch
                                                 let mame_executable_clone = mame_executable().clone();
                                                 let roms_path_clone = roms_path().clone();
                                                 let game_clone = game.clone();
                                                 let launch_handle: tokio::task::JoinHandle<Result<(), GameError>> = task::spawn_blocking(move || {
                                                     game_clone.launch(&mame_executable_clone, &roms_path_clone)
                                                 });

                                                 match launch_handle.await {
                                                    Ok(inner_result) => { // Changed variable name to avoid shadowing
                                                         if let Err(e) = inner_result {
                                                            let err = UiError::GameError(e); // Wrap the GameError
                                                            eprintln!("Error launching game from context menu: {}", err);
                                                            error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
                                                        }
                                                    },
                                                    Err(e) => {
                                                         let err = UiError::TaskJoinError(e); // Wrap the JoinError
                                                         eprintln!("Error joining context menu launch task: {}", err);
                                                         error_message.set(Some(t!("error_launching_game.error", error = err.to_string()).to_string())); // Added .to_string()
                                                    }
                                                 }
                                            }
                                            show_context_menu.set(false);
                                        },
                                        {t!("launch_game").to_string()} // Added .to_string()
                                    }
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| // No need for async here as add/remove_favourite don't return Result
                                        {
                                            let rom = selected_item_rom();
                                            error_message.set(None); // Clear previous error
                                            let action = context_menu_action().1; // Get the ContextAction enum value

                                            // Attempt to load config - handle potential error
                                            let config = match Config::new() {
                                                Ok(c) => c,
                                                Err(e) => {
                                                    let err = UiError::ConfigError(e); // Wrap the ConfigError
                                                    eprintln!("Error loading configuration for favourite action: {}", err);
                                                    error_message.set(Some(t!("error_loading_configuration.error", error = err.to_string()).to_string())); // Added .to_string()
                                                    // Return early or use a default config if appropriate for the action
                                                    // For now, returning early as saving without a valid config is likely not desired.
                                                    show_context_menu.set(false);
                                                    return;
                                                }
                                            };

                                            let mut current_favourites = favourites();

                                            // Find the game in the *full* games list if adding, or create a dummy for removing
                                            match action {
                                                ContextAction::AddFavourite => {
                                                     if let Some(selected_game) = all_games.read().iter().find(|game| game.rom() == rom) {
                                                        // add_favourite prints errors internally, no Result to handle here
                                                        let new_favourites = games::add_favourite(&config, &mut current_favourites, selected_game);
                                                        favourites.set(new_favourites);
                                                     }
                                                },
                                                ContextAction::RemoveFavourite => {
                                                     // Need to find the *specific* favourite to remove from the favourites list
                                                     // Pass a dummy game with just ROM for comparison/removal
                                                     let dummy_game = Game::new(&rom, &String::from(""), false);
                                                     // remove_favourite prints errors internally, no Result to handle here
                                                     let new_favourites = games::remove_favourite(&config, &mut current_favourites, &dummy_game);
                                                     favourites.set(new_favourites);
                                                },
                                            }

                                            // After modifying favourites, potentially re-filter the *current* list if it's the favourites tab
                                            if is_favourite_list {
                                                 items.set(favourites()); // Update the items list signal for the favourites tab
                                            }
                                            show_context_menu.set(false);
                                        },
                                        {context_menu_action().0.to_string()} // Use the calculated label - Added .to_string()
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
                                {filter_label_text.to_string()} // Added .to_string()
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
             // Display errors from this tab
            if let Some(err) = error_message() {
                div {
                    class: "error-message",
                    "{err}"
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
    /// Signal to report errors from this tab.
    error_message: Signal<Option<String>>,
) -> Element {
    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {t!("app_name.settings", app_name = APP_NAME ).to_string()} } // Added .to_string()
            // Settings Fields
            // MAME Executable Path
            div {
                class: "setting",
                label { {t!("mame_executable_path").to_string()} } // Added .to_string()
                input {
                    size: 50,
                    value: "{mame_executable().display().to_string()}",
                    onchange: move |evt| {
                        mame_executable.set(std::path::PathBuf::from(evt.value()));
                        error_message.set(None); // Clear previous error
                    }
                }
                button {
                    onclick: move |_| {
                        error_message.set(None); // Clear previous error
                        #[cfg(target_os = "linux")]
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(file) = FileDialog::new()
                                    .add_filter(t!("all_files").to_string(), &["*"]) // Added .to_string()
                                    .pick_file() {
                                    mame_executable.set(file);
                                }
                            },
                            Err(_) => {
                                 let err = UiError::MissingDialogUtility; // Use the specific error
                                 eprintln!("Error checking dialog utility: {}", err);
                                 error_message.set(Some(t!("error_checking_dialog_utility.error", error = err.to_string()).to_string())); // Added .to_string()
                            }
                        }

                        #[cfg(target_os = "windows")]
                        if let Some(file) = FileDialog::new()
                            .add_filter(&t!("all_files").to_string(), &["*"]) // Added .to_string()
                            .pick_file() {
                            mame_executable.set(file);
                        }
                    },
                    {t!("browse").to_string()} // Added .to_string()
                }
            }
            // ROMs Directory Path
            div {
                class: "setting",
                label { {t!("roms_directory").to_string()} } // Added .to_string()
                input {
                    size: 50,
                    value: "{roms_path().display().to_string()}",
                    onchange: move |evt| {
                        roms_path.set(std::path::PathBuf::from(evt.value()));
                        error_message.set(None); // Clear previous error
                    }
                }
                button {
                    onclick: move |_| {
                        error_message.set(None); // Clear previous error
                        #[cfg(target_os = "linux")]
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(directory) = FileDialog::new()
                                    .add_filter(t!("all_files").to_string(), &["*"]) // Added .to_string()
                                    .pick_folder() {
                                    roms_path.set(directory);
                                }
                            },
                            Err(_) => {
                                 let err = UiError::MissingDialogUtility; // Use the specific error
                                 eprintln!("Error checking dialog utility: {}", err);
                                 error_message.set(Some(t!("error_checking_dialog_utility.error", error = err.to_string()).to_string())); // Added .to_string()
                            }
                        }

                        #[cfg(target_os = "windows")]
                        if let Some(directory) = FileDialog::new()
                            .add_filter(&t!("all_files").to_string(), &["*"]) // Added .to_string()
                            .pick_folder() {
                            roms_path.set(directory);
                        }
                    },
                    {t!("browse").to_string()} // Added .to_string()
                }
            }
            // Snapshots Zip File Path
            div {
                class: "setting",
                label { {t!("snaps_zip_not_7z_file_path").to_string()} } // Added .to_string()
                input {
                    size: 50,
                    value: "{snap_file().display().to_string()}",
                    onchange: move |evt| {
                        snap_file.set(std::path::PathBuf::from(evt.value()));
                        error_message.set(None); // Clear previous error
                    }
                }
                button {
                    onclick: move |_| {
                        error_message.set(None); // Clear previous error
                        #[cfg(target_os = "linux")]
                        match check_dialog_utility() {
                            Ok(_) => {
                                if let Some(file) = FileDialog::new()
                                    .add_filter(t!("zip_file").to_string(), &["*.zip"]) // Added .to_string()
                                    .pick_file() {
                                    snap_file.set(file);
                                }
                            },
                            Err(_) => {
                                 let err = UiError::MissingDialogUtility; // Use the specific error
                                 eprintln!("Error checking dialog utility: {}", err);
                                 error_message.set(Some(t!("error_checking_dialog_utility.error", error = err.to_string()).to_string())); // Added .to_string()
                            }
                        }

                        #[cfg(target_os = "windows")]
                        if let Some(file) = FileDialog::new()
                            .add_filter(&t!("zip_file").to_string(), &["*.zip"]) // Added .to_string()
                            .pick_file() {
                            snap_file.set(file);
                        }
                    },
                    "Browse" // This specific string is not translated
                }
            }
            // Action Buttons
            div {
                div {
                    class: "row",
                    button {
                        onclick: move |_| {
                            error_message.set(None); // Clear previous error
                            let config = match Config::new() { // Use the new Config::new which returns Result
                                Ok(c) => c,
                                Err(e) => {
                                    let err = UiError::ConfigError(e); // Wrap ConfigError
                                    eprintln!("Error loading configuration to save: {}", err);
                                    status.set(t!("error_while_saving_settings.error", error = err.to_string()).to_string()); // Added .to_string()
                                    error_message.set(Some(t!("error_while_saving_settings.error", error = err.to_string()).to_string())); // Added .to_string()
                                    // Return a default config or handle appropriately; for saving, likely best to stop
                                    return;
                                }
                            };
                            let mut config_to_save = config.clone(); // Clone for modification
                            config_to_save.mame_executable = mame_executable();
                            config_to_save.roms_path = roms_path();
                            config_to_save.snap_file = snap_file();
                            match config_to_save.save() { // This now returns Result<(), ConfigErrorWithThisError>
                                Ok(_) => {
                                    status.set(t!("the_settings_have_been_saved_correctly").into());
                                },
                                Err(e) => {
                                    let err = UiError::ConfigError(e); // Wrap ConfigError
                                    eprintln!("Error saving settings: {}", err);
                                    status.set(t!("error_while_saving_settings.error", error = err.to_string()).to_string()); // Added .to_string()
                                    error_message.set(Some(t!("error_while_saving_settings.error", error = err.to_string()).to_string())); // Added .to_string()
                                }
                            }
                        },
                        {t!("save_settings").to_string()} // Added .to_string()
                    }
                    button {
                        class: "action-button",
                        onclick: move |_| async move {
                            error_message.set(None); // Clear previous error
                            let config = match Config::new() {
                                Ok(c) => c,
                                Err(e) => {
                                    let err = UiError::ConfigError(e); // Wrap ConfigError
                                    eprintln!("Error loading configuration for refresh: {}", err);
                                    error_message.set(Some(t!("error_loading_configuration.error", error = err.to_string()).to_string())); // Added .to_string()
                                    return; // Stop if config loading fails
                                }
                            };

                            status.set(t!("reading_the_roms_list_from_mame").into());
                            let config_clone = config.clone(); // Clone config for the async block
                            let xml_handle: tokio::task::JoinHandle<Result<String, GamesError>> = tokio::task::spawn_blocking(
                                move || {
                                    // Executed in a blocking thread
                                    games::get_xml_roms(&config_clone) // This now returns Result<String, GamesError>
                                }
                            );

                            let xml_string_result = xml_handle.await;
                            let xml_string: String = match xml_string_result {
                                Ok(inner_result) => {
                                    match inner_result {
                                        Ok(s) => s, // Got the XML string
                                        Err(e) => {
                                            let err = UiError::GamesError(e); // Wrap GamesError
                                            eprintln!("Error getting XML data from MAME: {}", err);
                                            error_message.set(Some(t!("cannot_get_xml_data_from_mame.error", error = err.to_string()).to_string())); // Added .to_string()
                                            status.set(t!("error").into());
                                            return; // Stop if XML fetching fails
                                        }
                                    }
                                },
                                Err(e) => {
                                    let err = UiError::TaskJoinError(e); // Wrap JoinError
                                    eprintln!("Error joining XML task: {}", err);
                                    error_message.set(Some(t!("cannot_get_xml_data_from_mame.error", error = err.to_string()).to_string())); // Added .to_string()
                                     status.set(t!("error").into());
                                    return; // Stop if the task joining fails
                                }
                            };

                            status.set(t!("parsing_all_roms_and_their_descriptions").into());
                            // Create a new blocking task for parsing the XML string.
                            let config_clone_for_parsing = config.clone(); // Clone config for parsing
                            let parse_handle: tokio::task::JoinHandle<Result<(Vec<String>, Vec<String>), GamesError>> = tokio::task::spawn_blocking(
                                move || {
                                    // This closure executes in a dedicated blocking thread pool.
                                    // It performs the potentially heavy XML parsing.
                                    games::get_all_roms(&config_clone_for_parsing, &xml_string) // This now returns Result
                                }
                            );

                            // Await the result from the parsing task.
                            let parse_result: Result<Result<(Vec<String>, Vec<String>), GamesError>, tokio::task::JoinError> = parse_handle.await;

                            // Extract the result and handle potential errors
                            let (roms, descriptions) = match parse_result {
                                Ok(inner_result) => {
                                    match inner_result {
                                        Ok(tuple) => tuple, // Got the parsed data
                                        Err(e) => {
                                            let err = UiError::GamesError(e); // Wrap GamesError
                                            eprintln!("Error parsing ROMs: {}", err);
                                            error_message.set(Some(t!("cannot_read_all_roms_and_their_descriptions.error", error = err.to_string()).to_string())); // Added .to_string()
                                             status.set(t!("error").into());
                                            return; // Stop if parsing fails
                                        }
                                    }
                                },
                                Err(e) => {
                                     let err = UiError::TaskJoinError(e); // Wrap JoinError
                                     eprintln!("Error joining parse task: {}", err);
                                     error_message.set(Some(t!("cannot_read_all_roms_and_their_descriptions.error", error = err.to_string()).to_string())); // Added .to_string()
                                     status.set(t!("error").into());
                                    return; // Stop if task joining fails
                                }
                            };

                            let mut all_games: Vec<Game> = Vec::new();
                            status.set(t!("verifying_working_roms").into());
                            let total_roms = roms.len();
                            let mut working_roms = Vec::with_capacity(total_roms);
                            let mut working_snaps = Vec::with_capacity(total_roms);
                            let num_batches = total_roms.div_ceil(BATCH_SIZE);

                            let config_clone_for_verification = config.clone(); // Clone config once for the verification loop

                            for batch_index in 0..num_batches {
                                let start_index = batch_index * BATCH_SIZE;
                                let end_index = usize::min((batch_index + 1) * BATCH_SIZE, total_roms);
                                status.set(t!("verifying_roms.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).to_string()); // Added .to_string()
                                let current_batch = &roms[start_index..end_index];
                                let current_batch_owned: Vec<String> = current_batch.to_vec();

                                // verify_batch_roms prints errors internally, no Result to handle from it directly
                                let roms_batch_results = games::verify_batch_roms(&config_clone_for_verification, &current_batch_owned);
                                working_roms.extend(roms_batch_results);

                                status.set(t!("verifying_snaps.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).to_string()); // Added .to_string()
                                let current_batch_owned: Vec<String> = current_batch.to_vec();

                                // verify_batch_snaps prints errors internally, no Result to handle from it directly
                                let snaps_batch_results = games::verify_batch_snaps(&config_clone_for_verification, &current_batch_owned);
                                working_snaps.extend(snaps_batch_results);
                            }

                            if !roms.is_empty() {
                                for (i, rom) in roms.iter().enumerate() {
                                    if working_roms.get(i).copied().unwrap_or(false) {
                                        let game = Game::new(rom, descriptions.get(i).unwrap_or(&String::from("Unknown Description")), working_snaps.get(i).copied().unwrap_or(false));
                                        all_games.push(game);
                                    }
                                }
                            }

                            status.set(t!("saving_all_games").into());
                            let config_clone_for_save = config.clone(); // Clone config for save
                            match games::save(&config_clone_for_save, &all_games) { // This now returns Result<(), GamesError>
                                Ok(_) => {},
                                Err(e) => {
                                    let err = UiError::GamesError(e); // Wrap GamesError
                                    eprintln!("Error saving games list: {}", err);
                                    error_message.set(Some(t!("error_while_saving_games_list.error", error = err.to_string()).to_string())); // Added .to_string()
                                     status.set(t!("error").into());
                                    return; // Stop if saving fails
                                }
                            };

                            status.set(t!("complete").into());
                            games.set(all_games); // Update the games signal in App
                        },
                        {t!("refresh_games_list").to_string()} // Added .to_string()
                    }
                }
                // Display errors from this tab
                if let Some(err) = error_message() {
                    div {
                        class: "error-message",
                        "{err}"
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
            h2 { {t!("about.app_name", app_name = APP_NAME).to_string()} } // Added .to_string()
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
                        {t!("app_name.version.by", app_name = APP_NAME, version = env!("CARGO_PKG_VERSION")).to_string()} // Added .to_string()
                        a {
                            href: "mailto:doriansoru@gmail.com",
                            "Dorian Soru"
                        },
                        ", 2025."
                    }
                    p {
                        {t!("released_under_the").to_string()} // Added .to_string()
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

    // Signals for displaying errors at the App level
    let mut config_load_error = use_signal(|| None::<String>);
    let mut games_load_error = use_signal(|| None::<String>);
    let mut fav_load_error = use_signal(|| None::<String>);
    // Error signals specific to tabs
    let mut games_tab_error = use_signal(|| None::<String>);
    let mut settings_tab_error = use_signal(|| None::<String>);


    // Load config - handle potential error using the new error type
    let config = match crate::rustmameuiconfig::Config::new() {
        Ok(c) => {
            config_load_error.set(None); // Clear any previous config error
            c
        },
        Err(e) => {
            let err = UiError::ConfigError(e); // Wrap the ConfigError
            let error_msg = t!("error_loading_configuration.error", error = err.to_string()).to_string(); // Added .to_string()
            config_load_error.set(Some(error_msg.clone())); // Set the error message
            eprintln!("Error loading config in App: {}", err); // Log the error for debugging
            Config::default() // Use the Default implementation for a minimal config
        }
    };

    let config_clone = config.clone();
    let mame_executable = use_signal(|| config_clone.mame_executable);
    let snap_file = use_signal(|| config_clone.snap_file);
    let roms_path = use_signal(|| config_clone.roms_path);

    // Handle error loading games - use the new error type
    let games = use_signal(|| {
        if config_load_error().is_none() { // Load games only if the configuration is valid
            match games::load(&config) { // games::load now returns Result<Vec<Game>, GamesError>
                Ok(g) => {
                    games_load_error.set(None); // Confirm no error
                    g
                },
                Err(e) => {
                    let err = UiError::GamesError(e); // Wrap the GamesError
                    let error_msg = t!("error_loading_games.error", error = err.to_string()).to_string(); // Added .to_string()
                    eprintln!("Error loading games in App: {}", err);
                    games_load_error.set(Some(error_msg)); // Set the error message
                    Vec::new() // Returns an empty Vec on error
                }
            }
        } else {
            Vec::new() // No games with invalid configuration
        }
    });

    // Handle error loading favourites - use the new error type
    let favourites = use_signal(|| {
        if config_load_error().is_none() { // Load favourites only if the configuration is valid
           match games::load_favourites(&config) { // games::load_favourites now returns Result<Vec<Game>, GamesError>
               Ok(f) => {
                   fav_load_error.set(None); // Confirm no error
                   f
               },
               Err(e) => {
                   let err = UiError::GamesError(e); // Wrap the GamesError
                   let error_msg = t!("error_loading_favourites.error", error = err.to_string()).to_string(); // Added .to_string()
                   eprintln!("Error loading favourites in App: {}", err);
                   fav_load_error.set(Some(error_msg)); // Set the error message
                   Vec::new() // Returns an empty Vec on error
               }
           }
       } else {
           Vec::new() // No favourites with invalid configuration
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
            // Display configuration loading error
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
                        onclick: move |_| {
                            selected_tab.set(Tab::Games);
                            games_tab_error.set(None); // Clear Games tab error on tab change
                        },
                        {t!("games").to_string()} // Added .to_string()
                    }
                    button {
                        class: if selected_tab() == Tab::Favourites { "tab-button active" } else { "tab-button" },
                        onclick: move |_| {
                             selected_tab.set(Tab::Favourites);
                             games_tab_error.set(None); // Clear Games tab error (shared error signal)
                        },
                        {t!("favourites").to_string()} // Added .to_string()
                    }
                    button {
                        class: if selected_tab() == Tab::Settings { "tab-button active" } else { "tab-button" },
                        onclick: move |_| {
                            selected_tab.set(Tab::Settings);
                            settings_tab_error.set(None); // Clear Settings tab error on tab change
                        },
                        {t!("settings").to_string()} // Added .to_string()
                    }
                    button {
                        class: if selected_tab() == Tab::About { "tab-button active" } else { "tab-button" },
                        onclick: move |_| {
                             selected_tab.set(Tab::About);
                             games_tab_error.set(None); // Clear Games tab error (shared error signal)
                             settings_tab_error.set(None); // Clear Settings tab error
                        },
                        {t!("about").to_string()} // Added .to_string()
                    }
                }

                // Show errors loaded at App startup
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
                    // Render Games tab
                    if selected_tab() == Tab::Games {
                         GameListTab {
                            items: games, // Pass the games list
                            all_games: games, // Pass games for add_favourite
                            favourites: favourites, // Pass favourites for context menu logic
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            show_context_menu: show_context_menu,
                            header_text: t!("all_games").to_string(),
                            filter_label_text: t!("type_the_name_of_a_game_to_search_for_it").to_string(),
                            is_favourite_list: false, // Flag for Games tab
                            error_message: games_tab_error, // Pass the error signal to the tab
                        }
                    }
                    // Render Favourites tab
                    if selected_tab() == Tab::Favourites {
                         GameListTab {
                            items: favourites, // Pass the favourites list
                            all_games: games, // Pass games for add_favourite (needed even from favourites tab if user tries to re-add?) - keep consistent
                            favourites: favourites, // Pass favourites for context menu logic (remove_favourite)
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            show_context_menu: show_context_menu,
                            header_text: t!("favourite_games").to_string(),
                            filter_label_text: t!("type_the_name_of_a_game_to_search_for_it").to_string(),
                            is_favourite_list: true, // Flag for Favourites tab
                             error_message: games_tab_error, // Pass the error signal to the tab (shared with Games tab)
                        }
                    }
                    // Render Settings tab
                    if selected_tab() == Tab::Settings {
                        SettingsTab {
                            mame_executable: mame_executable,
                            roms_path: roms_path,
                            snap_file: snap_file,
                            status: status,
                            games: games, // Pass games for the refresh logic in SettingsTab
                            error_message: settings_tab_error, // Pass the error signal to the tab
                        }
                    }
                    // Render About tab
                    if selected_tab() == Tab::About {
                        AboutTab {}
                    }
                }
            }
        }
    }
}