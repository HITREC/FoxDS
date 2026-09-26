// FoxDS Voice Translator Pro v3.0 - Full Real Control Engine

document.addEventListener('DOMContentLoaded', () => {
  initTabs();
  initSliders();
  initToggles();
  initDropdowns();
  initOverlayControls();
  initPreviewCards();
  initActionButtons();
  initIPC();
  startDualVUMeters();
  sendIPC('app_ready', {});
});

// --- Tab Switching ---
function initTabs() {
  const tabs = document.querySelectorAll('.nav-tab');
  const panes = document.querySelectorAll('.tab-pane');

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      const targetId = tab.getAttribute('data-tab');
      tabs.forEach(t => t.classList.remove('active'));
      panes.forEach(p => p.classList.remove('active'));

      tab.classList.add('active');
      const pane = document.getElementById(targetId);
      if (pane) pane.classList.add('active');
    });
  });
}

// --- Real Sliders Binding ---
const slidersDef = [
  {
    id: 'range-voice-speed',
    valId: 'val-voice-speed',
    format: (v) => `${v}%` + (v == 100 ? ' (Естественный)' : '')
  },
  {
    id: 'range-mic-gain',
    valId: 'val-mic-gain',
    format: (v) => `${v}%`
  },
  {
    id: 'range-tts-gain',
    valId: 'val-tts-gain',
    format: (v) => `${v}%`
  },
  {
    id: 'range-incoming-thresh',
    valId: 'val-incoming-thresh',
    format: (v) => `${v}`
  },
  {
    id: 'range-ai-confidence',
    valId: 'val-ai-confidence',
    format: (v) => `${v}%` + (v == 62 ? ' (Рекомендуется)' : '')
  },
  {
    id: 'range-ocr-delay',
    valId: 'val-ocr-delay',
    format: (v) => `${(v / 10).toFixed(1)} сек` + (v == 0 ? ' (Без паузы)' : '')
  },
  {
    id: 'range-ocr-duration',
    valId: 'val-ocr-duration',
    format: (v) => `${v} сек`
  },
  {
    id: 'range-overlay-font',
    valId: 'val-overlay-font',
    format: (v) => {
      const preview = document.querySelector('.hud-mini-body');
      if (preview) preview.style.fontSize = `${v}px`;
      return `${v} px`;
    }
  },
  {
    id: 'range-overlay-alpha',
    valId: 'val-overlay-alpha',
    format: (v) => {
      const preview = document.getElementById('hud-preview-box');
      if (preview) preview.style.opacity = (v / 100).toString();
      return `${v}%`;
    }
  },
  {
    id: 'range-overlay-border-w',
    valId: 'val-overlay-border-w',
    format: (v) => {
      const preview = document.getElementById('hud-preview-box');
      const val = parseInt(v, 10);
      if (preview) {
        if (val === 0) {
          preview.style.border = 'none';
          preview.style.boxShadow = '0 8px 24px rgba(0,0,0,0.85)';
        } else {
          const curCol = preview.style.borderColor || '#f25c05';
          preview.style.border = `${val}px solid ${curCol}`;
          preview.style.boxShadow = '';
        }
      }
      return val === 0 ? '0 px (Без рамки)' : `${val} px`;
    }
  },
  {
    id: 'range-overlay-width',
    valId: 'val-overlay-width',
    format: (v) => {
      const preview = document.getElementById('hud-preview-box');
      if (preview) {
        const pct = Math.min(100, Math.max(55, Math.round((v / 660) * 100)));
        preview.style.width = `${pct}%`;
      }
      return `${v} px`;
    }
  },
  {
    id: 'range-overlay-height',
    valId: 'val-overlay-height',
    format: (v) => {
      const preview = document.getElementById('hud-preview-box');
      if (preview) {
        preview.style.minHeight = `${Math.round(v * 0.8)}px`;
      }
      return `${v} px`;
    }
  }
];

function initSliders() {
  slidersDef.forEach(s => {
    const el = document.getElementById(s.id);
    const valEl = document.getElementById(s.valId);
    if (!el || !valEl) return;

    el.addEventListener('input', () => {
      valEl.textContent = s.format(el.value);
      sendIPC('slider_change', { id: s.id, value: parseFloat(el.value) });
    });

    el.addEventListener('change', () => {
      sendIPC('save_all_settings', {});
    });
  });
}

