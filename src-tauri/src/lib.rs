use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::info;

mod audio;
mod config;
mod hardware;
mod input;
mod transcription;

pub use audio::AudioRecorder;
pub use config::{ApiConfig, Config, ModelConfig, UiConfig};
pub use hardware::HardwareDetector;
pub use input::InputSimulator;
pub use transcription::Transcriber;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatus {
    pub is_recording: bool,
    pub is_transcribing: bool,
    pub last_result: Option<String>,
}

pub struct AppState {
    status: Mutex<AppStatus>,
    config: Mutex<Config>,
    audio_data: Mutex<Option<Vec<f32>>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load_or_create()?;
        
        Ok(Self {
            status: Mutex::new(AppStatus {
                is_recording: false,
                is_transcribing: false,
                last_result: None,
            }),
            config: Mutex::new(config),
            audio_data: Mutex::new(None),
        })
    }
    
    pub fn get_status(&self) -> AppStatus {
        self.status.lock().unwrap().clone()
    }
    
    pub fn set_recording(&self, recording: bool) {
        let mut status = self.status.lock().unwrap();
        status.is_recording = recording;
    }
    
    pub fn set_transcribing(&self, transcribing: bool) {
        let mut status = self.status.lock().unwrap();
        status.is_transcribing = transcribing;
    }
    
    pub fn set_result(&self, result: String) {
        let mut status = self.status.lock().unwrap();
        status.last_result = Some(result);
        status.is_transcribing = false;
        status.is_recording = false;
    }
    
    pub fn store_audio(&self, data: Vec<f32>) {
        let mut audio = self.audio_data.lock().unwrap();
        *audio = Some(data);
    }
    
    pub fn take_audio(&self) -> Option<Vec<f32>> {
        let mut audio = self.audio_data.lock().unwrap();
        audio.take()
    }
    
    pub fn get_config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }
    
    pub fn update_config(&self, config: Config) -> anyhow::Result<()> {
        config.save()?;
        let mut cfg = self.config.lock().unwrap();
        *cfg = config;
        Ok(())
    }
}

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    Ok(state.get_status())
}

#[tauri::command]
async fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Starting recording");
    
    state.set_recording(true);
    
    // Emit status update to frontend
    let _ = app.emit("status-update", "Recording...");
    
    // Start recording in a separate thread
    let app_clone = app.clone();
    let state_clone = Arc::new(state.inner().clone());
    
    std::thread::spawn(move || {
        let recorder = match AudioRecorder::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = app_clone.emit("status-update", format!("Error: {}", e));
                return;
            }
        };
        
        // Record for 5 seconds
        match recorder.record_for_seconds(5) {
            Ok(data) => {
                state_clone.store_audio(data);
            }
            Err(e) => {
                let _ = app_clone.emit("status-update", format!("Error: {}", e));
            }
        }
    });
    
    Ok(())
}

#[tauri::command]
async fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    info!("Stopping recording and transcribing");
    
    state.set_recording(false);
    state.set_transcribing(true);
    
    let _ = app.emit("status-update", "Transcribing...");
    
    // Get the audio data
    let audio_data = state.take_audio().ok_or("No audio data available")?;
    
    // Get model path
    let config = state.get_config();
    let model_path = get_model_path(&config).map_err(|e| e.to_string())?;
    
    // Transcribe
    let transcriber = Transcriber::new(&model_path).map_err(|e| e.to_string())?;
    let text = transcriber.transcribe(&audio_data, &config.language).map_err(|e| e.to_string())?;
    
    // Type the result
    let mut input = InputSimulator::new().map_err(|e| e.to_string())?;
    input.type_text(&text).map_err(|e| e.to_string())?;
    
    // Update state
    state.set_result(text.clone());
    
    // Emit result to frontend
    let _ = app.emit("transcription-update", &text);
    let _ = app.emit("status-update", "Ready");
    
    Ok(text)
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.get_config())
}

#[tauri::command]
async fn update_config(
    config: Config,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.update_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_available_models() -> Result<Vec<hardware::ModelCompatibility>, String> {
    let detector = HardwareDetector::new().map_err(|e| e.to_string())?;
    Ok(detector.get_available_models())
}

#[tauri::command]
fn get_system_info() -> Result<hardware::SystemInfo, String> {
    let detector = HardwareDetector::new().map_err(|e| e.to_string())?;
    Ok(detector.get_system_info())
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    // Show the main window if it's hidden
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

fn get_model_path(config: &Config) -> anyhow::Result<PathBuf> {
    let models_dir = Config::models_dir()?;
    let model_file = models_dir.join(format!("ggml-{}.bin", config.model.local_model));
    
    if model_file.exists() {
        Ok(model_file)
    } else {
        anyhow::bail!("model not found. use download-models.sh or --model-path")
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new().expect("Failed to create app state"))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            start_recording,
            stop_recording,
            get_config,
            update_config,
            get_available_models,
            get_system_info,
            open_settings,
        ])
        .setup(|app| {
            info!("Whisperia Tauri app starting...");
            
            // Setup system tray
            setup_tray(app)?;
            
            // Setup global hotkeys
            setup_hotkeys(app)?;
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(_app: &tauri::App) -> anyhow::Result<()> {
    use tray_icon::{TrayIconBuilder, menu::Menu, menu::MenuItem, menu::PredefinedMenuItem};
    
    let quit_i = MenuItem::new("Quit", true, None);
    let settings_i = MenuItem::new("Settings", true, None);
    let separator = PredefinedMenuItem::separator();
    let menu = Menu::with_items(&[&settings_i, &separator, &quit_i])?;
    
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Whisperia")
        .build()?;
    
    Ok(())
}

fn setup_hotkeys(_app: &tauri::App) -> anyhow::Result<()> {
    // Global hotkeys will be set up here using Tauri's global-shortcut feature
    // This requires additional setup in tauri.conf.json
    Ok(())
}

// Implement Clone for AppState to allow sharing
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            status: Mutex::new(self.get_status()),
            config: Mutex::new(self.get_config()),
            audio_data: Mutex::new(None),
        }
    }
}
