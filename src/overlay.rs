use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};

pub enum OverlayState {
    Hidden,
    Listening,
    Transcribing,
    Result(String),
}

pub struct Overlay {
    sender: Sender<OverlayCommand>,
}

pub enum OverlayCommand {
    Show(OverlayState),
    Hide,
    Update(OverlayState),
}

impl Overlay {
    pub fn new() -> Result<Self> {
        let (tx, _rx) = channel::<OverlayCommand>();

        // stub - overlay UI temporarily disabled
        // will be reimplemented with updated winit API later

        Ok(Self { sender: tx })
    }

    pub fn show(&self, _state: OverlayState) -> Result<()> {
        // stub
        Ok(())
    }

    pub fn hide(&self) -> Result<()> {
        // stub
        Ok(())
    }

    pub fn update(&self, _state: OverlayState) -> Result<()> {
        // stub
        Ok(())
    }
}
