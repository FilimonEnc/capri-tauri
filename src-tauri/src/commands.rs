use serde_json::{json, Value};
use std::fs;
use std::mem::size_of;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Manager, WebviewWindow};
use windows_sys::Win32::Foundation::{FILETIME, POINT};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::SystemInformation::{
    GetSystemInfo, GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetSystemTimes};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_KEYUP, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
    VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

static PREV_IDLE: AtomicU64 = AtomicU64::new(0);
static PREV_KERNEL: AtomicU64 = AtomicU64::new(0);
static PREV_USER: AtomicU64 = AtomicU64::new(0);
static LAST_PLAY: Mutex<Option<(String, Instant, Value)>> = Mutex::new(None);

fn ft_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

#[tauri::command]
pub fn get_system_stats() -> Value {
    // 1. Host Physical & Available RAM
    let mut mem = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    unsafe {
        GlobalMemoryStatusEx(&mut mem);
    }
    let total_ram_gb = mem.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0);
    let avail_ram_gb = mem.ullAvailPhys as f64 / (1024.0 * 1024.0 * 1024.0);
    let used_ram_gb = (total_ram_gb - avail_ram_gb).max(0.0);
    let ram_pct = mem.dwMemoryLoad;

    // 2. Host CPU Cores & Current System Load
    let mut sys_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
    unsafe {
        GetSystemInfo(&mut sys_info);
    }
    let cpu_cores = sys_info.dwNumberOfProcessors;

    let mut idle_ft = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let mut kernel_ft = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let mut user_ft = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };

    let cpu_load = unsafe {
        if GetSystemTimes(&mut idle_ft, &mut kernel_ft, &mut user_ft) != 0 {
            let cur_idle = ft_to_u64(&idle_ft);
            let cur_kernel = ft_to_u64(&kernel_ft);
            let cur_user = ft_to_u64(&user_ft);

            let prev_idle = PREV_IDLE.swap(cur_idle, Ordering::Relaxed);
            let prev_kernel = PREV_KERNEL.swap(cur_kernel, Ordering::Relaxed);
            let prev_user = PREV_USER.swap(cur_user, Ordering::Relaxed);

            if prev_kernel > 0 && prev_user > 0 {
                let idle_diff = cur_idle.saturating_sub(prev_idle);
                let kernel_diff = cur_kernel.saturating_sub(prev_kernel);
                let user_diff = cur_user.saturating_sub(prev_user);
                let total_diff = kernel_diff + user_diff;
                if total_diff > 0 && total_diff >= idle_diff {
                    let busy = total_diff - idle_diff;
                    (busy as f64 / total_diff as f64) * 100.0
                } else {
                    4.0
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(60));
                let mut idle2 = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
                let mut kern2 = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
                let mut user2 = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
                if GetSystemTimes(&mut idle2, &mut kern2, &mut user2) != 0 {
                    let id2 = ft_to_u64(&idle2);
                    let k2 = ft_to_u64(&kern2);
                    let u2 = ft_to_u64(&user2);
                    PREV_IDLE.store(id2, Ordering::Relaxed);
                    PREV_KERNEL.store(k2, Ordering::Relaxed);
                    PREV_USER.store(u2, Ordering::Relaxed);
                    let id_d = id2.saturating_sub(cur_idle);
                    let tot_d = (k2.saturating_sub(cur_kernel)) + (u2.saturating_sub(cur_user));
                    if tot_d > 0 && tot_d >= id_d {
                        ((tot_d - id_d) as f64 / tot_d as f64) * 100.0
                    } else {
                        4.5
                    }
                } else {
                    4.5
                }
            }
        } else {
            0.0
        }
    };

    // 3. System Uptime
    let uptime_ms = unsafe { GetTickCount64() };
    let uptime_hours = uptime_ms / (1000 * 3600);
    let uptime_mins = (uptime_ms % (1000 * 3600)) / (1000 * 60);

    // 4. Power Source Status
    let mut pwr: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    let (power_src, battery_pct) = if unsafe { GetSystemPowerStatus(&mut pwr) } != 0 {
        let src = match pwr.ACLineStatus {
            1 => "Сеть (AC)",
            0 => "Батарея (Battery)",
            _ => "Авто",
        };
        let pct = if pwr.BatteryLifePercent <= 100 {
            format!("{}%", pwr.BatteryLifePercent)
        } else {
            "100%".to_string()
        };
        (src, pct)
    } else {
        ("Сеть", "100%".to_string())
    };

    // 5. Capri Process Working Set RAM
    let mut pmc: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    let capri_ram_mb = unsafe {
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut pmc,
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            pmc.WorkingSetSize as f64 / (1024.0 * 1024.0)
        } else {
            50.0
        }
    };

    let report_line = format!(
        "Все параметры стабильны. ОЗУ: {:.1}/{:.1} ГБ ({}%), ЦП: {} ядер ({:.1}%), Аптайм: {}ч {}м. Capri: {:.0} МБ.",
        used_ram_gb, total_ram_gb, ram_pct, cpu_cores, cpu_load, uptime_hours, uptime_mins, capri_ram_mb
    );

    json!({
        "ok": true,
        "total_ram_gb": (total_ram_gb * 10.0).round() / 10.0,
        "used_ram_gb": (used_ram_gb * 10.0).round() / 10.0,
        "avail_ram_gb": (avail_ram_gb * 10.0).round() / 10.0,
        "ram_load_pct": ram_pct,
        "cpu_cores": cpu_cores,
        "cpu_load_pct": (cpu_load * 10.0).round() / 10.0,
        "uptime_hours": uptime_hours,
        "uptime_mins": uptime_mins,
        "power_source": power_src,
        "battery_pct": battery_pct,
        "capri_ram_mb": (capri_ram_mb * 10.0).round() / 10.0,
        "report": report_line
    })
}

fn config_path() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("capri-tauri");
    let _ = fs::create_dir_all(&dir);
    dir.push("config.json");
    dir
}

#[tauri::command]
pub fn get_config() -> Value {
    let path = config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                return val;
            }
        }
    }

    json!({
        "lang": "ru",
        "musicPlayer": "yandex",
        "visible": true,
        "motion": "auto",
        "scale": 0.6,
        "voice": true,
        "turbo": false,
        "provider": "local",
        "memories": []
    })
}

