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

/// The main entry point for drawing and launching the application UI.
///
/// It uses conditional compilation (`#[cfg]`) to select between a desktop
/// window configuration and a default launch based on the "desktop" feature flag.
pub fn draw() {
    // Configuration specific to the desktop feature
    #[cfg(feature = "desktop")]
    fn launch_app() {
        // Configure the window size and title
        let window = dioxus::desktop::tao::window::WindowBuilder::new()
        .with_resizable(true)
        .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(1024.0, 860.0))
        .with_title::<&str>(APP_NAME);

        // Launch the Dioxus application with the specified window config
        dioxus::LaunchBuilder::new().with_cfg(dioxus::desktop::Config::new().with_window(window).with_menu(None)).launch(App);
    }

    // Default launch configuration when not using the desktop feature
    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        // Standard Dioxus launch
        dioxus::launch(App);
    }

    // Call the selected launch function
    launch_app();
}

/// Checks if a native file dialog utility (zenity or kdialog) is installed.
/// Required by `rfd::FileDialog` on some Linux systems.
///
/// # Returns
///
/// `Ok(())` if a utility is found, otherwise `Err(String)` with an error message.
fn check_dialog_utility() -> Result<(), String> {
    // Check if zenity or kdialog are installed
    let mut found = false;
    if which::which("zenity").is_ok() {
        found = true;
    }
    if which::which("kdialog").is_ok() {
        found = true;
    }

    // If neither utility is found, return an error
    if !found {
        return Err(t!("error_no_dialog_utility").to_string());
        //"Error: No dialog utility has been found. Install 'zenity' or 'kdialog' to open the file chooser dialog."
    }

    // Otherwise, return Ok
    Ok(())

}

