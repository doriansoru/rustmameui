//! This module contains the Dioxus UI code for the application.
//! It defines the main application component (`App`), handles state management
//! using signals and memos, renders the tabbed interface for games, favourites,
//! settings, and about sections, and manages user interactions like selecting
//! games, launching games, adding/removing favourites, updating settings,
//! and refreshing the game list.

use dioxus::prelude::*;
use crate::rustmameuiconfig::Config;
use crate::game::Game;
use rfd::FileDialog; // Used for native file/folder dialogs
use rust_i18n::t; // Macro for internationalization (i18n)

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

/// Component for the Games tab content.
#[component]
fn GamesTab(
    games: Signal<Vec<Game>>,
    mame_executable: Signal<std::path::PathBuf>,
    roms_path: Signal<std::path::PathBuf>,
    snap_file: Signal<std::path::PathBuf>,
    favourites: Signal<Vec<Game>>,
    show_context_menu: Signal<bool>,
) -> Element {
    let mut menu_x = use_signal(|| 0_i32);
    let mut menu_y = use_signal(|| 0_i32);

    let mut game_filter = use_signal(String::new);

    let filtered_games = use_memo(move || {
        let filter = game_filter();
        let all_games = games();
        all_games
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

    let mut selected_game_popup_menu_label = use_signal(|| "".to_string());
    let selected_game_popup_menu_current_label = use_memo(move || selected_game_popup_menu_label());

    let mut selected_game = use_signal(|| (String::new(), String::new(), false));

    let selected_game_is_favourite = use_memo(move || {
        let (rom, _, _) = selected_game();
        let mut found = false;
        for favourite in favourites() {
            if favourite.rom() == rom {
                found = true;
            }
        }
        found
    });    
    
    let mut snap_data = use_signal(|| "".to_string());

    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {t!("all_games")} }
            p {
                margin_bottom: "1.5em",
                {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
            }
            // Content Layout
            div {
                class: "row",
                // Left Column: Game List and Filter
                div {
                    class: "column",
                    // Game List
                    select {
                        style: "max-width: 500px;",
                        resize: "both",
                        autocomplete: "on",
                        size: 20,
                        oncontextmenu: handle_context_menu,
                        onchange: move |event| {
                            let rom = event.value().clone();
                            let description = games.read().iter()
                                .find(|game| game.rom().clone() == rom)
                                .map(|game| game.description().clone())
                                .unwrap_or_default();
                            let snap = games.read().iter()
                                .find(|game| game.rom().clone() == rom)
                                .map(|game| game.snap())
                                .unwrap_or_default();
                            snap_data.set(Game::new(&rom, &description, snap).get_snap(&snap_file().display().to_string()).unwrap());
                            selected_game.set((rom, description, snap));
                        },
                        ondoubleclick: move |_| {
                            let (rom, description, snap) = selected_game();
                            let game = Game::new(&rom, &description, snap);
                            let _ = game.launch(&mame_executable(), &roms_path());
                        },
                        onkeypress: move |ev: Event<KeyboardData>| {
                            if ev.key() == Key::Enter {
                                let (rom, description, snap) = selected_game();
                                let game = Game::new(&rom, &description, snap);
                                let _ = game.launch(&mame_executable(), &roms_path());
                            }
                        },
                        for game in filtered_games() {
                            option {
                                value: game.rom().clone(),
                                {game.description().clone()}
                            }
                        }
                    }
                    // Context Menu
                    {
                        if show_context_menu() {
                            if selected_game_is_favourite() {
                                selected_game_popup_menu_label.set(t!("remove_from_favourites").to_string());
                            } else {
                                selected_game_popup_menu_label.set(t!("add_to_favourites").to_string());
                            }
                            rsx! {
                                div {
                                    class: "popup",
                                    style: "left: {menu_x()}px; top: {menu_y()}px;",
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| {
                                            let (rom, description, snap) = selected_game();
                                            let game = Game::new(&rom, &description, snap);
                                            let _ = game.launch(&mame_executable(), &roms_path());
                                            show_context_menu.set(false);
                                        },
                                        {t!("launch_game")}
                                    }
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| {
                                            let config = Config::new().unwrap();
                                            let (rom, description, snap) = selected_game();
                                            let favourite = Game::new(&rom, &description, snap);
                                            let new_favourites = if selected_game_is_favourite() {
                                                crate::games::remove_favourite(&config, &mut favourites(), &favourite)
                                            } else {
                                                crate::games::add_favourite(&config, &mut favourites(), &favourite)
                                            };
                                            favourites.set(new_favourites);
                                            show_context_menu.set(false);
                                        },
                                        {selected_game_popup_menu_current_label()}
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
                                    game_filter.set(evt.value());
                                }
                            }
                            label {
                                {t!("type_the_name_of_a_game_to_search_for_it")}
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

/// Component for the Favourites tab content.
#[component]
fn FavouritesTab(
    favourites: Signal<Vec<Game>>,
    mame_executable: Signal<std::path::PathBuf>,
    roms_path: Signal<std::path::PathBuf>,
    snap_file: Signal<std::path::PathBuf>,
    show_context_menu: Signal<bool>,
) -> Element {
    let mut menu_x = use_signal(|| 0_i32);
    let mut menu_y = use_signal(|| 0_i32);

    let mut favourite_filter = use_signal(String::new);
    let filtered_favourites = use_memo(move || {
        let filter = favourite_filter();
        let all_favourites = favourites();
        all_favourites
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

    let mut selected_favourite = use_signal(|| (String::new(), String::new(), false));

    let mut favourite_snap_data = use_signal(|| "".to_string());

    rsx! {
        div {
            class: "tab-panel active",
            // Header
            h2 { {t!("favourite_games")} }
            p {
                margin_bottom: "1.5em",
                {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
            }
            // Content Layout
            div {
                class: "row",
                // Left Column: Favourites List and Filter
                div {
                    class: "column",
                    // Favourites List
                    select {
                        style: "max-width: 500px;",
                        resize: "both",
                        autocomplete: "on",
                        size: 20,
                        oncontextmenu: handle_context_menu,
                        onchange: move |event| {
                            let rom = event.value().clone();
                            let description = favourites.read().iter()
                                .find(|game| game.rom().clone() == rom)
                                .map(|game| game.description().clone())
                                .unwrap_or_default();
                            let snap = favourites.read().iter()
                                .find(|game| game.rom().clone() == rom)
                                .map(|game| game.snap())
                                .unwrap_or_default();
                            favourite_snap_data.set(Game::new(&rom, &description, snap).get_snap(&snap_file().display().to_string()).unwrap());
                            selected_favourite.set((rom, description, snap));
                        },
                        ondoubleclick: move |_| {
                            let (rom, description, snap) = selected_favourite();
                            let game = Game::new(&rom, &description, snap);
                            let _ = game.launch(&mame_executable(), &roms_path());
                        },
                        onkeypress: move |ev: Event<KeyboardData>| {
                            if ev.key() == Key::Enter {
                                let (rom, description, snap) = selected_favourite();
                                let game = Game::new(&rom, &description, snap);
                                let _ = game.launch(&mame_executable(), &roms_path());
                            }
                        },
                        for game in filtered_favourites() {
                            option {
                                value: game.rom().clone(),
                                {game.description().clone()}
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
                                            let (rom, description, snap) = selected_favourite();
                                            let favourite = Game::new(&rom, &description, snap);
                                            let _ = favourite.launch(&mame_executable(), &roms_path());
                                            show_context_menu.set(false);
                                        },
                                        {t!("launch_game")}
                                    }
                                    div {
                                        style: "cursor: pointer; padding: 10px;",
                                        onclick: move |_| {
                                            let config = Config::new().unwrap();
                                            let (rom, description, snap) = selected_favourite();
                                            let favourite = Game::new(&rom, &description, snap);
                                            let new_favourites = crate::games::remove_favourite(&config, &mut favourites(), &favourite);
                                            favourites.set(new_favourites);
                                            show_context_menu.set(false);
                                        },
                                        {t!("remove_from_favourites")}
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
                                    favourite_filter.set(evt.value());
                                }
                            }
                            label {
                                {t!("type_the_name_of_a_game_to_search_for_it")}
                            }
                        }
                    }
                }
                // Right Column: Snapshot
                div {
                    class: "snap",
                    img {
                        src: "{favourite_snap_data()}",
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
                            let mut config = Config::new().unwrap();
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
                            let config_third_clone = Config::new().unwrap();
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
                                //let roms_batch_results = crate::games::verify_batch_roms(&config_fifth_clone, current_batch);
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
                            games.set(all_games);
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

    let config = match Config::new() {
        Ok(c) => c,
        Err(e) => panic!("{}", t!("error.error", error = e.to_string()).to_string()),
    };

    let config_clone = config.clone();
    let mame_executable = use_signal(|| config_clone.mame_executable);
    let snap_file = use_signal(|| config_clone.snap_file);
    let roms_path = use_signal(|| config_clone.roms_path);

    let games = use_signal(|| crate::games::load(&config).unwrap());
    let favourites = use_signal(|| crate::games::load_favourites(&config).unwrap());

    let status = use_signal(|| String::from(""));

    let mut show_context_menu = use_signal(|| false);

    let hide_menu = move |_| {
        show_context_menu.set(false);
    };

    rsx! {
        document::Link { rel: "stylesheet", href: TABS_CSS }
        div {
            class: "container",
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
            // Tab Content
            div {
                onclick: hide_menu,
                class: "tab-content",
                if selected_tab() == Tab::Games {
                    GamesTab {
                        games,
                        mame_executable,
                        roms_path,
                        snap_file,
                        favourites,
                        show_context_menu,
                    }
                }
                if selected_tab() == Tab::Favourites {
                    FavouritesTab {
                        favourites,
                        mame_executable,
                        roms_path,
                        snap_file,
                        show_context_menu,
                    }
                }
                if selected_tab() == Tab::Settings {
                    SettingsTab {
                        mame_executable,
                        roms_path,
                        snap_file,
                        status,
                        games,
                    }
                }
                if selected_tab() == Tab::About {
                    AboutTab {}
                }
            }
        }
    }
}