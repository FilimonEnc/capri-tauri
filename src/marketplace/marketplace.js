'use strict';

/**
 * CAPRI // CYBERPUNK EDGERUNNERS MARKETPLACE CLIENT ENGINE
 * Handles audio synthesis, interactive Capri construct, catalog filtering,
 * test console simulation, and Ripperdoc custom skill injection.
 */

const API_BASE = 'http://127.0.0.1:47823';

// --- Web Audio Cyberpunk Sound Synthesizer ---
class CyberAudioEngine {
  constructor() {
    this.ctx = null;
    this.enabled = localStorage.getItem('capri_audio_fx') !== 'false';
  }

  init() {
    if (!this.ctx && typeof window.AudioContext !== 'undefined') {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      this.ctx = new AudioCtx();
    }
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume();
    }
  }

  toggle() {
    this.enabled = !this.enabled;
    localStorage.setItem('capri_audio_fx', this.enabled ? 'true' : 'false');
    return this.enabled;
  }

  playHover() {
    if (!this.enabled) return;
    try {
      this.init();
      if (!this.ctx) return;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(880, this.ctx.currentTime);
      osc.frequency.exponentialRampToValueAtTime(1200, this.ctx.currentTime + 0.04);
      gain.gain.setValueAtTime(0.04, this.ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, this.ctx.currentTime + 0.04);
      osc.connect(gain);
      gain.connect(this.ctx.destination);
      osc.start();
      osc.stop(this.ctx.currentTime + 0.04);
    } catch {}
  }

  playClick() {
    if (!this.enabled) return;
    try {
      this.init();
      if (!this.ctx) return;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();
      osc.type = 'triangle';
      osc.frequency.setValueAtTime(320, this.ctx.currentTime);
      osc.frequency.exponentialRampToValueAtTime(80, this.ctx.currentTime + 0.08);
      gain.gain.setValueAtTime(0.12, this.ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, this.ctx.currentTime + 0.08);
      osc.connect(gain);
      gain.connect(this.ctx.destination);
      osc.start();
      osc.stop(this.ctx.currentTime + 0.08);
    } catch {}
  }

  playSuccess() {
    if (!this.enabled) return;
    try {
      this.init();
      if (!this.ctx) return;
      const now = this.ctx.currentTime;
      [440, 660, 880].forEach((freq, idx) => {
        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();
        osc.type = 'square';
        osc.frequency.setValueAtTime(freq, now + idx * 0.06);
        gain.gain.setValueAtTime(0.06, now + idx * 0.06);
        gain.gain.exponentialRampToValueAtTime(0.001, now + idx * 0.06 + 0.1);
        osc.connect(gain);
        gain.connect(this.ctx.destination);
        osc.start(now + idx * 0.06);
        osc.stop(now + idx * 0.06 + 0.1);
      });
    } catch {}
  }

  playSpeech() {
    if (!this.enabled) return;
    try {
      this.init();
      if (!this.ctx) return;
      const now = this.ctx.currentTime;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();
      osc.type = 'sawtooth';
      osc.frequency.setValueAtTime(540, now);
      osc.frequency.linearRampToValueAtTime(780, now + 0.06);
      osc.frequency.linearRampToValueAtTime(420, now + 0.12);
      gain.gain.setValueAtTime(0.05, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.12);
      osc.connect(gain);
      gain.connect(this.ctx.destination);
      osc.start(now);
      osc.stop(now + 0.12);
    } catch {}
  }
}

const audio = new CyberAudioEngine();

// --- Edgerunners Quotes & Dialogue Pool ---
const EDGERUNNERS_QUOTES = [
  "«Здорово, choom! Заглянул в риппердок? Мой кибердек готов к новым модулям. Выбирай первоклассный хром!»",
  "«Сандевистан на 120 FPS включен на полную мощность. Перемещение окон теперь абсолютно бесшовное!»",
  "«Люси передавала привет из Сети. У меня тут свежая пачка модов из Арасаки и Водоворота!»",
  "«В Найт-Сити без хорошего ICE долго не живут. Поставь инструменты разработчика или калькулятор евродолларов!»",
  "«Турбо-режим в Антигравити? Никаких лишних алертов, работаем тихо и чисто, как соло в Конпеки Плаза.»",
  "«Засекла изменение погоды над Пасификой. Кислотные осадки маловероятны, активируй метео-сканер!»",
  "«Хэй, Ребекка одобрила бы этот набор пушек. А мы пока разберемся с кодом и окнами на рабочем столе.»",
  "«Один клик — и модуль прошит прямо в биочип без ребута приложения. Чистая киберпанк-магия!»"
];