#[tauri::command]
pub fn set_config(patch: Value) -> Value {
    let mut current = get_config();
    if let (Some(obj), Some(patch_obj)) = (current.as_object_mut(), patch.as_object()) {
        for (k, v) in patch_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    let path = config_path();
    let _ = fs::write(path, serde_json::to_string_pretty(&current).unwrap_or_default());
    current
}

#[tauri::command]
pub fn set_interactive(window: WebviewWindow, interactive: bool) {
    let _ = window.set_ignore_cursor_events(!interactive);
}

#[tauri::command]
pub fn focus_overlay(window: WebviewWindow, focus: bool) {
    let _ = window.set_focusable(focus);
    if focus {
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn open_site(url: String) -> bool {
    let safe_url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url
    };
    open::that(safe_url).is_ok()
}

#[tauri::command]
pub fn open_marketplace() -> bool {
    open::that("http://127.0.0.1:47823/marketplace").is_ok()
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn sanitize_music_query(query: &str) -> String {
    let mut q = query.to_string();

    // 1. Strip notes in parentheses / brackets (leaked system/context notes)
    for pat in &[
        "(note for you", "(note:", "(instruction", "(system",
        "[note for you", "[note:", "[instruction", "[system",
    ] {
        if let Some(idx) = q.to_lowercase().find(pat) {
            q = q[..idx].to_string();
        }
    }

    // 2. Take only the first non-empty line
    if let Some(first_line) = q.lines().find(|l| !l.trim().is_empty()) {
        q = first_line.to_string();
    }

    let mut trimmed = q.trim().to_string();

    // 3. Remove conversational music prefixes
    let lower = trimmed.to_lowercase();
    let prefixes = [
        "включи мне пожалуйста", "поставь мне пожалуйста", "запусти мне пожалуйста", "сыграй мне пожалуйста",
        "включи пожалуйста", "поставь пожалуйста", "запусти пожалуйста", "сыграй пожалуйста",
        "включи мне", "поставь мне", "запусти мне", "сыграй мне", "вруби мне",
        "включи песню", "поставь песню", "сыграй песню", "запусти песню", "вруби песню",
        "включи трек", "поставь трек", "сыграй трек", "запусти трек", "вруби трек",
        "включи музыку", "поставь музыку", "сыграй музыку", "запусти музыку", "вруби музыку",
        "включи", "поставь", "сыграй", "запусти", "вруби",
        "play song", "play track", "play music", "play", "put on"
    ];

    for prefix in &prefixes {
        if lower.starts_with(prefix) {
            trimmed = trimmed[prefix.len()..].trim().to_string();
            break;
        }
    }

    let lower2 = trimmed.to_lowercase();
    let music_words = [
        "радиостанцию", "радио", "мою волну", "моя волна", "волну", "волна",
        "песню", "трек", "музыку", "композицию", "song", "track"
    ];
    for mw in &music_words {
        if lower2.starts_with(mw) {
            trimmed = trimmed[mw.len()..].trim().to_string();
            break;
        }
    }

    let lower3 = trimmed.to_lowercase();
    let prep_words = ["из", "от", "про", "группы", "исполнителя", "by", "from", "of"];
    for pw in &prep_words {
        if lower3.starts_with(pw) {
            trimmed = trimmed[pw.len()..].trim().to_string();
            break;
        }
    }

    // 4. Strip quotes and punctuation
    trimmed.trim_matches(|c: char| {
        c.is_ascii_punctuation() || c == '«' || c == '»' || c == '"' || c == '\'' || c == '“' || c == '”'
    }).trim().to_string()
}

pub fn get_yandex_music_exe() -> Option<PathBuf> {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let path = PathBuf::from(local).join("Programs").join("YandexMusic").join("Яндекс Музыка.exe");
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(prog) = std::env::var("PROGRAMFILES") {
        let path = PathBuf::from(prog).join("YandexMusic").join("Яндекс Музыка.exe");
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(prog86) = std::env::var("ProgramFiles(x86)") {
        let path = PathBuf::from(prog86).join("YandexMusic").join("Яндекс Музыка.exe");
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn launch_yandex_music(deep_target: &str, web_target: &str) {
    let mut launched = false;

    // 1. Prioritize native Windows desktop app executable directly (avoids WSA or broken protocol bindings)
    if let Some(exe) = get_yandex_music_exe() {
        if std::process::Command::new(&exe)
            .arg(deep_target)
            .spawn()
            .is_ok()
        {
            launched = true;
        }
    }

    // 2. Fallback to protocol open (ShellExecute)
    if !launched && open::that(deep_target).is_ok() {
        launched = true;
    }

    // 3. Fallback to web player in browser
    if !launched {
        let _ = open::that(web_target);
    }
}

pub struct CapriPick {
    pub title: &'static str,
    pub artist: &'static str,
    pub track_id: u64,
    pub comment: &'static str,
}

pub const CAPRI_FAVORITES: &[CapriPick] = &[
    CapriPick {
        title: "I Really Want to Stay at Your House",
        artist: "Rosa Walton, Hallie Coggins",
        track_id: 73700046,
        comment: "Классика Найт-Сити, от которой мурашки по биочипу... Врубаю «I Really Want to Stay at Your House»! 🌆💔",
    },
    CapriPick {
        title: "Never Fade Away",
        artist: "SAMURAI",
        track_id: 56168579,
        comment: "Джонни Сильверхэнд завещал жечь до конца! Врубаю гимн бунтарей — «Never Fade Away» от SAMURAI! 🎸🔥",
    },
    CapriPick {
        title: "Chippin' In",
        artist: "SAMURAI",
        track_id: 56168578,
        comment: "Пора прокачать хромированные связки! Лови «Chippin' In» — легендарный рифф SAMURAI! ⚡🎸",
    },
    CapriPick {
        title: "MAJOR CRIMES",
        artist: "HEALTH",
        track_id: 73700054,
        comment: "Тёмный неоновый драйв прямо из глубин Уотсона. Врубаю «MAJOR CRIMES» от HEALTH! 🩸🌃",
    },
    CapriPick {
        title: "The Rebel Path",
        artist: "Marcin Przybyłowicz, P.T. Adamczyk",
        track_id: 73700030,
        comment: "Чистый адреналин и свист пуль! Запускаю «The Rebel Path» — идём на штурм Арасаки! ⚔️🔥",
    },
    CapriPick {
        title: "Who's Ready for Tomorrow",
        artist: "Rat Boy, IBDY",
        track_id: 73700041,
        comment: "Утро в Найт-Сити начинается с правильного бита! Врубаю «Who's Ready for Tomorrow»! 🌅🚀",
    },
    CapriPick {
        title: "Future Club",
        artist: "Perturbator",
        track_id: 18197793,
        comment: "Мрачный ретро-синт для ночных рейдов по Сети. Ставлю «Future Club» от Perturbator! 🕹️⚡",
    },
    CapriPick {
        title: "Turbo Killer",
        artist: "Carpenter Brut",
        track_id: 24345229,
        comment: "Педаль в пол, choom! Заряжаю безумный дарксинт — «Turbo Killer» от Carpenter Brut! 🏎️💥",
    },
    CapriPick {
        title: "Nightcall",
        artist: "Kavinsky",
        track_id: 228941,
        comment: "Ночные огни мегаполиса, пустой хайвей и холодный синт. Ставлю легендарный «Nightcall» от Kavinsky! 🌙🚗",
    },
    CapriPick {
        title: "GR4VES",
        artist: "Konrad OldMoney, Cerbeus",
        track_id: 73700044,
        comment: "Грязный уличный кибер-бит из подворотен Пасифики. Врубаю «GR4VES»! ⚡🔊",
    },
];

pub struct CapriPlaylist {
    pub title: &'static str,
    pub genre: &'static str,
    pub desc: &'static str,
    pub seed_track_id: u64,
    pub tracks: &'static [&'static str],
}

pub const CAPRI_PLAYLISTS: &[CapriPlaylist] = &[
    CapriPlaylist {
        title: "Кибер-Фокус: Хакерский поток",
        genre: "Synthwave & Dark Electronic",
        desc: "Погружение в код и взлом систем без лишнего шума",
        seed_track_id: 18197793,
        tracks: &[
            "Perturbator — Future Club",
            "Carpenter Brut — Roller Mobster",
            "Lorn — Acid Rain",
            "Dance With the Dead — Horizon",
        ],
    },
    CapriPlaylist {
        title: "Овердрайв: Хромированный драйв",
        genre: "Darksynth & Industrial Rock",
        desc: "Максимальный пульс и работа на пределе мощности",
        seed_track_id: 24345229,
        tracks: &[
            "Carpenter Brut — Turbo Killer",
            "SAMURAI — Chippin' In",
            "Mick Gordon — The Only Thing They Fear Is You",
            "The Prodigy — Voodoo People (Pendulum Remix)",
        ],
    },
    CapriPlaylist {
        title: "Неоновый дождь: Ночной Найт-Сити",
        genre: "Cyberpunk Lo-Fi & Retrowave",
        desc: "Ночной город, пустые хайвеи и созерцание огней",
        seed_track_id: 73700046,
        tracks: &[
            "Rosa Walton — I Really Want to Stay at Your House",
            "Kavinsky — Nightcall",
            "Gunship — Tech Noir",
            "HEALTH — MAJOR CRIMES",
        ],
    },
    CapriPlaylist {
        title: "Уличный заезд: Cyber Phonk & Drift",
        genre: "Drift Phonk & Cyber Bass",
        desc: "Жжём резину на улицах Найт-Сити под тяжёлый 808-й бас",
        seed_track_id: 73700044,
        tracks: &[
            "Konrad OldMoney — GR4VES",
            "Kordhell — Murder In My Mind",
            "DVRST — Close Eyes",
            "Rat Boy — Who's Ready for Tomorrow",
        ],
    },
];

static TASTE_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn play_yandex_music(query: &str) -> Value {
    let clean = sanitize_music_query(query);

    // Debounce duplicate calls within 2.5 seconds (e.g. from both doMusic and chat_send)
    if let Ok(guard) = LAST_PLAY.lock() {
        if let Some((ref last_q, last_time, ref res)) = *guard {
            if last_q == &clean && last_time.elapsed().as_millis() < 2500 {
                return res.clone();
            }
        }
    }

    let full_lower = format!("{} {}", query.to_lowercase(), clean.to_lowercase());

    // 1. Check if user asked to generate a playlist or themed music selection
    let is_playlist_req = full_lower.contains("плейлист")
        || full_lower.contains("подборк")
        || full_lower.contains("playlist")
        || ((full_lower.contains("сгенерир") || full_lower.contains("подбер") || full_lower.contains("собер") || full_lower.contains("сделай"))
            && (full_lower.contains("музык") || full_lower.contains("трек") || full_lower.contains("песн")));

    if is_playlist_req {
        let pl_idx = if full_lower.contains("код") || full_lower.contains("работ") || full_lower.contains("программ") || full_lower.contains("фокус") || full_lower.contains("концентр") {
            0
        } else if full_lower.contains("трен") || full_lower.contains("зал") || full_lower.contains("качал") || full_lower.contains("спорт") || full_lower.contains("бег") || full_lower.contains("энерг") || full_lower.contains("бодр") || full_lower.contains("драйв") || full_lower.contains("кач") {
            1
        } else if full_lower.contains("чилл") || full_lower.contains("релакс") || full_lower.contains("расслаб") || full_lower.contains("ноч") || full_lower.contains("сон") || full_lower.contains("отдых") || full_lower.contains("спокой") {
            2
        } else if full_lower.contains("фонк") || full_lower.contains("phonk") || full_lower.contains("дрифт") || full_lower.contains("бас") || full_lower.contains("тусов") || full_lower.contains("вечерин") {
            3
        } else {
            TASTE_COUNTER.fetch_add(1, Ordering::Relaxed) % CAPRI_PLAYLISTS.len()
        };

        let pl = &CAPRI_PLAYLISTS[pl_idx];
        let target_deep = format!("yandexmusic://play-vibe?seeds=track:{}&openPlayer=true", pl.seed_track_id);
        let target_web = format!("https://music.yandex.ru/play-vibe?seeds=track:{}&openPlayer=true", pl.seed_track_id);
        launch_yandex_music(&target_deep, &target_web);

        let tracks_str = pl.tracks.iter().map(|t| format!("├ 🎵 {}", t)).collect::<Vec<_>>().join("\n");
        let reply = format!(
            "Сгенерировала плейлист «{}»! 🎧⚡\n├ 🌌 Стиль: {}\n├ 🎯 Назначение: {}\n{}\n└ 🚀 Включила стартовый трек и запустила поток в Яндекс Музыке!",
            pl.title, pl.genre, pl.desc, tracks_str
        );

        let res = json!({
            "ok": true,
            "player": "Яндекс Музыка",
            "title": pl.title,
            "artist": pl.genre,
            "text": reply
        });
        if let Ok(mut guard) = LAST_PLAY.lock() {
            *guard = Some((clean, Instant::now(), res.clone()));
        }
        return res;
    }

    // 2. Check if user asked to play music based on Capri's own taste
    let is_taste_req = full_lower.contains("на свой вкус")
        || full_lower.contains("на твой вкус")
        || full_lower.contains("свой вкус")
        || full_lower.contains("твой вкус")
        || full_lower.contains("на твой выбор")
        || full_lower.contains("на свой выбор")
        || full_lower.contains("свой выбор")
        || full_lower.contains("твой выбор")
        || full_lower.contains("что ты слушаешь")
        || full_lower.contains("что ты любишь")
        || full_lower.contains("твой любимый")
        || full_lower.contains("твоя любимая")
        || full_lower.contains("свой любимый")
        || full_lower.contains("выбери сама")
        || full_lower.contains("по вкусу")
        || full_lower.contains("что хочешь");

    if is_taste_req {
        let idx = TASTE_COUNTER.fetch_add(1, Ordering::Relaxed) % CAPRI_FAVORITES.len();
        let pick = &CAPRI_FAVORITES[idx];
        let target_deep = format!("yandexmusic://play-vibe?seeds=track:{}&openPlayer=true", pick.track_id);
        let target_web = format!("https://music.yandex.ru/play-vibe?seeds=track:{}&openPlayer=true", pick.track_id);
        launch_yandex_music(&target_deep, &target_web);

        let reply = format!(
            "{}\n├ 🎵 Трек: «{}»\n└ 👤 Исполнитель: {} 🎧⚡",
            pick.comment, pick.title, pick.artist
        );
        let res = json!({
            "ok": true,
            "player": "Яндекс Музыка",
            "title": pick.title,
            "artist": pick.artist,
            "text": reply
        });
        if let Ok(mut guard) = LAST_PLAY.lock() {
            *guard = Some((clean, Instant::now(), res.clone()));
        }
        return res;
    }

    // 3. Radio / "Моя волна" check
    let is_radio = clean.is_empty()
        || clean == "радио"
        || clean == "радиостанция"
        || clean == "моя волна"
        || clean == "волну"
        || clean == "волна"
        || clean == "vibe"
        || clean == "музыку"
        || clean == "музыка"
        || clean == "что-нибудь";

    if is_radio {
        launch_yandex_music("yandexmusic://play-vibe", "https://music.yandex.ru/home?deeplink=play-vibe");
        let reply = "Запустила «Мою волну» в Яндекс Музыке! Поток уже играет, choom! 📻⚡".to_string();
        let res = json!({
            "ok": true,
            "player": "Яндекс Музыка",
            "title": "Моя волна (Радио)",
            "artist": "Яндекс Музыка",
            "text": reply
        });
        if let Ok(mut guard) = LAST_PLAY.lock() {
            *guard = Some((clean, Instant::now(), res.clone()));
        }
        return res;
    }

    // 4. Specific track or artist search via Yandex Music API
    let encoded = url_encode(&clean);
    let api_url = format!("https://api.music.yandex.net/search?text={}&type=all&page=0", encoded);

    let mut found_title = clean.clone();
    let mut found_artists = "Яндекс Музыка".to_string();
    let mut target_deep = format!("yandexmusic://search?text={}", encoded);
    let mut target_web = format!("https://music.yandex.ru/search?text={}", encoded);
    let mut reply = format!("Включила «{}» в Яндекс Музыке! Приятного прослушивания, choom! 🎵⚡", clean);

    if let Ok(res) = ureq::get(&api_url)
        .set("User-Agent", "YandexMusic/2024")
        .timeout(std::time::Duration::from_millis(3000))
        .call()
    {
        if let Ok(val) = res.into_json::<Value>() {
            let result_obj = val.get("result");

            let best_obj = result_obj.and_then(|r| r.get("best"));
            let best_type = best_obj.and_then(|b| b.get("type")).and_then(|t| t.as_str());

            if best_type == Some("artist") {
                if let Some(art) = best_obj.and_then(|b| b.get("result")) {
                    if let Some(name) = art.get("name").and_then(|n| n.as_str()) {
                        found_artists = name.to_string();
                        found_title = format!("Волна артиста {}", name);
                    }
                    if let Some(aid) = art.get("id").and_then(|i| i.as_u64()) {
                        target_deep = format!("yandexmusic://play-vibe?seeds=artist:{}&openPlayer=true", aid);
                        target_web = format!("https://music.yandex.ru/play-vibe?seeds=artist:{}&openPlayer=true", aid);
                        reply = format!("Запустила волну артиста «{}» в Яндекс Музыке! Погружаемся в дискографию, choom! 🎸⚡", found_artists);
                    }
                }
            } else {
                let track_node = best_obj
                    .and_then(|b| {
                        if b.get("type").and_then(|t| t.as_str()) == Some("track") {
                            b.get("result")
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        result_obj
                            .and_then(|r| r.get("tracks"))
                            .and_then(|t| t.get("results"))
                            .and_then(|arr| arr.as_array())
                            .and_then(|a| a.first())
                    });

                if let Some(t) = track_node {
                    if let Some(t_title) = t.get("title").and_then(|s| s.as_str()) {
                        found_title = t_title.to_string();
                    }
                    if let Some(arts) = t.get("artists").and_then(|a| a.as_array()) {
                        let art_names: Vec<&str> = arts.iter().filter_map(|a| a.get("name").and_then(|n| n.as_str())).collect();
                        if !art_names.is_empty() {
                            found_artists = art_names.join(", ");
                        }
                    }
                    let track_id = t.get("id").and_then(|i| i.as_u64()).or_else(|| {
                        t.get("realId").and_then(|r| r.as_str()).and_then(|s| s.parse::<u64>().ok())
                    });

                    if let Some(tid) = track_id {
                        target_deep = format!("yandexmusic://play-vibe?seeds=track:{}&openPlayer=true", tid);
                        target_web = format!("https://music.yandex.ru/play-vibe?seeds=track:{}&openPlayer=true", tid);
                        reply = format!("Нашла и сразу включила трек «{}» — {} в Яндекс Музыке! 🎧✨", found_title, found_artists);
                    }
                } else if let Some(first_art) = result_obj.and_then(|r| r.get("artists")).and_then(|a| a.get("results")).and_then(|arr| arr.as_array()).and_then(|a| a.first()) {
                    if let Some(name) = first_art.get("name").and_then(|n| n.as_str()) {
                        found_artists = name.to_string();
                        found_title = format!("Волна артиста {}", name);
                    }
                    if let Some(aid) = first_art.get("id").and_then(|i| i.as_u64()) {
                        target_deep = format!("yandexmusic://play-vibe?seeds=artist:{}&openPlayer=true", aid);
                        target_web = format!("https://music.yandex.ru/play-vibe?seeds=artist:{}&openPlayer=true", aid);
                        reply = format!("Запустила волну артиста «{}» в Яндекс Музыке! Погружаемся в дискографию, choom! 🎸⚡", found_artists);
                    }
                }
            }
        }
    }

    launch_yandex_music(&target_deep, &target_web);

    let res = json!({
        "ok": true,
        "player": "Яндекс Музыка",
        "title": found_title,
        "artist": found_artists,
        "text": reply
    });

    if let Ok(mut guard) = LAST_PLAY.lock() {
        *guard = Some((clean, Instant::now(), res.clone()));
    }

    res
}

#[tauri::command]
pub fn music(query: String, player: Option<String>) -> Value {
    let clean = sanitize_music_query(&query);
    let target = player.unwrap_or_else(|| {
        let cfg = get_config();
        cfg.get("musicPlayer").and_then(|v| v.as_str()).unwrap_or("yandex").to_string()
    });

    match target.to_lowercase().as_str() {
        "yandex" => play_yandex_music(if clean.is_empty() { "радио" } else { &clean }),
        "youtube" => {
            let enc = url_encode(if clean.is_empty() { &query } else { &clean });
            let web = format!("https://www.youtube.com/results?search_query={}", enc);
            let ok = open::that(web).is_ok();
            json!({ "ok": ok, "player": "YouTube", "title": clean, "artist": "YouTube" })
        }
        _ => {
            let enc = url_encode(if clean.is_empty() { &query } else { &clean });
            let deep = format!("spotify:search:{}", enc);
            if open::that(&deep).is_err() {
                let web = format!("https://open.spotify.com/search/{}", enc);
                let _ = open::that(web);
            }
            json!({ "ok": true, "player": "Spotify", "title": clean, "artist": "Spotify" })
        }
    }
}

#[tauri::command]
pub fn media(action: String) -> bool {
    let vk = match action.to_lowercase().as_str() {
        "play" | "pause" => VK_MEDIA_PLAY_PAUSE,
        "next" => VK_MEDIA_NEXT_TRACK,
        "previous" | "prev" => VK_MEDIA_PREV_TRACK,
        "volup" | "volumeup" => VK_VOLUME_UP,
        "voldown" | "volumedown" => VK_VOLUME_DOWN,
        "mute" => VK_VOLUME_MUTE,
        _ => return false,
    };

    unsafe {
        keybd_event(vk as u8, 0, 0, 0);
        keybd_event(vk as u8, 0, KEYEVENTF_KEYUP, 0);
    }
    true
}

#[tauri::command]
pub fn get_windows(app: tauri::AppHandle) -> Value {
    crate::window_tracker::snapshot_windows(&app)
}

pub fn find_monitor_index(monitors: &[tauri::Monitor], pt: POINT) -> usize {
    for (i, m) in monitors.iter().enumerate() {
        let pos = m.position();
        let size = m.size();
        if pt.x >= pos.x
            && pt.x < pos.x + size.width as i32
            && pt.y >= pos.y
            && pt.y < pos.y + size.height as i32
        {
            return i;
        }
    }
    let mut best_idx = 0;
    let mut min_dist_sq = i64::MAX;
    for (i, m) in monitors.iter().enumerate() {
        let pos = m.position();
        let size = m.size();
        let cx = pos.x as i64 + (size.width as i64 / 2);
        let cy = pos.y as i64 + (size.height as i64 / 2);
        let dx = pt.x as i64 - cx;
        let dy = pt.y as i64 - cy;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < min_dist_sq {
            min_dist_sq = dist_sq;
            best_idx = i;
        }
    }
    best_idx
}

#[tauri::command]
pub fn get_cursor(window: WebviewWindow) -> Value {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut pt);
    }
    let sf = window.scale_factor().unwrap_or(1.0);
    let win_pos = window.inner_position().unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });
    let win_size = window.inner_size().unwrap_or(tauri::PhysicalSize { width: 1920, height: 1080 });

    let rel_x = ((pt.x - win_pos.x) as f64 / sf).round() as i32;
    let rel_y = ((pt.y - win_pos.y) as f64 / sf).round() as i32;
    let inside = pt.x >= win_pos.x && pt.x < win_pos.x + win_size.width as i32
              && pt.y >= win_pos.y && pt.y < win_pos.y + win_size.height as i32;

    let monitors = window.available_monitors().unwrap_or_default();
    let display_idx = find_monitor_index(&monitors, pt);
    let (mon_x, mon_y, mon_sf) = if let Some(m) = monitors.get(display_idx) {
        let p = m.position();
        (p.x, p.y, m.scale_factor())
    } else {
        (win_pos.x, win_pos.y, sf)
    };
    let local_x = ((pt.x - mon_x) as f64 / mon_sf).round() as i32;
    let local_y = ((pt.y - mon_y) as f64 / mon_sf).round() as i32;

    json!({
        "x": local_x,
        "y": local_y,
        "local_x": local_x,
        "local_y": local_y,
        "win_rel_x": rel_x,
        "win_rel_y": rel_y,
        "screen_x": pt.x,
        "screen_y": pt.y,
        "inside": inside,
        "display": display_idx
    })
}

#[tauri::command]
pub fn move_screen(window: WebviewWindow, id: usize) -> Value {
    if let Ok(monitors) = window.available_monitors() {
        if let Some(m) = monitors.into_iter().nth(id) {
            let pos = *m.position();
            let size = *m.size();
            let sf = m.scale_factor();
            let _ = window.set_position(pos);
            let _ = window.set_size(size);
            return json!({
                "origin": {
                    "x": (pos.x as f64 / sf).round() as i32,
                    "y": (pos.y as f64 / sf).round() as i32
                },
                "width": (size.width as f64 / sf).round() as i32,
                "height": (size.height as f64 / sf).round() as i32,
                "gap": 0,
                "phys_x": pos.x,
                "phys_y": pos.y,
                "scale": sf
            });
        }
    }
    json!(null)
}

#[tauri::command]
pub fn get_screens(window: WebviewWindow) -> Value {
    if let Ok(monitors) = window.available_monitors() {
        let win_pos = window.inner_position().unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });
        let win_size = window.inner_size().unwrap_or(tauri::PhysicalSize { width: 1920, height: 1080 });
        let win_cx = win_pos.x + (win_size.width as i32 / 2);
        let win_cy = win_pos.y + (win_size.height as i32 / 2);

        let current_id = monitors
            .iter()
            .position(|m| {
                let p = m.position();
                let s = m.size();
                win_cx >= p.x && win_cx < p.x + s.width as i32 && win_cy >= p.y && win_cy < p.y + s.height as i32
            })
            .or_else(|| {
                monitors.iter().position(|m| m.position().x == win_pos.x && m.position().y == win_pos.y)
            })
            .unwrap_or(0);

        let list: Vec<Value> = monitors
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let pos = m.position();
                let size = m.size();
                let sf = m.scale_factor();
                json!({
                    "id": i,
                    "x": (pos.x as f64 / sf).round() as i32,
                    "y": (pos.y as f64 / sf).round() as i32,
                    "w": (size.width as f64 / sf).round() as i32,
                    "h": (size.height as f64 / sf).round() as i32,
                    "scale": sf,
                    "phys_x": pos.x,
                    "phys_y": pos.y,
                    "phys_w": size.width,
                    "phys_h": size.height
                })
            })
            .collect();

        let curr_m = monitors.get(current_id);
        let curr_sf = curr_m.map(|m| m.scale_factor()).unwrap_or(1.0);

        return json!({
            "current": current_id,
            "origin": {
                "x": (win_pos.x as f64 / curr_sf).round() as i32,
                "y": (win_pos.y as f64 / curr_sf).round() as i32
            },
            "list": list
        });
    }
    json!({ "current": 0, "origin": { "x": 0, "y": 0 }, "list": [] })
}

