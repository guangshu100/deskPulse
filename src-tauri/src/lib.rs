#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::info;
use std::sync::Mutex;
use tauri::{Manager, Emitter, State};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

mod idle;
mod config;
mod ai;
mod tray;
mod crypto;
mod water;

pub struct AppState {
    pub config: Mutex<config::AppConfig>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> config::AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(state: State<AppState>, new_config: config::AppConfig) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    *config = new_config;
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_ai_config(state: State<AppState>) -> config::AiConfig {
    state.config.lock().unwrap().ai.clone()
}

#[tauri::command]
fn save_ai_config(state: State<AppState>, new_ai_config: config::AiConfig) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    config.ai = new_ai_config;
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_api_connection(state: State<'_, AppState>) -> Result<String, String> {
    let ai_config = state.config.lock().unwrap().ai.clone();
    ai::test_connection(&ai_config).await
}

#[tauri::command]
async fn generate_message(
    context_type: String,
    context: ai::GenerateContext,
    ai_config: config::AiConfig,
) -> Result<String, String> {
    ai::generate(&context_type, context, &ai_config).await
}

#[tauri::command]
fn get_idle_time() -> u64 {
    idle::get_idle_time()
}

#[tauri::command]
fn toggle_pause(state: State<AppState>, app: tauri::AppHandle) {
    let (is_paused, eye_health) = {
        let mut config = state.config.lock().unwrap();
        config.run_in_tray = !config.run_in_tray;
        let is_paused = !config.run_in_tray;
        let _ = config.save();
        (is_paused, idle::get_eye_health())
    };

    idle::set_paused(is_paused);

    let tooltip = if is_paused {
        "EyeCare - 已暂停".to_string()
    } else {
        format!("EyeCare - 监控中 | 眼睛生命值: {}%", eye_health)
    };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tooltip.as_str()));
    }
}

#[tauri::command]
fn get_status(state: State<AppState>) -> idle::Status {
    let mut status = idle::get_status();
    // 计算下次提醒时间
    if let Ok(config) = state.config.lock() {
        let work_threshold_secs = config.idle_threshold as u64 * 60;
        let continuous_work = status.continuous_work_seconds;
        if continuous_work < work_threshold_secs {
            status.next_reminder_seconds = Some(work_threshold_secs - continuous_work);
        } else {
            status.next_reminder_seconds = None;
        }
    }
    status
}

#[tauri::command]
fn reset_work_timer() {
    idle::reset_continuous_work_timer();
}

#[tauri::command]
async fn show_fullscreen(
    app: tauri::AppHandle, 
    forced: Option<bool>, 
    severity: Option<u32>, 
    eye_health: Option<u32>, 
    skip_history_json: Option<String>, 
    total_skipped_seconds: Option<u64>,
    ai_title: Option<String>,
    ai_main_text: Option<String>,
    ai_sub_text: Option<String>,
    ai_interaction: Option<String>,
) {
    let forced = forced.unwrap_or(false);
    let severity = severity.unwrap_or(1);
    let eye_health = eye_health.unwrap_or(100);
    let skip_history: Vec<idle::SkipRecord> = skip_history_json
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let total_skipped = total_skipped_seconds.unwrap_or(0);
    
    let (ai_title, ai_main_text, ai_sub_text, ai_interaction) = 
        if ai_main_text.is_some() {
            (ai_title, ai_main_text, ai_sub_text, ai_interaction)
        } else {
            // 如果前端没传（如 testBreak / 快捷键），在后台生成 AI 内容
            let app_clone = app.clone();
            let (t, m, s, i) = idle::generate_ai_content(&app_clone, severity, eye_health, 0, 0);
            (t, m, s, i)
        };
    
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        tray::show_fullscreen_window(
            &app_clone, 
            forced, 
            severity, 
            eye_health, 
            &skip_history, 
            total_skipped,
            ai_title.as_deref(),
            ai_main_text.as_deref(),
            ai_sub_text.as_deref(),
            ai_interaction.as_deref(),
        );
    });
}

