use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};

pub struct InputSimulator {
    enigo: Enigo,
}

impl InputSimulator {
    pub fn new() -> Result<Self> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings)?;
        Ok(Self { enigo })
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        self.enigo.text(text)?;
        Ok(())
    }
}
