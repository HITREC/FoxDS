# 🦀 FoxDS Voice Translator Pro — 100% Pure Rust Linux Guide (Flatpak)

**FoxDS Voice Translator Pro** теперь написан **на 100% чистом Rust (Pure Rust)**!
Все компоненты переведены на нативные высокопроизводительные крейты:
- **Интерфейс (GUI):** Tao + Wry (WebKitGTK на Linux, WebView2 на Windows);
- **Распознавание речи (STT):** Google Chromium STT API + нативное сжатие `flacenc`;
- **Нейросетевой перевод:** Google Translate Dict API через `reqwest` с постоянным пулом соединений;
- **Нейросетевой голос (TTS):** Microsoft Edge-TTS (Andrew, Christopher, Brian и др.) напрямую через WebSocket `tungstenite` с протоколом Sec-MS-GEC;
- **Аудио-тракт и микшер:** `rodio` + `cpal` (прямой захват микрофона, расчет RMS в реальном времени, вывод в виртуальный кабель и наушники).

**Python больше не используется вообще!** Нет никаких скрытых скриптов, нет `pip`, нет интерпретатора. Только один чистый нативный бинарник.

---

## 📦 Сборка и отправка другу через Flatpak

Благодаря [Flatpak](https://flatpak.org/), приложение упаковывается в **один единственный файл `foxds-voice-translator.flatpak`**.

### Способ 1. Автоматическая сборка в GitHub Actions (без Linux)
1. Отправьте изменения в ваш GitHub-репозиторий (`git push`).
2. В репозитории откройте вкладку **Actions** → выберите **Build Linux Flatpak Bundle** → **Run workflow**.
3. Скачайте готовый артефакт **`foxds-voice-translator-flatpak`**.
4. Отправьте файл другу через Telegram, Discord или флешку.

### Способ 2. Сборка на Linux или WSL2 в одну команду
```bash
chmod +x packaging/flatpak/build-flatpak.sh
./packaging/flatpak/build-flatpak.sh
```
Скрипт скомпилирует Rust-код и создаст файл `foxds-voice-translator.flatpak`.

---

## 📥 Инструкция для друга

1. **Установка пакета в один клик:**
   Дважды кликните по файлу `foxds-voice-translator.flatpak` (или выполните в терминале):
   ```bash
   flatpak install --user foxds-voice-translator.flatpak
   ```

2. **Включение виртуального микрофона для игр и Discord:**
   ```bash
   flatpak run --command=setup-virtual-mic-linux.sh com.foxds.VoiceTranslator start
   ```

3. **Запуск:**
   Запустите из меню приложений **«FoxDS Voice Translator Pro»** или командой:
   ```bash
   flatpak run com.foxds.VoiceTranslator
   ```

4. **Настройка в игре (Foxhole, CS2, Squad) или Discord:**
   - В FoxDS: В поле *«Куда вещать»* выберите **`FoxDS_Virtual_Cable`**.
   - В игре / Discord: В качестве микрофона выберите **`FoxDS_Microphone`**.

Зажимайте **F4**, говорите по-русски — программа мгновенно переведет и озвучит ваш голос нейросетью на чистом английском напрямую в игровой войс-чат!