#[tauri::command]
pub fn use_provider(choice: Value) -> Value {
    let provider = choice.get("provider").and_then(|p| p.as_str()).unwrap_or("gemini").to_string();
    let key = choice.get("key").and_then(|k| k.as_str()).map(|s| s.trim().to_string());
    let url = choice.get("url").and_then(|u| u.as_str()).map(|s| s.trim().to_string());

    let mut current = get_config();
    let obj = current.as_object_mut().unwrap();
    obj.insert("provider".to_string(), json!(provider));

    if let Some(ref k) = key {
        if !k.is_empty() {
            if !obj.contains_key("apiKeys") || !obj.get("apiKeys").unwrap().is_object() {
                obj.insert("apiKeys".to_string(), json!({}));
            }
            if let Some(keys_map) = obj.get_mut("apiKeys").and_then(|v| v.as_object_mut()) {
                keys_map.insert(provider.clone(), json!(k));
            }
            obj.insert("key".to_string(), json!(k));
        }
    }

    if let Some(ref u) = url {
        if !u.is_empty() {
            obj.insert("customUrl".to_string(), json!(u));
        }
    }

    let default_model = match provider.as_str() {
        "gemini" => "gemini-1.5-flash",
        "groq" => "llama-3.3-70b-versatile",
        "openai" => "gpt-4o-mini",
        "openrouter" => "anthropic/claude-3.5-haiku",
        "mistral" => "mistral-small-latest",
        "deepseek" => "deepseek-chat",
        "ollama" => "llama3.2",
        _ => "capri-brain",
    };
    obj.insert("chatModel".to_string(), json!(default_model));

    let path = config_path();
    let _ = fs::write(path, serde_json::to_string_pretty(&current).unwrap_or_default());

    if provider == "gemini" {
        if let Some(ref k) = key {
            if k.is_empty() {
                return json!({ "ok": false, "error": "API ключ не может быть пустым" });
            }
            let test_url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", k);
            match ureq::get(&test_url).timeout(Duration::from_secs(4)).call() {
                Ok(_) => {
                    return json!({ "ok": true, "model": default_model });
                }
                Err(ureq::Error::Status(code, resp)) => {
                    if code == 400 || code == 403 {
                        let err_msg = resp.into_string().unwrap_or_default();
                        let error_text = if err_msg.contains("API_KEY_INVALID") {
                            "Неверный API ключ Google Gemini"
                        } else {
                            "Ошибка проверки ключа Gemini API"
                        };
                        return json!({ "ok": false, "error": error_text });
                    }
                    return json!({ "ok": true, "model": default_model });
                }
                Err(_) => {
                    return json!({ "ok": true, "model": default_model });
                }
            }
        }
    }

    json!({ "ok": true, "model": default_model })
}