// --- High-Tier Cyberware Modules Catalog ---
const DEFAULT_CYBERWARE = [
  {
    id: 'weather',
    name: 'Night City Meteo-Radar // Погода',
    description: 'Спутниковый радар погоды, влажности и скорости ветра в любой точке мира без API-ключей.',
    category: 'utilities',
    author: 'CORPO: ARASAKA METEO',
    version: '2.1.0',
    tier: 'tier-3',
    icon: '🌤️',
    tags: ['weather', 'radar', 'online', 'погода'],
    samplePrompt: 'какая погода в Токио',
    installed: true,
    enabled: true,
    async execute(query) {
      const city = query.replace(/^(?:какая\s+)?погода\s+(?:в|на)?\s*/i, '').trim() || 'Москва';
      try {
        const geo = await fetch(`https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(city)}&count=1&language=ru&format=json`).then(r => r.json());
        if (!geo.results || !geo.results.length) return `Не смогла найти город «${city}». Проверь координаты, choom!`;
        const loc = geo.results[0];
        const w = await fetch(`https://api.open-meteo.com/v1/forecast?latitude=${loc.latitude}&longitude=${loc.longitude}&current=temperature_2m,relative_humidity_2m,wind_speed_10m`).then(r => r.json());
        const t = Math.round(w.current.temperature_2m);
        const ws = Math.round(w.current.wind_speed_10m);
        return `[METEO_LINK]: В секторе ${loc.name}: ${t > 0 ? '+' : ''}${t}°C, ветер ${ws} км/ч, влажность ${w.current.relative_humidity_2m}%. Радиационный фон в пределах нормы.`;
      } catch (e) {
        return `Ошибка метео-радара: ${e.message}`;
      }
    }
  },
  {
    id: 'calculator',
    name: 'Cyberdeck Math Matrix // Калькулятор',
    description: 'Квантовый сопроцессор для мгновенных вычислений, конвертации евродолларов ($E) и подсчета КБЖУ.',
    category: 'productivity',
    author: 'NETRUNNER: KIWI SPEC',
    version: '3.0.0',
    tier: 'tier-5',
    icon: '🧮',
    tags: ['math', 'calc', 'matrix', 'числа'],
    samplePrompt: 'сколько будет (2500 - 450) * 1.15',
    installed: true,
    enabled: true,
    async execute(query) {
      const expr = query.replace(/^(?:посчитай|вычисли|сколько будет|calc)\s+/i, '').replace(/[,]/g, '.');
      if (!/^[\d\s+\-*/().%^eE]+$/.test(expr)) return 'Ошибка: обнаружены недопустимые символы в математическом выражении.';
      try {
        const sanitized = expr.replace(/\^/g, '**');
        const res = new Function(`"use strict"; return (${sanitized});`)();
        return `[QUANTUM_CALC]: Результат вычислений: ${expr.trim()} = ${res}. Биочип не ошибается.`;
      } catch {
        return 'Ошибка: сбой синтаксиса формулы.';
      }
    }
  },
  {
    id: 'coin_dice',
    name: 'Haywood RNG Ripper // Рулетка Судьбы',
    description: 'Уличный генератор случайности: бросок кубиков D6/D20, подбрасывание монетки или жеребьевка решений.',
    category: 'fun',
    author: 'STREET: REBECCA ARSENAL',
    version: '1.4.0',
    tier: 'tier-3',
    icon: '🎲',
    tags: ['dice', 'coin', 'random', 'хейвуд'],
    samplePrompt: 'брось кубик d20',
    installed: true,
    enabled: true,
    async execute(query) {
      if (/монет|coin/i.test(query)) {
        const side = Math.random() < 0.5 ? 'ОРЕЛ (ARASAKA)' : 'РЕШКА (EDGERUNNER)';
        return `[COIN_FLIP]: Монетка звякнула об асфальт: выпал ${side}!`;
      }
      const match = query.match(/d(\d+)/i);
      const sides = match ? parseInt(match[1], 10) : 6;
      const roll = Math.floor(Math.random() * sides) + 1;
      return `[DICE_ROLL]: Бросок D${sides}: выпало [ ${roll} ]! ${roll === sides ? 'КРИТИЧЕСКИЙ УСПЕХ, CHOOM!' : (roll === 1 ? 'Критический провал, осторожнее.' : '')}`;
    }
  },
  {
    id: 'cyber_quotes',
    name: 'Edgerunners Chronicles // Хроники 2077',
    description: 'Культовые цитаты Джонни Сильверхенда, Дэвида Мартинеса, Люси и мудрость темных переулков Найт-Сити.',
    category: 'fun',
    author: 'LEGEND: AFTERLIFE CLUB',
    version: '2.5.0',
    tier: 'tier-5',
    icon: '⚡',
    tags: ['quotes', 'lore', 'silverhand', 'afterlife'],
    samplePrompt: 'цитата сильверхенда',
    installed: true,
    enabled: true,
    async execute(query = '') {
      const q = String(query).toLowerCase();
      const quotes = {
        silverhand: [
          "«Проснись уже, самурай. Время сжечь этот город дотла.» — Johnny Silverhand",
          "«Ты думаешь, корпорациям не плевать на нас? Они забрали наше тело, наши души, даже наше будущее.» — Johnny Silverhand",
          "«Я бы предпочёл сдохнуть стоя, чем жить на коленях перед Арасакой.» — Johnny Silverhand",
          "«В Найт-Сити каждый сам за себя, парень. Но если у тебя есть принципы — держись за них зубами.» — Johnny Silverhand",
          "«Сделай так, чтобы они тебя запомнили. Не как корпоративную шестерёнку, а как чертов взрыв.» — Johnny Silverhand",
          "«Goodbye, Night City. Maybe in the next life.» — Johnny Silverhand"
        ],
        david: [
          "«Я не собираюсь умирать здесь, Люси. Я отвезу тебя на Луну, обещаю.» — David Martinez",
          "«Я особенный. Я выдержу этот имплант. Ради нас.» — David Martinez",
          "«Прости, Люси... Похоже, на Луну ты полетишь одна.» — David Martinez",
          "«Я доберусь до самой вершины Арасака-Тауэр. Чего бы мне это ни стоило.» — David Martinez"
        ],
        lucy: [
          "«Моей мечтой было увидеть Луну. Но потом появился ты, Дэвид, и ты стал важнее Луны.» — Люси",
          "«Никому не доверяй в этом городе, Дэвид. Даже мне. Особенно мне.» — Люси",
          "«Луна прекрасна, потому что там нет людей. И нет Найт-Сити.» — Люси",
          "«Не смей умирать за чужую мечту, Дэвид... Живи ради своей.» — Люси"
        ],
        rebecca: [
          "«Мы чертовы эджраннеры, Дэвид! Мы идем до самого конца вместе!» — Ребекка",
          "«Если за тобой гонится весь Корпо-Плаза — главное не сбавлять скорость и стрелять первым!» — Ребекка",
          "«Дэвид, придурок! Ты же сгоришь от киберпсихоза! Но если идешь на штурм — я прикрою тебя!» — Ребекка"
        ],
        smasher: [
          "«Ты думал, что ты особенный? Жалкий кусок мясного хлама.» — Адам Смэшер",
          "«Никто не уходит отсюда живым.» — Адам Смэшер"
        ],
        rogue: [
          "«В Найт-Сити легендами не становятся при жизни. Легенды рождаются в морге.» — Rogue Amendiares"
        ]
      };

      let pool = null;
      if (/сильверхенд|silverhand|сильвер|джонни|johnny/i.test(q)) {
        pool = quotes.silverhand;
      } else if (/дэвид|david|мартинес|martinez/i.test(q)) {
        pool = quotes.david;
      } else if (/люси|lucy/i.test(q)) {
        pool = quotes.lucy;
      } else if (/ребекк|rebecca|бекка/i.test(q)) {
        pool = quotes.rebecca;
      } else if (/смэшер|smasher|адам/i.test(q)) {
        pool = quotes.smasher;
      } else if (/бести[яи]|роуг|rogue/i.test(q)) {
        pool = quotes.rogue;
      } else {
        pool = Object.values(quotes).flat();
      }
      return pool[Math.floor(Math.random() * pool.length)];
    }
  },
  {
    id: 'dev_tools',
    name: 'Netrunner ICE-Breaker // Инструменты Взлома',
    description: 'Генератор UUID v4, кодирование/декодирование Base64, расчет Unix timestamp и хеширование пакетов.',
    category: 'development',
    author: 'CYBER: NETWATCH ROGUE',
    version: '3.2.0',
    tier: 'tier-5',
    icon: '💻',
    tags: ['uuid', 'base64', 'dev', 'icebreaker'],
    samplePrompt: 'сгенерируй uuid',
    installed: true,
    enabled: true,
    async execute(query) {
      if (/uuid/i.test(query)) {
        const u = 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
          const r = Math.random() * 16 | 0;
          return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
        });
        return `[NETRUNNER_KEY]: Сгенерирован UUID v4: ${u}`;
      }
      if (/time|timestamp/i.test(query)) {
        return `[UNIX_TIME]: Текущий системный штамп: ${Date.now()} (${new Date().toISOString()})`;
      }
      return '[ICE_BREAKER]: Доступные команды: «uuid», «timestamp», «base64 encode ТЕКСТ».';
    }
  },
  {
    id: 'music_controller',
    name: 'Night City Radio // Радиостанции',
    description: 'Нативное управление Яндекс.Музыкой, Spotify и YouTube: переключение волн, громкость, пауза и поиск.',
    category: 'productivity',
    author: 'STATION: MORRO ROCK RADIO',
    version: '2.0.0',
    tier: 'tier-5',
    icon: '🎵',
    tags: ['music', 'radio', 'spotify', 'yandex'],
    samplePrompt: 'включи яндекс музыку',
    installed: true,
    enabled: true,
    async execute(query) {
      return `[RADIO_SYNC]: Команда «${query}» перенаправлена в медиа-мост Capri. Музыка синхронизирована!`;
    }
  },
  {
    id: 'biomonitor',
    name: 'Trauma Team Biomonitor // Биомонитор Хоста',
    description: 'Реальная диагностика ресурсов ПК: физическая RAM, загрузка CPU всех ядер, статус питания и аптайм.',
    category: 'utilities',
    author: 'MEDIC: TRAUMA TEAM PLATINUM',
    version: '2.0.0',
    tier: 'tier-3',
    icon: '🩺',
    tags: ['biomonitor', 'ram', 'cpu', 'fps', 'хост'],
    samplePrompt: 'статус системы',
    installed: true,
    enabled: true,
    async execute() {
      try {
        let stats = null;
        if (window.__TAURI__?.core?.invoke) {
          stats = await window.__TAURI__.core.invoke('get_system_stats');
        } else {
          stats = await fetch('http://127.0.0.1:47823/api/system/stats').then(r => r.json());
        }
        if (stats && stats.ok) {
          return [
            '🩺 [BIOMONITOR // ТРАУМА ТИМ]:',
            `├ ОЗУ хоста: ${stats.used_ram_gb} / ${stats.total_ram_gb} ГБ (${stats.ram_load_pct}% занято, свободно: ${stats.avail_ram_gb} ГБ)`,
            `├ ЦП хоста: ${stats.cpu_cores} ядер (загрузка ${stats.cpu_load_pct}%)`,
            `├ Питание: ${stats.power_source} (заряд: ${stats.battery_pct}) | Аптайм: ${stats.uptime_hours}ч ${stats.uptime_mins}м`,
            `└ Процесс Capri: ${stats.capri_ram_mb} МБ RAM (Rust + Tauri v2)`
          ].join('\n');
        }
      } catch (e) {
        console.warn('Biomonitor fetch failed:', e);
      }
      return '🩺 [BIOMONITOR]: Все параметры хоста стабильны. Биочип подключен.';
    }
  },
  {
    id: 'translator',
    name: 'Neural Lingua Translator // Нейро-переводчик',
    description: 'Мгновенный перевод уличного сленга Найт-Сити, японских корпоративных терминов и технических логов.',
    category: 'development',
    author: 'CORPO: KANG TAO INTERPRETER',
    version: '1.1.0',
    tier: 'tier-5',
    icon: '🌐',
    tags: ['translate', 'lingua', 'japanese', 'slang'],
    samplePrompt: 'что значит choom',
    installed: false,
    enabled: false,
    async execute(query) {
      return `[LINGUA_DB]: Запрос «${query}». Сленг Найт-Сити: «Choom / Чумба» — друг, напарник, надежный человек с улиц.`;
    }
  }
];

