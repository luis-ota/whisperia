#!/bin/bash

# download whisper models for testing

MODELS_DIR="${HOME}/.local/share/whisperia/models"
mkdir -p "$MODELS_DIR"

echo "downloading whisper models..."
echo "models will be saved to: $MODELS_DIR"
echo ""

# funcao para baixar modelo
download_model() {
    local model_name=$1
    local url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${model_name}.bin"
    local output="${MODELS_DIR}/ggml-${model_name}.bin"
    
    if [ -f "$output" ]; then
        echo "model $model_name already exists, skipping..."
        return
    fi
    
    echo "downloading $model_name..."
    wget -q --show-progress -O "$output" "$url"
    
    if [ $? -eq 0 ]; then
        echo "[ok] $model_name downloaded successfully"
    else
        echo "[x] failed to download $model_name"
        rm -f "$output"
    fi
}

# baixar modelos pequenos para teste
download_model "tiny"
download_model "base"

echo ""
echo "done! models available in: $MODELS_DIR"
echo ""
echo "to test transcription, run:"
echo "  ./target/release/whisperia --transcribe 5 --model-path ${MODELS_DIR}/ggml-base.bin"