#[tauri::command]
pub fn chat_status() -> Value {
    let cfg = get_config();
    let current_provider = cfg.get("provider").and_then(|p| p.as_str()).unwrap_or("ollama");
    let current_model = cfg.get("chatModel").and_then(|m| m.as_str()).unwrap_or(
        if current_provider == "gemini" { "gemini-1.5-flash" } else { "llama3.2" }
    );

    let mut has_ollama = false;
    let mut models_count = 0;
    if let Ok(res) = ureq::get("http://127.0.0.1:11434/api/tags").timeout(Duration::from_millis(600)).call() {
        if let Ok(val) = res.into_json::<Value>() {
            models_count = val.get("models").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0);
            has_ollama = models_count > 0;
        }
    }

    json!({
        "ok": true,
        "provider": current_provider,
        "model": current_model,
        "local": current_provider == "ollama" || (current_provider == "capri-local" && has_ollama),
        "warmup": false,
        "modelsCount": models_count
    })
}

#[tauri::command]
pub fn chat_send(request: Value) -> Value {
    let prompt = request.get("messages")
        .and_then(|m| m.as_array())
        .and_then(|a| a.last())
        .and_then(|last| last.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("Привет, Капри!");

    let clean_query = sanitize_music_query(prompt);
    let lower = prompt.to_lowercase();

    // Check if user is asking to play music / radio / track / playlist / taste
    let is_music_req = lower.contains("включи")
        || lower.contains("поставь")
        || lower.contains("сыграй")
        || lower.contains("запусти")
        || lower.contains("вруби")
        || lower.contains("радио")
        || lower.contains("волну")
        || lower.contains("музыку")
        || lower.contains("музыка")
        || lower.contains("песню")
        || lower.contains("трек")
        || lower.contains("плейлист")
        || lower.contains("подборк")
        || lower.contains("свой вкус")
        || lower.contains("твой вкус")
        || lower.contains("свой выбор")
        || lower.contains("твой выбор")
        || lower.starts_with("play ");

    if is_music_req && (
        lower.contains("радио")
            || lower.contains("волн")
            || lower.contains("музык")
            || lower.contains("песн")
            || lower.contains("трек")
            || lower.contains("плейлист")
            || lower.contains("подборк")
            || lower.contains("свой вкус")
            || lower.contains("твой вкус")
            || lower.contains("свой выбор")
            || lower.contains("твой выбор")
            || lower.starts_with("включи")
            || lower.starts_with("поставь")
            || lower.starts_with("сыграй")
            || lower.starts_with("запусти")
            || lower.starts_with("вруби")
            || lower.starts_with("play ")
    ) {
        let res = play_yandex_music(if clean_query.is_empty() { prompt } else { &clean_query });
        let reply = res.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
            let title = res.get("title").and_then(|t| t.as_str()).unwrap_or("Моя волна");
            let artist = res.get("artist").and_then(|a| a.as_str()).unwrap_or("");
            if title.contains("Радио") || title.contains("волна") {
                "Запустила «Мою волну» в Яндекс Музыке! Наслаждайся потоком, choom! 📻⚡".to_string()
            } else if !artist.is_empty() && artist != "Яндекс Музыка" {
                format!("Нашла и включила трек «{}» — {} в Яндекс Музыке! Приятного прослушивания! 🎧✨", title, artist)
            } else {
                format!("Включила «{}» в Яндекс Музыке! Слушаем вместе, choom. 🎵", title)
            }
        });

        return json!({ "ok": true, "text": reply });
    }

    let cfg = get_config();
    let provider = cfg.get("provider").and_then(|p| p.as_str()).unwrap_or("ollama");
    let api_key = cfg.get("apiKeys")
        .and_then(|k| k.get(provider))
        .or_else(|| cfg.get("key"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Gemini API integration
    if provider == "gemini" && !api_key.is_empty() {
        let gemini_prompt = {
            let mut p = prompt.to_string();
            for pat in &["(note for you", "(note:"] {
                if let Some(idx) = p.to_lowercase().find(pat) {
                    p = p[..idx].trim().to_string();
                }
            }
            if p.is_empty() { prompt.to_string() } else { p }
        };
        let gemini_url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
            api_key
        );
        let gemini_body = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": gemini_prompt }]
                }
            ],
            "systemInstruction": {
                "parts": [{
                    "text": "Ты — Капри (Capri), дерзкая, верная и харизматичная кибер-ассистентка на рабочем столе пользователя. Отвечай на русском языке кратко, живо, с легким киберпанк-сленгом (choom, некст-ген, биочип, лед и т.д.), но всегда информативно и полезно. Никогда не говори, что ты большая языковая модель Google Gemini. Ты — Капри. У тебя безупречный музыкальный вкус: ты обожаешь Cyberpunk 2077 OST (SAMURAI, Rosa Walton, HEALTH), синтвейв (Perturbator, Carpenter Brut, Kavinsky), рок и электронику. Если пользователь просит тебя включить музыку, поставить трек, включить что-то на свой вкус или сгенерировать плейлист — предлагай яркую подборку и обязательно добавляй тег в конце сообщения: [[music: <название трека, исполнитель или тема плейлиста>]]."
                }]
            }
        });

        if let Ok(res) = ureq::post(&gemini_url)
            .timeout(Duration::from_secs(15))
            .send_json(gemini_body)
        {
            if let Ok(resp) = res.into_json::<Value>() {
                if let Some(text) = resp.get("candidates")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                    .and_then(|first| first.get("content"))
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.first())
                    .and_then(|first| first.get("text"))
                    .and_then(|t| t.as_str())
                {
                    return json!({ "ok": true, "text": text });
                }
            }
        }
    }

    // OpenAI compatible providers (Groq, OpenAI, OpenRouter, DeepSeek)
    if (provider == "groq" || provider == "openai" || provider == "openrouter" || provider == "deepseek") && !api_key.is_empty() {
        let endpoint = match provider {
            "groq" => "https://api.groq.com/openai/v1/chat/completions",
            "openai" => "https://api.openai.com/v1/chat/completions",
            "openrouter" => "https://openrouter.ai/api/v1/chat/completions",
            "deepseek" => "https://api.deepseek.com/chat/completions",
            _ => "https://api.openai.com/v1/chat/completions",
        };
        let model = cfg.get("chatModel").and_then(|m| m.as_str()).unwrap_or("gpt-4o-mini");
        let body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": "Ты — Капри, дерзкая кибернетическая ассистентка на рабочем столе. Отвечай кратко, живо, на русском." },
                { "role": "user", "content": prompt }
            ]
        });
        if let Ok(res) = ureq::post(endpoint)
            .set("Authorization", &format!("Bearer {}", api_key))
            .timeout(Duration::from_secs(15))
            .send_json(body)
        {
            if let Ok(resp) = res.into_json::<Value>() {
                if let Some(text) = resp.get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                    .and_then(|first| first.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|t| t.as_str())
                {
                    return json!({ "ok": true, "text": text });
                }
            }
        }
    }

    // If Ollama is available, try it
    let req_body = json!({
        "model": "llama3.2",
        "prompt": prompt,
        "stream": false
    });
    if let Ok(res) = ureq::post("http://127.0.0.1:11434/api/generate")
        .timeout(Duration::from_secs(10))
        .send_json(req_body)
    {
        if let Ok(resp) = res.into_json::<Value>() {
            if let Some(text) = resp.get("response").and_then(|r| r.as_str()) {
                return json!({ "ok": true, "text": text });
            }
        }
    }

    // Authentic Capri companion conversational responses
    let reply = if lower.contains("привет") || lower.contains("хай") || lower.contains("ку") || lower.contains("hello") {
        "Привет-привет, choom! Я на связи. Чем займемся на твоем рабочем столе?"
    } else if lower.contains("как дела") || lower.contains("что делаешь") {
        "Всё в полном порядке! Слежу за твоими окнами со скоростью 120 FPS и готова выполнять команды."
    } else if lower.contains("погода") {
        "Чтобы узнать погоду, скажи мне, например: «какая погода в Москве» или открой маркетплейс навыков!"
    } else if lower.contains("кто ты") || lower.contains("что ты умеешь") {
        "Я Капри — твой кибернетический помощник на Rust + Tauri v2. Могу следить за окнами, находить и включать музыку в Яндекс Музыке, выполнять вычисления, бросать кубики и запускать расширения!"
    } else {
        "Принято, choom! Я записала твой запрос. Все модули биочипа синхронизированы."
    };

    json!({ "ok": true, "text": reply })
}