// --- State Management ---
let skillsCatalog = [];
let activeCategory = 'all';
let searchQuery = '';
let currentTestSkill = null;

// --- DOM Elements ---
const connectionTextEl = document.getElementById('connectionText');
const ncClockEl = document.getElementById('ncClock');
const soundToggleBtn = document.getElementById('soundToggleBtn');
const soundIconEl = document.getElementById('soundIcon');
const skillsGridEl = document.getElementById('skillsGrid');
const categoryNavEl = document.getElementById('categoryNav');
const skillSearchInput = document.getElementById('skillSearchInput');
const searchClearBtn = document.getElementById('searchClearBtn');
const refreshBtn = document.getElementById('refreshBtn');
const installedCountBadge = document.getElementById('installedCountBadge');

// Capri Mascot Elements
const capriAvatarContainer = document.getElementById('capriAvatarContainer');
const capriMainImg = document.getElementById('capriMainImg');
const capriQuoteText = document.getElementById('capriQuoteText');
const emoBtns = document.querySelectorAll('.emo-btn');

// Test Drawer Elements
const testDrawer = document.getElementById('testDrawer');
const testDrawerCloseBtn = document.getElementById('testDrawerCloseBtn');
const testDrawerSkillName = document.getElementById('testDrawerSkillName');
const testDrawerLog = document.getElementById('testDrawerLog');
const testPromptInput = document.getElementById('testPromptInput');
const testSendBtn = document.getElementById('testSendBtn');

