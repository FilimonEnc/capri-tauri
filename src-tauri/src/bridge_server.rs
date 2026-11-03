use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Response, Server, StatusCode};

// Embedded static marketplace assets for 100% reliable standalone / portable execution
const MARKETPLACE_HTML: &str = include_str!("../../src/marketplace/index.html");
const MARKETPLACE_CSS: &str = include_str!("../../src/marketplace/marketplace.css");
const MARKETPLACE_JS: &str = include_str!("../../src/marketplace/marketplace.js");

const CAPRI_CUTOUT: &[u8] = include_bytes!("../../src/assets/capri/capri-cutout.png");
const CAPRI_SMUG: &[u8] = include_bytes!("../../src/assets/capri/face-smug.png");
const CAPRI_LAUGH: &[u8] = include_bytes!("../../src/assets/capri/face-laugh.png");
const CAPRI_CALM: &[u8] = include_bytes!("../../src/assets/capri/face-calm.png");
const CAPRI_WIDE: &[u8] = include_bytes!("../../src/assets/capri/face-wide.png");
const CAPRI_EYES: &[u8] = include_bytes!("../../src/assets/capri/cutin-eyes.png");
const CAPRI_FULL: &[u8] = include_bytes!("../../src/assets/capri/capri-full.png");

#[allow(dead_code)]
pub struct BridgeServer {
    running: Arc<AtomicBool>,
}

