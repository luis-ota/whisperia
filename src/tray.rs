use anyhow::Result;
use std::sync::mpsc::Sender;

pub enum AppEvent {
    StartRecording,
    StopRecording,
    OpenSettings,
    Quit,
}

pub fn setup_tray(_event_tx: Sender<AppEvent>) -> Result<()> {
    // stub - tray temporarily disabled due to MenuItem Send issues
    // will be reimplemented with proper thread handling later

    Ok(())
}
