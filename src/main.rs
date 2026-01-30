use anyhow::Result;
use clap::Parser;
use tracing::info;

mod config;
mod hardware;

use config::Config;
use hardware::HardwareDetector;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("whisperia=info")
        .init();

    info!("starting whisperia v{}", env!("CARGO_PKG_VERSION"));

    let cli = Cli::parse();
    let config = Config::load_or_create()?;
    
    // initialize hardware detection
    let hardware = HardwareDetector::new()?;
    
    if cli.check_hardware {
        hardware.print_system_info();
        return Ok(());
    }
    
    if let Some(model_id) = cli.check_model {
        info!("checking huggingface model: {}", model_id);
        let compatibility = hardware.check_huggingface_model(&model_id).await?;
        
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
    println!("  --check-model <id>    check if hf model works (e.g., 'openai/whisper-base')");
    println!("  --list-models         list all available models");
    
    println!("\ngui version coming soon!");
    println!("========================================\n");
    
    Ok(())
}