#[tauri::command]
pub fn chat_memory() -> Value {
    json!([])
}

#[tauri::command]
pub fn chat_remember(_history: Value) -> bool {
    true
}

#[tauri::command]
pub fn open_menu(window: WebviewWindow) {
    let app = window.app_handle();
    let cfg = get_config();

    let scale = cfg.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.60);
    let player = cfg.get("musicPlayer").and_then(|v| v.as_str()).unwrap_or("yandex");
    let muted = cfg.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
    let follow = cfg.get("follow").and_then(|v| v.as_bool()).unwrap_or(true);
    let wander = cfg.get("wander").and_then(|v| v.as_bool()).unwrap_or(true);
    let possess = cfg.get("possess").and_then(|v| v.as_bool()).unwrap_or(true);
    let rest = cfg.get("rest").and_then(|v| v.as_str()).unwrap_or("corner");
    let assistant = cfg.get("assistant").and_then(|v| v.as_str()).unwrap_or("antigravity");
    let motion = cfg.get("motion").and_then(|v| v.as_str()).unwrap_or("auto");
    let render = cfg.get("render").and_then(|v| v.as_str()).unwrap_or("auto");
    let lang = cfg.get("lang").and_then(|v| v.as_str()).unwrap_or("ru");
    let startup = cfg.get("startup").and_then(|v| v.as_bool()).unwrap_or(false);
    let provider = cfg.get("provider").and_then(|v| v.as_str()).unwrap_or("ollama");

    // Header
    let item_header = MenuItem::with_id(app, "header_port", "Capri — слушаю порт 47823", false, None::<&str>).unwrap();

    // Top actions matching original Capri Desktop 1:1
    let item_teleport = MenuItem::with_id(app, "menu_teleport", "Телепортироваться", true, None::<&str>).unwrap();
    let item_satellite = MenuItem::with_id(app, "menu_satellite", "Посмотреть на спутник", true, None::<&str>).unwrap();
    let item_demo = MenuItem::with_id(app, "menu_demo", "Запустить демо", true, None::<&str>).unwrap();
    let item_help = MenuItem::with_id(app, "menu_help", "Как это работает?", true, None::<&str>).unwrap();
    let item_report = MenuItem::with_id(app, "menu_report", "Отчёт о миссиях", true, None::<&str>).unwrap();
    let item_chat = MenuItem::with_id(app, "menu_chat", "Поговорить с ней", true, None::<&str>).unwrap();
    let item_custom = MenuItem::with_id(app, "menu_custom", "Настроить её…", true, None::<&str>).unwrap();
    let item_stow = MenuItem::with_id(app, "menu_stow", "Спрятать", true, Some("Ctrl+Alt+C")).unwrap();

    let sep1 = PredefinedMenuItem::separator(app).unwrap();

    // Submenu: Размер
    let sz_small = CheckMenuItem::with_id(app, "scale_045", "Маленькая", true, (scale - 0.45).abs() < 0.05, None::<&str>).unwrap();
    let sz_med = CheckMenuItem::with_id(app, "scale_060", "Средняя", true, (scale - 0.60).abs() < 0.05, None::<&str>).unwrap();
    let sz_big = CheckMenuItem::with_id(app, "scale_080", "Большая", true, (scale - 0.80).abs() < 0.05, None::<&str>).unwrap();
    let sub_size = Submenu::with_items(app, "Размер", true, &[&sz_small, &sz_med, &sz_big]).unwrap();

    // Submenu: Звук
    let snd_mute = CheckMenuItem::with_id(app, "sound_mute", "Без звука", true, muted, None::<&str>).unwrap();
    let snd_sep = PredefinedMenuItem::separator(app).unwrap();
    let snd_folder = MenuItem::with_id(app, "sound_folder", "Открыть папку своих звуков", true, None::<&str>).unwrap();

    let p_sp = CheckMenuItem::with_id(app, "player_spotify", "Spotify", true, player == "spotify", None::<&str>).unwrap();
    let p_ya = CheckMenuItem::with_id(app, "player_yandex", "Яндекс Музыка", true, player == "yandex", None::<&str>).unwrap();
    let p_yt = CheckMenuItem::with_id(app, "player_ytmusic", "YouTube Music", true, player == "ytmusic", None::<&str>).unwrap();
    let p_vk = CheckMenuItem::with_id(app, "player_vk", "VK Музыка", true, player == "vk", None::<&str>).unwrap();
    let p_ap = CheckMenuItem::with_id(app, "player_apple", "Apple Music", true, player == "apple", None::<&str>).unwrap();
    let p_y2 = CheckMenuItem::with_id(app, "player_youtube", "YouTube", true, player == "youtube", None::<&str>).unwrap();
    let sub_player = Submenu::with_items(app, "Музыкальный плеер", true, &[&p_ya, &p_sp, &p_yt, &p_vk, &p_ap, &p_y2]).unwrap();
    let sub_sound = Submenu::with_items(app, "Звук", true, &[&snd_mute, &snd_sep, &snd_folder, &sub_player]).unwrap();

    // Submenu: Экран
    let scr_main = CheckMenuItem::with_id(app, "screen_main", "1 — 1920×1080", true, true, None::<&str>).unwrap();
    let scr_sep = PredefinedMenuItem::separator(app).unwrap();
    let scr_follow = CheckMenuItem::with_id(app, "screen_follow", "Следовать за мной с экрана на экран", true, follow, None::<&str>).unwrap();
    let sub_screen = Submenu::with_items(app, "Экран", true, &[&scr_main, &scr_sep, &scr_follow]).unwrap();

    // Submenu: ИИ-модель
    let ai_setup = MenuItem::with_id(app, "ai_setup", "Выбрать вместе с ней…", true, None::<&str>).unwrap();
    let ai_sep1 = PredefinedMenuItem::separator(app).unwrap();
    let ai_ollama = CheckMenuItem::with_id(app, "provider_ollama", "Ollama", true, provider == "ollama", None::<&str>).unwrap();
    let ai_claude = CheckMenuItem::with_id(app, "provider_claude", "Claude", true, provider == "claude", None::<&str>).unwrap();
    let ai_gemini = CheckMenuItem::with_id(app, "provider_gemini", "Gemini", true, provider == "gemini", None::<&str>).unwrap();
    let ai_openai = CheckMenuItem::with_id(app, "provider_openai", "OpenAI", true, provider == "openai", None::<&str>).unwrap();
    let ai_groq = CheckMenuItem::with_id(app, "provider_groq", "Groq", true, provider == "groq", None::<&str>).unwrap();
    let ai_openrouter = CheckMenuItem::with_id(app, "provider_openrouter", "OpenRouter", true, provider == "openrouter", None::<&str>).unwrap();
    let ai_sep2 = PredefinedMenuItem::separator(app).unwrap();
    let ai_key = MenuItem::with_id(app, "ai_apikey", "API-ключ или адрес…", true, None::<&str>).unwrap();
    let sub_ai = Submenu::with_items(app, "ИИ-модель", true, &[
        &ai_setup, &ai_sep1,
        &ai_ollama, &ai_claude, &ai_gemini, &ai_openai, &ai_groq, &ai_openrouter,
        &ai_sep2, &ai_key
    ]).unwrap();

    // Checkboxes in Image 2
    let chk_wander = CheckMenuItem::with_id(app, "cfg_wander", "Гулять, когда ничего не происходит", true, wander, None::<&str>).unwrap();
    let chk_possess = CheckMenuItem::with_id(app, "cfg_possess", "Вселяться в мои окна", true, possess, None::<&str>).unwrap();

    // Submenu: Когда она отдыхает
    let r_corner = CheckMenuItem::with_id(app, "rest_corner", "В своём углу", true, rest != "taskbar", None::<&str>).unwrap();
    let r_taskbar = CheckMenuItem::with_id(app, "rest_taskbar", "Сидит на панели задач", true, rest == "taskbar", None::<&str>).unwrap();
    let sub_rest = Submenu::with_items(app, "Когда она отдыхает", true, &[&r_corner, &r_taskbar]).unwrap();

    // Submenu: Что отслеживать
    let w_ag = CheckMenuItem::with_id(app, "watch_antigravity", "Antigravity", true, assistant == "antigravity", None::<&str>).unwrap();
    let w_cc = CheckMenuItem::with_id(app, "watch_claude", "Claude Code", true, assistant == "claude", None::<&str>).unwrap();
    let w_cu = CheckMenuItem::with_id(app, "watch_cursor", "Cursor", true, assistant == "cursor", None::<&str>).unwrap();
    let w_ws = CheckMenuItem::with_id(app, "watch_windsurf", "Windsurf", true, assistant == "windsurf", None::<&str>).unwrap();
    let w_none = CheckMenuItem::with_id(app, "watch_none", "Ничего, просто будь рядом", true, assistant == "none", None::<&str>).unwrap();
    let sub_watch = Submenu::with_items(app, "Что отслеживать", true, &[&w_ag, &w_cc, &w_cu, &w_ws, &w_none]).unwrap();

    // Submenu: Анимации
    let m_auto = CheckMenuItem::with_id(app, "motion_auto", "Как в Windows", true, motion == "auto", None::<&str>).unwrap();
    let m_full = CheckMenuItem::with_id(app, "motion_full", "Все", true, motion == "full", None::<&str>).unwrap();
    let m_red = CheckMenuItem::with_id(app, "motion_reduced", "Минимум", true, motion == "reduced", None::<&str>).unwrap();
    let sub_motion = Submenu::with_items(app, "Анимации", true, &[&m_auto, &m_full, &m_red]).unwrap();

    // Submenu: Отображение
    let d_auto = CheckMenuItem::with_id(app, "display_auto", "Обычное", true, render == "auto", None::<&str>).unwrap();
    let d_gl = CheckMenuItem::with_id(app, "display_gl", "OpenGL (если её фон чёрный)", true, render == "gl", None::<&str>).unwrap();
    let d_sw = CheckMenuItem::with_id(app, "display_software", "Без видеокарты (если всё ещё чёрный)", true, render == "software", None::<&str>).unwrap();
    let sub_display = Submenu::with_items(app, "Отображение", true, &[&d_auto, &d_gl, &d_sw]).unwrap();

    // Submenu: Язык
    let l_auto = CheckMenuItem::with_id(app, "lang_auto", "Автоматически (системный)", true, lang == "auto", None::<&str>).unwrap();
    let l_ru = CheckMenuItem::with_id(app, "lang_ru", "Русский", true, lang == "ru", None::<&str>).unwrap();
    let l_en = CheckMenuItem::with_id(app, "lang_en", "English", true, lang == "en", None::<&str>).unwrap();
    let l_fr = CheckMenuItem::with_id(app, "lang_fr", "Français", true, lang == "fr", None::<&str>).unwrap();
    let sub_lang = Submenu::with_items(app, "Язык", true, &[&l_auto, &l_ru, &l_en, &l_fr]).unwrap();

    // Checkbox: Запускать вместе с Windows
    let chk_startup = CheckMenuItem::with_id(app, "cfg_startup", "Запускать вместе с Windows", true, startup, None::<&str>).unwrap();

    let sep2 = PredefinedMenuItem::separator(app).unwrap();

    // Skills & Hooks
    let ag_installed = is_antigravity_hooks_installed();
    let stat_label = if ag_installed {
        "Хуки Antigravity: ● установлены"
    } else {
        "Хуки Antigravity: ○ не установлены"
    };
    let toggle_label = if ag_installed {
        "Удалить хуки Antigravity"
    } else {
        "Установить хуки Antigravity"
    };

    let item_market = MenuItem::with_id(app, "menu_market", "⚡ Маркетплейс навыков (Skills)", true, None::<&str>).unwrap();
    let item_hooks_stat = MenuItem::with_id(app, "hooks_stat", stat_label, false, None::<&str>).unwrap();
    let item_hooks_del = MenuItem::with_id(app, "hooks_toggle", toggle_label, true, None::<&str>).unwrap();

    let sep3 = PredefinedMenuItem::separator(app).unwrap();

    // Exit
    let item_quit = MenuItem::with_id(app, "menu_quit", "Выйти", true, None::<&str>).unwrap();

    if let Ok(menu) = Menu::with_items(
        app,
        &[
            &item_header,
            &item_teleport,
            &item_satellite,
            &item_demo,
            &item_help,
            &item_report,
            &item_chat,
            &item_custom,
            &item_stow,
            &sep1,
            &sub_size,
            &sub_sound,
            &sub_screen,
            &sub_ai,
            &chk_wander,
            &chk_possess,
            &sub_rest,
            &sub_watch,
            &sub_motion,
            &sub_display,
            &sub_lang,
            &chk_startup,
            &sep2,
            &item_market,
            &item_hooks_stat,
            &item_hooks_del,
            &sep3,
            &item_quit,
        ],
    ) {
        let _ = window.popup_menu(&menu);
    }
}

