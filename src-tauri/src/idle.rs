use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use chrono::Timelike;

use crate::config::{AppConfig, PetData};

static PAUSED: AtomicBool = AtomicBool::new(false);
static IS_RUNNING: AtomicBool = AtomicBool::new(false);
static SKIP_COUNT_TODAY: AtomicU32 = AtomicU32::new(0);
static NOTIFIED: AtomicBool = AtomicBool::new(false);

/// Eye health value: 0..100. Decays while user works, recovers during breaks.
static EYE_HEALTH: AtomicU32 = AtomicU32::new(100);

/// Continuous work time in seconds (resets when user takes natural break)
static CONTINUOUS_WORK_SECONDS: AtomicU64 = AtomicU64::new(0);

/// Accumulated work seconds for eye health decay (resets every 60 seconds)
static DECAY_ACCUMULATOR: AtomicU64 = AtomicU64::new(0);

/// Threshold in seconds to consider idle as natural break
static NATURAL_BREAK_THRESHOLD_SECS: u64 = 20;

/// Track if user was on natural break in previous check
static WAS_ON_NATURAL_BREAK: AtomicBool = AtomicBool::new(false);

/// Track if currently in whitelist app (no reminder when true)
static IN_WHITELIST_APP: AtomicBool = AtomicBool::new(false);

/// Cached pet data for Status emission (updated periodically)
static PET_CACHE: Mutex<Option<PetData>> = Mutex::new(None);

#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub paused: bool,
    pub idle_seconds: u64,
    pub next_reminder_seconds: Option<u64>,
    pub is_monitoring: bool,
    pub skip_count_today: u32,
    pub eye_health: u32,
    pub severity: u32,
    pub continuous_work_seconds: u64,
    pub pet: Option<PetData>,
}

/// Payload emitted when fullscreen break should be triggered
#[derive(Clone, Serialize)]
pub struct FullscreenTrigger {
    pub forced: bool,
    pub severity: u32,
    pub eye_health: u32,
    pub skip_history: Vec<SkipRecord>,
    pub total_skipped_seconds: u64,
    pub ai_title: Option<String>,
    pub ai_main_text: Option<String>,
    pub ai_sub_text: Option<String>,
    pub ai_interaction: Option<String>,
}

/// Record of a single skip event
#[derive(Clone, Serialize, Deserialize)]
pub struct SkipRecord {
    pub time: String,
    pub skipped_after: u32,
}

/// Global skip history (today only, cleared on day rollover)
static SKIP_HISTORY: Mutex<Vec<SkipRecord>> = Mutex::new(Vec::new());

/// Total skipped rest seconds accumulated today
static TOTAL_SKIPPED_SECONDS: AtomicU32 = AtomicU32::new(0);

// ==================== Severity Calculation ====================

/// Calculate message severity (1-5) based on skip count and eye health.
/// 1 = gentle, 5 = dark humor / dramatic
pub fn calculate_severity(skip_count: u32, eye_health: u32) -> u32 {
    let skip_severity = if skip_count >= 8 { 5 }
        else if skip_count >= 6 { 4 }
        else if skip_count >= 4 { 3 }
        else if skip_count >= 2 { 2 }
        else { 1 };

    let health_severity = if eye_health <= 10 { 5 }
        else if eye_health <= 25 { 4 }
        else if eye_health <= 45 { 3 }
        else if eye_health <= 65 { 2 }
        else { 1 };

    skip_severity.max(health_severity).min(5)
}

// ==================== Platform Idle Detection ====================

/// Cross-platform idle time detection (returns seconds)
pub fn get_idle_time() -> u64 {
    #[cfg(target_os = "windows")]
    {
        get_idle_time_windows()
    }

    #[cfg(target_os = "macos")]
    {
        get_idle_time_macos()
    }

    #[cfg(target_os = "linux")]
    {
        get_idle_time_linux()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

#[cfg(target_os = "windows")]
fn get_idle_time_windows() -> u64 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut last_input).as_bool() {
            let tick_count = GetTickCount();
            let idle_ms = tick_count - last_input.dwTime;
            idle_ms as u64 / 1000
        } else {
            error!("GetLastInputInfo failed");
            0
        }
    }
}

