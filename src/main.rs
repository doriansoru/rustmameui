//! The main entry point for the MAME UI application.
//!
//! This file handles the initial setup, including declaring modules,
//! configuring internationalization (i18n) based on the system locale,
//! and launching the main user interface.

// Declare the application's modules.
mod rustmameuiconfig; // Module for application configuration handling.
mod game;             // Module defining the Game struct and related methods.
mod games;            // Module for game list management, MAME interaction, saving/loading games.
mod ui;               // Module containing the Dioxus-based user interface logic.

// Import the Locale struct for determining the system locale.
use locale_config::Locale;

// Initialize the rust-i18n crate, specifying the "locales" directory
// for translation files and setting "en" as the fallback language.
rust_i18n::i18n!("locales", fallback = "en");

/// The main function where the application starts execution.
///
/// It initializes the application by setting the appropriate language
/// based on the system's locale and then launches the graphical user interface.
///
/// # Returns
///
/// Returns `Ok(())` on successful execution, or a `Box<dyn std::error::Error>`
/// if an error occurs during the process (though current implementation is simple
/// and less prone to errors here).
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the system locale string (e.g., "en_US.UTF-8", "it_IT").
    let sys_locale = Locale::current().to_string();

    // Extract the base language code from the locale string (e.g., "en", "it").
    // Defaults to "en" if splitting fails.
    let lang_code = sys_locale.split('_').next().unwrap_or("en");

    // Set the application's locale for rust-i18n based on the extracted code.
    rust_i18n::set_locale(lang_code);

    match ui::check_dialog_utility() {
        Ok(_) => {
            // Draw and launch the main application UI. This is typically a blocking call
            // until the UI window is closed.
            ui::draw();

            // Return Ok(()) indicating successful completion.
            Ok(())
        },
        Err(e) => {
            println!("{}", e);
            Ok(())
        }
    }
}