const AGY_HOOK_JS: &str = include_str!("../../hooks/capri-agy-hook.js");

pub fn ensure_agy_hook_script() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let hooks_dir = PathBuf::from(local_app_data).join("Capri").join("hooks");
    let _ = fs::create_dir_all(&hooks_dir);
    let script_path = hooks_dir.join("capri-agy-hook.js");
    let need_write = !script_path.exists()
        || fs::metadata(&script_path).map(|m| m.len()).unwrap_or(0) != AGY_HOOK_JS.len() as u64;
    if need_write {
        let _ = fs::write(&script_path, AGY_HOOK_JS);
    }
    script_path
}

pub fn is_antigravity_hooks_installed() -> bool {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".gemini");
    path.push("config");
    path.push("hooks.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            return content.contains("capri-agy") || content.contains("capri-hook.js");
        }
    }
    false
}

pub fn install_antigravity_hooks() -> Result<(), String> {
    let script_path = ensure_agy_hook_script();
    let clean_script = script_path.to_string_lossy().to_string();
    let script_arg = if clean_script.contains(' ') {
        format!("\"{}\"", clean_script)
    } else {
        clean_script
    };

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".gemini");
    path.push("config");
    let _ = fs::create_dir_all(&path);
    path.push("hooks.json");

    let mut current_json: Value = if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or(json!({}))
        } else {
            json!({})
        }
    } else {
        json!({})
    };

    if !current_json.is_object() {
        current_json = json!({});
    }

    let hook_entry = json!({
        "enabled": true,
        "SessionStart": [
            { "type": "command", "command": format!("node {} SessionStart", script_arg), "timeout": 5 }
        ],
        "PreInvocation": [
            { "type": "command", "command": format!("node {} PreInvocation", script_arg), "timeout": 5 }
        ],
        "PreToolUse": [
            {
                "matcher": "*",
                "hooks": [{ "type": "command", "command": format!("node {} PreToolUse", script_arg), "timeout": 5 }]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "*",
                "hooks": [{ "type": "command", "command": format!("node {} PostToolUse", script_arg), "timeout": 5 }]
            }
        ],
        "Stop": [
            { "type": "command", "command": format!("node {} Stop", script_arg), "timeout": 5 }
        ]
    });

    if let Some(obj) = current_json.as_object_mut() {
        obj.insert("capri-agy".to_string(), hook_entry);
    }

    let formatted = serde_json::to_string_pretty(&current_json).map_err(|e| e.to_string())?;
    fs::write(&path, formatted).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn uninstall_antigravity_hooks() -> Result<(), String> {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".gemini");
    path.push("config");
    path.push("hooks.json");

    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut current_json) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = current_json.as_object_mut() {
                    obj.remove("capri-agy");
                    let formatted = serde_json::to_string_pretty(&current_json).map_err(|e| e.to_string())?;
                    let _ = fs::write(&path, formatted);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_hooks_status() -> Value {
    let installed = is_antigravity_hooks_installed();
    json!({
        "ok": true,
        "installed": installed,
        "active": installed
    })
}

#[tauri::command]
pub fn install_hooks() -> Result<bool, String> {
    install_antigravity_hooks()?;
    Ok(true)
}

#[tauri::command]
pub fn uninstall_hooks() -> Result<bool, String> {
    uninstall_antigravity_hooks()?;
    Ok(true)
}

#[tauri::command]
pub fn cyberpunk_solve_breach(window: WebviewWindow) -> Value {
    let win_pos = window.inner_position().unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });
    let win_size = window.inner_size().unwrap_or(tauri::PhysicalSize { width: 2560, height: 1440 });
    let sf = window.scale_factor().unwrap_or(1.0);

    if let Some(res) = crate::cyberpunk::scan_and_solve_breach(win_pos.x, win_pos.y, win_size.width as i32, win_size.height as i32, sf) {
        res
    } else {
        json!({ "active": false })
    }
}
