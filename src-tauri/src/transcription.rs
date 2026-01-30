use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    context: WhisperContext,
}

impl Transcriber {
    pub fn new(model_path: &PathBuf) -> Result<Self> {
        info!("loading whisper model from: {:?}", model_path);

        if !model_path.exists() {
            anyhow::bail!("model file not found: {:?}", model_path);
        }

        let context_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(
            model_path.to_str().context("invalid model path")?,
            context_params,
        )
        .context("failed to load whisper model")?;

        info!("whisper model loaded successfully");

        Ok(Self { context })
    }

    pub fn transcribe(&self, audio_data: &[f32], language: &str) -> Result<String> {
        info!("transcribing {} samples", audio_data.len());

        // create a state for this transcription
        let mut state = self
            .context
            .create_state()
            .context("failed to create whisper state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_n_threads(4);

        // whisper aceita f32 diretamente agora
        state
            .full(params, audio_data)
            .context("transcription failed")?;

        // iterar pelos segmentos usando o novo metodo as_iter
        let mut text = String::new();

        for segment in state.as_iter() {
            text.push_str(&segment.to_string());
            text.push(' ');
        }

        let text = text.trim().to_string();
        info!("transcription complete: {} chars", text.len());

        Ok(text)
    }
}