// --- Toggle Switches ---
function initToggles() {
  const toggleIds = [
    'chk-passthrough',
    'chk-radio-filter',
    'chk-play-self',
    'chk-incoming-subtitles',
    'chk-incoming-tts',
    'chk-filter-ru',
    'chk-ignore-mic',
    'chk-auto-match',
    'chk-ocr-enabled'
  ];

  toggleIds.forEach(id => {
    const chk = document.getElementById(id);
    if (chk) {
      chk.addEventListener('change', () => {
        sendIPC('toggle_change', { id: id, checked: chk.checked });
      });
    }
  });
}

// --- Dropdowns & Selection ---
function initDropdowns() {
  const voiceSel = document.getElementById('cfg-voice-select');
  if (voiceSel) {
    voiceSel.addEventListener('change', () => {
      sendIPC('voice_change', { voice: voiceSel.value });
    });
  }

  const inVoiceSel = document.getElementById('cfg-incoming-voice-select');
  if (inVoiceSel) {
    inVoiceSel.addEventListener('change', () => {
      sendIPC('incoming_voice_change', { voice: inVoiceSel.value });
    });
  }

  const micSel = document.getElementById('sel-mic-device');
  if (micSel) {
    micSel.addEventListener('change', () => {
      sendIPC('device_change', { type: 'mic', name: micSel.value });
    });
  }

  const spkSel = document.getElementById('sel-spk-device');
  if (spkSel) {
    spkSel.addEventListener('change', () => {
      sendIPC('device_change', { type: 'spk', name: spkSel.value });
    });
  }

  const cableSel = document.getElementById('sel-cable-device');
  if (cableSel) {
    cableSel.addEventListener('change', () => {
      sendIPC('device_change', { type: 'cable', name: cableSel.value });
    });
  }
}

// --- Overlay Controls & Presets ---
let isOverlayLocked = false;

function updateLockUI(locked, notifyBackend = true) {
  isOverlayLocked = !!locked;

  // 1. Header Lock Button
  const headerBtn = document.getElementById('btn-header-lock') || document.getElementById('badge-lock');
  const lockIcon = document.getElementById('badge-lock-icon');
  const lockText = document.getElementById('badge-lock-text');

  if (headerBtn) {
    if (locked) {
      if (lockIcon) lockIcon.textContent = '🔒';
      if (lockText) lockText.textContent = 'Оверлей зафиксирован [Разблокировать]';
      else headerBtn.textContent = '🔒 Оверлей зафиксирован [Разблокировать]';
      headerBtn.classList.add('locked');
      headerBtn.style.color = '#3fb950';
      headerBtn.style.borderColor = 'rgba(63, 185, 80, 0.6)';
      headerBtn.style.background = 'rgba(63, 185, 80, 0.15)';
    } else {
      if (lockIcon) lockIcon.textContent = '🔓';
      if (lockText) lockText.textContent = 'Оверлей подвижен [Зафиксировать]';
      else headerBtn.textContent = '🔓 Оверлей подвижен [Зафиксировать]';
      headerBtn.classList.remove('locked');
      headerBtn.style.color = '#f25c05';
      headerBtn.style.borderColor = 'rgba(242, 92, 5, 0.6)';
      headerBtn.style.background = 'rgba(242, 92, 5, 0.15)';
    }
  }

  // 2. Tab 3 Hero Card
  const heroIcon = document.getElementById('lock-hero-icon');
  const heroTitle = document.getElementById('lock-hero-title');
  const heroDesc = document.getElementById('lock-hero-desc');
  const heroBtnIcon = document.getElementById('btn-hero-icon');
  const heroBtnText = document.getElementById('btn-hero-text');
  const heroBtn = document.getElementById('btn-toggle-lock-hero');

  if (heroTitle) {
    if (locked) {
      if (heroIcon) heroIcon.textContent = '🔒';
      heroTitle.textContent = 'Оверлей зафиксирован («Клик-сквозь» в игру активен)';
      if (heroDesc) heroDesc.textContent = 'Оверлей закреплен на экране. Клики мыши проходят прямо в игру. Чтобы переместить оверлей в другое место, нажмите кнопку ниже.';
      if (heroBtnIcon) heroBtnIcon.textContent = '🔓';
      if (heroBtnText) heroBtnText.textContent = 'Разблокировать оверлей для перемещения';
      if (heroBtn) {
        heroBtn.style.background = 'linear-gradient(135deg, #238636, #2ea043)';
        heroBtn.style.borderColor = '#3fb950';
        heroBtn.style.boxShadow = '0 4px 14px rgba(46, 160, 67, 0.35)';
      }
    } else {
      if (heroIcon) heroIcon.textContent = '🔓';
      heroTitle.textContent = 'Оверлей разблокирован — можно перетаскивать мышкой';
      if (heroDesc) heroDesc.textContent = 'Зажмите левую кнопку мыши на полупрозрачной рамке оверлея на экране и перетащите в удобное место. Затем нажмите кнопку фиксации, чтобы закрепить.';
      if (heroBtnIcon) heroBtnIcon.textContent = '🔒';
      if (heroBtnText) heroBtnText.textContent = 'Зафиксировать оверлей на этом месте';
      if (heroBtn) {
        heroBtn.style.background = 'linear-gradient(135deg, #f25c05, #ff7b29)';
        heroBtn.style.borderColor = '#f25c05';
        heroBtn.style.boxShadow = '0 4px 14px rgba(242, 92, 5, 0.35)';
      }
    }
  }

  // 3. Tab 4 Presets Sub Lock Button
  const subBtn = document.getElementById('btn-toggle-lock-sub');
  const subIcon = document.getElementById('btn-sub-lock-icon');
  const subText = document.getElementById('btn-sub-lock-text');
  if (subBtn) {
    if (locked) {
      if (subIcon) subIcon.textContent = '🔓';
      if (subText) subText.textContent = 'Разблокировать оверлей для перемещения';
      subBtn.style.background = 'linear-gradient(135deg, #238636, #2ea043)';
      subBtn.style.borderColor = '#3fb950';
      subBtn.style.boxShadow = '0 4px 14px rgba(46, 160, 67, 0.35)';
    } else {
      if (subIcon) subIcon.textContent = '🔒';
      if (subText) subText.textContent = 'Зафиксировать оверлей на этом месте';
      subBtn.style.background = 'linear-gradient(135deg, #f25c05, #ff7b29)';
      subBtn.style.borderColor = '#f25c05';
      subBtn.style.boxShadow = '0 4px 14px rgba(242, 92, 5, 0.35)';
    }
  }

  // 4. Tab 4 Checkbox
  const chk = document.getElementById('chk-click-through');
  if (chk) chk.checked = locked;

  if (notifyBackend) {
    sendIPC('set_click_through', { locked: locked });
  }
}