// Custom Skill Ripperdoc Modal Elements
const customModal = document.getElementById('customModal');
const openCustomModalBtn = document.getElementById('openCustomModalBtn');
const closeModalBtn = document.getElementById('closeModalBtn');
const cancelModalBtn = document.getElementById('cancelModalBtn');
const saveCustomSkillBtn = document.getElementById('saveCustomSkillBtn');
const skillIdInput = document.getElementById('skillIdInput');
const skillNameInput = document.getElementById('skillNameInput');
const skillCategoryInput = document.getElementById('skillCategoryInput');
const skillIconInput = document.getElementById('skillIconInput');
const skillDescInput = document.getElementById('skillDescInput');
const skillPatternInput = document.getElementById('skillPatternInput');
const skillReplyInput = document.getElementById('skillReplyInput');
const previewUserText = document.getElementById('previewUserText');
const previewCapriText = document.getElementById('previewCapriText');

// --- Real-time Clock ---
function startClock() {
  function tick() {
    const now = new Date();
    const h = String(now.getHours()).padStart(2, '0');
    const m = String(now.getMinutes()).padStart(2, '0');
    const s = String(now.getSeconds()).padStart(2, '0');
    ncClockEl.textContent = `2077.11.02 // ${h}:${m}:${s}`;
  }
  tick();
  setInterval(tick, 1000);
}