impl BridgeServer {
    pub fn start(app: AppHandle) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        thread::spawn(move || {
            let server = match Server::http("127.0.0.1:47823") {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[bridge] Failed to bind 127.0.0.1:47823: {}", e);
                    return;
                }
            };

            while r.load(Ordering::Relaxed) {
                let mut request = match server.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(Some(req)) => req,
                    Ok(None) => continue,
                    Err(_) => break,
                };

                let url = request.url().to_string();
                let method = request.method().as_str().to_uppercase();

                // Standard CORS headers
                let cors_headers = vec![
                    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap(),
                    Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type"[..]).unwrap(),
                ];

                if method == "OPTIONS" {
                    let mut res = Response::empty(StatusCode(204));
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // GET /health
                if method == "GET" && (url == "/health" || url.starts_with("/health?")) {
                    let body = json!({ "ok": true, "name": "capri", "pid": std::process::id() }).to_string();
                    let mut res = Response::from_string(body);
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // POST /event - Claude / Antigravity / Codex hook events
                if method == "POST" && url == "/event" {
                    let mut content = String::new();
                    let _ = request.as_reader().read_to_string(&mut content);

                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        let _ = app.emit("claude:event", val);
                    }

                    let mut res = Response::from_string("{\"ok\":true}");
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // GET / POST /api/skills/reload
                if url.starts_with("/api/skills/reload") {
                    let _ = app.emit("skills:reload", json!({ "status": "ok" }));
                    let mut res = Response::from_string(json!({ "ok": true, "reloaded": true }).to_string());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // GET /api/skills - with persisted installed / enabled state
                if method == "GET" && url.starts_with("/api/skills") {
                    let skills_json = get_skills_with_saved_state();
                    let mut res = Response::from_string(skills_json.to_string());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // POST /api/skills/install, uninstall, toggle
                if method == "POST" && url.starts_with("/api/skills/") {
                    let action = if url.starts_with("/api/skills/install") {
                        "install"
                    } else if url.starts_with("/api/skills/uninstall") {
                        "uninstall"
                    } else if url.starts_with("/api/skills/toggle") {
                        "toggle"
                    } else {
                        "unknown"
                    };

                    let mut body_str = String::new();
                    let _ = request.as_reader().read_to_string(&mut body_str);
                    if let Ok(body_json) = serde_json::from_str::<Value>(&body_str) {
                        if let Some(id) = body_json.get("id").and_then(|v| v.as_str()) {
                            let cfg = crate::commands::get_config();
                            let mut skills_map = cfg.get("skills")
                                .and_then(|v| v.as_object().cloned())
                                .unwrap_or_default();

                            let mut current_state = skills_map.get(id)
                                .and_then(|v| v.as_object().cloned())
                                .unwrap_or_default();

                            match action {
                                "install" => {
                                    current_state.insert("installed".into(), json!(true));
                                    current_state.insert("enabled".into(), json!(true));
                                }
                                "uninstall" => {
                                    current_state.insert("installed".into(), json!(false));
                                    current_state.insert("enabled".into(), json!(false));
                                }
                                "toggle" => {
                                    let en = body_json.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                                    current_state.insert("enabled".into(), json!(en));
                                    if en && !current_state.get("installed").and_then(|v| v.as_bool()).unwrap_or(false) {
                                        current_state.insert("installed".into(), json!(true));
                                    }
                                }
                                _ => {}
                            }

                            skills_map.insert(id.to_string(), Value::Object(current_state));
                            let _ = crate::commands::set_config(json!({ "skills": skills_map }));
                        }
                    }

                    let mut res = Response::from_string("{\"ok\":true}");
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // GET /api/system/stats - Real Host Diagnostics for Biomonitor
                if method == "GET" && url.starts_with("/api/system/stats") {
                    let stats = crate::commands::get_system_stats();
                    let mut res = Response::from_string(stats.to_string());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // Skills Marketplace Static Files (Embedded into executable)
                let clean_url = url.split('?').next().unwrap_or(&url);
                if clean_url == "/marketplace" || clean_url == "/marketplace/" || clean_url == "/marketplace/index.html" {
                    let mut res = Response::from_string(MARKETPLACE_HTML);
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                if clean_url == "/marketplace/marketplace.css" || clean_url == "/marketplace.css" || clean_url.ends_with(".css") {
                    let mut res = Response::from_string(MARKETPLACE_CSS);
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"text/css; charset=utf-8"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                if clean_url == "/marketplace/marketplace.js" || clean_url == "/marketplace.js" || clean_url.ends_with("marketplace.js") {
                    let mut res = Response::from_string(MARKETPLACE_JS);
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/javascript; charset=utf-8"[..]).unwrap());
                    for h in cors_headers {
                        res.add_header(h);
                    }
                    let _ = request.respond(res);
                    continue;
                }

                // Embedded Images
                if clean_url.ends_with("capri-cutout.png") {
                    let mut res = Response::from_data(CAPRI_CUTOUT.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("face-smug.png") {
                    let mut res = Response::from_data(CAPRI_SMUG.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("face-laugh.png") {
                    let mut res = Response::from_data(CAPRI_LAUGH.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("face-calm.png") {
                    let mut res = Response::from_data(CAPRI_CALM.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("face-wide.png") {
                    let mut res = Response::from_data(CAPRI_WIDE.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("cutin-eyes.png") {
                    let mut res = Response::from_data(CAPRI_EYES.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }
                if clean_url.ends_with("capri-full.png") {
                    let mut res = Response::from_data(CAPRI_FULL.to_vec());
                    res.add_header(Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap());
                    for h in cors_headers { res.add_header(h); }
                    let _ = request.respond(res);
                    continue;
                }

                // Default 404
                let mut res = Response::from_string("{\"error\":\"Not found\"}");
                res = res.with_status_code(StatusCode(404));
                for h in cors_headers {
                    res.add_header(h);
                }
                let _ = request.respond(res);
            }
        });

        Self { running }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

pub fn get_default_skills() -> Value {
    json!([
        {
            "id": "breach_protocol",
            "name": "Arasaka Breach Protocol Solver // Авто-взлом 2077",
            "description": "Нейросетевой взломщик мини-игры Cyberpunk 2077: распознает матрицу 6x6, находит идеальный маршрут через граф и подсвечивает путь неоновыми молниями прямо поверх экрана игры.",
            "category": "fun",
            "author": "NETRUNNER: T-BUG SPEC",
            "version": "1.0.0",
            "icon": "⚡",
            "tags": ["cyberpunk", "breach", "hack", "взлом", "киберпанк", "мини-игра"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "weather",
            "name": "Night City Meteo-Radar // Погода",
            "description": "Спутниковый радар погоды, влажности и скорости ветра в любой точке мира.",
            "category": "utilities",
            "author": "CORPO: ARASAKA METEO",
            "version": "2.1.0",
            "icon": "🌤️",
            "tags": ["weather", "radar", "online", "погода"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "calculator",
            "name": "Cyberdeck Math Matrix // Калькулятор",
            "description": "Квантовый сопроцессор для мгновенных вычислений и конвертации евродолларов ($E).",
            "category": "productivity",
            "author": "NETRUNNER: KIWI SPEC",
            "version": "3.0.0",
            "icon": "🧮",
            "tags": ["math", "calc", "matrix", "числа"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "coin_dice",
            "name": "Haywood RNG Ripper // Рулетка Судьбы",
            "description": "Уличный генератор случайности: бросок кубиков D6/D20, монетка или жеребьевка решений.",
            "category": "fun",
            "author": "STREET: REBECCA ARSENAL",
            "version": "1.4.0",
            "icon": "🎲",
            "tags": ["dice", "coin", "random", "хейвуд"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "cyber_quotes",
            "name": "Edgerunners Chronicles // Хроники 2077",
            "description": "Культовые цитаты Джонни Сильверхенда, Дэвида Мартинеса, Люси и мудрость улиц Найт-Сити.",
            "category": "fun",
            "author": "LEGEND: AFTERLIFE CLUB",
            "version": "2.5.0",
            "icon": "⚡",
            "tags": ["quotes", "lore", "silverhand", "afterlife"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "dev_tools",
            "name": "Netrunner ICE-Breaker // Инструменты Взлома",
            "description": "Генератор UUID v4, кодирование/декодирование Base64, расчет Unix timestamp.",
            "category": "development",
            "author": "CYBER: NETWATCH ROGUE",
            "version": "3.2.0",
            "icon": "💻",
            "tags": ["uuid", "base64", "dev", "icebreaker"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "music_controller",
            "name": "Night City Radio // Радиостанции",
            "description": "Нативное управление Яндекс.Музыкой, Spotify и YouTube: треки, громкость, поиск.",
            "category": "productivity",
            "author": "STATION: MORRO ROCK RADIO",
            "version": "2.0.0",
            "icon": "🎵",
            "tags": ["music", "radio", "spotify", "yandex"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "biomonitor",
            "name": "Trauma Team Biomonitor // Биомонитор Хоста",
            "description": "Реальная диагностика ресурсов ПК: физическая RAM, загрузка CPU всех ядер, статус питания и аптайм.",
            "category": "utilities",
            "author": "MEDIC: TRAUMA TEAM PLATINUM",
            "version": "2.0.0",
            "icon": "🩺",
            "tags": ["biomonitor", "ram", "cpu", "fps", "хост"],
            "installed": true,
            "enabled": true
        },
        {
            "id": "translator",
            "name": "Neural Lingua Translator // Нейро-переводчик",
            "description": "Мгновенный перевод уличного сленга Найт-Сити, японских корпоративных терминов и технических логов.",
            "category": "development",
            "author": "CORPO: KANG TAO INTERPRETER",
            "version": "1.1.0",
            "icon": "🌐",
            "tags": ["translate", "lingua", "japanese", "slang"],
            "installed": false,
            "enabled": false
        }
    ])
}

pub fn get_skills_with_saved_state() -> Value {
    let mut skills = get_default_skills();
    let cfg = crate::commands::get_config();
    if let (Some(arr), Some(saved_skills)) = (skills.as_array_mut(), cfg.get("skills").and_then(|s| s.as_object())) {
        for item in arr.iter_mut() {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                if let Some(saved) = saved_skills.get(id) {
                    if let Some(installed) = saved.get("installed").and_then(|v| v.as_bool()) {
                        item["installed"] = json!(installed);
                    }
                    if let Some(enabled) = saved.get("enabled").and_then(|v| v.as_bool()) {
                        item["enabled"] = json!(enabled);
                    }
                }
            }
        }
    }
    skills
}