window.onOverlayLockChanged = function(locked) {
  updateLockUI(locked, false);
};

// --- Overlay Controls & Presets ---
function initOverlayControls() {
  const toggleHandler = () => {
    updateLockUI(!isOverlayLocked, true);
  };

  const headerBtn = document.getElementById('btn-header-lock') || document.getElementById('badge-lock');
  if (headerBtn) headerBtn.addEventListener('click', toggleHandler);

  const heroBtn = document.getElementById('btn-toggle-lock-hero');
  if (heroBtn) heroBtn.addEventListener('click', toggleHandler);

  const subBtn = document.getElementById('btn-toggle-lock-sub');
  if (subBtn) subBtn.addEventListener('click', toggleHandler);

  const chkClickThrough = document.getElementById('chk-click-through');
  if (chkClickThrough) {
    chkClickThrough.addEventListener('change', (e) => {
      updateLockUI(e.target.checked, true);
    });
  }

  // Presets
  const presetBtns = document.querySelectorAll('.btn-preset');
  presetBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      presetBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const pos = btn.getAttribute('data-pos');
      sendIPC('set_overlay_preset', { preset: pos });
    });
  });

  // Color Palette
  const colorDots = document.querySelectorAll('.color-dot');
  const previewBox = document.getElementById('hud-preview-box');

  colorDots.forEach(dot => {
    dot.addEventListener('click', () => {
      colorDots.forEach(d => d.classList.remove('active'));
      dot.classList.add('active');
      const col = dot.getAttribute('data-color');
      if (previewBox) {
        previewBox.style.borderColor = col;
        const bwInput = document.getElementById('range-overlay-border-w');
        const curBw = bwInput ? parseInt(bwInput.value, 10) : 2;
        if (curBw > 0) {
          previewBox.style.border = `${curBw}px solid ${col}`;
        }
      }
      sendIPC('set_overlay_color', { color: col });
    });
  });

  // Text Color Palette
  const textColorDots = document.querySelectorAll('.text-color-dot');
  textColorDots.forEach(dot => {
    dot.addEventListener('click', () => {
      textColorDots.forEach(d => d.classList.remove('active'));
      dot.classList.add('active');
      const col = dot.getAttribute('data-color');
      const previewText = document.querySelector('.hud-mini-body');
      if (previewText) previewText.style.color = col;
      sendIPC('set_overlay_text_color', { color: col });
    });
  });
}