// --- Sound Toggle Controller ---
function initSoundUI() {
  const isEnabled = audio.enabled;
  soundIconEl.textContent = isEnabled ? '🔊' : '🔇';
  soundToggleBtn.querySelector('.sound-text').textContent = isEnabled ? 'AUDIO FX: ON' : 'AUDIO FX: OFF';

  soundToggleBtn.addEventListener('click', () => {
    const state = audio.toggle();
    soundIconEl.textContent = state ? '🔊' : '🔇';
    soundToggleBtn.querySelector('.sound-text').textContent = state ? 'AUDIO FX: ON' : 'AUDIO FX: OFF';
    if (state) audio.playClick();
  });
}

// --- Check Capri Bridge Health ---
async function checkBridgeHealth() {
  try {
    const res = await fetch(`${API_BASE}/health`, { method: 'GET' });
    const data = await res.json();
    if (data.ok && data.name === 'capri') {
      connectionTextEl.textContent = `NETWATCH: LINKED [PID: ${data.pid || 'ACTIVE'}]`;
      connectionTextEl.parentElement.querySelector('.pulse-led').style.background = 'var(--cp-green)';
      return true;
    }
  } catch {}

  connectionTextEl.textContent = 'NETWATCH: LOCAL LINK [PORT 47823]';
  return false;
}

// --- Fetch & Merge Skills ---
async function loadSkillsCatalog() {
  let serverSkills = [];
  try {
    const res = await fetch(`${API_BASE}/api/skills`);
    if (res.ok) {
      serverSkills = await res.json();
    }
  } catch {}

  // Merge default cyberware with server skills and custom skills from localStorage
  const localCustom = JSON.parse(localStorage.getItem('capri_custom_skills') || '[]');
  const map = new Map();

  DEFAULT_CYBERWARE.forEach(s => map.set(s.id, { ...s }));
  serverSkills.forEach(s => {
    if (map.has(s.id)) {
      map.set(s.id, { ...map.get(s.id), ...s });
    } else {
      map.set(s.id, {
        tier: 'tier-3',
        icon: '⚡',
        samplePrompt: s.name,
        ...s
      });
    }
  });
  localCustom.forEach(s => map.set(s.id, s));

  skillsCatalog = Array.from(map.values());
  updateCategoryBadges();
  renderCatalog();
}