#[tauri::command]
fn send_notification(app: tauri::AppHandle, title: String, body: String) -> Result<(), String> {
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| e.to_string())
}

/// Called from fullscreen page when the user closes the break page.
#[tauri::command]
fn on_fullscreen_closed(state: State<AppState>, app: tauri::AppHandle, early: bool, remaining: u32, break_duration: u32) {
    if early {
        let skipped = break_duration - remaining;
        idle::record_skip_with_pet(skipped, &app);
        info!("Break skipped ({}s remaining)", remaining);
    } else {
        idle::record_break_completed_with_pet(break_duration, &app);
        idle::reset_continuous_work_timer();
        info!("Break completed successfully ({}s)", break_duration);
    }

    if let Ok(mut config) = state.config.lock() {
        config.skip_count_today = idle::get_skip_count();
        if !early {
            config.break_count_today += 1;
            config.pet.on_break_completed();
        } else {
            config.pet.on_break_skipped();
        }
        let _ = config.save();
    }

    // Update tray tooltip
    let eye_health = idle::get_eye_health();
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&format!("EyeCare - 监控中 | 眼睛生命值: {}%", eye_health)));
    }

    let status = idle::get_status();
    let _ = app.emit("idle-status", status);

    // Close the fullscreen window
    if let Some(window) = app.get_webview_window("fullscreen") {
        let _ = window.close();
        info!("Fullscreen window closed via on_fullscreen_closed");
    }
}

/// Close fullscreen window and handle break completion
#[tauri::command]
fn close_fullscreen_window(state: State<AppState>, app: tauri::AppHandle, early: bool, remaining: u32, break_duration: u32) {
    // Handle break completion logic - update eye health and skip records
    if early {
        let skipped = break_duration - remaining;
        idle::record_skip(skipped);
        info!("Break skipped ({}s remaining)", remaining);
    } else {
        idle::record_break_completed(break_duration);
        idle::reset_continuous_work_timer();
        info!("Break completed successfully ({}s)", break_duration);
    }

    // Update config with minimal lock time
    {
        let mut config = state.config.lock().unwrap();
        config.skip_count_today = idle::get_skip_count();
        config.continuous_work_seconds = idle::get_continuous_work_seconds();
        if !early {
            config.break_count_today += 1;
            config.pet.on_break_completed();
        } else {
            config.pet.on_break_skipped();
        }
        let _ = config.save();
    }

    // Update tray tooltip
    let eye_health = idle::get_eye_health();
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&format!("EyeCare - 监控中 | 眼睛生命值: {}%", eye_health)));
    }

    let status = idle::get_status();
    let _ = app.emit("idle-status", status);

    // Close the fullscreen window
    if let Some(window) = app.get_webview_window("fullscreen") {
        let _ = window.close();
        info!("Fullscreen window closed");
    }
}

#[tauri::command]
fn get_pet_state(state: State<AppState>) -> config::PetData {
    state.config.lock().unwrap().pet.clone()
}

#[tauri::command]
fn pet_interact(state: State<AppState>) {
    // Petting the pet gives a small mood boost
    if let Ok(mut config) = state.config.lock() {
        config.pet.mood = (config.pet.mood + 3).min(100);
        let _ = config.save();
    }
}

#[tauri::command]
fn rename_pet(state: State<AppState>, new_name: String) -> Result<(), String> {
    if new_name.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if new_name.len() > 12 {
        return Err("Name too long (max 12 chars)".to_string());
    }
    let mut config = state.config.lock().unwrap();
    config.pet.name = new_name.trim().to_string();
    config.save().map_err(|e| e.to_string())
}

// ==================== 喝水提醒命令 ====================

#[tauri::command]
fn get_water_config(state: State<AppState>) -> config::WaterConfig {
    state.config.lock().unwrap().water.clone()
}

