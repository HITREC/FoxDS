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

## 📦 Скачивание и передача другу

Другу **не нужно ничего компилировать** — приложение упаковано в **один готовый автономный файл `foxds-voice-translator.flatpak`**.

### 🔗 Прямая ссылка для друга:
👉 **[Скачать foxds-voice-translator.flatpak](https://github.com/HITREC/FoxDS/releases/latest/download/foxds-voice-translator.flatpak)**

Или страница последнего релиза: **[https://github.com/HITREC/FoxDS/releases/latest](https://github.com/HITREC/FoxDS/releases/latest)**

---

### 🛠️ Сборка из исходников (для разработчиков)

- **Автоматическая сборка в GitHub Actions:** При каждом коммите в `main` сервер GitHub сам собирает приложение и обновляет файл по ссылке выше.
- **Локальная сборка на Linux или WSL2 в одну команду:**
  ```bash
  chmod +x packaging/flatpak/build-flatpak.sh
  ./packaging/flatpak/build-flatpak.sh
  ```


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
