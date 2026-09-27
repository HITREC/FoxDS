#!/bin/bash
# ==============================================================================
# FoxDS Voice Translator Pro — Сборка единого .flatpak пакета для Linux
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

echo "===================================================================="
echo "      FoxDS Voice Translator Pro — Сборка Flatpak Bundle            "
echo "===================================================================="

# 1. Проверка наличия flatpak и flatpak-builder
if ! command -v flatpak >/dev/null 2>&1; then
    echo "❌ Ошибка: flatpak не установлен!"
    echo "Установите его:"
    echo "  Ubuntu/Debian: sudo apt update && sudo apt install -y flatpak flatpak-builder"
    echo "  Fedora:        sudo dnf install -y flatpak flatpak-builder"
    echo "  Arch/Manjaro:  sudo pacman -S --needed flatpak flatpak-builder"
    exit 1
fi

if ! command -v flatpak-builder >/dev/null 2>&1; then
    echo "❌ Ошибка: flatpak-builder не установлен!"
    echo "  Ubuntu/Debian: sudo apt install -y flatpak-builder"
    echo "  Fedora:        sudo dnf install -y flatpak-builder"
    echo "  Arch/Manjaro:  sudo pacman -S --needed flatpak-builder"
    exit 1
fi

# 2. Подключение Flathub репозитория
echo "📦 Проверка подключения Flathub..."
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

# 3. Установка GNOME 47 Platform & Rust SDK
echo "⬇️  Проверка зависимостей Flatpak SDK (GNOME 47 + Rust)..."
flatpak install --user -y --noninteractive flathub \
    org.gnome.Platform//47 \
    org.gnome.Sdk//47 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08 || true

# 4. Сборка проекта
echo "🔨 Сборка приложения через flatpak-builder..."
rm -rf build-dir .flatpak-builder
flatpak-builder --disable-rofiles-fuse --force-clean --user \
    --install-deps-from=flathub \
    --repo=foxds-repo \
    build-dir \
    packaging/flatpak/com.foxds.VoiceTranslator.yml

# 5. Экспорт в автономный файл-бандл .flatpak
echo "📦 Упаковка в автономный файл 'foxds-voice-translator.flatpak'..."
flatpak build-bundle foxds-repo foxds-voice-translator.flatpak com.foxds.VoiceTranslator

echo ""
echo "===================================================================="
echo "🎉 СБОРКА УСПЕШНО ЗАВЕРШЕНА!"
echo "===================================================================="
echo "Готовый файл для отправки другу:"
echo "👉 $(pwd)/foxds-voice-translator.flatpak"
echo ""
echo "Инструкция для друга на Linux:"
echo "  1. Установить бандл в один клик или командой:"
echo "     flatpak install --user foxds-voice-translator.flatpak"
echo "  2. Запустить:"
echo "     flatpak run com.foxds.VoiceTranslator"
echo "     (или через иконку в меню приложений 'FoxDS Voice Translator Pro')"
echo "===================================================================="
