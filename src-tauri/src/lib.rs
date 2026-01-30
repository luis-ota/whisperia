use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri::tray::TrayIconBuilder;
use tauri::tray::TrayIconEvent;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tracing::{info, error, warn};

#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::ConnectionExt;

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
    hotkey_manager: Mutex<Option<GlobalHotKeyManager>>,
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
            hotkey_manager: Mutex::new(None),
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
    
    pub fn set_hotkey_manager(&self, manager: GlobalHotKeyManager) {
        let mut hm = self.hotkey_manager.lock().unwrap();
        *hm = Some(manager);
    }
}

// Implement Clone for AppState
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            status: Mutex::new(self.get_status()),
            config: Mutex::new(self.get_config()),
            audio_data: Mutex::new(None),
            hotkey_manager: Mutex::new(None),
        }
    }
}

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    Ok(state.get_status())
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
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.center();
    }
    Ok(())
}

#[tauri::command]
async fn show_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let (x, y) = get_cursor_position();
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn hide_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
async fn start_recording(app: AppHandle) -> Result<(), String> {
    info!("start_recording command invoked");
    trigger_transcription_flow(app).map_err(|e| {
        error!("Failed to start recording: {}", e);
        e.to_string()
    })
}

#[tauri::command]
async fn stop_recording(app: AppHandle) -> Result<(), String> {
    info!("stop_recording command invoked");
    // For now, this just hides the overlay and resets state
    let state = app.state::<AppState>();
    state.set_recording(false);
    state.set_transcribing(false);
    hide_overlay_window(&app).map_err(|e| e.to_string())
}

fn get_model_path(config: &Config) -> anyhow::Result<PathBuf> {
    let models_dir = Config::models_dir()?;
    let model_file = models_dir.join(format!("ggml-{}.bin", config.model.local_model));
    
    if model_file.exists() {
        Ok(model_file)
    } else {
        anyhow::bail!("Model not found. Use download-models.sh or --model-path")
    }
}

fn get_cursor_position() -> (i32, i32) {
    #[cfg(target_os = "linux")]
    {
        use x11rb::connection::Connection;
        
        if let Ok((conn, screen_num)) = x11rb::connect(None) {
            let screen = &conn.setup().roots[screen_num];
            if let Ok(cookie) = conn.query_pointer(screen.root) {
                if let Ok(reply) = cookie.reply() {
                    return (reply.root_x as i32, reply.root_y as i32);
                }
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::mem::zeroed;
        unsafe {
            let mut point: windows::Win32::Foundation::POINT = zeroed();
            if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point).is_ok() {
                return (point.x, point.y);
            }
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::NSEvent;
        use cocoa::base::nil;
        use cocoa::foundation::NSPoint;
        
        unsafe {
            let point: NSPoint = NSEvent::mouseLocation(nil);
            return (point.x as i32, point.y as i32);
        }
    }
    
    // Default position if we can't get cursor position
    (100, 100)
}

fn setup_tray(app: &mut tauri::App) -> anyhow::Result<()> {
    // Create menu items
    let transcribe_i = MenuItem::with_id(app, "transcribe", "Transcrever", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    
    // Create menu
    let menu = Menu::with_items(app, &[&transcribe_i, &settings_i, &separator, &quit_i])?;
    
    // Build tray icon with event handler
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Whisperia")
        .icon(app.default_window_icon().unwrap().clone())
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                let app = tray.app_handle();
                info!("Tray icon clicked - triggering transcription");
                if let Err(e) = trigger_transcription_flow(app.clone()) {
                    error!("Failed to trigger transcription from tray: {}", e);
                }
            }
        })
        .build(app)?;
    
    Ok(())
}

fn setup_hotkeys(app: &mut tauri::App) -> anyhow::Result<()> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    
    let manager = GlobalHotKeyManager::new()?;
    
    // Register Super+Shift+T (Cmd+Shift+T on macOS, Win+Shift+T on Windows/Linux)
    let hotkey = HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT);
    manager.register(hotkey)?;
    
    // Store manager in app state
    let state = app.state::<AppState>();
    state.set_hotkey_manager(manager);
    
    info!("Global hotkey Super+Shift+T registered successfully");
    
    Ok(())
}