// --- Dynamic Preview Cards Hover ---
function initPreviewCards() {
  const voicePreviews = {
    speech_speed: {
      title: "Скорость произношения ИИ",
      body: "Регулировка темпа речи английского голоса. В динамичных боевых операциях Foxhole рекомендуется 100%-110% для моментальной передачи приказов."
    },
    mic_gain: {
      title: "Усиление микрофона",
      body: "Калибровка чувствительности вашего физического микрофона. Предотвращает клиппинг и делает тихую речь разборчивой."
    },
    tts_gain: {
      title: "Громкость озвучки перевода (ИИ)",
      body: "Уровень громкости, с которым переведенная английская реплика поступает в CABLE Input. Ваши союзники будут слышать вас четко на фоне звуков боя."
    },
    passthrough: {
      title: "Сквозной микрофон (Passthrough)",
      body: "Когда вы не зажимаете F4, ваш русский голос передается в штатном режиме без задержек. При зажатии F4 микрофон моментально глушится."
    },
    radio_filter: {
      title: "Военный радиофильтр Foxhole",
      body: "Тактический полосовой фильтр 300Hz-3400Hz с мягким насыщением. Голос звучит как через настоящую военную рацию Walkie-Talkie."
    },
    play_self: {
      title: "Самопрослушивание в наушниках",
      body: "Вы сразу слышите в наушниках то, что ИИ сказал в рацию вашим тиммейтам, контролируя точность перевода."
    }
  };

  const pTitle = document.getElementById('preview-heading-voice');
  const pBody = document.getElementById('preview-body-voice');

  document.querySelectorAll('#tab-voice .slider-card, #tab-voice .toggle-card').forEach(card => {
    card.addEventListener('mouseenter', () => {
      const type = card.getAttribute('data-preview-type');
      if (voicePreviews[type] && pTitle && pBody) {
        pTitle.textContent = voicePreviews[type].title;
        pBody.textContent = voicePreviews[type].body;
      }
    });
  });
}

// --- Action & Test Buttons ---
function initActionButtons() {
  // Test F4
  const btnTestF4 = document.getElementById('btn-test-f4');
  if (btnTestF4) {
    btnTestF4.addEventListener('click', () => {
      btnTestF4.innerHTML = '<span>🔊 Озвучивание теста в наушники...</span>';
      updateLiveStream("Вражеский танк на дороге, нужна помощь!", "Enemy tank on the road, need backup!");
      addHistoryItem("РАЦИЯ F4", "tag-f4", "Вражеский танк на дороге, нужна помощь!", "Enemy tank on the road, need backup!");

      setTimeout(() => {
        btnTestF4.innerHTML = '<span>▶️ Тест перевода F4 в наушники</span>';
      }, 2500);
      sendIPC('test_f4', {});
    });
  }

  // Test Incoming
  const btnTestInc = document.getElementById('btn-test-incoming');
  if (btnTestInc) {
    btnTestInc.addEventListener('click', () => {
      btnTestInc.innerHTML = '<span>🎧 Тест входящего войса...</span>';
      updateLiveStream("Watch out, sniper in the clock tower!", "Осторожно, снайпер на часовой башне!");
      addHistoryItem("ТИММЕЙТ", "tag-spk", "Watch out, sniper in the clock tower!", "Осторожно, снайпер на часовой башне!");

      setTimeout(() => {
        btnTestInc.innerHTML = '<span>🎧 Проверить перевод речи тиммейта (EN -> RU)</span>';
      }, 2500);
      sendIPC('test_incoming', {});
    });
  }

  // Trigger Snipping
  const btnSnip = document.getElementById('btn-trigger-snipping');
  if (btnSnip) {
    btnSnip.addEventListener('click', () => {
      sendIPC('trigger_ocr', {});
    });
  }

  // Save Hotkeys
  document.getElementById('btn-save-voice-hk')?.addEventListener('click', () => {
    const val = document.getElementById('cfg-voice-hotkey')?.value || 'F4';
    document.getElementById('badge-hotkey').textContent = `🎙️ Рация: [${val.toUpperCase()}]`;
    sendIPC('save_voice_hotkey', { key: val });
  });

  document.getElementById('btn-save-ocr-hk')?.addEventListener('click', () => {
    const val = document.getElementById('cfg-ocr-hotkey')?.value || 'ALT+Q';
    document.getElementById('badge-ocr').textContent = `✂️ Экран: [${val.toUpperCase()}]`;
    sendIPC('save_ocr_hotkey', { key: val });
  });

  // Save Overlay
  document.getElementById('btn-save-overlay-cfg')?.addEventListener('click', () => {
    const btn = document.getElementById('btn-save-overlay-cfg');
    btn.innerHTML = '<span>✓ Настройки сохранены!</span>';
    setTimeout(() => { btn.innerHTML = '<span>Применить настройки оверлея</span>'; }, 1500);
    sendIPC('save_overlay_settings', {});
  });

  // Clear History
  document.getElementById('btn-clear-history')?.addEventListener('click', () => {
    const list = document.getElementById('history-items-list');
    if (list) list.innerHTML = '';
  });
}

