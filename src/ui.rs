use dioxus::prelude::*;
use crate::rustmameuiconfig::Config;
use crate::game::Game;
use rfd::FileDialog;
use rust_i18n::t;

const TABS_CSS: Asset = asset!("/assets/tabs.css");
const ABOUT: Asset = asset!("/assets/about.svg");
const APP_NAME: &str = "MAMEUI";
const BATCH_SIZE: usize = 100;


pub fn draw() {
    #[cfg(feature = "desktop")]
    fn launch_app() {
        let window = dioxus::desktop::tao::window::WindowBuilder::new()
        .with_resizable(true)
        .with_inner_size(dioxus::desktop::tao::dpi::LogicalSize::new(1024.0, 860.0))
        .with_title::<&str>(APP_NAME);
        dioxus::LaunchBuilder::new().with_cfg(dioxus::desktop::Config::new().with_window(window).with_menu(None)).launch(App);
    }

    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        dioxus::launch(App);
    }

    launch_app();
}


fn check_dialog_utility() -> Result<(), String> {
    // Check if zenity or kdialog are installed
    let mut found = false;
    if which::which("zenity").is_ok() {
        found = true;
    }
    if which::which("kdialog").is_ok() {
        found = true;
    }

    if !found {
        return Err(t!("error_no_dialog_utility").to_string());
        //"Error: No dialog utility has been found. Install 'zenity' or 'kdialog' to open the file chooser dialog."
    }

    Ok(())

}

