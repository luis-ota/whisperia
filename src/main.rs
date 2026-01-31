use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use tracing::info;

mod audio;
mod config;
mod hardware;
mod hotkeys;
mod input;
mod overlay;
mod tray;
mod transcription;

use audio::AudioRecorder;
use config::Config;
use hardware::HardwareDetector;
use input::InputSimulator;
use overlay::{OverlayCommand, OverlayState};
use tray::{setup_tray, AppEvent};
use hotkeys::setup_hotkeys;
use transcription::Transcriber;

#[derive(Parser)]
#[command(name = "whisperia")]
#[command(about = "voice transcription tool for linux tiling window managers")]
struct Cli {
    /// check hardware compatibility
    #[arg(long)]
    check_hardware: bool,
    
    /// check if a huggingface model will work
    #[arg(long, value_name = "model_id")]
    check_model: Option<String>,
    
    /// download a model
    #[arg(long, value_name = "model")]
    download_model: Option<String>,
    
    /// list available models
    #[arg(long)]
    list_models: bool,
    
    /// record and transcribe audio (seconds to record)
    #[arg(long, value_name = "seconds")]
    transcribe: Option<u64>,
    
    /// path to whisper model file
    #[arg(long, value_name = "path")]
    model_path: Option<String>,
    
    /// run in daemon mode with UI
    #[arg(long)]
    daemon: bool,
    
    /// record until ctrl+c is pressed (interactive mode)
    #[arg(long)]
    interactive: bool,
}

fn main() -> Result<()> {
    // initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("whisperia=info")
        .init();

    info!("starting whisperia v{}", env!("CARGO_PKG_VERSION"));

    let cli = Cli::parse();
    
    if cli.daemon {
        run_daemon()?;
    } else {
        run_cli(cli)?;
    }
    
    Ok(())
}

fn run_daemon() -> Result<()> {
    println!("whisperia daemon starting...");
    println!("use tray icon or hotkey to transcribe");
    
    let config = Config::load_or_create()?;
    let (event_tx, event_rx) = channel::<AppEvent>();
    
    // setup system tray
    setup_tray(event_tx.clone())?;
    
    // setup global hotkeys
    setup_hotkeys(event_tx.clone(), &config.shortcut)?;
    
    // setup overlay
    let (overlay_tx, _overlay_rx) = channel::<OverlayCommand>();
    
    // setup input simulator
    let mut input = InputSimulator::new()?;
    
    // get model path
    let model_path = get_model_path(&config)?;
    
    println!("daemon ready!");
    println!("hotkey: {}", config.shortcut);
    
    // main event loop
    loop {
        if let Ok(event) = event_rx.recv() {
            match event {
                AppEvent::StartRecording => {
                    // show overlay
                    let _ = overlay_tx.send(OverlayCommand::Show(OverlayState::Listening));
                    
                    // record audio
                    println!("gravando...");
                    let recorder = AudioRecorder::new()?;
                    let audio_data = recorder.record_for_seconds(5)?;
                    
                    // update overlay
                    let _ = overlay_tx.send(OverlayCommand::Update(OverlayState::Transcribing));
                    
                    // transcribe
                    println!("transcrevendo...");
                    let transcriber = Transcriber::new(&model_path)?;
                    let text = transcriber.transcribe(&audio_data, &config.language)?;
                    
                    // show result
                    let _ = overlay_tx.send(OverlayCommand::Show(OverlayState::Result(text.clone())));
                    
                    // type the result
                    println!("digitando: {}", text);
                    input.type_text(&text)?;
                    
                    // hide overlay after a delay
                    thread::sleep(std::time::Duration::from_millis(2000));
                    let _ = overlay_tx.send(OverlayCommand::Hide);
                }
                AppEvent::StopRecording => {
                    // handled above
                }
                AppEvent::OpenSettings => {
                    println!("configuracao aberta (nao implementado ainda)");
                }
                AppEvent::Quit => {
                    println!("encerrando whisperia...");
                    break;
                }
            }
        }
    }
    
    Ok(())
}

