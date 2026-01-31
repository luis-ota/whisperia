#!/bin/bash

# Download quantized Whisper models (Q5_0 - best quality/size ratio)
# These are 40% smaller with minimal quality loss

MODELS_DIR="${HOME}/.local/share/whisperia/models"
mkdir -p "$MODELS_DIR"

echo "==================================="
echo "Baixando modelos Whisper quantizados"
echo "==================================="
echo ""

# Function to download model
download_model() {
    local model_name=$1
    local url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${model_name}"
    local output="${MODELS_DIR}/${model_name}"
    
    if [ -f "$output" ]; then
        local size=$(du -h "$output" | cut -f1)
        echo "✓ $model_name já existe (${size})"
        return
    fi
    
    echo "⬇️  Baixando $model_name..."
    wget -q --show-progress -O "$output" "$url"
    
    if [ $? -eq 0 ]; then
        local size=$(du -h "$output" | cut -f1)
        echo "✅ $model_name baixado (${size})"
    else
        echo "❌ Erro ao baixar $model_name"
        rm -f "$output"
    fi
    echo ""
}

# Download quantized models (Q5_0 = best quality/size ratio)
download_model "ggml-tiny-q5_0.bin"      # ~50MB   - Ultra rápido, básico
download_model "ggml-base-q5_0.bin"      # ~60MB   - Rápido, bom equilíbrio  
download_model "ggml-small-q5_0.bin"     # ~150MB  - Muito bom!
download_model "ggml-medium-q5_0.bin"    # ~450MB  - Excelente qualidade

echo "==================================="
echo "Modelos disponíveis:"
echo "==================================="
ls -lh ${MODELS_DIR}/ggml-*.bin 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
echo ""
echo "Para usar:"
echo "  ./target/release/whisperia --transcribe 5 --model-path ~/.local/share/whisperia/models/ggml-small-q5_0.bin"
