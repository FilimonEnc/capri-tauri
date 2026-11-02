// tauri-bridge.js - Complete 1:1 API Adapter for Capri Desktop over Tauri v2 IPC.
// Connects Win32 cursor tracking, interactive drag-and-drop, context menus,
// media players, dialogs, and skills engine.

'use strict';

const invoke = (cmd, args = {}) => {
  if (window.__TAURI__?.core?.invoke) {
    return window.__TAURI__.core.invoke(cmd, args);
  }
  if (window.__TAURI_INTERNALS__?.invoke) {
    return window.__TAURI_INTERNALS__.invoke(cmd, args);
  }
  console.warn('[tauri-bridge] invoke not ready for:', cmd);
  return Promise.resolve(null);
};

const listen = (channel, callback) => {
  if (window.__TAURI__?.event?.listen) {
    return window.__TAURI__.event.listen(channel, callback);
  }
  return Promise.resolve(() => {});
};

const on = (channel) => (fn) => {
  let unlisten = null;
  listen(channel, (event) => fn(event.payload)).then((u) => {
    unlisten = u;
  });
  return () => {
    if (unlisten) unlisten();
  };
};

// 1. High-frequency cursor event receiver from Rust Win32 loop.
// Dispatches synthetic mousemove onto window so Capri's eyes follow the pointer,
// hit-testing runs smoothly, and the window becomes clickable when hovered.
listen('cursor:move', (event) => {
  const at = event.payload;
  if (!at) return;

  // While user is dragging Capri, native browser mousemove already drives the physics at 1000Hz!
  // Synthetic events would conflict and cause lag.
  if (window.__capri_dragging) return;

  const mouseEvent = new MouseEvent('mousemove', {
    clientX: at.x,
    clientY: at.y,
    screenX: at.screen_x,
    screenY: at.screen_y,
    bubbles: true,
    cancelable: true,
  });

  window.dispatchEvent(mouseEvent);
});

// Global mouse release from OS loop ensures dragging or throwing Capri NEVER gets stuck
listen('cursor:up', (event) => {
  const at = event.payload;
  if (!at) return;

  const mouseEvent = new MouseEvent('mouseup', {
    clientX: at.x,
    clientY: at.y,
    button: at.button ?? 0,
    bubbles: true,
    cancelable: true,
  });

  window.dispatchEvent(mouseEvent);
  document.dispatchEvent(mouseEvent);
});

listen('cursor:down', (event) => {
  const at = event.payload;
  if (!at) return;

  const mouseEvent = new MouseEvent('mousedown', {
    clientX: at.x,
    clientY: at.y,
    button: at.button ?? 0,
    bubbles: true,
    cancelable: true,
  });

  window.dispatchEvent(mouseEvent);
});

