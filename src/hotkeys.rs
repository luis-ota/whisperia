use anyhow::Result;

use crate::tray::AppEvent;

pub fn setup_hotkeys(_event_tx: std::sync::mpsc::Sender<AppEvent>, _shortcut: &str) -> Result<()> {
    // stub - hotkeys temporarily disabled
    // will be reimplemented with updated global_hotkey API later

    Ok(())
}