// Cached DOM references for high-frequency updates
let domElements = {
  liveOrig: null,
  liveTrans: null,
  micFill: null,
  spkFill: null,
  micStatus: null,
  engineStatus: null,
  historyList: null
};

function getDomEl(key, selector, isQuery = false) {
  if (!domElements[key]) {
    domElements[key] = isQuery ? document.querySelector(selector) : document.getElementById(selector);
  }
  return domElements[key];
}

function updateLiveStream(orig, trans) {
  const origEl = getDomEl('liveOrig', '#live-orig span', true);
  const transEl = getDomEl('liveTrans', '#live-trans span', true);
  if (origEl) origEl.textContent = `"${orig}"`;
  if (transEl) transEl.textContent = `"${trans}"`;
}

function addHistoryItem(tagLabel, tagClass, orig, trans) {
  const list = getDomEl('historyList', 'history-items-list');
  if (!list) return;

  const now = new Date();
  const timeStr = now.toTimeString().split(' ')[0];

  const item = document.createElement('div');
  item.className = 'history-item';
  item.innerHTML = `
    <span class="hist-time">${timeStr}</span>
    <span class="hist-tag ${tagClass}">${tagLabel}</span>
    <div class="hist-content">
      <div class="hist-ru">${orig}</div>
      <div class="hist-en">${trans}</div>
    </div>
  `;
  list.insertBefore(item, list.firstChild);

  // Cap history items to 50 to prevent layout thrashing and DOM bloat
  while (list.children.length > 50) {
    list.removeChild(list.lastChild);
  }
}

// --- Live Dual VU Meters ---
let lastRealVUTime = 0;
let lastMicStatusText = '';
let lastEngineStatusText = '';

function startDualVUMeters() {
  const micFill = getDomEl('micFill', 'vu-mic-fill');
  const spkFill = getDomEl('spkFill', 'vu-spk-fill');
  const micStatus = getDomEl('micStatus', 'vu-mic-status');

  setInterval(() => {
    // Only use fallback simulation if no real VU events have been received recently
    if (Date.now() - lastRealVUTime > 1500) {
      const t = Date.now() / 400;
      const wave = Math.sin(t);
      const micVal = Math.max(3, Math.min(30, 8 + wave * 6));
      const spkVal = Math.max(2, Math.min(25, 6 + Math.cos(t * 0.8) * 5));

      if (micFill) micFill.style.width = `${micVal}%`;
      if (spkFill) spkFill.style.width = `${spkVal}%`;
      if (micStatus && lastMicStatusText !== 'ТИШИНА') {
        micStatus.textContent = 'ТИШИНА';
        micStatus.style.color = '#8b8b9e';
        lastMicStatusText = 'ТИШИНА';
      }
    }
  }, 100);
}

