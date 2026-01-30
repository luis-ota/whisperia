use anyhow::Result;
use serde::Deserialize;
use sysinfo::System;
use tracing::{info, warn};

pub struct HardwareDetector {
    sys: System,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub total_memory_gb: u64,
    pub available_memory_gb: u64,
    pub cpu_cores: usize,
    pub cpu_name: String,
    pub has_gpu: bool,
    pub gpu_vram_gb: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ModelCompatibility {
    pub model: String,
    pub can_run: bool,
    pub ram_required_gb: u64,
    #[allow(dead_code)]
    pub vram_required_gb: Option<u64>,
    pub estimated_speed: String,
}

impl HardwareDetector {
    pub fn new() -> Result<Self> {
        let sys = System::new_all();

        Ok(Self { sys })
    }

    pub fn get_system_info(&self) -> SystemInfo {
        let total_memory = self.sys.total_memory();
        let available_memory = self.sys.available_memory();

        SystemInfo {
            total_memory_gb: total_memory / 1024 / 1024 / 1024,
            available_memory_gb: available_memory / 1024 / 1024 / 1024,
            cpu_cores: self.sys.physical_core_count().unwrap_or(1),
            cpu_name: self
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().to_string())
                .unwrap_or_default(),
            has_gpu: false, // will be updated if gpu detection is enabled
            gpu_vram_gb: None,
        }
    }

    pub fn check_model_compatibility(&self, model: &str) -> ModelCompatibility {
        let sys_info = self.get_system_info();

        let (ram_required, vram_required, speed) = match model {
            "tiny" => (1_u64, Some(1_u64), "rapido - qualidade basica"),
            "base" => (2_u64, Some(1_u64), "muito rapido - boa qualidade"),
            "small" => (3_u64, Some(2_u64), "rapido - otima qualidade"),
            "medium" => (6_u64, Some(4_u64), "moderado - excelente qualidade"),
            "large" => (10_u64, Some(8_u64), "lento - qualidade maxima"),
            _ => (4_u64, None, "depende do modelo"),
        };

        let can_run = sys_info.available_memory_gb >= ram_required;
        // so verifica vram se tem gpu. se nao tem gpu, ignora o requisito de vram
        let has_enough_vram = if sys_info.has_gpu {
            vram_required
                .map(|v| sys_info.gpu_vram_gb.unwrap_or(0) >= v)
                .unwrap_or(true)
        } else {
            true // sem gpu = nao precisa de vram
        };

        ModelCompatibility {
            model: model.to_string(),
            can_run: can_run && has_enough_vram,
            ram_required_gb: ram_required,
            vram_required_gb: vram_required,
            estimated_speed: speed.to_string(),
        }
    }

    pub fn get_available_models(&self) -> Vec<ModelCompatibility> {
        let models = vec!["tiny", "base", "small", "medium", "large"];
        models
            .into_iter()
            .map(|m| self.check_model_compatibility(m))
            .collect()
    }

    pub fn print_system_info(&self) {
        let info = self.get_system_info();
        info!("system info:");
        info!("  cpu: {} ({} cores)", info.cpu_name, info.cpu_cores);
        info!(
            "  ram: {} gb total, {} gb available",
            info.total_memory_gb, info.available_memory_gb
        );

        if let Some(vram) = info.gpu_vram_gb {
            info!("  gpu vram: {} gb", vram);
        }

        info!("available whisper models:");
        for model in self.get_available_models() {
            let status = if model.can_run { "[ok]" } else { "[x]" };
            info!(
                "  {} {} - ram: {}gb, speed: {}",
                status, model.model, model.ram_required_gb, model.estimated_speed
            );
        }
    }

