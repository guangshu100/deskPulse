use log::{error, info};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::idle::SkipRecord;

/// Simple base64url encode (no padding, URL-safe)
fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input.as_bytes())
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停监控", true, None::<&str>)?;
    let rest = MenuItem::with_id(app, "rest", "给眼睛放个假", true, None::<&str>)?;
    let drink_water = MenuItem::with_id(app, "drink_water", "💧 已喝一杯水", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[&pause, &rest, &drink_water, &sep1, &settings, &show, &sep2, &quit],
    )?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("EyeCare - 监控中 | 眼睛生命值: 100%")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "quit" => {
                info!("Quit requested from tray");
                app.exit(0);
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "pause" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    let mut config = state.config.lock().unwrap();
                    config.run_in_tray = !config.run_in_tray;
                    let is_paused = !config.run_in_tray;
                    let _ = config.save();

                    let eye_health = crate::idle::get_eye_health();
                    let tooltip = if is_paused {
                        "EyeCare - 已暂停".to_string()
                    } else {
                        format!("EyeCare - 监控中 | 眼睛生命值: {}%", eye_health)
                    };
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        let _ = tray.set_tooltip(Some(tooltip.as_str()));
                    }

                    crate::idle::set_paused(is_paused);
                    info!("Tray menu toggle pause: {}", is_paused);
                }
            }
            "rest" => {
                let eye_health = crate::idle::get_eye_health();
                let skip_count = crate::idle::get_skip_count();
                let severity = crate::idle::calculate_severity(skip_count, eye_health);
                let skip_history = crate::idle::get_skip_history();
                let total_skipped = crate::idle::get_total_skipped_seconds();
                let work_minutes = crate::idle::get_continuous_work_seconds() / 60;
                let (ai_title, ai_main, ai_sub, ai_inter) = crate::idle::generate_ai_content(app, severity, eye_health, skip_count, work_minutes);
                show_fullscreen_window(app, false, severity, eye_health, &skip_history, total_skipped, ai_title.as_deref(), ai_main.as_deref(), ai_sub.as_deref(), ai_inter.as_deref());
            }
            "drink_water" => {
                let status = crate::water::record_drink(app, 250);
                info!("Water recorded from tray: {}ml total", status.total_ml);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    info!("Tray icon created");
    Ok(())
}

/// Show the fullscreen break page with severity and eye health context.
pub fn show_fullscreen_window(
    app: &AppHandle,
    forced: bool,
    severity: u32,
    eye_health: u32,
    skip_history: &[SkipRecord],
    total_skipped_seconds: u64,
    ai_title: Option<&str>,
    ai_main_text: Option<&str>,
    ai_sub_text: Option<&str>,
    ai_interaction: Option<&str>,
) {
    if let Some(window) = app.get_webview_window("fullscreen") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let (theme_color, break_duration) =
        if let Some(state) = app.try_state::<crate::AppState>() {
            match state.config.try_lock() {
                Ok(config) => (config.theme_color.clone(), config.break_duration),
                Err(_) => {
                    info!("Config lock busy, using defaults");
                    ("#E8F4F8".to_string(), 30)
                }
            }
        } else {
            ("#E8F4F8".to_string(), 30)
        };

    // Serialize skip history for URL parameter
    let history_json = serde_json::to_string(skip_history).unwrap_or_else(|_| "[]".to_string());
    let history_encoded = base64_encode(&history_json);

    // Choose background color based on eye health
    let effective_theme = if eye_health <= 20 && !forced {
        "#FFF3E0".to_string()
    } else {
        theme_color.clone()
    };

    // Build URL with AI params
    let mut url = format!(
        "fullscreen.html?duration={}&theme={}&forced={}&severity={}&eye_health={}&skip_history={}&total_skipped={}",
        break_duration, effective_theme, forced, severity, eye_health,
        history_encoded,
        total_skipped_seconds
    );

    // Add AI parameters if available
    if let Some(title) = ai_title {
        url.push_str(&format!("&ai_title={}", urlencoding::encode(title)));
    }
    if let Some(main_text) = ai_main_text {
        url.push_str(&format!("&ai_main_text={}", urlencoding::encode(main_text)));
    }
    if let Some(sub_text) = ai_sub_text {
        url.push_str(&format!("&ai_sub_text={}", urlencoding::encode(sub_text)));
    }
    if let Some(interaction) = ai_interaction {
        url.push_str(&format!("&ai_interaction={}", urlencoding::encode(interaction)));
    }

    let url = WebviewUrl::App(url.into());

    match WebviewWindowBuilder::new(app, "fullscreen", url)
        .title("EyeCare - 休息一下")
        .fullscreen(true)
        .always_on_top(true)
        .focused(true)
        .build()
    {
        Ok(window) => {
            info!("Fullscreen window created (forced: {}, severity: {}, eye_health: {}, ai: {})", 
                forced, severity, eye_health, ai_main_text.is_some());
            let _ = window.set_focus();
        }
        Err(e) => {
            error!("Failed to create fullscreen window: {}", e);
        }
    }
}