/// The main application component tree rendered by Dioxus.
///
/// This component manages the application's global state using signals
/// for the selected tab, configuration settings, game lists, selected games,
/// filter strings, UI status messages, and context menu visibility/position.
/// It uses memos for derived state like filtered game lists and context menu labels.
/// The component structure defines the tabbed interface and the content
/// for each tab (Games, Favourites, Settings, About).
#[component]
fn App() -> Element {
    // Get the application version from Cargo.toml
    let app_version = env!("CARGO_PKG_VERSION");

    // State signal to track the currently selected tab. Defaults to the Games tab.
    let mut selected_tab = use_signal(|| Tab::Games);

    // Load the application configuration. Panics if loading fails.
    let config = match Config::new() {
        Ok(c) => {
            c
        },
        Err(e) => {
            panic!("{}",
                t!("error.error", error = e.to_string()).to_string()
            );
        }
    };

    // Create clones of the configuration paths to be used in signals
    let config_clone = config.clone();
    let mut mame_executable = use_signal(|| config_clone.mame_executable);
    let mut snap_file = use_signal(|| config_clone.snap_file);
    let mut roms_path = use_signal(|| config_clone.roms_path);

    // State signals to hold Base64 encoded snapshot data for display
    let mut snap_data = use_signal(|| "".to_string()); // For the Games tab
    let mut favourite_snap_data = use_signal(|| "".to_string()); // For the Favourites tab

    // Use signals for games and favourites lists, loaded from storage on startup.
    // Panics if loading fails.
    let mut games = use_signal(|| crate::games::load(&config).unwrap());
    let mut favourites = use_signal(|| crate::games::load_favourites(&config).unwrap());

    // Signal to track the rom, description and snap availability of the game selected in the Games tab list.
    let mut selected_game = use_signal(|| (String::new(), String::new(), false));
    // (rom, description, snap)

    // Signal to track the rom, description and snap availability of the game selected in the Favourites tab list.
    let mut selected_favourite = use_signal(|| (String::new(), String::new(), false));
    // (rom, description, snap)

    // State signals for filtering the game lists by description
    let mut game_filter = use_signal(String::new); // For the Games tab
    let mut favourite_filter = use_signal(String::new); // For the Favourites tab

    // Memoized state for the filtered list of all games.
    // Recalculated only when `game_filter` or `games` signals change.
    let filtered_games = use_memo(move || {
        let filter = game_filter(); // Get current filter value
        let all_games = games(); // Get current games list
        // Filter games whose description contains the filter string (case-insensitive)
        all_games.iter()
            .filter(|item| item.description().to_lowercase().contains(&filter.to_lowercase()))
            .cloned() // Clone to own the data in the new vector
            .collect::<Vec<Game>>()
    });

    // Memoized state for the filtered list of favourite games.
    // Recalculated only when `favourite_filter` or `favourites` signals change.
    let filtered_favourites = use_memo(move || {
        let filter = favourite_filter(); // Get current filter value
        let all_favourites = favourites(); // Get current favourites list
        // Filter favourites whose description contains the filter string (case-insensitive)
        all_favourites.iter()
            .filter(|item| item.description().to_lowercase().contains(&filter.to_lowercase()))
            .cloned() // Clone to own the data in the new vector
            .collect::<Vec<Game>>()
    });

    // State signal for displaying status messages to the user (e.g., saving settings, refresh progress).
    let mut status = use_signal(|| String::from(""));

    // State signal for the label displayed in the context menu for the *selected game* in the Games tab.
    let mut selected_game_popup_menu_label = use_signal(|| "".to_string());
    // Memoized state for the current label value of the context menu for the Games tab.
    // Updates when `selected_game_popup_menu_label` changes.
    let selected_game_popup_menu_current_label = use_memo(move || {
        selected_game_popup_menu_label()
    });

    // Memoized state to check if the currently selected game in the Games tab is already a favourite.
    // Recalculated when `selected_game` or `favourites` signals change.
    let selected_game_is_favourite = use_memo(move || {
        let (rom, _, _) = selected_game(); // Get ROM of the selected game
        let mut found = false;
        // Iterate through favourites to see if the ROM exists
        for favourite in favourites() {
            if favourite.rom() == rom {
                found = true;
            }
        }

        found
    });

    // State signal for the label displayed in the context menu for the *selected game* in the Favourites tab.
    let mut selected_favourite_popup_menu_label = use_signal(|| "".to_string());
    // Memoized state for the current label value of the context menu for the Favourites tab.
    // Updates when `selected_favourite_popup_menu_label` changes.
    let selected_favourite_popup_menu_current_label = use_memo(move || {
        selected_favourite_popup_menu_label()
    });

    // Memoized state to check if the currently selected game in the Favourites tab is already a favourite.
    // This is redundant for the Favourites tab as all games *should* be favourites,
    // but the logic mirrors the Games tab for potential future use or consistency.
    let selected_favourite_is_favourite = use_memo(move || {
        let (rom, _, _) = selected_favourite(); // Get ROM of the selected favourite
        let mut found = false;
        // Iterate through favourites to see if the ROM exists
        for favourite in favourites() {
            if favourite.rom() == rom {
                found = true;
            }
        }

        found
    });


    // State signal to control the visibility of the context menu.
    let mut show_context_menu = use_signal(|| false);
    // State signals for the context menu's position (x, y coordinates).
    let mut menu_x = use_signal(|| 0_i32);
    let mut menu_y = use_signal(|| 0_i32);

    // Event handler for the context menu (right-click).
    // Updates the menu position and makes it visible. Stops event propagation.
    let handle_context_menu = move |event: Event<MouseData>|
    {
        event.stop_propagation(); // Prevent the event from bubbling up
        event.prevent_default(); // Prevent the browser's default context menu

        // Use client coordinates to position the custom menu
        let (x, y) = (event.coordinates().client().x as i32, event.coordinates().client().y as i32);
        menu_x.set(x); // Set menu X position
        menu_y.set(y); // Set menu Y position
        show_context_menu.set(true); // Make the menu visible
    };

    // Event handler to hide the context menu.
    let hide_menu = move |_| {
        show_context_menu.set(false); // Hide the menu
    };

    // The main Dioxus Rendering Section (RSX)
    rsx! {
        // Riferimento allo stylesheet esterno come asset
        document::Link { rel: "stylesheet", href: TABS_CSS } // Link to the external CSS file

        // Main container div
        div {
            class: "container",
            // Tab header section
            div {
                class: "tabs-header",
                // Button for the Games tab
                button {

                    // Apply 'active' class if this is the selected tab
                    class: if selected_tab() == Tab::Games { "tab-button active" } else { "tab-button" },
                    // On click, set the selected tab to Games
                    onclick: move |_| selected_tab.set(Tab::Games),
                    {t!("games")} // Localized text for "Games"
                    //"Games"
                }
                // Button for the Favourites tab
                button {
                    // Apply 'active' class if this is the selected tab
                    class: if selected_tab() == Tab::Favourites { "tab-button
                    active" } else { "tab-button" },
                    // On click, set the selected tab to Favourites
                    onclick: move |_| {
                        selected_tab.set(Tab::Favourites);
                    },
                    {t!("favourites")} // Localized text for "Favourites"
                    //"Favourites"
                }
                // Button for the Settings tab
                button {
                    // Apply 'active' class if this is the selected tab
                    class: if selected_tab() == Tab::Settings { "tab-button
                    active" } else { "tab-button" },
                    // On click, set the selected tab to Settings
                    onclick: move |_| selected_tab.set(Tab::Settings),
                    {t!("settings")} // Localized text for "Settings"
                    //"Settings"
                }
                // Button for the About tab
                button {
                    // Apply 'active' class if this is the selected tab
                    class: if selected_tab() == Tab::About { "tab-button
                    active" } else { "tab-button" },
                    // On click, set the selected tab to About
                    onclick: move |_| selected_tab.set(Tab::About),
                    {t!("about")} // Localized text for "About"
                    //"About"
                }
            }
            // Tab content area. Clicks here hide the context menu.
            div {
                onclick: hide_menu,

                class: "tab-content",

                // --- Games Tab Content ---
                div {
                    // Display this panel only if the Games tab is selected
                    class: if selected_tab() == Tab::Games { "tab-panel active" } else { "tab-panel" },

                    h2 {
                        {t!("all_games")} // Localized text for "All Games"
                        //"All Games"
                    }
                    p {
                        // Add some bottom margin
                        margin_bottom: "1.5em",
                        // Instruction text
                        {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
                        //"Double click on a game to launch. Right click for more options"
                    }

                    div {
                        class: "row", // Layout using a row
                        div {
                            class: "column", // Left column for the game list and filter

                            // Select element to display the list of games
                            select {
                                style: "max-width: 500px;", // Limit width
                                resize: "both", // Allow resizing (might need specific CSS)

                                autocomplete: "on", // Enable browser autocomplete (if applicable)
                                size: 20, // Show 20 rows by default
                                oncontextmenu: handle_context_menu, // Attach context menu handler

                                // Event handler for when a game is selected in the list
                                onchange: move |event| {
                                    let rom = event.value().clone(); // Get the selected ROM name

                                    // Find the game description based on the selected ROM
                                    let description = games.read().iter()
                                    .find(|game| game.rom().clone() == rom) // Find the game by ROM name
                                    .map(|game| game.description().clone()) // Get its description
                                    .unwrap_or_default(); // Use empty string if not found

                                    // Find the game's snapshot availability based on the selected ROM
                                    let snap = games.read().iter()
                                    .find(|game| game.rom().clone() == rom) // Find the game by ROM name
                                    .map(|game| game.snap()) // Get its snap availability
                                    .unwrap_or_default(); // Use false if not found

                                    // Get the snapshot data (Base64 data URL) for the selected game
                                    // Panics if snapshot retrieval fails (e.g., zip error)
                                    snap_data.set(Game::new(&rom, &description, snap).get_snap(&snap_file().display().to_string()).unwrap());

                                    // Update the signal holding the currently selected game details
                                    selected_game.set((rom, description, snap));

                                },
                                // Event handler for double-clicking a game in the list
                                ondoubleclick: move |_| {
                                    // Get the selected game details from the signal
                                    let (rom, description, snap) = selected_game();
                                    let game = Game::new(&rom, &description, snap); // Create a Game struct instance
                                    // Launch the game using the configured MAME executable and ROMs path
                                    let _ = game.launch(&mame_executable(), &roms_path());
                                },
                                // Event handler for key presses while the list is focused
                                onkeypress: move |ev: Event<KeyboardData>| {
                                    // If the Enter key is pressed
                                    if ev.key() == Key::Enter {
                                        // Get selected game details and launch the game, similar to double-click
                                        let (rom, description, snap) = selected_game();
                                        let game = Game::new(&rom, &description, snap);
                                        let _ = game.launch(&mame_executable(), &roms_path());
                                    }
                                },
                                // Iterate through the filtered games list and create an option for each game
                                for game in filtered_games() {

                                    option {
                                        value: game.rom().clone(), // Option value is the ROM name
                                        {game.description().clone()} // Option text is the game description
                                    }
                                }
                            },

                            // Render the custom context menu if `show_context_menu` is true
                            {
                                if show_context_menu() {
                                    // Determine the correct label for the context menu item
                                    // based on whether the selected game is a favourite.
                                    if selected_game_is_favourite() {
                                        //"Remove from favourites"
                                        selected_game_popup_menu_label.set(t!("remove_from_favourites").to_string());
                                    } else {
                                        //"Add to favourites"
                                        selected_game_popup_menu_label.set(t!("add_to_favourites").to_string());
                                    }

                                    // This block seems to redundantly set the favourite popup label
                                    // based on `selected_favourite_is_favourite()`. Given this is within
                                    // the *Games* tab context menu logic, this might be a copy-paste error
                                    // or intended for a different context menu instance. It doesn't
                                    // affect the label displayed for the Games tab context menu, which
                                    // uses `selected_game_popup_menu_current_label()`.
                                    if selected_favourite_is_favourite() {
                                        //"Remove from favourites"
                                        selected_favourite_popup_menu_label.set(t!("remove_from_favourites").to_string());
                                    } else {
                                        //"Add to favourites"
                                        selected_favourite_popup_menu_label.set(t!("add_to_favourites").to_string());
                                    }

                                    // RSX for the context menu div
                                    rsx! {
                                        div {
                                            class: "popup", // CSS class for styling

                                            // Position the popup using inline styles and signal values
                                            style: "left: {menu_x()}px; top: {menu_y()}px;",

                                            // The context menu item (clickable div)
                                            div {
                                                style: "cursor: pointer; padding: 10px;", // Styling for the item

                                                // Event handler for clicking the context menu item
                                                onclick: move |_| {
                                                    // Load config again (potentially redundant if config is stable)
                                                    let config = Config::new().unwrap();
                                                    // Get the currently selected favourite from the signal.
                                                    // Note: This uses `selected_game()` which might be a copy-paste error
                                                    // and perhaps intended to use `selected_favourite()`.
                                                    let (rom, description, snap) = selected_game();
                                                    let game = Game::new(&rom, &description, snap); // Create Game instance
                                                    let _ = game.launch(&config.mame_executable, &config.roms_path);
                                                    // Hide the context menu after action
                                                    show_context_menu.set(false);
                                                },
                                                // Display the calculated context menu label for favourites
                                                {t!("launch_game")}
                                            }      
                                            // The context menu item (clickable div)
                                            div {
                                                style: "cursor: pointer; padding: 10px;", // Styling for the item

                                                // Event handler for clicking the context menu item
                                                onclick: move |_| {
                                                    // Load config again (potentially redundant if config is stable)
                                                    let config = Config::new().unwrap();
                                                    // Get the currently selected game from the signal
                                                    let (rom, description, snap) = selected_game();
                                                    let favourite = Game::new(&rom, &description, snap); // Create Game instance

                                                    // Check if the selected game is currently a favourite
                                                    let new_favourites = if selected_game_is_favourite() {
                                                        // If yes, remove it from favourites
                                                        crate::games::remove_favourite(&config, &mut favourites(), &favourite)
                                                    } else {
                                                        // If no, add it to favourites
                                                        crate::games::add_favourite(&config, &mut favourites(), &favourite)
                                                    };
                                                    // Update the favourites signal with the new list
                                                    favourites.set(new_favourites);
                                                    // Hide the context menu after action
                                                    show_context_menu.set(false);
                                                },
                                                // Display the calculated context menu label
                                                {selected_game_popup_menu_current_label()}
                                            }
                                        }
                                    }
                                // If `show_context_menu` is false, render nothing for the menu
                                } else {
                                    rsx! {}
                                }
                            },

                            // Div for the game filter input field
                            div {

                                div {
                                    class: "column", // Another column layout (potentially nested or intended differently)
                                    input {

                                        type: "text", // Input type is text
                                        size: 50, // Input size

                                        // Event handler for input changes (typing)
                                        oninput: move |evt| {
                                            // Update the game filter signal with the input value
                                            game_filter.set(evt.value());
                                        }
                                    }
                                    label {

                                        {t!("type_the_name_of_a_game_to_search_for_it")} // Localized label text
                                        //"Type the name of a game to search for it"
                                    }
                                }
                            }
                        }

                        // Right column for displaying the snapshot image
                        div {
                            class: "snap", // CSS class for snapshot container
                            img {

                                src: "{snap_data()}", // Image source is bound to the `snap_data` signal
                                width: 500, // Image width
                            }
                        }
                    }
                }

                // --- Favourites Tab Content ---
                div {
                    // Display this panel only if the Favourites tab is selected
                    class: if selected_tab() == Tab::Favourites { "tab-panel active" } else { "tab-panel" },

                    h2 {
                        {t!("favourite_games")} // Localized text for "Favourite Games"
                        //"Favourite Games"
                    }

                    p {
                        margin_bottom: "1.5em", // Add some bottom margin
                        // Instruction text (same as Games tab)
                        {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
                        //"Double click on a game to launch.
                        //Right click for more options"
                    }
                    div {
                        class: "row", // Layout using a row
                        div {

                            class: "column", // Left column for the favourites list and filter
                            select {
                                style: "max-width: 500px;", // Limit width

                                resize: "both", // Allow resizing (might need specific CSS)
                                autocomplete: "on", // Enable browser autocomplete (if applicable)
                                size: 20, // Show 20 rows by default

                                oncontextmenu: handle_context_menu, // Attach context menu handler (same as Games tab)

                                // Event handler for when a favourite is selected in the list
                                onchange: move |event| {
                                    let rom = event.value().clone(); // Get the selected ROM name

                                    // Find the favourite description based on the selected ROM
                                    let description = favourites.read().iter()
                                    .find(|game| game.rom().clone() == rom) // Find the game by ROM name in favourites
                                    .map(|game| game.description().clone()) // Get its description
                                    .unwrap_or_default(); // Use empty string if not found


                                    // Find the favourite's snapshot availability based on the selected ROM
                                    let snap = favourites.read().iter()
                                    .find(|game|
                                    game.rom().clone() == rom) // Find the game by ROM name in favourites
                                    .map(|game| game.snap()) // Get its snap availability
                                    .unwrap_or_default(); // Use false if not found

                                    // Get the snapshot data (Base64 data URL) for the selected favourite
                                    // Panics if snapshot retrieval fails (e.g., zip error)
                                    favourite_snap_data.set(Game::new(&rom, &description, snap).get_snap(&snap_file().display().to_string()).unwrap());
                                    // Update the signal holding the currently selected favourite details
                                    selected_favourite.set((rom, description, snap));
                                },
                                // Event handler for double-clicking a favourite in the list
                                ondoubleclick: move |_| {
                                    // Get the selected favourite details from the signal
                                    let (rom, description, snap) = selected_favourite();
                                    let game = Game::new(&rom, &description, snap); // Create a Game struct instance
                                    // Launch the game using the configured MAME executable and ROMs path
                                    let _ = game.launch(&mame_executable(), &roms_path());
                                },
                                // Event handler for key presses while the list is focused
                                onkeypress: move |ev: Event<KeyboardData>| {
                                    // If the Enter key is pressed
                                    if ev.key() == Key::Enter {
                                        // Get selected favourite details and launch the game, similar to double-click
                                        let (rom, description, snap) = selected_favourite();
                                        let game = Game::new(&rom, &description, snap);
                                        let _ = game.launch(&mame_executable(), &roms_path());
                                    }
                                },
                                // Iterate through the filtered favourites list and create an option for each favourite
                                for game in filtered_favourites() {

                                    option {
                                        value: game.rom().clone(), // Option value is the ROM name
                                        {game.description().clone()} // Option text is the favourite description
                                    }
                                }
                            },

                            // Render the custom context menu if `show_context_menu` is true
                            {
                                if show_context_menu() {
                                    // RSX for the context menu div
                                    rsx! {
                                        div {
                                            class: "popup", // CSS class for styling

                                            // Position the popup using inline styles and signal values
                                            style: "left: {menu_x()}px; top: {menu_y()}px;",

                                            // The context menu item (clickable div)
                                            div {
                                                style: "cursor: pointer; padding: 10px;", // Styling for the item

                                                // Event handler for clicking the context menu item
                                                onclick: move |_| {
                                                    // Load config again (potentially redundant if config is stable)
                                                    let config = Config::new().unwrap();
                                                    // Get the currently selected favourite from the signal.
                                                    // Note: This uses `selected_game()` which might be a copy-paste error
                                                    // and perhaps intended to use `selected_favourite()`.
                                                    let (rom, description, snap) = selected_favourite();
                                                    let favourite = Game::new(&rom, &description, snap); // Create Game instance
                                                    let _ = favourite.launch(&config.mame_executable, &config.roms_path);
                                                    // Hide the context menu after action
                                                    show_context_menu.set(false);
                                                },
                                                // Display the calculated context menu label for favourites
                                                {t!("launch_game")}
                                            }                                            
                                            div {
                                                style: "cursor: pointer; padding: 10px;", // Styling for the item

                                                // Event handler for clicking the context menu item
                                                onclick: move |_| {
                                                    // Load config again (potentially redundant if config is stable)
                                                    let config = Config::new().unwrap();
                                                    
                                                    // Get the currently selected favourite from the signal.
                                                    let (rom, description, snap) = selected_favourite();
                                                    let favourite = Game::new(&rom, &description, snap); // Create Game instance

                                                    // Check if the selected item is currently a favourite.
                                                    let new_favourites = if selected_favourite_is_favourite() {
                                                        // If yes, remove it from favourites
                                                        crate::games::remove_favourite(&config, &mut favourites(), &favourite)
                                                    } else {
                                                        // If no, add it to favourites
                                                        crate::games::add_favourite(&config, &mut favourites(), &favourite)
                                                    };
                                                    // Update the favourites signal with the new list
                                                    favourites.set(new_favourites);
                                                    // Hide the context menu after action
                                                    show_context_menu.set(false);
                                                },
                                                // Display the calculated context menu label for favourites
                                                {selected_favourite_popup_menu_current_label()}
                                            }
                                        }
                                    }
                                // If `show_context_menu` is false, render nothing for the menu
                                } else {
                                    rsx! {}
                                }
                            },

                            // Div for the favourite filter input field
                            div {

                                div {
                                    class: "column", // Another column layout (potentially nested or intended differently)
                                    input {

                                        type: "text", // Input type is text
                                        size: 50, // Input size

                                        // Event handler for input changes (typing)
                                        oninput: move |evt| {
                                            // Update the favourite filter signal with the input value
                                            favourite_filter.set(evt.value());
                                        }
                                    }
                                    label {

                                        {t!("type_the_name_of_a_game_to_search_for_it")} // Localized label text (same as Games tab)
                                        //"Type the name of a game to search for it"
                                    }
                                }
                            }
                        }

                        // Right column for displaying the snapshot image for favourites
                        div {
                            class: "snap", // CSS class for snapshot container
                            img {

                                src: "{favourite_snap_data()}", // Image source is bound to the `favourite_snap_data` signal
                                width: 500, // Image width
                            }
                        }
                    }
                }

                // --- Settings Tab Content ---
                div {
                    // Display this panel only if the Settings tab is selected
                    class: if selected_tab() == Tab::Settings { "tab-panel active" } else { "tab-panel" },

                    h2 {
                        {t!("app_name.settings", app_name = APP_NAME )} // Localized settings title
                        //"{APP_NAME} Settings"
                    }

                    // Setting row for MAME executable path
                    div {
                        class: "setting", // CSS class for a setting row
                        label {
                            {t!("mame_executable_path")} // Localized label
                            //"MAME executable path"
                        }
                        input {
                            size: 50, // Input size

                            value: "{mame_executable().display().to_string()}", // Input value bound to `mame_executable` signal
                            // Event handler for input changes
                            onchange: move |evt| {
                                // Update the `mame_executable` signal with the new path from the input value
                                mame_executable.set(std::path::PathBuf::from(evt.value()));
                            }
                        }
                        button {
                            // Event handler for the "Browse" button
                            onclick: move |_| {
                                // Check if a dialog utility is available
                                match check_dialog_utility() {
                                    Ok(_) => {
                                        // If available, open a file dialog to pick an executable file
                                        if let Some(file) = FileDialog::new()
                                            .add_filter(
                                                t!("all_files"), // Localized filter label
                                                //"All files",
                                                &["*"]) // Filter for all file types
                                            .pick_file() {
                                                // If a file is selected, update the `mame_executable` signal
                                                mame_executable.set(file);
                                        }
                                    },
                                    Err(e) => {
                                        // If no utility is found, panic with the error message
                                        panic!("{}", e);
                                    }
                                }
                            },
                            {t!("browse")} // Localized button text
                            //"Browse"
                        }
                    }

                    // Setting row for ROMs directory path
                    div {

                        class: "setting", // CSS class for a setting row
                        label {
                            {t!("roms_directory")} // Localized label
                            //"ROMS directory"
                        }
                        input {
                            size: 50, // Input size
                            value: "{roms_path().display().to_string()}", // Input value bound to `roms_path` signal
                            // Event handler for input changes
                            onchange: move |evt| {
                                // Update the `roms_path` signal with the new path from the input value
                                roms_path.set(std::path::PathBuf::from(evt.value()));
                            }
                        }
                        button {
                            // Event handler for the "Browse" button
                            onclick: move |_| {
                                // Check if a dialog utility is available
                                match check_dialog_utility() {
                                    Ok(_) => {
                                        // If available, open a folder dialog to pick the ROMs directory
                                        if let Some(directory) = FileDialog::new()
                                            .add_filter(
                                                t!("all_files"), // Localized filter label
                                                //"All files",
                                                &["*"]) // Filter (seems redundant for folder picker but present in original code)
                                            .pick_folder() {
                                                // If a folder is selected, update the `roms_path` signal
                                                roms_path.set(directory);
                                        }
                                    },
                                    Err(e) => {
                                        // If no utility is found, panic with the error message
                                        panic!("{}", e);
                                    }
                                }
                            },
                            {t!("browse")} // Localized button text
                            //"Browse"
                        }
                    }

                    // Setting row for Snapshots zip file path
                    div {

                        class: "setting", // CSS class for a setting row
                        label {
                            {t!("snaps_zip_not_7z_file_path")} // Localized label
                            //"SNAPS zip (not 7z) file path"
                        }
                        input {
                            size: 50, // Input size
                            value: "{snap_file().display().to_string()}", // Input value bound to `snap_file` signal

                            // Event handler for input changes
                            onchange: move |evt| {
                                // Update the `snap_file` signal with the new path from the input value
                                snap_file.set(std::path::PathBuf::from(evt.value()));
                            }
                        }
                        button {
                            // Event handler for the "Browse" button
                            onclick: move |_| {
                                // Check if a dialog utility is available
                                match check_dialog_utility() {
                                    Ok(_) => {
                                        // If available, open a file dialog to pick the snapshots zip file
                                        if let Some(file) = FileDialog::new()
                                            .add_filter(
                                                t!("zip_file"), // Localized filter label
                                                //"Zip file",
                                                &["*.zip"]) // Filter specifically for .zip files
                                            .pick_file() {
                                                // If a file is selected, update the `snap_file` signal
                                                snap_file.set(file);
                                        }
                                    },
                                    Err(e) => {
                                        // If no utility is found, panic with the error message
                                        panic!("{}", e);
                                    }
                                }
                            },
                            "Browse" // Button text (not localized in original code)
                        }
                    }

                    // Div containing action buttons for settings
                    div {
                        div {

                            class: "row", // Layout using a row

                            // Button to save the current settings
                            button {
                                // Event handler for the "Save settings" button
                                onclick: move |_| {
                                    // Load config again (might be redundant if config is stable)
                                    let mut config = Config::new().unwrap();
                                    // Update the config struct with the current signal values
                                    config.mame_executable = mame_executable();
                                    config.roms_path = roms_path();
                                    config.snap_file = snap_file();
                                    // Attempt to save the updated configuration
                                    match config.save() {
                                        Ok(_) => {
                                            // On success, update the status message
                                            status.set(
                                                t!("the_settings_have_been_saved_correctly").into()
                                                //"The settings have been saved correctly."
                                            );
                                        },
                                        Err(e) => {
                                            // On error, update the status message with the error details
                                            status.set(
                                                t!("error_while_saving_settings.error", error = e.to_string()).to_string()
                                                //format!("Error while saving settings: {}", e)
                                            );
                                        }
                                    }
                                },
                                "Save settings" // Button text (not localized in original code)
                            }

                            // Button to refresh the list of games by rescanning MAME output and ROM/snap files.
                            button {
                                class: "action-button", // CSS class
                                // Event handler for the "Refresh games list" button. This is an async closure.
                                onclick: move |_| async move {
                                    // Load config again (might be redundant)
                                    let config_third_clone = Config::new().unwrap();
                                    // Update status to indicate process started
                                    status.set(
                                        t!("reading_the_roms_list_from_mame").into()
                                        //"Reading the roms list from mame..."
                                    );
                                    // Small delay to allow UI to update status before potentially long operations
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                    // Get XML output from MAME. Panics on error.
                                    let xml_string = crate::games::get_xml_roms(&config_third_clone)
                                        .unwrap_or_else(|_| { panic!("{}", t!("cannot_get_xml_data_from_mame").to_string()) });
                                    // Update status
                                    status.set("Parsing the roms list to get valid roms...".into());
                                    // Small delay
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                    // Parse XML and get ROM names and descriptions. Panics on error.
                                    let (roms, descriptions) = crate::games::get_all_roms(&config_third_clone, &xml_string)
                                        .unwrap_or_else(|_| { panic!("{}", t!("cannot_read_all_roms_and_their_descriptions").to_string()) });
                                    let mut all_games: Vec<Game> = Vec::new();

                                    // Update status
                                    status.set(
                                        t!("verifying_working_roms").into()
                                        //"Verifying working roms..."
                                    );
                                    // Small delay
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                    // Prepare vectors to store verification results
                                    let total_roms = roms.len();
                                    let mut working_roms = Vec::with_capacity(total_roms);
                                    let mut working_snaps = Vec::with_capacity(total_roms);
                                    // Calculate the number of batches for verification
                                    let num_batches = total_roms.div_ceil(BATCH_SIZE);
                                    // Process ROMs in batches
                                    for batch_index in 0..num_batches {
                                        let start_index = batch_index * BATCH_SIZE;
                                        let end_index = usize::min((batch_index + 1) * BATCH_SIZE, total_roms);
                                        // Update status with current batch progress
                                        status.set(
                                            t!("verifying_roms.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into()
                                            //format!("Verifying ROMS {}...{} of {}...", (start_index + 1), end_index, total_roms)
                                        );
                                        // Small delay
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                        // Verify the current batch of ROM files
                                        let current_batch = &roms[start_index..end_index];
                                        let roms_batch_results = crate::games::verify_batch_roms(&config_third_clone, current_batch);
                                        working_roms.extend(roms_batch_results); // Extend results vector
                                        // Update status for snap verification
                                        status.set(
                                            t!("verifying_snaps.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into()
                                            //format!("Verifying SNAPS {}...{} of {}...", (start_index + 1), end_index, total_roms)
                                        );
                                        // Small delay
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                        // Verify the current batch of snapshot files
                                        let snaps_batch_results = crate::games::verify_batch_snaps(&config_third_clone, current_batch);
                                        working_snaps.extend(snaps_batch_results); // Extend results vector
                                    }

                                    // Build the final list of games based on successful ROM verification
                                    if !roms.is_empty() {
                                        for (i, rom) in roms.iter().enumerate() {
                                            // Check if the ROM was verified as working
                                            if working_roms.get(i).copied().unwrap_or(false) {
                                                // Create a Game instance with ROM, description, and snap availability
                                                let game = Game::new(rom, descriptions.get(i).unwrap(), working_snaps.get(i).copied().unwrap_or(false));
                                                all_games.push(game); // Add to the list of all games
                                            }
                                        }
                                    }

                                    // Update status
                                    status.set(
                                        t!("saving_all_games").into()
                                        //"Saving all games..."
                                    );
                                    // Small delay
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                    // Save the compiled list of all valid games to storage. Panics on error.
                                    match crate::games::save(&config_third_clone, &all_games) {
                                        Ok(_) => {}, // Do nothing on success
                                        Err(e) => {
                                            // On error, panic with the error message
                                            panic!("{}",
                                                t!("error_while_saving_games_list.error", error = e.to_string()).to_string()
                                                //"Error while saving games list: {}", e
                                            );
                                        }
                                    };
                                    // Update final status
                                    status.set(
                                        t!("complete").into()
                                        //"Complete!".into()
                                    );
                                    // Update the `games` signal to refresh the UI list
                                    games.set(all_games);
                                },
                                {t!("refresh_games_list")} // Localized button text
                                //"Refresh games list"
                            }
                        }
                        // Div to display the current status message
                        div {
                            {status()} // Display the value of the `status` signal
                        }
                    }
                }

                // --- About Tab Content ---
                div {
                    // Display this panel only if the About tab is selected
                    class:
                    if selected_tab() == Tab::About { "tab-panel active" } else { "tab-panel" },
                    h2 {
                        {t!("about.app_name", app_name = APP_NAME)} // Localized About title
                        //"About {APP_NAME}"
                    }

                    // Content row for About section
                    div {
                        class: "row", // Layout using a row
                        // Left column for the About image
                        div {

                            img {
                                src: "{ABOUT}", // Image source is bound to the ABOUT asset
                                width: 300, // Image width

                                style: "margin-right: 1em", // Add right margin
                            }
                        }
                        // Right column for About text
                        div {

                            p {
                                // Localized text for app name, version, and author
                                {t!("app_name.version.by", app_name = APP_NAME, version = app_version)}
                                //"{APP_NAME} {app_version}, by "

                                // Link to the author's email
                                a {
                                    href: "mailto:doriansoru@gmail.com",
                                    "Dorian Soru" // Author name
                                },
                                ", 2025." // Year
                            }
                            p {
                                // Localized text for license
                                {t!("released_under_the")}
                                //"Released under the "

                                // Link to the GPL 3.0 license
                                a {
                                    href: "https://www.gnu.org/licenses/gpl-3.0.html",

                                    "GNU General Public License 3.0" // License name
                                },
                                "."
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Enum to represent the different tabs in the application UI.
///
/// Derived traits enable equality comparisons and cloning.
#[derive(PartialEq, Eq, Clone)]
enum Tab {
    /// The "Games" tab, displaying all found games.
    Games,
    /// The "Favourites" tab, displaying favourite games.
    Favourites,
    /// The "Settings" tab, for configuring application paths.
    Settings,
    /// The "About" tab, showing information about the application.
    About,
}