#[cfg(target_os = "macos")]
fn get_idle_time_macos() -> u64 {
    use cocoa::appkit::NSEvent;
    use cocoa::base::nil;

    unsafe { NSEvent::eventSecondsSinceLastInput(nil) as u64 }
}

/// Linux idle detection via XScreenSaver extension.
/// Requires: libx11-dev, libxss-dev at build time.
#[cfg(target_os = "linux")]
fn get_idle_time_linux() -> u64 {
    use std::ptr;

    unsafe {
        let display = x11::xlib::XOpenDisplay(ptr::null());
        if display.is_null() {
            error!("Failed to open X display for idle detection (is DISPLAY set?)");
            return 0;
        }

        let root = x11::xlib::XDefaultRootWindow(display);
        let info_ptr = x11::xss::XScreenSaverAllocInfo();

        if info_ptr.is_null() {
            error!("XScreenSaverAllocInfo returned null");
            x11::xlib::XCloseDisplay(display);
            return 0;
        }

        let result = x11::xss::XScreenSaverQueryInfo(display, root, info_ptr);
        let idle_ms = if result != 0 { (*info_ptr).idle } else { 0 };

        x11::xlib::XFree(info_ptr as *mut std::ffi::c_void);
        x11::xlib::XCloseDisplay(display);
        idle_ms as u64 / 1000
    }
}

// ==================== State Management ====================

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    info!("EyeCare monitoring paused: {}", paused);
}

pub fn increment_skip_count() {
    let count = SKIP_COUNT_TODAY.fetch_add(1, Ordering::SeqCst) + 1;
    info!("Skip count today: {}", count);
}

/// Record a skip event with timestamp and context + update pet
pub fn record_skip(skipped_after_seconds: u32) {
    record_skip_internal(skipped_after_seconds);
}

/// Record skip without pet (for internal use)
fn record_skip_internal(skipped_after_seconds: u32) {
    increment_skip_count();
    let now = chrono::Local::now().format("%H:%M").to_string();
    if let Ok(mut history) = SKIP_HISTORY.lock() {
        history.push(SkipRecord {
            time: now,
            skipped_after: skipped_after_seconds,
        });
        // Keep last 20 records
        let len = history.len();
        if len > 20 {
            history.drain(0..len - 20);
        }
    }
    TOTAL_SKIPPED_SECONDS.fetch_add(skipped_after_seconds, Ordering::SeqCst);
}

/// Record a skip event + update pet mood via app_handle
pub fn record_skip_with_pet(skipped_after_seconds: u32, app_handle: &AppHandle) {
    record_skip_internal(skipped_after_seconds);
    let should_save = if let Some(state) = app_handle.try_state::<crate::AppState>() {
        if let Ok(mut config) = state.config.lock() {
            config.pet.on_break_skipped();
            true
        } else {
            false
        }
    } else {
        false
    };
    if should_save {
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(mut config) = state.config.lock() {
                let _ = config.save();
            }
        }
    }
}

/// Called when a break is completed — recovers eye health + updates pet
pub fn record_break_completed(break_seconds: u32) {
    let current = EYE_HEALTH.load(Ordering::SeqCst);
    let recovery = (break_seconds as u32 * 3).min(100 - current);
    EYE_HEALTH.store(current + recovery, Ordering::SeqCst);
    info!("Break completed ({}s), eye health +{} -> {}", break_seconds, recovery, EYE_HEALTH.load(Ordering::SeqCst));
}

