#!/bin/bash
# ==============================================================================
# FoxDS Voice Translator Pro — Настройка виртуального микрофона для Linux
# Работает со всеми современными дистрибутивами (PipeWire и PulseAudio)
# ==============================================================================

SINK_NAME="FoxDS_VirtualMic"
SOURCE_NAME="FoxDS_Microphone"

check_pactl() {
    if ! command -v pactl >/dev/null 2>&1; then
        echo "❌ Ошибка: Утилита 'pactl' не найдена!"
        echo "Установите её: sudo apt install pulseaudio-utils (Ubuntu/Debian) или sudo dnf install pulseaudio-utils (Fedora)"
        exit 1
    fi
}

start_virtual_mic() {
    check_pactl
    echo "=========================================================="
    echo "  FoxDS Pro: Создание виртуального микрофона в Linux..."
    echo "=========================================================="

    # Проверка, создан ли уже
    if pactl list short sinks | grep -q "$SINK_NAME"; then
        echo "ℹ️  Виртуальный кабель '$SINK_NAME' уже запущен."
    else
        pactl load-module module-null-sink \
            sink_name="$SINK_NAME" \
            sink_properties=device.description="FoxDS_Virtual_Cable" >/dev/null
        echo "✅ Создан виртуальный аудиокабель: FoxDS_Virtual_Cable"
    fi

    if pactl list short sources | grep -q "$SOURCE_NAME"; then
        echo "ℹ️  Виртуальный микрофон '$SOURCE_NAME' уже активен."
    else
        pactl load-module module-remap-source \
            master="${SINK_NAME}.monitor" \
            source_name="$SOURCE_NAME" \
            source_properties=device.description="FoxDS_Microphone" >/dev/null
        echo "✅ Создан виртуальный микрофон: FoxDS_Microphone"
    fi

    echo ""
    echo "🎉 УСПЕШНО НАСТРОЕНО!"
    echo "----------------------------------------------------------"
    echo "1. В FoxDS Voice Translator:"
    echo "   В поле 'Куда вещать' выберите 'FoxDS_Virtual_Cable' (или FoxDS_VirtualMic)"
    echo "2. В игре (Foxhole, CS2, Squad) или в Discord:"
    echo "   В настройках звука выберите микрофон: 'FoxDS_Microphone'"
    echo "----------------------------------------------------------"
}

stop_virtual_mic() {
    check_pactl
    echo "Удаление виртуальных аудиоустройств FoxDS..."
    
    # Поиск и выгрузка модулей
    pactl list short modules | grep "$SINK_NAME" | while read -r line; do
        mod_id=$(echo "$line" | awk '{print $1}')
        if [ -n "$mod_id" ]; then
            pactl unload-module "$mod_id" 2>/dev/null && echo "Выгружен модуль $mod_id"
        fi
    done

    pactl list short modules | grep "$SOURCE_NAME" | while read -r line; do
        mod_id=$(echo "$line" | awk '{print $1}')
        if [ -n "$mod_id" ]; then
            pactl unload-module "$mod_id" 2>/dev/null && echo "Выгружен модуль $mod_id"
        fi
    done

    echo "✅ Виртуальный микрофон остановлен и удален."
}

status_virtual_mic() {
    check_pactl
    echo "Статус устройств FoxDS:"
    echo "--- Sinks (Выходы для перевода) ---"
    pactl list short sinks | grep "$SINK_NAME" || echo "Не запущен"
    echo "--- Sources (Микрофоны для игр) ---"
    pactl list short sources | grep -E "$SOURCE_NAME|$SINK_NAME" || echo "Не запущен"
}

case "$1" in
    start|"")
        start_virtual_mic
        ;;
    stop)
        stop_virtual_mic
        ;;
    status)
        status_virtual_mic
        ;;
    restart)
        stop_virtual_mic
        sleep 1
        start_virtual_mic
        ;;
    *)
        echo "Использование: $0 [start|stop|status|restart]"
        exit 1
        ;;
esac