fn trigger_transcription_flow(app: AppHandle) -> anyhow::Result<()> {
    info!("Triggering transcription flow");
    
    let state = app.state::<AppState>();
    
    // Check if already recording
    if state.get_status().is_recording || state.get_status().is_transcribing {
        info!("Already recording or transcribing, skipping");
        return Ok(());
    }
    
    // Show overlay at cursor position
    if let Some(overlay) = app.get_webview_window("overlay") {
        let (x, y) = get_cursor_position();
        let _ = overlay.set_position(tauri::Position::Physical(tauri::PhysicalPosition { 
            x: x.saturating_sub(200),
            y: y.saturating_sub(75),
        }));
        let _ = overlay.show();
        let _ = overlay.set_focus();
        let _ = overlay.emit("status-update", "Recording...");
    }
    
    // Start recording
    state.set_recording(true);
    
    // Clone for thread
    let app_clone = app.clone();
    let state_clone = Arc::new(state.inner().clone());
    
    // Spawn recording thread
    thread::spawn(move || {
        info!("Starting audio recording thread");
        
        // Record audio
        let recorder = match AudioRecorder::new() {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to create audio recorder: {}", e);
                let _ = app_clone.emit("status-update", format!("Error: {}", e));
                let _ = app_clone.emit("transcription-error", format!("Audio recorder error: {}", e));
                let _ = hide_overlay_window(&app_clone);
                state_clone.set_recording(false);
                return;
            }
        };
        
        // Record for 5 seconds
        match recorder.record_for_seconds(5) {
            Ok(data) => {
                info!("Audio recorded successfully: {} samples", data.len());
                state_clone.store_audio(data);
                let _ = app_clone.emit("status-update", "Transcribing...");
                if let Some(overlay) = app_clone.get_webview_window("overlay") {
                    let _ = overlay.emit("status-update", "Transcribing...");
                }
                
                // Process transcription
                let state = app_clone.state::<AppState>();
                state.set_recording(false);
                state.set_transcribing(true);
                
                // Get audio data
                let audio_data = match state.take_audio() {
                    Some(data) => data,
                    None => {
                        error!("No audio data available after recording");
                        let _ = app_clone.emit("status-update", "Error: No audio data");
                        let _ = app_clone.emit("transcription-error", "No audio data available");
                        let _ = hide_overlay_window(&app_clone);
                        state.set_transcribing(false);
                        return;
                    }
                };
                
                // Get model path
                let config = state.get_config();
                let model_path = match get_model_path(&config) {
                    Ok(path) => path,
                    Err(e) => {
                        error!("Model not found: {}", e);
                        let _ = app_clone.emit("status-update", format!("Error: {}", e));
                        let _ = app_clone.emit("transcription-error", format!("Model error: {}", e));
                        let _ = hide_overlay_window(&app_clone);
                        state.set_transcribing(false);
                        return;
                    }
                };
                
                info!("Loading transcriber with model: {:?}", model_path);
                
                // Transcribe
                let transcriber = match Transcriber::new(&model_path) {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to load transcriber: {}", e);
                        let _ = app_clone.emit("status-update", format!("Error: {}", e));
                        let _ = app_clone.emit("transcription-error", format!("Transcriber error: {}", e));
                        let _ = hide_overlay_window(&app_clone);
                        state.set_transcribing(false);
                        return;
                    }
                };
                
                info!("Starting transcription with language: {}", config.language);
                
                let text = match transcriber.transcribe(&audio_data, &config.language) {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Transcription failed: {}", e);
                        let _ = app_clone.emit("status-update", format!("Error: {}", e));
                        let _ = app_clone.emit("transcription-error", format!("Transcription failed: {}", e));
                        let _ = hide_overlay_window(&app_clone);
                        state.set_transcribing(false);
                        return;
                    }
                };
                
                info!("Transcription complete: '{}'", text);
                
                // Type the result
                let mut input = match InputSimulator::new() {
                    Ok(i) => i,
                    Err(e) => {
                        error!("Failed to create input simulator: {}", e);
                        let _ = app_clone.emit("status-update", format!("Error: {}", e));
                        let _ = app_clone.emit("transcription-error", format!("Input error: {}", e));
                        let _ = hide_overlay_window(&app_clone);
                        state.set_transcribing(false);
                        return;
                    }
                };
                
                if let Err(e) = input.type_text(&text) {
                    warn!("Failed to type text: {}", e);
                    let _ = app_clone.emit("status-update", format!("Error typing: {}", e));
                }
                
                // Update state
                state.set_result(text.clone());
                
                // Emit to frontend
                let _ = app_clone.emit("transcription-update", &text);
                let _ = app_clone.emit("status-update", "Ready");
                let _ = app_clone.emit("transcription-complete", &text);
                if let Some(overlay) = app_clone.get_webview_window("overlay") {
                    let _ = overlay.emit("transcription-complete", &text);
                }
                
                // Hide overlay after a delay
                thread::sleep(Duration::from_millis(1000));
                let _ = hide_overlay_window(&app_clone);
            }
            Err(e) => {
                error!("Audio recording failed: {}", e);
                let _ = app_clone.emit("status-update", format!("Error: {}", e));
                let _ = app_clone.emit("transcription-error", format!("Recording error: {}", e));
                let _ = hide_overlay_window(&app_clone);
                state_clone.set_recording(false);
            }
        }
    });
    
    Ok(())
}

fn hide_overlay_window(app: &AppHandle) -> anyhow::Result<()> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new().expect("Failed to create app state"))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_config,
            update_config,
            get_available_models,
            get_system_info,
            open_settings,
            show_overlay,
            hide_overlay,
            start_recording,
            stop_recording,
        ])
        .setup(|app| {
            info!("Whisperia Tauri app starting...");
            
            // Hide main window on startup
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            
            // Setup system tray
            setup_tray(app)?;
            
            // Setup global hotkeys
            setup_hotkeys(app)?;
            
            // Setup menu event handler
            let app_handle = app.handle().clone();
            app.on_menu_event(move |app, event| {
                match event.id.as_ref() {
                    "transcribe" => {
                        info!("Menu 'transcribe' clicked");
                        if let Err(e) = trigger_transcription_flow(app.clone()) {
                            error!("Failed to trigger transcription from menu: {}", e);
                        }
                    }
                    "settings" => {
                        info!("Menu 'settings' clicked");
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.center();
                        }
                    }
                    "quit" => {
                        info!("Menu 'quit' clicked - exiting application");
                        app.exit(0);
                    }
                    _ => {}
                }
            });
            
            // Setup global hotkey event receiver thread
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                info!("Starting global hotkey event listener thread");
                let receiver = GlobalHotKeyEvent::receiver();
                loop {
                    if let Ok(event) = receiver.recv() {
                        info!("Global hotkey event received: {:?}", event);
                        if event.state == HotKeyState::Pressed {
                            if let Err(e) = trigger_transcription_flow(app_handle.clone()) {
                                error!("Failed to trigger transcription from hotkey: {}", e);
                            }
                        }
                    }
                }
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            // Handle window close - hide instead of exit for main window
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
