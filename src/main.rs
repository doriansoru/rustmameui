mod rustmameuiconfig;
mod game;
mod games;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ui::draw();
    Ok(())
}

