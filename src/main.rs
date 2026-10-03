use std::path::PathBuf;

mod ui;

fn main() -> anyhow::Result<()> {
    let initial = std::env::args().nth(1).map(PathBuf::from);
    let root = std::env::var_os("UMBRA_NOTE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("local/UmbraNote/Notes"))
        })
        .unwrap_or_else(|| PathBuf::from("local/UmbraNote/Notes"));
    let state = umbra_note::app::AppState::load(root, initial)?;
    ui::run(state);
    Ok(())
}