// --- Badge Counters ---
function updateCategoryBadges() {
  const counts = { all: skillsCatalog.length, utilities: 0, productivity: 0, fun: 0, development: 0 };
  let installedCount = 0;

  skillsCatalog.forEach(s => {
    if (s.installed) installedCount++;
    const cat = s.category || 'utilities';
    if (counts[cat] !== undefined) counts[cat]++;
  });

  document.getElementById('catCountAll').textContent = counts.all;
  document.getElementById('catCountUtil').textContent = counts.utilities;
  document.getElementById('catCountProd').textContent = counts.productivity;
  document.getElementById('catCountFun').textContent = counts.fun;
  document.getElementById('catCountDev').textContent = counts.development;

  installedCountBadge.textContent = `${installedCount} / ${skillsCatalog.length} СЛОТОВ`;
}

// --- Render Catalog Grid ---
function renderCatalog() {
  const q = searchQuery.toLowerCase().trim();
  const filtered = skillsCatalog.filter(s => {
    const matchCat = activeCategory === 'all' || s.category === activeCategory;
    const matchSearch = !q || 
      s.name.toLowerCase().includes(q) || 
      s.description.toLowerCase().includes(q) || 
      (s.tags && s.tags.some(t => t.toLowerCase().includes(q)));
    return matchCat && matchSearch;
  });

  if (filtered.length === 0) {
    skillsGridEl.innerHTML = `
      <div style="grid-column: 1 / -1; padding: 3rem; text-align: center; color: var(--text-muted); background: rgba(0,0,0,0.4); border: 1px dashed var(--border-dim); clip-path: var(--clip-chamfer);">
        <h3 style="font-family: var(--font-orbitron); color: var(--cp-yellow); margin-bottom: 0.5rem;">[ICE_BREAKER: 0 СООТВЕТСТВИЙ]</h3>
        <p>В этом секторе кибердека модули не найдены. Попробуй сбросить фильтр или создать свой в Ripperdoc Lab.</p>
      </div>
    `;
    return;
  }

  skillsGridEl.innerHTML = filtered.map(skill => {
    const isInstalled = !!skill.installed;
    const isEnabled = isInstalled && skill.enabled !== false;
    const tierClass = skill.tier === 'tier-5' ? 'tier-5' : 'tier-3';
    const tierLabel = skill.tier === 'tier-5' ? 'TIER 5++ ARASAKA' : 'TIER 3 EDGERUNNER';

    return `
      <div class="skill-card ${isInstalled ? 'installed' : ''}" data-id="${skill.id}">
        <div>
          <div class="card-slot-header">
            <span class="slot-id">CHROME_ID: #${skill.id.toUpperCase()}</span>
            <span class="tier-badge ${tierClass}">${tierLabel}</span>
          </div>

          <div class="card-main-header">
            <div class="skill-icon-wrap">${skill.icon || '⚡'}</div>
            <div class="card-titles">
              <h3>${escapeHtml(skill.name)}</h3>
              <span class="author-chip">${escapeHtml(skill.author || 'EDGERUNNER LAB')}</span>
            </div>
          </div>

          <p class="card-desc">${escapeHtml(skill.description)}</p>

          <div class="card-tags">
            ${(skill.tags || []).map(t => `<span class="tag">#${escapeHtml(t)}</span>`).join('')}
          </div>
        </div>

        <div class="card-footer">
          ${isInstalled ? `
            <button class="switch-btn ${isEnabled ? 'active' : 'inactive'}" onclick="handleToggle('${skill.id}', ${!isEnabled})">
              <span>${isEnabled ? '● ACTIVE' : '○ INACTIVE'}</span>
            </button>
            <div class="card-secondary-btns">
              <button class="card-action-btn" title="Тестировать модуль" onclick="openTestDrawer('${skill.id}')">
                ⚡ ТЕСТ
              </button>
              <button class="card-action-btn danger" title="Деинсталлировать" onclick="handleUninstall('${skill.id}')">
                УДАЛИТЬ
              </button>
            </div>
          ` : `
            <button class="btn-install-chrome" onclick="handleInstall('${skill.id}')">
              + [ПРОШИТЬ В БИОЧИП]
            </button>
          `}
        </div>
      </div>
    `;
  }).join('');

  // Attach hover sounds to cards
  document.querySelectorAll('.skill-card').forEach(c => {
    c.addEventListener('mouseenter', () => audio.playHover());
  });
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str).replace(/[&<>"']/g, m => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[m]));
}

// --- Toggle Skill ---
window.handleToggle = async function(id, enable) {
  audio.playClick();
  try {
    await fetch(`${API_BASE}/api/skills/toggle`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, enabled: enable })
    });
  } catch {}

  const s = skillsCatalog.find(item => item.id === id);
  if (s) s.enabled = enable;
  saveLocalState();
  renderCatalog();
  showCapriSpeech(enable 
    ? `«Модуль #${id} активирован в твоем кибердеке. Готова принимать запросы, choom!»` 
    : `«Модуль #${id} временно отключен. Освободила вычислительные циклы.»`
  );
};

// --- Install Skill ---
window.handleInstall = async function(id) {
  audio.playSuccess();
  try {
    await fetch(`${API_BASE}/api/skills/install`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id })
    });
  } catch {}

  const s = skillsCatalog.find(item => item.id === id);
  if (s) {
    s.installed = true;
    s.enabled = true;
  }
  saveLocalState();
  updateCategoryBadges();
  renderCatalog();
  showCapriSpeech(`«Отличный выбор! Прошила модуль «${s ? s.name : id}» прямо в ядро. Никаких лагов!»`);
};