fn run_cli(cli: Cli) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let config = Config::load_or_create()?;
    
    // initialize hardware detection
    let hardware = HardwareDetector::new()?;
    
    if cli.check_hardware {
        hardware.print_system_info();
        return Ok(());
    }
    
    if let Some(model_id) = cli.check_model {
        info!("checking huggingface model: {}", model_id);
        let compatibility = rt.block_on(hardware.check_huggingface_model(&model_id))?;
        
        println!("\nmodel compatibility report");
        println!("========================================");
        println!("model id: {}", compatibility.model_id);
        println!("estimated size: {} mb", compatibility.estimated_size_mb);
        println!("ram required: {} gb", compatibility.ram_required_gb);
        if let Some(vram) = compatibility.vram_required_gb {
            println!("vram required: {} gb", vram);
        }
        println!("performance: {}", compatibility.performance_rating);
        println!("can run: {}", if compatibility.can_run { "yes" } else { "no" });
        println!("recommendation: {}", compatibility.recommendation);
        println!("========================================\n");
        
        return Ok(());
    }
    
    if cli.list_models {
        println!("\navailable whisper models");
        println!("========================================");
        for model in hardware.get_available_models() {
            let status = if model.can_run { "[ok]" } else { "[x]" };
            println!("{} {} - ram: {}gb - {}", 
                status, model.model, model.ram_required_gb, model.estimated_speed);
        }
        println!("========================================\n");
        return Ok(());
    }
    
    if let Some(model) = cli.download_model {
        info!("downloading model: {}", model);
        println!("model download not yet implemented in this version");
        return Ok(());
    }
    
    // transcribe audio with fixed duration
    if let Some(seconds) = cli.transcribe {
        let model_path = if let Some(path) = cli.model_path {
            PathBuf::from(path)
        } else {
            get_model_path(&config)?
        };
        
        println!("\nwhisperia transcription");
        println!("========================================");
        println!("recording for {} seconds...", seconds);
        println!("speak now!\n");
        
        // record audio
        let recorder = AudioRecorder::new()?;
        let audio_data = recorder.record_for_seconds(seconds)?;
        
        println!("recording complete! transcribing...\n");
        
        // transcribe
        let transcriber = Transcriber::new(&model_path)?;
        let text = transcriber.transcribe(&audio_data, &config.language)?;
        
        println!("transcription result:");
        println!("\"{}\"", text);
        println!("========================================\n");
        
        return Ok(());
    }
    
    // interactive mode: record until ctrl+c
    if cli.interactive {
        let model_path = if let Some(path) = cli.model_path {
            PathBuf::from(path)
        } else {
            get_model_path(&config)?
        };
        
        println!("\nwhisperia transcription (interactive mode)");
        println!("========================================");
        println!("gravando... pressione ctrl+c para parar\n");
        
        // record audio until ctrl+c
        let recorder = AudioRecorder::new()?;
        let audio_data = recorder.record_until_interrupt()?;
        
        println!("\ntranscrevendo...\n");
        
        // transcribe
        let transcriber = Transcriber::new(&model_path)?;
        let text = transcriber.transcribe(&audio_data, &config.language)?;
        
        println!("transcription result:");
        println!("\"{}\"", text);
        println!("========================================\n");
        
        return Ok(());
    }
    
    // default: show info and hardware
    println!("\nwhisperia voice transcription tool");
    println!("========================================");
    println!("version: {}", env!("CARGO_PKG_VERSION"));
    println!("config: {:?}", Config::config_path()?);
    println!("\nsystem information:");
    
    let sys_info = hardware.get_system_info();
    println!("  cpu: {} ({} cores)", sys_info.cpu_name, sys_info.cpu_cores);
    println!("  ram: {} gb total, {} gb available", 
        sys_info.total_memory_gb, sys_info.available_memory_gb);
    
    println!("\navailable models:");
    for model in hardware.get_available_models() {
        let status = if model.can_run { "[ok]" } else { "[x]" };
        println!("  {} {} - {}gb ram - {}", 
            status, model.model, model.ram_required_gb, model.estimated_speed);
    }
    
    println!("\nconfiguration:");
    println!("  shortcut: {}", config.shortcut);
    println!("  language: {}", config.language);
    println!("  model: {} ({})", config.model.local_model, config.model.model_type);
    
    println!("\nusage:");
    println!("  --check-hardware      check system compatibility");
    println!("  --check-model <id>    check if hf model works");
    println!("  --list-models         list all available models");
    println!("  --transcribe <secs>   record for fixed seconds");
    println!("  --interactive         record until ctrl+c");
    println!("  --model-path <path>   use specific model file");
    
    println!("\nexamples:");
    println!("  whisperia --transcribe 5");
    println!("  whisperia --interactive");
    println!("  whisperia --transcribe 10 --model-path ~/.local/share/whisperia/models/ggml-small.bin");
    println!("========================================\n");
    
    Ok(())
}

fn get_model_path(config: &Config) -> Result<PathBuf> {
    let models_dir = Config::models_dir()?;
    let model_file = models_dir.join(format!("ggml-{}.bin", config.model.local_model));
    
    if model_file.exists() {
        Ok(model_file)
    } else {
        anyhow::bail!("model not found. use download-models.sh or --model-path")
    }
}
