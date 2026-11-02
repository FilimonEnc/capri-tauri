mod bridge_server;
mod commands;
mod window_tracker;

use serde_json::json;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use bridge_server::BridgeServer;
use window_tracker::WindowTracker;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::set_interactive,
            commands::focus_overlay,
            commands::open_site,
            commands::open_marketplace,
            commands::open_menu,
            commands::music,
            commands::media,
            commands::get_cursor,
            commands::get_screens,
            commands::get_windows,
            commands::move_screen,
            commands::chat_status,
            commands::chat_send,
            commands::chat_memory,
            commands::chat_remember,
            commands::use_provider,
            commands::get_system_stats,
            commands::get_hooks_status,
            commands::install_hooks,
            commands::uninstall_hooks,
        ])
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "menu_quit" || id == "quit" {
                std::process::exit(0);
            }
            if id == "menu_market" || id == "marketplace" {
                let _ = open::that("http://127.0.0.1:47823/marketplace");
                return;
            }
            if id == "sound_folder" {
                let mut dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                dir.push("capri-tauri");
                dir.push("sounds");
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(&dir);
                return;
            }

            if let Some(w) = app.get_webview_window("main") {
                match id {
                    "menu_chat" | "chat" => {
                        let _ = w.emit("capri:command", json!({ "name": "chat" }));
                    }
                    "menu_teleport" => {
                        let _ = w.emit("capri:command", json!({ "name": "teleport" }));
                    }
                    "menu_satellite" => {
                        let _ = w.emit("capri:command", json!({ "name": "satellite" }));
                    }
                    "menu_demo" => {
                        let _ = w.emit("capri:command", json!({ "name": "intro" }));
                    }
                    "menu_welcome" | "menu_help" => {
                        let _ = w.emit("capri:command", json!({ "name": "welcome" }));
                    }
                    "menu_report" => {
                        let _ = w.emit("capri:command", json!({ "name": "report" }));
                    }
                    "menu_custom" => {
                        let _ = w.emit("capri:command", json!({ "name": "customize" }));
                    }
                    "menu_stow" => {
                        let cur = commands::get_config();
                        let vis = cur.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                        let _ = commands::set_config(json!({ "visible": !vis }));
                        let _ = w.emit("capri:command", json!({ "name": "stow", "on": vis }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "scale_045" => {
                        let _ = commands::set_config(json!({ "scale": 0.45 }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "scale_060" => {
                        let _ = commands::set_config(json!({ "scale": 0.60 }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "scale_080" => {
                        let _ = commands::set_config(json!({ "scale": 0.80 }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "sound_mute" => {
                        let cur = commands::get_config();
                        let m = cur.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                        let _ = commands::set_config(json!({ "muted": !m }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_yandex" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "yandex" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_spotify" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "spotify" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_ytmusic" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "ytmusic" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_vk" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "vk" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_apple" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "apple" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "player_youtube" => {
                        let _ = commands::set_config(json!({ "musicPlayer": "youtube" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "screen_follow" => {
                        let cur = commands::get_config();
                        let f = cur.get("follow").and_then(|v| v.as_bool()).unwrap_or(true);
                        let _ = commands::set_config(json!({ "follow": !f }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "ai_setup" => {
                        let _ = w.emit("capri:command", json!({ "name": "setupChat" }));
                    }
                    "ai_apikey" => {
                        let _ = w.emit("capri:command", json!({ "name": "setupKey" }));
                    }
                    "provider_ollama" => {
                        let _ = commands::set_config(json!({ "provider": "ollama" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "provider_claude" => {
                        let _ = commands::set_config(json!({ "provider": "claude" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "provider_gemini" => {
                        let _ = commands::set_config(json!({ "provider": "gemini" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "provider_openai" => {
                        let _ = commands::set_config(json!({ "provider": "openai" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "provider_groq" => {
                        let _ = commands::set_config(json!({ "provider": "groq" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "provider_openrouter" => {
                        let _ = commands::set_config(json!({ "provider": "openrouter" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "cfg_wander" => {
                        let cur = commands::get_config();
                        let wnd = cur.get("wander").and_then(|v| v.as_bool()).unwrap_or(true);
                        let _ = commands::set_config(json!({ "wander": !wnd }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "cfg_possess" => {
                        let cur = commands::get_config();
                        let pos = cur.get("possess").and_then(|v| v.as_bool()).unwrap_or(true);
                        let _ = commands::set_config(json!({ "possess": !pos }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "rest_corner" => {
                        let _ = commands::set_config(json!({ "rest": "corner" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "rest_taskbar" => {
                        let _ = commands::set_config(json!({ "rest": "taskbar" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "watch_antigravity" => {
                        let _ = commands::install_antigravity_hooks();
                        let _ = commands::set_config(json!({ "assistant": "antigravity" }));
                        let _ = w.emit("capri:config", commands::get_config());
                        let _ = w.emit("capri:command", json!({ "name": "assistantChanged" }));
                    }
                    "watch_claude" => {
                        let _ = commands::set_config(json!({ "assistant": "claude" }));
                        let _ = w.emit("capri:config", commands::get_config());
                        let _ = w.emit("capri:command", json!({ "name": "assistantChanged" }));
                    }
                    "watch_cursor" => {
                        let _ = commands::set_config(json!({ "assistant": "cursor" }));
                        let _ = w.emit("capri:config", commands::get_config());
                        let _ = w.emit("capri:command", json!({ "name": "assistantChanged" }));
                    }
                    "watch_windsurf" => {
                        let _ = commands::set_config(json!({ "assistant": "windsurf" }));
                        let _ = w.emit("capri:config", commands::get_config());
                        let _ = w.emit("capri:command", json!({ "name": "assistantChanged" }));
                    }
                    "watch_none" => {
                        let _ = commands::set_config(json!({ "assistant": "none" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "motion_auto" => {
                        let _ = commands::set_config(json!({ "motion": "auto" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "motion_full" => {
                        let _ = commands::set_config(json!({ "motion": "full" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "motion_reduced" => {
                        let _ = commands::set_config(json!({ "motion": "reduced" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "display_auto" => {
                        let _ = commands::set_config(json!({ "render": "auto" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "display_gl" => {
                        let _ = commands::set_config(json!({ "render": "gl" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "display_software" => {
                        let _ = commands::set_config(json!({ "render": "software" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "lang_auto" => {
                        let _ = commands::set_config(json!({ "lang": "auto" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "lang_ru" => {
                        let _ = commands::set_config(json!({ "lang": "ru" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "lang_en" => {
                        let _ = commands::set_config(json!({ "lang": "en" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "lang_fr" => {
                        let _ = commands::set_config(json!({ "lang": "fr" }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "cfg_startup" => {
                        let cur = commands::get_config();
                        let st = cur.get("startup").and_then(|v| v.as_bool()).unwrap_or(false);
                        let _ = commands::set_config(json!({ "startup": !st }));
                        let _ = w.emit("capri:config", commands::get_config());
                    }
                    "hooks_toggle" => {
                        let installed = commands::is_antigravity_hooks_installed();
                        if installed {
                            let _ = commands::uninstall_antigravity_hooks();
                            let _ = w.emit("capri:command", json!({ "name": "hooks", "ok": true, "action": "uninstall" }));
                        } else {
                            let _ = commands::install_antigravity_hooks();
                            let _ = w.emit("capri:command", json!({ "name": "hooks", "ok": true, "action": "install" }));
                        }
                    }
                    _ => {}
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Ensure Antigravity hooks if Antigravity assistant is chosen
            let cfg = commands::get_config();
            let asst = cfg.get("assistant").and_then(|v| v.as_str()).unwrap_or("antigravity");
            if asst == "antigravity" || asst == "agy" {
                let _ = commands::install_antigravity_hooks();
            }

            // Start native Win32 Window & Cursor Tracker (120 FPS / 60 FPS)
            let _tracker = WindowTracker::start(handle.clone());

            // Start local HTTP Bridge (:47823) with embedded marketplace
            let _bridge = BridgeServer::start(handle.clone());

            // Configure main transparent window
            if let Some(window) = app.get_webview_window("main") {
                // Initial click-through and non-focusable until chat opens
                let _ = window.set_ignore_cursor_events(true);
                let _ = window.set_focusable(false);
                let _ = window.set_always_on_top(true);
                let _ = window.set_shadow(false);
                let _ = window.set_decorations(false);
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let pos = monitor.position();
                    let size = monitor.size();
                    let _ = window.set_position(*pos);
                    let _ = window.set_size(*size);
                }
            }

            // System Tray Menu with official Capri icon
            let item_status = MenuItem::with_id(app, "tray_status", "● Капри онлайн (:47823)", false, None::<&str>)?;
            let item_chat = MenuItem::with_id(app, "chat", "💬 Поговорить с ней (Chat)", true, None::<&str>)?;
            let item_market = MenuItem::with_id(app, "marketplace", "⚡ Маркетплейс навыков (Skills)", true, None::<&str>)?;
            let item_quit = MenuItem::with_id(app, "quit", "❌ Выход / Quit", true, None::<&str>)?;

            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(app, &[&item_status, &sep1, &item_chat, &item_market, &sep2, &item_quit])?;

            let app_icon = tauri::include_image!("icons/32x32.png");

            let tray_builder = TrayIconBuilder::new()
                .tooltip("Capri")
                .icon(app_icon)
                .menu(&menu)
                .show_menu_on_left_click(false);

            let _ = tray_builder
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.emit("capri:command", json!({ "name": "chat" }));
                        }
                    }
                })
                .build(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Capri desktop");
}