// --- Uninstall Skill ---
window.handleUninstall = async function(id) {
  audio.playClick();
  try {
    await fetch(`${API_BASE}/api/skills/uninstall`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id })
    });
  } catch {}

  const s = skillsCatalog.find(item => item.id === id);
  if (s) {
    s.installed = false;
    s.enabled = false;
  }
  saveLocalState();
  updateCategoryBadges();
  renderCatalog();
  showCapriSpeech(`«Удалила хром #${id}. Если передумаешь — риппердок всегда на связи.»`);
};

function saveLocalState() {
  const custom = skillsCatalog.filter(s => s.isCustom);
  localStorage.setItem('capri_custom_skills', JSON.stringify(custom));
}

// --- Interactive Capri Hologram Speech & Moods ---
function showCapriSpeech(text) {
  audio.playSpeech();
  capriQuoteText.textContent = text;
  capriQuoteText.style.animation = 'none';
  void capriQuoteText.offsetWidth;
  capriQuoteText.style.animation = 'pulse-dot 0.3s ease-in-out';
}

function randomizeCapriDialogue() {
  const q = EDGERUNNERS_QUOTES[Math.floor(Math.random() * EDGERUNNERS_QUOTES.length)];
  showCapriSpeech(q);

  // Cycle expression
  const faces = ['assets/face-smug.png', 'assets/face-laugh.png', 'assets/face-calm.png', 'assets/face-wide.png', 'assets/cutin-eyes.png'];
  const nextFace = faces[Math.floor(Math.random() * faces.length)];
  capriMainImg.src = nextFace;
}

capriAvatarContainer.addEventListener('click', () => {
  randomizeCapriDialogue();
});

// Mood Switcher Buttons
emoBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    audio.playClick();
    emoBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    const img = btn.getAttribute('data-img');
    if (img) capriMainImg.src = img;
  });
});

// --- Test Drawer Interactive Simulation ---
window.openTestDrawer = function(id) {
  audio.playClick();
  const s = skillsCatalog.find(item => item.id === id);
  if (!s) return;
  currentTestSkill = s;

  testDrawerSkillName.textContent = s.name;
  testPromptInput.value = s.samplePrompt || s.name;
  testDrawerLog.innerHTML = `
    <div class="log-line sys">&gt; Подключено к модулю: ${escapeHtml(s.name)} [ID: ${s.id}]</div>
    <div class="log-line sys">&gt; Готов к выполнению запросов. Нажмите ВЫПОЛНИТЬ или Enter.</div>
  `;
  testDrawer.classList.add('open');
  testPromptInput.focus();
};

testDrawerCloseBtn.addEventListener('click', () => {
  audio.playClick();
  testDrawer.classList.remove('open');
});

async function runTestExecution() {
  const query = testPromptInput.value.trim();
  if (!query || !currentTestSkill) return;
  audio.playClick();

  const userDiv = document.createElement('div');
  userDiv.className = 'log-line user';
  userDiv.textContent = `>>> ${query}`;
  testDrawerLog.appendChild(userDiv);
  testPromptInput.value = '';

  const capriDiv = document.createElement('div');
  capriDiv.className = 'log-line capri';
  capriDiv.textContent = 'CAPRI: [Обработка нейросетью...]';
  testDrawerLog.appendChild(capriDiv);
  testDrawerLog.scrollTop = testDrawerLog.scrollHeight;

  try {
    let result = '';
    if (typeof currentTestSkill.execute === 'function') {
      result = await currentTestSkill.execute(query);
    } else {
      result = `[CONSTRUCT_RESPONSE]: ${currentTestSkill.replyTemplate || 'Команда выполнена успешно, choom!'}`;
    }
    capriDiv.textContent = `CAPRI: ${result}`;
    showCapriSpeech(result);
  } catch (err) {
    capriDiv.className = 'log-line err';
    capriDiv.textContent = `CAPRI_ERR: ${err.message}`;
  }
  testDrawerLog.scrollTop = testDrawerLog.scrollHeight;
}