#[tauri::command]
fn save_water_config(state: State<AppState>, new_config: config::WaterConfig) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    config.water = new_config;
    water::update_next_reminder(config.water.interval_minutes as u64 * 60);
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_water_status(state: State<AppState>) -> water::WaterStatus {
    let config = state.config.lock().unwrap();
    water::get_water_status(&config.water)
}

#[tauri::command]
fn record_water_intake(app: tauri::AppHandle, ml: u32) -> water::WaterStatus {
    water::record_drink(&app, ml)
}

#[tauri::command]
fn drink_one_cup(app: tauri::AppHandle) -> water::WaterStatus {
    let cup_size = if let Some(state) = app.try_state::<AppState>() {
        if let Ok(config) = state.config.lock() {
            config.water.cup_size_ml
        } else {
            250
        }
    } else {
        250
    };
    water::record_drink(&app, cup_size)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("EyeCare starting...");

    let app_config = config::AppConfig::load().unwrap_or_default();
    
    // 从配置加载连续工作时长
    idle::load_continuous_work(app_config.continuous_work_seconds);
    
    let app_state = AppState {
        config: Mutex::new(app_config),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .setup(|app| {
            info!("EyeCare setup started...");

            tray::setup_tray(app)?;

            // Handle main window close event - show dialog instead of closing
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Prevent the window from closing
                        api.prevent_close();
                        // Hide the window (minimize to tray instead)
                        let _ = window_clone.hide();
                        info!("Main window hidden to tray");
                    }
                });
            }

            // Register global shortcuts
            use tauri_plugin_global_shortcut::{ShortcutEvent, ShortcutState};

            let rest_shortcut: tauri_plugin_global_shortcut::Shortcut =
                "ctrl+shift+r".parse().expect("Invalid shortcut");
            app.global_shortcut()
                .on_shortcuts(vec![rest_shortcut], |app_handle, _shortcut, event: ShortcutEvent| {
                    if event.state == ShortcutState::Pressed {
                        let eye_health = idle::get_eye_health();
                        let skip_count = idle::get_skip_count();
                        let severity = idle::calculate_severity(skip_count, eye_health);
                        let skip_history = idle::get_skip_history();
                        let total_skipped = idle::get_total_skipped_seconds();
                        let work_minutes = idle::get_continuous_work_seconds() / 60;
                        let (ai_title, ai_main, ai_sub, ai_inter) = idle::generate_ai_content(app_handle, severity, eye_health, skip_count, work_minutes);
                        tray::show_fullscreen_window(app_handle, false, severity, eye_health, &skip_history, total_skipped, ai_title.as_deref(), ai_main.as_deref(), ai_sub.as_deref(), ai_inter.as_deref());
                    }
                })?;

            let pause_shortcut: tauri_plugin_global_shortcut::Shortcut =
                "ctrl+shift+p".parse().expect("Invalid shortcut");
            app.global_shortcut()
                .on_shortcuts(vec![pause_shortcut], |app_handle, _shortcut, event: ShortcutEvent| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let is_paused = {
                                let config = state.config.lock().unwrap();
                                !config.run_in_tray
                            };
                            idle::set_paused(is_paused);
                            if let Ok(mut config) = state.config.lock() {
                                config.run_in_tray = !is_paused;
                                let _ = config.save();
                            }
                            info!("Global shortcut toggle pause: {}", is_paused);
                        }
                    }
                })?;

            // Start idle monitoring
            idle::start_monitoring(app.handle().clone())?;
            
            // Start water reminder timer
            water::start_water_timer(app.handle().clone());

            info!("EyeCare setup completed");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_ai_config,
            save_ai_config,
            test_api_connection,
            generate_message,
            get_idle_time,
            toggle_pause,
            get_status,
            reset_work_timer,
            show_fullscreen,
            send_notification,
            on_fullscreen_closed,
            close_fullscreen_window,
            get_pet_state,
            pet_interact,
            rename_pet,
            get_water_config,
            save_water_config,
            get_water_status,
            record_water_intake,
            drink_one_cup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
