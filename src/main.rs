mod rustmameuiconfig;
mod game;
mod games;
mod ui;
use locale_config::Locale;

rust_i18n::i18n!("locales", fallback = "en");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the system locale
    let sys_locale = Locale::current().to_string();

    // Extract the lang code
    let lang_code = sys_locale.split('_').next().unwrap_or("en");

    // Set the locale
    rust_i18n::set_locale(lang_code);
    
    ui::draw();
    Ok(())
}