/// Called when a break is completed — updates pet via app_handle
pub fn record_break_completed_with_pet(break_seconds: u32, app_handle: &AppHandle) {
    record_break_completed(break_seconds);
    let (should_save, old_level, new_level) = if let Some(state) = app_handle.try_state::<crate::AppState>() {
        if let Ok(mut config) = state.config.lock() {
            let old_level = config.pet.level;
            config.pet.on_break_completed();
            (true, old_level, config.pet.level)
        } else {
            (false, 0, 0)
        }
    } else {
        (false, 0, 0)
    };
    if should_save {
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(mut config) = state.config.lock() {
                let _ = config.save();
            }
        }
        if new_level > old_level {
            info!("Pet leveled up! Lv.{} -> Lv.{}", old_level, new_level);
        }
    }
}

pub fn get_skip_count() -> u32 {
    SKIP_COUNT_TODAY.load(Ordering::SeqCst)
}

pub fn set_skip_count(count: u32) {
    SKIP_COUNT_TODAY.store(count, Ordering::SeqCst);
}

pub fn get_eye_health() -> u32 {
    EYE_HEALTH.load(Ordering::SeqCst)
}

/// Decay eye health by one tick (called every check_interval seconds while user is active)
fn decay_eye_health(amount: u32) {
    let current = EYE_HEALTH.load(Ordering::SeqCst);
    let new_health = current.saturating_sub(amount);
    EYE_HEALTH.store(new_health, Ordering::SeqCst);
}

/// Pet mood decay counter (decay 1 mood every ~5 minutes of work)
static MOOD_DECAY_ACCUMULATOR: AtomicU64 = AtomicU64::new(0);

pub fn reset_daily_counts() {
    SKIP_COUNT_TODAY.store(0, Ordering::SeqCst);
    TOTAL_SKIPPED_SECONDS.store(0, Ordering::SeqCst);
    EYE_HEALTH.store(100, Ordering::SeqCst);
    CONTINUOUS_WORK_SECONDS.store(0, Ordering::SeqCst);
    DECAY_ACCUMULATOR.store(0, Ordering::SeqCst);
    MOOD_DECAY_ACCUMULATOR.store(0, Ordering::SeqCst);
    if let Ok(mut history) = SKIP_HISTORY.lock() {
        history.clear();
    }
    info!("Daily counts reset (eye health restored to 100)");
}