    pub async fn check_huggingface_model(&self, model_id: &str) ->  Result<HuggingFaceCompatibility> {
        let client = reqwest::Client::new();
        
        // try to fetch model info from huggingface api
        let api_url = format!("https://huggingface.co/api/models/{}", model_id);
        
        let response = client.get(&api_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        
        let model_info = match response {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<HuggingFaceModelInfo>().await.ok()
            }
            _ => None,
        };
        
        // estimate model size from model_id or safetensors info
        let (estimated_size_mb, model_type) = self.estimate_model_size(model_id, model_info.as_ref()).await;
        
        // calculate requirements (2x for runtime overhead + 1gb base)
        let ram_required_gb = ((estimated_size_mb * 2) / 1024 + 1024) / 1024;
        let vram_required_gb = if self.get_system_info().has_gpu {
            Some(ram_required_gb)
        } else {
            None
        };
        
        let sys_info = self.get_system_info();
        let can_run = sys_info.available_memory_gb >= ram_required_gb;
        let has_enough_vram = vram_required_gb
            .map(|v| sys_info.gpu_vram_gb.unwrap_or(0) >= v)
            .unwrap_or(true);
        
        let performance_rating = if ram_required_gb <= 2 {
            "rapido"
        } else if ram_required_gb <= 4 {
            "moderado"
        } else if ram_required_gb <= 8 {
            "lento"
        } else {
            "muito lento"
        };
        
        let recommendation = if !can_run {
            format!("memoria insuficiente. necessario {}gb, disponivel {}gb", 
                ram_required_gb, sys_info.available_memory_gb)
        } else if ram_required_gb > 4 {
            "considere usar um modelo menor para melhor performance".to_string()
        } else {
            "modelo adequado para este sistema".to_string()
        };
        
        Ok(HuggingFaceCompatibility {
            model_id: model_id.to_string(),
            can_run: can_run && has_enough_vram,
            estimated_size_mb,
            ram_required_gb,
            vram_required_gb,
            model_type,
            performance_rating: performance_rating.to_string(),
            recommendation,
        })
    }
    
    async fn estimate_model_size(&self, model_id: &str, model_info: Option<&HuggingFaceModelInfo>) -> (u64, String) {
        // if we got model info from api, use it
        if let Some(info) = model_info {
            // try to find safetensors size
            if let Some(siblings) = &info.siblings {
                let total_size: u64 = siblings
                    .iter()
                    .filter(|s| s.rfilename.ends_with(".safetensors") || s.rfilename.ends_with(".bin"))
                    .map(|s| s.size.unwrap_or(0))
                    .sum();
                
                if total_size > 0 {
                    return (total_size / (1024 * 1024), info.model_type.clone());
                }
            }
        }
        
        // fallback: estimate from model_id naming conventions
        let lower_id = model_id.to_lowercase();
        
        if lower_id.contains("tiny") {
            (150, "tiny".to_string())  // ~150mb
        } else if lower_id.contains("small") {
            (500, "small".to_string())  // ~500mb
        } else if lower_id.contains("medium") {
            (1500, "medium".to_string())  // ~1.5gb
        } else if lower_id.contains("large") || lower_id.contains("turbo") {
            (3000, "large".to_string())  // ~3gb
        } else if lower_id.contains("base") {
            (300, "base".to_string())  // ~300mb
        } else {
            // default estimate for unknown models
            warn!("unknown model size for {}, using default estimate", model_id);
            (500, "unknown".to_string())  // ~500mb default
        }
    }
}

#[derive(Debug, Clone)]
pub struct HuggingFaceCompatibility {
    pub model_id: String,
    pub can_run: bool,
    pub estimated_size_mb: u64,
    pub ram_required_gb: u64,
    pub vram_required_gb: Option<u64>,
    #[allow(dead_code)]
    pub model_type: String,
    pub performance_rating: String,
    pub recommendation: String,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceModelInfo {
    #[allow(dead_code)]
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(rename = "modelType")]
    model_type: String,
    siblings: Option<Vec<ModelFileInfo>>,
}

#[derive(Debug, Deserialize)]
struct ModelFileInfo {
    rfilename: String,
    size: Option<u64>,
}