window.updateRealVU = function(micPct, spkPct, isMicActive) {
  lastRealVUTime = Date.now();
  const micFill = getDomEl('micFill', 'vu-mic-fill');
  const spkFill = getDomEl('spkFill', 'vu-spk-fill');
  const micStatus = getDomEl('micStatus', 'vu-mic-status');

  const mVal = Math.min(100, Math.max(0, micPct));
  const sVal = Math.min(100, Math.max(0, spkPct));

  if (micFill) micFill.style.width = `${mVal}%`;
  if (spkFill) spkFill.style.width = `${sVal}%`;

  if (micStatus) {
    const isSpeaking = (isMicActive || mVal > 20);
    const newText = isSpeaking ? 'РЕЧЬ' : 'ТИШИНА';
    if (lastMicStatusText !== newText) {
      micStatus.textContent = newText;
      micStatus.style.color = isSpeaking ? '#3ecf8e' : '#8b8b9e';
      lastMicStatusText = newText;
    }
  }
};

window.updateEngineStatus = function(statusText, badgeColor) {
  if (lastEngineStatusText === statusText) return;
  lastEngineStatusText = statusText;
  const badge = getDomEl('engineStatus', 'live-engine-status');
  if (badge) {
    badge.textContent = statusText;
    if (badgeColor) {
      badge.style.color = badgeColor;
    }
  }
};

// --- IPC Bridge to Rust Backend ---
function initIPC() {
  document.getElementById('btn-min')?.addEventListener('click', () => sendIPC('minimize', {}));
  document.getElementById('btn-max')?.addEventListener('click', () => sendIPC('maximize', {}));
  document.getElementById('btn-close')?.addEventListener('click', () => sendIPC('close', {}));

  const titlebar = document.querySelector('.titlebar');
  if (titlebar) {
    titlebar.addEventListener('mousedown', (e) => {
      if (e.target.closest('.win-btn') || e.target.closest('.status-pill') || e.target.closest('button')) return;
      sendIPC('drag_window', {});
    });
    titlebar.addEventListener('dblclick', (e) => {
      if (e.target.closest('.win-btn') || e.target.closest('.status-pill') || e.target.closest('button')) return;
      sendIPC('maximize', {});
    });
  }
}

function sendIPC(command, payload) {
  const msg = JSON.stringify({ cmd: command, ...payload });
  if (window.ipc) {
    window.ipc.postMessage(msg);
  } else {
    console.log('[IPC Sent]:', msg);
  }
}

// Global hook for speech events from Rust backend
window.onRustSpeechEvent = function(eventType, origText, transText) {
  updateLiveStream(origText, transText);
  if (eventType === 'outgoing') {
    addHistoryItem('РАЦИЯ F4', 'tag-f4', `RU: "${origText}"`, `EN: "${transText}"`);
  } else if (eventType === 'incoming') {
    addHistoryItem('ТИММЕЙТ', 'tag-spk', `EN: "${origText}"`, `RU: "${transText}"`);
  } else if (eventType === 'ocr') {
    addHistoryItem('OCR ЭКРАН', 'tag-ocr', `EN: "${origText}"`, `RU: "${transText}"`);
    const o1 = document.getElementById('ocr-last-orig');
    const o2 = document.getElementById('ocr-last-trans');
    if (o1) o1.textContent = `"${origText}"`;
    if (o2) o2.textContent = `"${transText}"`;
  }
};