/// Decay pet mood by 1 via AppState
fn decay_pet_mood(app_handle: &AppHandle) {
    let should_save = if let Some(state) = app_handle.try_state::<crate::AppState>() {
        if let Ok(mut config) = state.config.lock() {
            if config.pet.mood > 0 {
                config.pet.mood -= 1;
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
    
    if should_save {
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(mut config) = state.config.lock() {
                let _ = config.save();
            }
        }
    }
}

pub fn get_status() -> Status {
    let idle = get_idle_time();
    let skip_count = SKIP_COUNT_TODAY.load(Ordering::SeqCst);
    let eye_health = EYE_HEALTH.load(Ordering::SeqCst);
    let continuous_work = CONTINUOUS_WORK_SECONDS.load(Ordering::SeqCst);
    let pet = PET_CACHE.lock().ok().and_then(|mut g| g.take());
    Status {
        paused: PAUSED.load(Ordering::SeqCst),
        idle_seconds: idle,
        next_reminder_seconds: None,
        is_monitoring: IS_RUNNING.load(Ordering::SeqCst),
        skip_count_today: skip_count,
        eye_health,
        severity: calculate_severity(skip_count, eye_health),
        continuous_work_seconds: continuous_work,
        pet,
    }
}

/// 从配置加载连续工作时长（程序启动时调用）
pub fn load_continuous_work(seconds: u64) {
    CONTINUOUS_WORK_SECONDS.store(seconds, Ordering::SeqCst);
    info!("Loaded continuous work seconds: {}", seconds);
}

/// 获取当前连续工作时长（用于保存到配置）
pub fn get_continuous_work_seconds() -> u64 {
    CONTINUOUS_WORK_SECONDS.load(Ordering::SeqCst)
}

pub fn get_skip_history() -> Vec<SkipRecord> {
    SKIP_HISTORY.lock().map(|h| h.clone()).unwrap_or_default()
}

pub fn get_total_skipped_seconds() -> u64 {
    TOTAL_SKIPPED_SECONDS.load(Ordering::SeqCst) as u64
}

pub fn is_in_whitelist_app() -> bool {
    IN_WHITELIST_APP.load(Ordering::SeqCst)
}

pub fn is_paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}

/// Get work threshold for display
pub fn get_work_threshold() -> u64 {
    30 * 60 // default, will be overridden by config
}

/// Check if whitelist apps are running (Windows)
#[cfg(target_os = "windows")]
fn check_whitelist_apps(app_handle: &AppHandle) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        // Get whitelist from config
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(config) = state.config.lock() {
                let whitelist = &config.whitelist_apps;
                if whitelist.is_empty() {
                    return false;
                }

                // Get process name by PID (simplified - in production would use PSAPI)
                // For now, we'll check against known meeting app window titles
                let mut title_buf = [0u16; 260];
                let len = windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(
                    hwnd,
                    &mut title_buf,
                );

                if len > 0 {
                    let title = String::from_utf16_lossy(&title_buf[..len as usize]);
                    let title_lower = title.to_lowercase();

                    for app in whitelist {
                        let app_lower = app.to_lowercase();
                        // Check if window title contains the whitelist app name
                        if title_lower.contains(&app_lower) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn check_whitelist_apps(_app_handle: &AppHandle) -> bool {
    false
}

/// Update tray tooltip with countdown
fn update_tray_tooltip(app_handle: &AppHandle, continuous_work: u64, threshold: u64) {
    let in_whitelist = check_whitelist_apps(app_handle);
    IN_WHITELIST_APP.store(in_whitelist, Ordering::SeqCst);

    let eye_health = EYE_HEALTH.load(Ordering::SeqCst);

    let tooltip = if PAUSED.load(Ordering::SeqCst) {
        "EyeCare - 已暂停".to_string()
    } else if in_whitelist {
        "EyeCare - 白名单中 (暂停计时)".to_string()
    } else if continuous_work >= threshold {
        format!("EyeCare - 该休息了! | 👁️ {}%", eye_health)
    } else {
        let remaining = threshold.saturating_sub(continuous_work);
        let mins = remaining / 60;
        let secs = remaining % 60;
        format!("EyeCare - 再工作 {:02}:{:02} | 👁️ {}%", mins, secs, eye_health)
    };

    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

// ==================== Monitoring Loop ====================

/// Core monitoring loop implementing the full reminder flow.
/// 
/// 关键设计（参考 Stretchly）：
/// 1. 连续工作时长达到阈值时触发提醒（而非空闲达到阈值）
/// 2. 用户空闲时暂停计时器（自然休息检测）
/// 3. 用户恢复活动时继续计时
/// 4. 眼睛生命值按实际工作时间衰减
pub fn start_monitoring(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if IS_RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }

    IS_RUNNING.store(true, Ordering::SeqCst);

    std::thread::spawn(move || {
        let mut last_check_date = String::new();
        let mut waiting_for_fullscreen = false;
        let mut fullscreen_trigger_time: Option<Instant> = None;
        let mut last_check = Instant::now();
        let mut emit_counter: u32 = 0;

        loop {
            let now = Instant::now();
            let gap = now.duration_since(last_check);
            last_check = now;

            // Read config with minimal lock time
            let (work_threshold_secs, check_interval, notify_delay, max_skips) = {
                match app_handle.try_state::<crate::AppState>() {
                    Some(state) => {
                        match state.config.lock() {
                            Ok(cfg) => (
                                cfg.idle_threshold as u64 * 60, // 连续工作阈值（分钟转秒）
                                cfg.check_interval as u64,
                                cfg.notify_before_fullscreen as u64,
                                cfg.max_skips_per_day as u32,
                            ),
                            Err(e) => {
                                error!("Failed to lock config: {}", e);
                                (30 * 60, 2, 5, 3)
                            }
                        }
                    }
                    None => (30 * 60, 2, 5, 3)
                }
            };

            // ---- System sleep/wake detection ----
            if gap.as_secs() > check_interval * 3 {
                info!(
                    "System sleep/wake detected ({}s gap, expected {}s)",
                    gap.as_secs(),
                    check_interval
                );
                let _ = app_handle.emit("system-resumed", gap.as_secs());
                waiting_for_fullscreen = false;
                fullscreen_trigger_time = None;
                NOTIFIED.store(false, Ordering::SeqCst);
                WAS_ON_NATURAL_BREAK.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(check_interval));
                continue;
            }

            // ---- Day rollover check ----
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if today != last_check_date && !last_check_date.is_empty() {
                reset_daily_counts();
            }
            if last_check_date.is_empty() {
                last_check_date = today;
            }

            // ---- Pause check ----
            if PAUSED.load(Ordering::SeqCst) {
                waiting_for_fullscreen = false;
                fullscreen_trigger_time = None;
                NOTIFIED.store(false, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(check_interval));
                continue;
            }

            let idle = get_idle_time();
            
            // ---- 自然休息检测（参考 Stretchly NaturalBreaksManager）----
            let is_on_natural_break = idle >= NATURAL_BREAK_THRESHOLD_SECS;
            let was_on_break = WAS_ON_NATURAL_BREAK.swap(is_on_natural_break, Ordering::SeqCst);
            
            if is_on_natural_break && !was_on_break {
                // 用户开始休息
                info!("Natural break started (idle: {}s)", idle);
            } else if !is_on_natural_break && was_on_break {
                // 用户从休息中恢复
                // 检查休息时长是否足够（空闲超过60秒视为有效休息）
                if idle >= 60 {
                    info!("Natural break completed ({}s), resetting work timer", idle);
                    // 有效休息，重置连续工作计时器
                    CONTINUOUS_WORK_SECONDS.store(0, Ordering::SeqCst);
                    // 眼睛生命值恢复
                    let recovery = ((idle / 60) * 5).min((100 - EYE_HEALTH.load(Ordering::SeqCst)) as u64) as u32;
                    if recovery > 0 {
                        let current = EYE_HEALTH.load(Ordering::SeqCst);
                        EYE_HEALTH.store(current + recovery, Ordering::SeqCst);
                        info!("Eye health recovered {} from natural break", recovery);
                    }
                } else {
                    info!("Natural break ended but too short ({}s), continuing work timer", idle);
                }
                // 重置全屏等待状态
                waiting_for_fullscreen = false;
                fullscreen_trigger_time = None;
                NOTIFIED.store(false, Ordering::SeqCst);
            }

            // ---- 连续工作时长计算 ----
            // 只有在用户活动时（空闲时间很短）才累计工作时间
            let is_user_active = idle < check_interval; // 用户在最近检测间隔内有活动
            
            if is_user_active && !is_on_natural_break {
                // 用户正在工作，累计工作时间
                let prev = CONTINUOUS_WORK_SECONDS.fetch_add(check_interval, Ordering::SeqCst);
                let continuous_work = prev + check_interval;
                
                // 眼睛生命值衰减（按实际工作时间）
                // 每60秒工作衰减约1点生命值
                let acc = DECAY_ACCUMULATOR.fetch_add(check_interval, Ordering::SeqCst) + check_interval;
                if acc >= 60 {
                    // 每累计60秒衰减1点
                    DECAY_ACCUMULATOR.store(acc - 60, Ordering::SeqCst);
                    decay_eye_health(1);
                }

                // 宠物心情衰减（每5分钟衰减1点）
                let mood_acc = MOOD_DECAY_ACCUMULATOR.fetch_add(check_interval, Ordering::SeqCst) + check_interval;
                if mood_acc >= 300 {
                    MOOD_DECAY_ACCUMULATOR.store(mood_acc - 300, Ordering::SeqCst);
                    decay_pet_mood(&app_handle);
                }
                
                // 记录日志（每分钟记录一次）
                if continuous_work % 60 < check_interval {
                    info!("Continuous work: {}min, eye health: {}%", 
                          continuous_work / 60, 
                          EYE_HEALTH.load(Ordering::SeqCst));
                }
            }

            let skip_count = SKIP_COUNT_TODAY.load(Ordering::SeqCst);
            let eye_health = EYE_HEALTH.load(Ordering::SeqCst);
            let severity = calculate_severity(skip_count, eye_health);
            let continuous_work = CONTINUOUS_WORK_SECONDS.load(Ordering::SeqCst);

            // Emit status for frontend (reduce frequency to avoid UI flood)
            emit_counter += 1;
            if emit_counter >= 5 {  // Emit every 5th iteration (10 seconds with 2s interval)
                emit_counter = 0;
                let next_reminder = if continuous_work < work_threshold_secs {
                    Some(work_threshold_secs - continuous_work)
                } else {
                    None
                };

                // Refresh pet cache from config
                if let Some(state) = app_handle.try_state::<crate::AppState>() {
                    if let Ok(cfg) = state.config.lock() {
                        if let Ok(mut cache) = PET_CACHE.lock() {
                            *cache = Some(cfg.pet.clone());
                        }
                    }
                }

                let pet_data = PET_CACHE.lock().ok().and_then(|mut g| g.take());
                let _ = app_handle.emit(
                    "idle-status",
                    Status {
                        paused: false,
                        idle_seconds: idle,
                        next_reminder_seconds: next_reminder,
                        is_monitoring: true,
                        skip_count_today: skip_count,
                        eye_health,
                        severity,
                        continuous_work_seconds: continuous_work,
                        pet: pet_data,
                    },
                );

                // Update tray tooltip with countdown
                update_tray_tooltip(&app_handle, continuous_work, work_threshold_secs);
            }

            // ---- 白名单检测 ----
            let in_whitelist = check_whitelist_apps(&app_handle);
            IN_WHITELIST_APP.store(in_whitelist, Ordering::SeqCst);

            // ---- Step 1: 连续工作达到阈值 -> send notification ----
            if continuous_work >= work_threshold_secs
                && !NOTIFIED.load(Ordering::SeqCst)
                && !waiting_for_fullscreen
                && !is_on_natural_break // 用户不在休息中才触发
                && !in_whitelist // 用户在白名单应用中不触发
            {
                info!(
                    "Work threshold reached ({}min), severity={}, eye_health={}",
                    continuous_work / 60, severity, eye_health
                );
                let _ = app_handle.emit("trigger-notification", ());
                NOTIFIED.store(true, Ordering::SeqCst);
                waiting_for_fullscreen = true;
                fullscreen_trigger_time = Some(Instant::now());
            }

            // ---- Step 2: After notification, wait then show fullscreen ----
            if waiting_for_fullscreen {
                if let Some(trigger_time) = fullscreen_trigger_time {
                    let elapsed = trigger_time.elapsed().as_secs();
                    if elapsed >= notify_delay {
                        // 检查用户是否仍在工作（不在自然休息中，且不在白名单中）
                        if !is_on_natural_break && !in_whitelist {
                            let forced = skip_count >= max_skips;
                            let skip_history = get_skip_history();
                            let total_skipped = get_total_skipped_seconds();
                            info!(
                                "Showing fullscreen (forced: {}, severity: {}, eye_health: {})",
                                forced, severity, eye_health
                            );

                            // 生成 AI 内容
                            let (ai_title, ai_main_text, ai_sub_text, ai_interaction) =
                                generate_ai_content(&app_handle, severity, eye_health, skip_count, continuous_work / 60);

                            let _ = app_handle.emit(
                                "trigger-fullscreen",
                                FullscreenTrigger {
                                    forced,
                                    severity,
                                    eye_health,
                                    skip_history,
                                    total_skipped_seconds: total_skipped,
                                    ai_title,
                                    ai_main_text,
                                    ai_sub_text,
                                    ai_interaction,
                                },
                            );
                        } else if in_whitelist {
                            info!("User is in whitelist app, postponing fullscreen");
                        } else {
                            info!("User is on natural break, postponing fullscreen");
                        }
                        waiting_for_fullscreen = false;
                        fullscreen_trigger_time = None;
                        NOTIFIED.store(false, Ordering::SeqCst);
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(check_interval));
        }
    });

    Ok(())
}

/// 重置连续工作计时器（休息完成后调用）
pub fn reset_continuous_work_timer() {
    CONTINUOUS_WORK_SECONDS.store(0, Ordering::SeqCst);
    info!("Continuous work timer reset");
}

/// Generate AI content for the fullscreen break
pub fn generate_ai_content(
    app_handle: &AppHandle,
    severity: u32,
    eye_health: u32,
    skip_count: u32,
    work_minutes: u64,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    // Get AI config first
    let ai_config = match app_handle.try_state::<crate::AppState>() {
        Some(state) => match state.config.lock() {
            Ok(config) => {
                if config.ai.enabled && !config.ai.api_key.is_empty() {
                    Some(config.ai.clone())
                } else {
                    info!("AI disabled or no API key configured (enabled={}, key_empty={})", config.ai.enabled, config.ai.api_key.is_empty());
                    None
                }
            }
            Err(_) => {
                info!("AI skipped: config lock busy");
                None
            }
        },
        None => {
            info!("AI skipped: AppState not available");
            None
        }
    };

    let Some(ai_config) = ai_config else {
        return (None, None, None, None);
    };

    info!("AI generating content (severity={}, eye_health={}, work_min={})", severity, eye_health, work_minutes);

    // Build context
    let break_duration = match app_handle.try_state::<crate::AppState>() {
        Some(state) => state.config.lock().map(|c| c.break_duration).unwrap_or(60),
        None => 60,
    };
    let breaks_today = match app_handle.try_state::<crate::AppState>() {
        Some(state) => state.config.lock().map(|c| c.break_count_today).unwrap_or(0),
        None => 0,
    };
    let context = crate::ai::GenerateContext {
        idle_minutes: work_minutes as u32,
        breaks_today,
        total_breaks: breaks_today,
        skip_count,
        hour: chrono::Local::now().hour(),
        day_of_week: chrono::Local::now().format("%A").to_string(),
        consecutive_work_minutes: work_minutes as u32,
        preferred_style: ai_config.preferred_style.clone(),
        user_name: ai_config.user_name.clone(),
        last_break_type: String::new(),
        break_duration,
        time_period: get_time_period(),
        user_type: if !ai_config.user_profession.is_empty() { ai_config.user_profession.clone() } else { "developer".to_string() },
        severity,
        eye_health,
        total_skipped_seconds: get_total_skipped_seconds() as u64,
    };

    // Use blocking call in a spawned thread to avoid blocking the monitor thread
    let result = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            crate::ai::generate("fullscreen", context, &ai_config).await.ok()
        })
    }).join().ok().flatten();

    if let Some(content) = result {
        let parts: Vec<&str> = content.split('|').collect();
        let main_text = parts.get(0).map(|s| s.to_string());
        let sub_text = parts.get(1).map(|s| s.to_string());
        let interaction = parts.get(2).map(|s| s.to_string());
        info!("AI content generated successfully: main={:?}, sub={:?}, inter={:?}", main_text, sub_text, interaction);
        (Some("AI 温馨提示".to_string()), main_text, sub_text, interaction)
    } else {
        info!("AI content generation failed, falling back to local messages");
        (None, None, None, None)
    }
}

/// Get time period string
fn get_time_period() -> String {
    let hour = chrono::Local::now().hour();
    if hour < 6 { "深夜".to_string() }
    else if hour < 9 { "早晨".to_string() }
    else if hour < 12 { "上午".to_string() }
    else if hour < 14 { "中午".to_string() }
    else if hour < 18 { "下午".to_string() }
    else if hour < 22 { "晚上".to_string() }
    else { "深夜".to_string() }
}