#[component]
fn App() -> Element {
    let app_version = env!("CARGO_PKG_VERSION");
    let mut selected_tab = use_signal(|| Tab::Games);
    let config = match Config::new() {
        Ok(c) => {
            c
        },
        Err(e) => {
            panic!("{}",
                t!("error.error", error = e.to_string()).to_string()
                //"{}", format!("Error: {}", e)
            );
        }
    };

    let config_clone = config.clone();
    let mut mame_executable = use_signal(|| config_clone.mame_executable);
    let mut snap_file = use_signal(|| config_clone.snap_file);
    let mut roms_path = use_signal(|| config_clone.roms_path);
    let mut snap_data = use_signal(|| "".to_string());
    let mut favourite_snap_data = use_signal(|| "".to_string());

    // Use signals for games and favourites
    let mut games = use_signal(|| crate::games::load(&config).unwrap());
    let mut favourites = use_signal(|| crate::games::load_favourites(&config).unwrap());
    // Signal to track the rom, description and snap of the selected game
    let mut selected_game = use_signal(|| (String::new(), String::new(), false)); // (rom, description, snap)
    let mut selected_favourite = use_signal(|| (String::new(), String::new(), false)); // (rom, description, snap)
    let mut game_filter = use_signal(String::new);
    let mut favourite_filter = use_signal(String::new);
    let filtered_games = use_memo(move || {
        let filter = game_filter();
        let all_games = games();
        all_games.iter()
            .filter(|item| item.description().to_lowercase().contains(&filter.to_lowercase()))
            .cloned()
            .collect::<Vec<Game>>()
    });
    let filtered_favourites = use_memo(move || {
        let filter = favourite_filter();
        let all_favourites = favourites();
        all_favourites.iter()
            .filter(|item| item.description().to_lowercase().contains(&filter.to_lowercase()))
            .cloned()
            .collect::<Vec<Game>>()
    });
    let mut status = use_signal(|| String::from(""));
    let mut selected_game_popup_menu_label = use_signal(|| "".to_string());
    let selected_game_popup_menu_current_label = use_memo(move || { 
        selected_game_popup_menu_label()
    });

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

    let mut selected_favourite_popup_menu_label = use_signal(|| "".to_string());
    let selected_favourite_popup_menu_current_label = use_memo(move || { 
        selected_favourite_popup_menu_label()
    });

    let selected_favourite_is_favourite = use_memo(move || {
        let (rom, _, _) = selected_favourite();
        let mut found = false;
        for favourite in favourites() {
            if favourite.rom() == rom {
                found = true;
            }
        }
        found
    });

    let mut show_context_menu = use_signal(|| false);
 
    let mut menu_x = use_signal(|| 0_i32);
    let mut menu_y = use_signal(|| 0_i32);

    let handle_context_menu = move |event: Event<MouseData>| {
        event.stop_propagation();
        event.prevent_default();
        // Use client coordinates
        let (x, y) = (event.coordinates().client().x as i32, event.coordinates().client().y as i32);
        menu_x.set(x);
        menu_y.set(y);
        show_context_menu.set(true);
    };

    let hide_menu = move |_| {
        show_context_menu.set(false);
    };

    rsx! {
        // Riferimento allo stylesheet esterno come asset
        document::Link { rel: "stylesheet", href: TABS_CSS }

        div {
            class: "container",
            div {
                class: "tabs-header",
                button {
                    class: if selected_tab() == Tab::Games { "tab-button active" } else { "tab-button" },
                    onclick: move |_| selected_tab.set(Tab::Games),
                    {t!("games")}
                    //"Games"
                }
                button {
                    class: if selected_tab() == Tab::Favourites { "tab-button active" } else { "tab-button" },
                    onclick: move |_| {
                        selected_tab.set(Tab::Favourites);
                    },
                    {t!("favourites")}
                    //"Favourites"
                }
                button {
                    class: if selected_tab() == Tab::Settings { "tab-button active" } else { "tab-button" },
                    onclick: move |_| selected_tab.set(Tab::Settings),
                    {t!("settings")}
                    //"Settings"
                }
                button {
                    class: if selected_tab() == Tab::About { "tab-button active" } else { "tab-button" },
                    onclick: move |_| selected_tab.set(Tab::About),
                    {t!("about")}
                    //"About"
                }
            }
            div {
                onclick: hide_menu,
                class: "tab-content",
                // First tab
                div {
                    class: if selected_tab() == Tab::Games { "tab-panel active" } else { "tab-panel" },
                    h2 { 
                        {t!("all_games")}
                        //"All Games" 
                    }
                    p {
                        margin_bottom: "1.5em",
                        {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
                        //"Double click on a game to launch. Right click for more options"
                    }
                    div {
                        class: "row",
                        div {
                            class: "column",
                            select {
                                style: "max-width: 500px;",
                                resize: "both",
                                autocomplete: "on",
                                size: 20,
                                oncontextmenu: handle_context_menu,
                                onchange: move |event| {
                                    let rom = event.value().clone();
                                    // Find the rom description
                                    let description = games.read().iter()
                                    .find(|game| game.rom().clone() == rom)
                                    .map(|game| game.description().clone())
                                    .unwrap_or_default();

                                    // Find the rom snap
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
                            },
                            {
                                if show_context_menu() {                            
                                    // Imposta l'etichetta corretta basata sullo stato attuale
                                    if selected_game_is_favourite() {
                                        //"Remove from favourites"
                                        selected_game_popup_menu_label.set(t!("remove_from_favourites").to_string());
                                    } else {
                                        //"Add to favourites"
                                        selected_game_popup_menu_label.set(t!("add_to_favourites").to_string());
                                    }
                                    
                                    if selected_favourite_is_favourite() {
                                        //"Remove from favourites"
                                        selected_favourite_popup_menu_label.set(t!("remove_from_favourites").to_string());
                                    } else {
                                        //"Add to favourites"
                                        selected_favourite_popup_menu_label.set(t!("add_to_favourites").to_string());
                                    }
                                    rsx! {
                                        div {
                                            class: "popup",
                                            // Popup coordinates
                                            style: "left: {menu_x()}px; top: {menu_y()}px;",
                                            div {
                                                style: "cursor: pointer; padding: 10px;",
                                                onclick: move |_| {
                                                    let config = Config::new().unwrap();
                                                    let (rom, description, snap) = selected_game();
                                                    let favourite = Game::new(&rom, &description, snap);
                                                    let new_favourites;
                                                    if selected_game_is_favourite() {
                                                        new_favourites = crate::games::remove_favourite(&config, &mut favourites(), &favourite);
                                                    } else {
                                                        new_favourites = crate::games::add_favourite(&config, &mut favourites(), &favourite);
                                                    }
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
                            },
                            div {
                                div {
                                    class: "column",
                                    input {
                                        type: "text",
                                        size: 50,
                                        oninput: move |evt| {
                                            game_filter.set(evt.value());
                                        }
                                    }    
                                    label {
                                        {t!("type_the_name_of_a_game_to_search_for_it")}
                                        //"Type the name of a game to search for it"
                                    }
                                }
                            }
                        }
                        div {
                            class: "snap",
                            img {
                                src: "{snap_data()}",
                                width: 500,
                            }
                        }
                    }
                }

                // Second tab
                div {
                    class: if selected_tab() == Tab::Favourites { "tab-panel active" } else { "tab-panel" },
                    h2 { 
                        {t!("favourite_games")}
                        //"Favourite Games"
                    }
                    p {
                        margin_bottom: "1.5em",
                        {t!("double_click_on_a_game_to_launch_right_click_for_more_options")}
                        //"Double click on a game to launch. Right click for more options"
                    }
                    div {
                        class: "row",
                        div {
                            class: "column",
                            select {
                                style: "max-width: 500px;",
                                resize: "both",
                                autocomplete: "on",
                                size: 20,
                                oncontextmenu: handle_context_menu,
                                onchange: move |event| {
                                    let rom = event.value().clone();
                                    // Find the rom description
                                    let description = favourites.read().iter()
                                    .find(|game| game.rom().clone() == rom)
                                    .map(|game| game.description().clone())
                                    .unwrap_or_default();

                                    // Find the rom snap
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
                            },
                            {
                                if show_context_menu() {
                                    rsx! {
                                        div {
                                            class: "popup",
                                            // Popup coordinates
                                            style: "left: {menu_x()}px; top: {menu_y()}px;",

                                            div {
                                                style: "cursor: pointer; padding: 10px;",
                                                onclick: move |_| {
                                                    let config = Config::new().unwrap();
                                                    let (rom, description, snap) = selected_game();
                                                    let favourite = Game::new(&rom, &description, snap);
                                                    let new_favourites;
                                                    if selected_favourite_is_favourite() {
                                                        new_favourites = crate::games::remove_favourite(&config, &mut favourites(), &favourite);
                                                    } else {
                                                        new_favourites = crate::games::add_favourite(&config, &mut favourites(), &favourite);
                                                    }
                                                    favourites.set(new_favourites);
                                                    show_context_menu.set(false);
                                                },
                                                {selected_favourite_popup_menu_current_label()}
                                            }
                                        }
                                    }
                                } else {
                                    rsx! {}
                                }
                            },
                            div {
                                div {
                                    class: "column",
                                    input {
                                        type: "text",
                                        size: 50,
                                        oninput: move |evt| {
                                            favourite_filter.set(evt.value());
                                        }
                                    }    
                                    label {
                                        {t!("type_the_name_of_a_game_to_search_for_it")}
                                        //"Type the name of a game to search for it"
                                    }
                                }
                            }
                        }
                        div {
                            class: "snap",
                            img {
                                src: "{favourite_snap_data()}",
                                width: 500,
                            }
                        }
                    }
                }
                // Third tab
                div {
                    class: if selected_tab() == Tab::Settings { "tab-panel active" } else { "tab-panel" },
                    h2 { 
                        {t!("app_name.settings", app_name = APP_NAME )}
                        //"{APP_NAME} Settings"
                    }
                    div {
                        class: "setting",
                        label {
                            {t!("mame_executable_path")}
                            //"MAME executable path"
                        }
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
                                            .add_filter(
                                                t!("all_files"),
                                                //"All files", 
                                                &["*"]) // Filter for all files
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
                            //"Browse"
                        }
                    }
                    div {
                        class: "setting",
                        label {
                            {t!("roms_directory")}
                            //"ROMS directory"
                        }
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
                                            .add_filter(
                                                t!("all_files"),
                                                //"All files", 
                                                &["*"]) // Filter for all files
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
                            //"Browse"
                        }
                    }
                    div {
                        class: "setting",
                        label {
                            {t!("snaps_zip_not_7z_file_path")}
                            //"SNAPS zip (not 7z) file path"
                        }
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
                                            .add_filter(
                                                t!("zip_file"),
                                                //"Zip file",
                                                &["*.zip"]) // Filter for all zip files
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
                                                status.set(
                                                    t!("the_settings_have_been_saved_correctly").into()
                                                    //"The settings have been saved correctly."
                                                );
                                            },
                                            Err(e) => {
                                                status.set(
                                                    t!("error_while_saving_settings.error", error = e.to_string()).to_string()
                                                    //format!("Error while saving settings: {}", e)
                                                );
                                            }
                                        }
                                    },
                                    "Save settings"
                                }
                                // Refresh games
                                button {
                                    class: "action-button",
                                    onclick: move |_| async move {
                                        let config_third_clone = Config::new().unwrap();
                                        status.set(
                                            t!("reading_the_roms_list_from_mame").into()
                                            //"Reading the roms list from mame..."
                                        );
                                        // Wait to update the status
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                        let xml_string = crate::games::get_xml_roms(&config_third_clone).expect(
                                            t!("cannot_get_xml_data_from_mame").to_string().as_str()
                                            //"Cannot get xml data from mame"
                                        );

                                        status.set("Parsing the roms list to get valid roms...".into());
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                        let (roms, descriptions) = crate::games::get_all_roms(&config_third_clone, &xml_string).expect(
                                            t!("cannot_read_all_roms_and_their_descriptions").to_string().as_str()
                                            //"Cannot read all roms and their descriptions"
                                        );
                                        let mut all_games: Vec<Game> = Vec::new();

                                        status.set(
                                            t!("verifying_working_roms").into()
                                            //"Verifying working roms..."
                                        );
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                        let total_roms = roms.len();
                                        let mut working_roms = Vec::with_capacity(total_roms);
                                        let mut working_snaps = Vec::with_capacity(total_roms);
                                        let num_batches = total_roms.div_ceil(BATCH_SIZE);

                                        for batch_index in 0..num_batches {
                                            let start_index = batch_index * BATCH_SIZE;
                                            let end_index = usize::min((batch_index + 1) * BATCH_SIZE, total_roms);
                                            status.set(
                                                t!("verifying_roms.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into()
                                                //format!("Verifying ROMS {}...{} of {}...", (start_index + 1), end_index, total_roms)
                                            );
                                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                            let current_batch = &roms[start_index..end_index];
                                            let roms_batch_results = crate::games::verify_batch_roms(&config_third_clone, current_batch);
                                            working_roms.extend(roms_batch_results);

                                            status.set(
                                                t!("verifying_snaps.from.to.of.total", from = (start_index + 1), to = end_index, total = total_roms).into()
                                                //format!("Verifying SNAPS {}...{} of {}...", (start_index + 1), end_index, total_roms)
                                            );
                                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                            let snaps_batch_results = crate::games::verify_batch_snaps(&config_third_clone, current_batch);
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

                                        status.set(
                                            t!("saving_all_games").into()
                                            //"Saving all games..."
                                        );
                                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                                        match crate::games::save(&config_third_clone, &all_games) {
                                            Ok(_) => {},
                                            Err(e) => {
                                                panic!("{}",
                                                    t!("error_while_saving_games_list.error", error = e.to_string()).to_string()
                                                    //"Error while saving games list: {}", e
                                                );
                                            }
                                        };

                                        status.set(
                                            t!("complete").into()
                                            //"Complete!".into()
                                        );
                                        games.set(all_games);
                                    },
                                    {t!("refresh_games_list")}
                                    //"Refresh games list"
                                }
                        }
                        div {
                            {status()}
                        }
                    }
                }
                // Fourth tab
                div {
                    class: if selected_tab() == Tab::About { "tab-panel active" } else { "tab-panel" },
                    h2 { 
                        {t!("about.app_name", app_name = APP_NAME)}
                        //"About {APP_NAME}"
                    }
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
                                {t!("app_name.version.by", app_name = APP_NAME, version = app_version)}
                                //"{APP_NAME} {app_version}, by "
                                a {
                                    href: "mailto:doriansoru@gmail.com",
                                    "Dorian Soru"
                                },
                                ", 2025."
                            }
                            p {
                                {t!("released_under_the")}
                                //"Released under the "
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
    }
}



#[derive(PartialEq, Eq, Clone)]
enum Tab {
    Games,
    Favourites,
    Settings,
    About,
}