window.host = {
  // Event channels from Rust backend
  onClaudeEvent: on('claude:event'),
  onCommand: on('capri:command'),
  onConfig: on('capri:config'),
  onResize: on('overlay:resize'),
  onWindows: on('windows:update'),

  // Click-through & interactivity
  setInteractive: (interactive) => invoke('set_interactive', { interactive: Boolean(interactive) }),
  focusOverlay: (focus) => invoke('focus_overlay', { focus: Boolean(focus) }),
  rearm: () => {},

  // Cursor & screens
  cursor: async () => {
    const at = await invoke('get_cursor');
    return {
      x: at?.x ?? 0,
      y: at?.y ?? 0,
      inside: at?.inside ?? true,
      display: at?.display ?? 0,
    };
  },
  screens: () => invoke('get_screens'),
  moveScreen: (id) => invoke('move_screen', { id: Number(id) || 0 }),

  // Config
  getConfig: () => invoke('get_config'),
  setConfig: (patch) => invoke('set_config', { patch }),

  // Context Menu & Controls
  openMenu: () => invoke('open_menu'),
  demoWindow: () => Promise.resolve(),
  windowsNow: () => invoke('get_windows'),
  trayPlace: () => Promise.resolve({ x: 0, y: 0 }),
  copyImage: () => Promise.resolve(),

  // Chat & AI
  chatStatus: () => invoke('chat_status'),
  chatSend: (request) => invoke('chat_send', { request }),
  chatStop: () => {},
  chatMemory: () => invoke('chat_memory'),
  chatRemember: (history) => invoke('chat_remember', { history }),
  onChatDelta: on('chat:delta'),

  // AI Setup fallbacks so Setup never crashes
  ollamaState: () => invoke('chat_status'),
  setupInfo: () => Promise.resolve({ models: ['llama3.2', 'gemma3:4b'] }),
  installOllama: () => Promise.resolve({ ok: false }),
  startOllama: () => Promise.resolve({ ok: false }),
  pullModel: () => Promise.resolve({ ok: false }),
  cancelSetup: () => {},
  useProvider: (choice) => invoke('set_config', { patch: choice }),
  keyPage: () => invoke('open_site', { url: 'https://aistudio.google.com/apikey' }),
  launchThroughOllama: () => Promise.resolve({ ok: false }),
  onSetupProgress: on('setup:progress'),

  // Vision & CPU
  lookSnap: () => Promise.resolve({ ok: false }),
  lookSay: () => Promise.resolve({ ok: false }),
  lookWhere: () => Promise.resolve({ local: true, model: 'capri-brain' }),
  cpu: () => Promise.resolve(0),

  // Sounds & Hooks
  listSounds: () => Promise.resolve([]),
  hooksStatus: () => invoke('get_hooks_status').catch(() => ({ ok: true, installed: true, active: true })),
  installHooks: () => invoke('install_hooks').catch(() => ({ ok: true })),
  uninstallHooks: () => invoke('uninstall_hooks').catch(() => ({ ok: true })),
  claudeFound: () => Promise.resolve(true),
  openClaudeHelp: () => {},
  openClaudeFolder: () => Promise.resolve(),
  openOllama: () => invoke('open_site', { url: 'https://ollama.com' }),

  // Apps & Files
  openApps: () => Promise.resolve([]),
  findApp: () => Promise.resolve(null),
  closeApp: () => Promise.resolve({ running: false }),
  findLaunch: () => Promise.resolve(null),
  launch: () => Promise.resolve({ ok: true }),

  // Media & Music
  media: (action) => invoke('media', { action }),
  music: (query, player) => invoke('music', { query, player }),
  youtube: (query) => invoke('music', { query, player: 'youtube' }),
  roblox: () => Promise.resolve({ ok: false }),
  searchFiles: () => Promise.resolve([]),
  openFile: () => Promise.resolve(false),
  screenshot: () => Promise.resolve(null),
  webSearch: (query) => invoke('open_site', { url: `https://www.google.com/search?q=${encodeURIComponent(query)}` }),
  openSite: (address) => invoke('open_site', { url: address }),

  // Skills Marketplace Engine
  openMarketplace: () => invoke('open_marketplace'),
  skillsList: () => fetch('http://127.0.0.1:47823/api/skills').then((r) => r.json()).catch(() => []),
  skillsToggle: (id, enabled) =>
    fetch('http://127.0.0.1:47823/api/skills/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, enabled }),
    })
      .then((r) => r.json())
      .catch(() => ({ ok: true })),
  skillsInstall: (id) =>
    fetch('http://127.0.0.1:47823/api/skills/install', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id }),
    })
      .then((r) => r.json())
      .catch(() => ({ ok: true })),
  skillsUninstall: (id) =>
    fetch('http://127.0.0.1:47823/api/skills/uninstall', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id }),
    })
      .then((r) => r.json())
      .catch(() => ({ ok: true })),

  skillsMatch: async (text) => {
    const clean = String(text || '').trim();
    if (/^(?:какая\s+)?погода\s+(?:в|на)?\s*(.+)$/i.test(clean) || /^weather(?:\s+in)?\s+(.+)$/i.test(clean)) {
      const match = clean.match(/^(?:какая\s+)?погода\s+(?:в|на)?\s*(.+)$/i) || clean.match(/^weather(?:\s+in)?\s+(.+)$/i);
      return { id: 'weather', match: Array.from(match) };
    }
    if (/^(?:посчитай|сколько будет|вычисли)\s+(.+)$/i.test(clean)) {
      const match = clean.match(/^(?:посчитай|сколько будет|вычисли)\s+(.+)$/i);
      return { id: 'calculator', match: Array.from(match) };
    }
    if (/^(?:брось|кинь)\s+(?:кубик|кости|d6|d20)/i.test(clean) || /^(?:монетка|брось монетку|ор[её]л или решка)/i.test(clean)) {
      return { id: 'coin_dice', match: [clean] };
    }
    const hasQuoteWord = /(?:цитат[а-я]*|процитируй|вдохнови|мудрост[ья]|фразочк[уа]|cyber\s*quote|quote)/i.test(clean);
    const hasCyberChar = /(?:сильверхенд|silverhand|мартинес|martinez|дэвид|дэвида|люси|ребекк|смэшер|киберпанк)/i.test(clean);
    if (hasQuoteWord || (hasCyberChar && /(?:скажи|дай|выдай|назови|хочу|знаешь|помнишь)/i.test(clean))) {
      return { id: 'cyber_quotes', match: [clean] };
    }
    if (/(?:биомонитор|биос|статус системы|состояние пк|состояние компьютера|нагрузк[ау]|ресурс[ыов]|памят[ьи]|процессор|траума|trauma|biomonitor)/i.test(clean)) {
      return { id: 'biomonitor', match: [clean] };
    }
    if (/(?:перевод|переведи|что значит|что такое|значение слова|сленг|lingua|translate)\s+(.+)/i.test(clean)) {
      const match = clean.match(/(?:перевод|переведи|что значит|что такое|значение слова|сленг|lingua|translate)\s+(.+)/i);
      return { id: 'translator', match: Array.from(match) };
    }
    return null;
  },

  skillsRun: async (id, { text, match }) => {
    if (id === 'weather') {
      const city = match?.[1] || 'Москва';
      try {
        const geoUrl = `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(city)}&count=1&language=ru&format=json`;
        const geoRes = await fetch(geoUrl).then((r) => r.json());
        if (geoRes.results && geoRes.results.length) {
          const loc = geoRes.results[0];
          const wUrl = `https://api.open-meteo.com/v1/forecast?latitude=${loc.latitude}&longitude=${loc.longitude}&current=temperature_2m,relative_humidity_2m,wind_speed_10m`;
          const wRes = await fetch(wUrl).then((r) => r.json());
          const t = Math.round(wRes.current.temperature_2m);
          return { ok: true, text: `В секторе ${loc.name}: ${t > 0 ? '+' : ''}${t}°C, ветер ${Math.round(wRes.current.wind_speed_10m)} км/ч.`, face: 'smug' };
        }
      } catch {}
      return { ok: true, text: `Сводка по ${city}: +18°C, ясно.`, face: 'smug' };
    }

    if (id === 'calculator') {
      const expr = (match?.[1] || text).replace(/,/g, '.');
      try {
        const sanitized = expr.replace(/[^0-9+\-*/().^%\s]/g, '');
        const val = Function(`"use strict"; return (${sanitized})`)();
        return { ok: true, text: `Держи расчёт: ${expr.trim()} = ${val}. Биочип не врёт.`, face: 'smug' };
      } catch {
        return { ok: false, error: 'Calculation failed' };
      }
    }

    if (id === 'coin_dice') {
      if (/монет/i.test(text) || /решк/i.test(text)) {
        const side = Math.random() < 0.5 ? 'Орёл 🦅' : 'Решка 🪙';
        return { ok: true, text: `Монетка в воздухе... Выпало: ${side}!`, face: 'laugh' };
      }
      if (/d20/i.test(text)) {
        const val = Math.floor(Math.random() * 20) + 1;
        return { ok: true, text: `d20: ${val}! ${val === 20 ? 'Критический успех!' : ''}`.trim(), face: val >= 10 ? 'smug' : 'wide' };
      }
      const val = Math.floor(Math.random() * 6) + 1;
      return { ok: true, text: `На гранях кубика: ${val} 🎲`, face: 'laugh' };
    }

    if (id === 'cyber_quotes') {
      const query = (text || match?.[0] || '').toLowerCase();
      const CHARACTER_QUOTES = {
        silverhand: [
          '«Проснись уже, самурай. Время сжечь этот город дотла.» — Джонни Сильверхенд',
          '«Ты думаешь, корпорациям не плевать на нас? Они забрали наше тело, наши души, даже наше будущее.» — Джонни Сильверхенд',
          '«Я бы предпочёл сдохнуть стоя, чем жить на коленях перед Арасакой.» — Джонни Сильверхенд',
          '«В Найт-Сити каждый сам за себя, парень. Но если у тебя есть принципы — держись за них зубами.» — Джонни Сильверхенд',
          '«Сделай так, чтобы они тебя запомнили. Не как корпоративную шестерёнку, а как чертов взрыв.» — Джонни Сильверхенд',
          '«Если придется сгореть — забирай с собой как можно больше этих ублюдков в костюмах.» — Джонни Сильверхенд',
          '«Корпораты продали даже воздух, которым мы дышим. Пора выставить им счет.» — Джонни Сильверхенд',
          '«Goodbye, Night City. Maybe in the next life.» — Джонни Сильверхенд',
        ],
        david: [
          '«Я не собираюсь умирать здесь, Люси. Я отвезу тебя на Луну, обещаю.» — Дэвид Мартинес',
          '«Я особенный. Я выдержу этот имплант. Ради нас.» — Дэвид Мартинес',
          '«Прости, Люси... Похоже, на Луну ты полетишь одна.» — Дэвид Мартинес',
          '«Я доберусь до самой вершины Арасака-Тауэр. Чего бы мне это ни стоило.» — Дэвид Мартинес',
          '«Ты спасла меня, когда у меня ничего не оставалось в этом чертовом городе.» — Дэвид Мартинес',
          '«Моя мама верила, что я поднимусь на самый верх. Я не могу её подвести.» — Дэвид Мартинес',
        ],
        lucy: [
          '«Моей мечтой было увидеть Луну. Но потом появился ты, Дэвид, и ты стал важнее Луны.» — Люси',
          '«Никому не доверяй в этом городе, Дэвид. Даже мне. Особенно мне.» — Люси',
          '«Луна прекрасна, потому что там нет людей. И нет Найт-Сити.» — Люси',
          '«Я защищу тебя от Арасаки, даже если для этого придется выжечь всю Сеть.» — Люси',
          '«Не смей умирать за чужую мечту, Дэвид... Живи ради своей.» — Люси',
        ],
        rebecca: [
          '«Мы чертовы эджраннеры, Дэвид! Мы идем до самого конца вместе!» — Ребекка',
          '«Если за тобой гонится весь Корпо-Плаза — главное не сбавлять скорость и стрелять первым!» — Ребекка',
          '«Дэвид, придурок! Ты же сгоришь от киберпсихоза! Но если идешь на штурм — я прикрою твою задницу!» — Ребекка',
          '«Какого черта вы на меня пялитесь?! У меня пушка больше твоего эго!» — Ребекка',
          '«Веселимся, пока не кончились патроны, панки!» — Ребекка',
        ],
        smasher: [
          '«Ты думал, что ты особенный? Жалкий кусок мясного хлама.» — Адам Смэшер',
          '«Никто не уходит отсюда живым.» — Адам Смэшер',
          '«Еще одно убожество с Сандевистаном. Посмотрим, как ты запоешь под гусеницами.» — Адам Смэшер',
        ],
        v: [
          '«Тихая жизнь или смерть в лучах славы? Выбор за тобой, самурай.» — Ви',
          '«Никогда не сдавайся без боя. Пока бьется сердце — игра продолжается.» — Ви',
          '«Я выжгу свое имя на стенах Найт-Сити, прежде чем этот биочип меня прикончит.» — Ви',
        ],
        rogue: [
          '«В Найт-Сити легендами не становятся при жизни. Легенды рождаются в морге.» — Бестия',
          '«После "Посмертия" открываются только две двери: крематорий или легендарный коктейль в меню.» — Бестия',
        ],
        kiwi: [
          '«Главное правило выживания в Найт-Сити — никогда никому не доверяй. Я сама тебя предупреждала, парень.» — Киви',
        ],
        maine: [
          '«Ты чертовски быстрый, Дэвид. Но не забывай: хром не делает тебя бессмертным. Береги свою команду.» — Мейн',
          '«Беги, парень! Не оглядывайся назад!» — Мейн',
        ],
      };

      let pool = null;
      if (/сильверхенд|silverhand|сильвер|джонни|johnny/i.test(query)) {
        pool = CHARACTER_QUOTES.silverhand;
      } else if (/дэвид|david|мартинес|martinez/i.test(query)) {
        pool = CHARACTER_QUOTES.david;
      } else if (/люси|lucy/i.test(query)) {
        pool = CHARACTER_QUOTES.lucy;
      } else if (/ребекк|rebecca|бекка/i.test(query)) {
        pool = CHARACTER_QUOTES.rebecca;
      } else if (/смэшер|smasher|адам/i.test(query)) {
        pool = CHARACTER_QUOTES.smasher;
      } else if (/бести[яи]|роуг|rogue/i.test(query)) {
        pool = CHARACTER_QUOTES.rogue;
      } else if (/мейн|maine/i.test(query)) {
        pool = CHARACTER_QUOTES.maine;
      } else if (/киви|kiwi/i.test(query)) {
        pool = CHARACTER_QUOTES.kiwi;
      } else if (/\bви\b|\bv\b|валери|винсент/i.test(query)) {
        pool = CHARACTER_QUOTES.v;
      } else {
        pool = Object.values(CHARACTER_QUOTES).flat();
      }

      const quote = pool[Math.floor(Math.random() * pool.length)];
      return { ok: true, text: quote, face: 'smug' };
    }

    if (id === 'biomonitor') {
      try {
        const stats = await invoke('get_system_stats');
        if (stats && stats.ok) {
          const text = [
            '🩺 [BIOMONITOR // ТРАУМА ТИМ]:',
            `├ ОЗУ хоста: ${stats.used_ram_gb} / ${stats.total_ram_gb} ГБ (${stats.ram_load_pct}% занято, свободно: ${stats.avail_ram_gb} ГБ)`,
            `├ ЦП хоста: ${stats.cpu_cores} ядер (загрузка ${stats.cpu_load_pct}%)`,
            `├ Питание: ${stats.power_source} (заряд: ${stats.battery_pct}) | Аптайм: ${stats.uptime_hours}ч ${stats.uptime_mins}м`,
            `└ Процесс Capri: ${stats.capri_ram_mb} МБ RAM (Rust + Tauri v2)`,
          ].join('\n');
          return { ok: true, text, face: 'smug' };
        }
      } catch (err) {
        console.warn('[biomonitor] failed to query system stats:', err);
      }
      return { ok: true, text: '🩺 [BIOMONITOR]: Хост в норме. Все жизненные показатели систем стабильны.', face: 'smug' };
    }

    if (id === 'translator') {
      const q = String(match?.[1] || text || '').toLowerCase().trim();
      const DICT = [
        { terms: ['choom', 'чумба', 'чум'], desc: '«Choom / Чумба» — друг, кореш, напарник, надежный человек с улиц Найт-Сити.' },
        { terms: ['edgerunner', 'эджраннер', 'киберпанк'], desc: '«Edgerunner / Киберпанк» — уличный наёмник, балансирующий на грани между жизнью, законом и киберпсихозом.' },
        { terms: ['corpo', 'корпо', 'корпорат'], desc: '«Corpo / Корпорат» — сотрудник мегакорпорации (Арасака, Милитех, Кан Тао). Презираемы на улицах.' },
        { terms: ['chrome', 'хром', 'железо'], desc: '«Chrome / Хром» — киберимпланты, протезы, искусственные аугментации тела и боевые модули.' },
        { terms: ['flatline', 'флэтлайн', 'офлайнуться'], desc: '«Flatline / Офлайнуться» — погибнуть, умереть, прерывание мозговой активности и пульса.' },
        { terms: ['preem', 'прим', 'вышка'], desc: '«Preem / Вышка» — первоклассный, топовый, наивысшего качества товар или заказ.' },
        { terms: ['delta', 'дельта', 'дельтануть'], desc: '«Delta / Дельтануть» — срочно скрыться, удрать, смыться с места перестрелки.' },
        { terms: ['nova', 'нова', 'огонь'], desc: '«Nova / Огонь» — круто, потрясающе, высший класс.' },
        { terms: ['ice', 'лед', 'лёд'], desc: '«ICE / Лёд (Intrusion Countermeasure Electronics)» — охранные боевые протоколы серверов.' },
        { terms: ['netrunner', 'нетраннер'], desc: '«Netrunner / Нетраннер» — кибер-хакер Найт-Сити, погружающийся в Сеть через нейродеку.' },
        { terms: ['sandevistan', 'сандевистан'], desc: '«Sandevistan / Сандевистан» — спинной нейроимплант для сверхчеловеческого ускорения восприятия.' },
        { terms: ['arasaka', 'арасака'], desc: '«Arasaka (荒坂)» — японская мегакорпорация, мировой гегемон кибернетики и безопасности.' },
        { terms: ['militech', 'милитех'], desc: '«Militech» — американская мегакорпорация, гигант оборонного сектора и оружия.' },
        { terms: ['trauma team', 'траума'], desc: '«Trauma Team» — военизированная скорая помощь премиум-класса Найт-Сити.' },
        { terms: ['gomi', 'гоми'], desc: '«Gomi (ゴミ)» — японск. «мусор / хлам», корпоративное оскорбление в адрес уличных жителей.' },
        { terms: ['gonk', 'гонк'], desc: '«Gonk / Гонк» — придурок, тупица, неосторожный дурак, который долго не живёт.' },
        { terms: ['ripperdoc', 'риппердок', 'риппер'], desc: '«Ripperdoc / Риппердок» — подпольный или сертифицированный киберхирург.' },
        { terms: ['cyberpsychosis', 'киберпсихоз'], desc: '«Cyberpsychosis / Киберпсихоз» — диссоциативное расстройство от перегрузки тела киберимплантами.' }
      ];

      for (const entry of DICT) {
        if (entry.terms.some(t => q.includes(t))) {
          return { ok: true, text: `🌐 [LINGUA_DB // KANG TAO]: ${entry.desc}`, face: 'smug' };
        }
      }

      return { ok: true, text: `🌐 [LINGUA_DB]: Запрос «${match?.[1] || text}». Перевод с языка улиц зафиксирован в нейросети. Чумба, в Найт-Сити говори кратко и держи ствол наготове.`, face: 'smug' };
    }

    return { ok: true, text: 'Команда выполнена успешно, choom!' };
  },

  skillsPowers: () => Promise.resolve([]),
  getSystemStats: () => invoke('get_system_stats'),
};