// Global hook for initializing UI controls from config
window.initFromConfig = function(cfg) {
  if (!cfg) return;

  // Sliders
  const mapSliders = {
    'range-voice-speed': cfg.speech_speed ?? 100,
    'range-mic-gain': Math.round((cfg.mic_gain ?? 1.0) * 100),
    'range-tts-gain': Math.round((cfg.tts_gain ?? 1.2) * 100),
    'range-incoming-thresh': Math.round(cfg.rms_threshold ?? 35),
    'range-ai-confidence': Math.round((cfg.min_confidence ?? 0.62) * 100),
    'range-ocr-delay': Math.round((cfg.ocr_appear_delay ?? 0.6) * 10),
    'range-ocr-duration': cfg.ocr_display_duration ?? 12,
    'range-overlay-font': cfg.overlay_font_size ?? 12,
    'range-overlay-alpha': Math.round((cfg.overlay_alpha ?? 0.92) * 100),
    'range-overlay-border-w': cfg.overlay_border_width ?? 2,
    'range-overlay-width': cfg.overlay_w ?? 660,
    'range-overlay-height': cfg.overlay_h ?? 95
  };

  slidersDef.forEach(s => {
    if (mapSliders[s.id] !== undefined) {
      const el = document.getElementById(s.id);
      const valEl = document.getElementById(s.valId);
      if (el) {
        el.value = mapSliders[s.id];
        if (valEl) valEl.textContent = s.format(el.value);
      }
    }
  });

  // Toggles
  const setChk = (id, val) => {
    const el = document.getElementById(id);
    if (el && val !== undefined) el.checked = !!val;
  };
  setChk('chk-passthrough', cfg.passthrough_enabled);
  setChk('chk-radio-filter', cfg.radio_effect);
  setChk('chk-play-self', cfg.play_self_audio);
  setChk('chk-incoming-subtitles', cfg.incoming_enabled);
  setChk('chk-incoming-tts', cfg.incoming_tts_enabled);
  setChk('chk-filter-ru', cfg.filter_russian);
  setChk('chk-ignore-mic', cfg.ignore_own_mic);
  setChk('chk-auto-match', cfg.auto_volume_match);
  setChk('chk-ocr-enabled', cfg.ocr_enabled);
  setChk('chk-click-through', cfg.overlay_locked);

  if (cfg.incoming_voice) {
    const ivSel = document.getElementById('cfg-incoming-voice-select');
    if (ivSel) ivSel.value = cfg.incoming_voice;
  }

  // Lock UI state
  updateLockUI(!!cfg.overlay_locked, false);

  // Voice Select
  if (cfg.voice) {
    const vSel = document.getElementById('cfg-voice-select');
    if (vSel) vSel.value = cfg.voice;
  }

  // Hotkeys
  if (cfg.hotkey) {
    const hInput = document.getElementById('cfg-voice-hotkey');
    if (hInput) hInput.value = cfg.hotkey;
    const hBadge = document.getElementById('badge-hotkey');
    if (hBadge) hBadge.textContent = `🎙️ Рация: [${cfg.hotkey.toUpperCase()}]`;
  }
  if (cfg.ocr_hotkey) {
    const oInput = document.getElementById('cfg-ocr-hotkey');
    if (oInput) oInput.value = cfg.ocr_hotkey;
    const oBadge = document.getElementById('badge-ocr');
    if (oBadge) oBadge.textContent = `✂️ Экран: [${cfg.ocr_hotkey.toUpperCase()}]`;
  }

  // Overlay Border Color
  if (cfg.overlay_border_color) {
    const previewBox = document.getElementById('hud-preview-box');
    if (previewBox) {
      previewBox.style.borderColor = cfg.overlay_border_color;
      if (cfg.overlay_border_width && cfg.overlay_border_width > 0) {
        previewBox.style.border = `${cfg.overlay_border_width}px solid ${cfg.overlay_border_color}`;
      } else if (cfg.overlay_border_width === 0) {
        previewBox.style.border = 'none';
      }
    }
    document.querySelectorAll('.color-dot').forEach(dot => {
      dot.classList.toggle('active', dot.getAttribute('data-color') === cfg.overlay_border_color);
    });
  }

  // Overlay Text Color
  if (cfg.overlay_text_color) {
    const previewText = document.querySelector('.hud-mini-body');
    if (previewText) previewText.style.color = cfg.overlay_text_color;
    document.querySelectorAll('.text-color-dot').forEach(dot => {
      dot.classList.toggle('active', dot.getAttribute('data-color') === cfg.overlay_text_color);
    });
  }

  // Overlay Preset
  if (cfg.overlay_preset) {
    document.querySelectorAll('.btn-preset').forEach(btn => {
      btn.classList.toggle('active', btn.getAttribute('data-pos') === cfg.overlay_preset);
    });
  }
};

// Global hook for populating device dropdowns
window.setAudioDevices = function(data) {
  if (!data) return;
  const inputs = data.inputs || [];
  const outputs = data.outputs || [];

  const fillSelect = (id, list, defaultLabel) => {
    const sel = document.getElementById(id);
    if (!sel) return;
    const curVal = sel.value;
    sel.innerHTML = '';
    const defOpt = document.createElement('option');
    defOpt.value = 'Default';
    defOpt.textContent = defaultLabel;
    sel.appendChild(defOpt);

    list.forEach(d => {
      const opt = document.createElement('option');
      opt.value = d.name;
      opt.textContent = d.name;
      sel.appendChild(opt);
    });

    if (curVal) sel.value = curVal;
  };

  fillSelect('sel-mic-device', inputs, 'По умолчанию (Системный)');
  fillSelect('sel-spk-device', outputs, 'По умолчанию (Наушники)');
  fillSelect('sel-cable-device', outputs, 'CABLE Input (VB-Audio Virtual Cable)');
};
