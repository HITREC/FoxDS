#!/bin/bash
set -e

# FoxDS Voice Translator Pro — Pure Rust Launcher

if [ -z "$FOXDS_CONFIG_PATH" ]; then
    mkdir -p "$HOME/.config/foxds-voice-translator"
    export FOXDS_CONFIG_PATH="$HOME/.config/foxds-voice-translator/config.json"
fi

# Stability for WebKitGTK in sandboxed Flatpak (prevents GPU / DMA-BUF hangs on Wayland & NVIDIA)
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1

# Inform user if virtual mic exists or can be created
if command -v pactl >/dev/null 2>&1; then
    if ! pactl list sources short 2>/dev/null | grep -qi "FoxDS"; then
        echo "[FoxDS] Примечание: Виртуальный микрофон FoxDS еще не создан."
        echo "[FoxDS] Для трансляции перевода в игры запустите: setup-virtual-mic-linux.sh"
    fi
fi

# Execute pure Rust native binary
exec /app/bin/foxds_voice_translator "$@"