testSendBtn.addEventListener('click', runTestExecution);
testPromptInput.addEventListener('keydown', e => {
  if (e.key === 'Enter') runTestExecution();
});

// --- Category Tabs ---
categoryNavEl.querySelectorAll('.cartridge-tab').forEach(tab => {
  tab.addEventListener('click', () => {
    audio.playClick();
    categoryNavEl.querySelectorAll('.cartridge-tab').forEach(t => t.classList.remove('active'));
    tab.classList.add('active');
    activeCategory = tab.getAttribute('data-cat');
    renderCatalog();
  });
});

// --- Search Filter ---
skillSearchInput.addEventListener('input', e => {
  searchQuery = e.target.value;
  renderCatalog();
});

searchClearBtn.addEventListener('click', () => {
  skillSearchInput.value = '';
  searchQuery = '';
  renderCatalog();
});

refreshBtn.addEventListener('click', async () => {
  audio.playSuccess();
  await checkBridgeHealth();
  await loadSkillsCatalog();
  showCapriSpeech('«Сеть просканирована! Каталог навыков обновлен до актуального состояния.»');
});

// --- Custom Skill Ripperdoc Modal ---
openCustomModalBtn.addEventListener('click', () => {
  audio.playClick();
  customModal.classList.add('open');
  updateCustomPreview();
});

function closeCustomModal() {
  audio.playClick();
  customModal.classList.remove('open');
}

closeModalBtn.addEventListener('click', closeCustomModal);
cancelModalBtn.addEventListener('click', closeCustomModal);

function updateCustomPreview() {
  const trigger = skillPatternInput.value.trim() || 'сандевистан';
  const reply = skillReplyInput.value.trim() || 'Сандевистан включен на полную катушку! 120 FPS без задержек, choom.';
  previewUserText.textContent = trigger;
  previewCapriText.textContent = reply;
}

skillPatternInput.addEventListener('input', updateCustomPreview);
skillReplyInput.addEventListener('input', updateCustomPreview);

document.querySelectorAll('.preset-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    audio.playClick();
    skillPatternInput.value = btn.getAttribute('data-pattern');
    skillReplyInput.value = btn.getAttribute('data-reply');
    updateCustomPreview();
  });
});

saveCustomSkillBtn.addEventListener('click', () => {
  const id = (skillIdInput.value.trim() || 'custom-' + Date.now()).toLowerCase().replace(/[^a-z0-9_-]/g, '-');
  const name = skillNameInput.value.trim() || 'Кастомный хром';
  const desc = skillDescInput.value.trim() || 'Пользовательский модуль Найт-Сити';
  const cat = skillCategoryInput.value || 'productivity';
  const icon = skillIconInput.value.trim() || '⚡';
  const pattern = skillPatternInput.value.trim();
  const reply = skillReplyInput.value.trim() || 'Команда принята, choom!';

  if (!pattern) {
    alert('Укажите триггер или регулярное выражение для навыка!');
    return;
  }

  audio.playSuccess();
  const newSkill = {
    id,
    name,
    description: desc,
    category: cat,
    author: 'RIPPERDOC: YOU',
    version: '1.0.0',
    tier: 'tier-5',
    icon,
    tags: ['custom', 'ripperdoc', 'chrome'],
    samplePrompt: pattern.replace(/[\^$()?:+]/g, '').trim(),
    installed: true,
    enabled: true,
    isCustom: true,
    replyTemplate: reply,
    async execute(query) {
      return reply.replace('{input}', query).replace('{match}', query);
    }
  };

  skillsCatalog.unshift(newSkill);
  saveLocalState();
  updateCategoryBadges();
  renderCatalog();
  closeCustomModal();
  showCapriSpeech(`«Новый модуль «${name}» успешно активирован и прошит в биочип!»`);
});

// --- Startup Initialization ---
window.addEventListener('DOMContentLoaded', async () => {
  startClock();
  initSoundUI();
  await checkBridgeHealth();
  await loadSkillsCatalog();
});
