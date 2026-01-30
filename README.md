# whisperia

ferramenta de transcrição de voz de alta performance para linux (especialmente tiling window managers como i3, sway, dwm).

## features

- transcrição de voz local usando whisper
- compatibilidade com qualquer modelo huggingface
- detecção automática de hardware e compatibilidade
- cli funcional
- muito performático - escrito em rust

## instalação

### pré-requisitos (arch linux)

```bash
sudo pacman -s --needed base-devel cmake
```

### build

```bash
cd whisperia
cargo build --release
```

o binário estará em `target/release/whisperia`

## como usar

### verificar informações do sistema:
```bash
./target/release/whisperia
```

### verificar compatibilidade de hardware:
```bash
./target/release/whisperia --check-hardware
```

### listar modelos disponíveis:
```bash
./target/release/whisperia --list-models
```

### verificar se um modelo huggingface vai funcionar:
```bash
./target/release/whisperia --check-model openai/whisper-base
./target/release/whisperia --check-model openai/whisper-large-v3
```

### transcrever audio (modo cli):
```bash
./target/release/whisperia --transcribe 5 --model-path ~/.local/share/whisperia/models/ggml-base.bin
```

## configuração

o arquivo de configuração é criado automaticamente em:
- linux: `~/.config/whisperia/config.toml`

### exemplo de configuração:
```toml
shortcut = "super+shift+t"
language = "pt"
auto_paste = true

[model]
model_type = "local"
local_model = "base"
use_quantized = true

[api]
provider = "openai"
api_key = ""
model = "whisper-1"

[ui]
theme = "glass"
opacity = 0.9
position = "cursor"
auto_hide_delay = 3000
```

## modelos disponíveis

| modelo | tamanho | ram | performance |
|--------|---------|-----|-------------|
| tiny | 39m | 1gb | rápido |
| base | 74m | 2gb | muito rápido |
| small | 244m | 3gb | rápido |
| medium | 769m | 6gb | moderado |
| large | 1550m | 10gb | lento |

## arquitetura

```
whisperia/
├── src/
│   ├── main.rs           # entry point e cli
│   ├── config/           # gerenciamento de configuração
│   └── hardware/         # detecção de hardware
├── cargo.toml
└── readme.md
```

## roadmap

- [x] detecção de hardware
- [x] verificação de compatibilidade huggingface
- [x] cli funcional
- [x] gravação de áudio
- [x] transcrição whisper local
- [ ] integração com apis (openai, openrouter)
- [ ] system tray
- [ ] hotkeys globais
- [ ] overlay flutuante
- [ ] simulação de teclado
- [ ] frontend/gui (tauri)

## licença

mit license
