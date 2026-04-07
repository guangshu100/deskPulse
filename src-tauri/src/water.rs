use chrono::Timelike;
use log::info;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::WaterConfig;

static WATER_TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static LAST_REMINDER_TIME: Mutex<Option<Instant>> = Mutex::new(None);
static NEXT_REMINDER_SECONDS: AtomicU64 = AtomicU64::new(0);

// ==================== 上班模式排班定义 ====================

/// 上班时段喝水排班（9:00-17:30 六个时段）
pub struct WaterScheduleSlot {
    pub start_minutes: u32,  // 距午夜分钟数
    pub end_minutes: u32,
    pub amount_ml: u32,
    pub label: &'static str,
    pub message: &'static str,
    pub icon: &'static str,
    pub time_display: &'static str,
}

pub const WATER_SCHEDULE: [WaterScheduleSlot; 6] = [
    WaterScheduleSlot {
        start_minutes: 540,   // 9:00
        end_minutes: 570,     // 9:30
        amount_ml: 250,
        label: "到岗补水",
        message: "早晨身体缺水，先来杯温水唤醒新陈代谢吧！",
        icon: "🌅",
        time_display: "9:00-9:30",
    },
    WaterScheduleSlot {
        start_minutes: 630,   // 10:30
        end_minutes: 660,     // 11:00
        amount_ml: 200,
        label: "工作间隙",
        message: "工作近两小时了，起来活动活动，顺手喝杯水～",
        icon: "💻",
        time_display: "10:30-11:00",
    },
    WaterScheduleSlot {
        start_minutes: 690,   // 11:30
        end_minutes: 720,     // 12:00
        amount_ml: 150,
        label: "午餐前",
        message: "午餐前喝点水，避免用餐时过量饮水哦",
        icon: "🍽️",
        time_display: "11:30-12:00",
    },
    WaterScheduleSlot {
        start_minutes: 810,   // 13:30
        end_minutes: 840,     // 14:00
        amount_ml: 200,
        label: "午休后",
        message: "午睡醒来喝杯温水，快速恢复精神！",
        icon: "😴",
        time_display: "13:30-14:00",
    },
    WaterScheduleSlot {
        start_minutes: 900,   // 15:00
        end_minutes: 930,     // 15:30
        amount_ml: 200,
        label: "下午茶时间",
        message: "下午犯困了？喝杯水搭配拉伸，提提神！",
        icon: "🍵",
        time_display: "15:00-15:30",
    },
    WaterScheduleSlot {
        start_minutes: 1020,  // 17:00
        end_minutes: 1050,    // 17:30
        amount_ml: 150,
        label: "下班前",
        message: "下班前补杯水，路上注意补充水分哦",
        icon: "🚶",
        time_display: "17:00-17:30",
    },
];

/// 排班时段信息（供前端展示）
#[derive(Clone, serde::Serialize)]
pub struct ScheduleSlotInfo {
    pub index: u32,
    pub time_range: String,
    pub amount_ml: u32,
    pub label: String,
    pub icon: String,
    pub completed: bool,
    pub is_current: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct WaterStatus {
    pub enabled: bool,
    pub total_ml: u32,
    pub drink_count: u32,
    pub daily_goal_ml: u32,
    pub progress_percent: u32,
    pub next_reminder_seconds: u64,
    pub is_in_active_hours: bool,
    /// 上班排班模式
    pub schedule_enabled: bool,
    /// 当前时段索引（0-5）
    pub current_slot_index: Option<u32>,
    /// 距下一时段分钟数
    pub next_slot_minutes: Option<u64>,
    /// 下一时段名称
    pub next_slot_label: Option<String>,
    /// 当前时段建议喝水量
    pub current_slot_amount: Option<u32>,
    /// 当前时段提醒文案
    pub current_slot_message: Option<String>,
    /// 已完成时段数
    pub schedule_completed_count: u32,
    /// 排班总时段数
    pub schedule_total_slots: u32,
    /// 排班详情列表
    pub schedule_slots: Vec<ScheduleSlotInfo>,
}

/// 获取当前分钟数（距午夜）
fn current_minutes_of_day() -> u32 {
    let now = chrono::Local::now();
    now.hour() as u32 * 60 + now.minute() as u32
}

/// 查找当前所在排班时段
fn find_current_slot(minutes: u32) -> Option<usize> {
    WATER_SCHEDULE.iter().position(|slot| {
        minutes >= slot.start_minutes && minutes <= slot.end_minutes
    })
}

/// 查找下一个排班时段
fn find_next_slot(minutes: u32) -> Option<usize> {
    WATER_SCHEDULE.iter().position(|slot| {
        minutes < slot.start_minutes
    })
}

pub fn get_water_status(config: &WaterConfig) -> WaterStatus {
    let next = NEXT_REMINDER_SECONDS.load(Ordering::SeqCst);
    let now = chrono::Local::now();
    let current_hour = now.hour() as u32;
    let is_in_active_hours = current_hour >= config.start_hour && current_hour < config.end_hour;

    if !config.schedule_enabled {
        // 间隔模式（原有逻辑）
        return WaterStatus {
            enabled: config.enabled,
            total_ml: config.stats.total_ml,
            drink_count: config.stats.drink_count,
            daily_goal_ml: config.daily_goal_ml,
            progress_percent: config.progress_percent(),
            next_reminder_seconds: if is_in_active_hours { next } else { 0 },
            is_in_active_hours,
            schedule_enabled: false,
            current_slot_index: None,
            next_slot_minutes: None,
            next_slot_label: None,
            current_slot_amount: None,
            current_slot_message: None,
            schedule_completed_count: 0,
            schedule_total_slots: 6,
            schedule_slots: Vec::new(),
        };
    }

    // 排班模式
    let minutes = current_minutes_of_day();
    let current_idx = find_current_slot(minutes);
    let next_idx = find_next_slot(minutes);
    let completed_count = config.stats.schedule_completed.iter().filter(|&&c| c).count() as u32;

    let (current_slot_index, next_slot_minutes, next_slot_label, current_slot_amount, current_slot_message) =
        if let Some(idx) = current_idx {
            let slot = &WATER_SCHEDULE[idx];
            let completed = config.stats.schedule_completed.get(idx).copied().unwrap_or(false);
            if completed {
                // 当前时段已完成，找下一个
                let remaining = if let Some(ni) = next_idx {
                    (WATER_SCHEDULE[ni].start_minutes - minutes) as u64 * 60
                } else {
                    0
                };
                let label = next_idx.map(|ni| WATER_SCHEDULE[ni].label.to_string());
                (Some(idx as u32), Some(remaining / 60), label, None, None)
            } else {
                // 当前时段未完成
                (Some(idx as u32), None, None, Some(slot.amount_ml), Some(slot.message.to_string()))
            }
        } else if let Some(ni) = next_idx {
            let slot = &WATER_SCHEDULE[ni];
            let remaining = (slot.start_minutes - minutes) as u64;
            (None, Some(remaining), Some(slot.label.to_string()), None, None)
        } else {
            (None, None, None, None, None)
        };

    // 构建排班列表
    let schedule_slots: Vec<ScheduleSlotInfo> = WATER_SCHEDULE.iter().enumerate().map(|(i, slot)| {
        let completed = config.stats.schedule_completed.get(i).copied().unwrap_or(false);
        ScheduleSlotInfo {
            index: i as u32,
            time_range: slot.time_display.to_string(),
            amount_ml: slot.amount_ml,
            label: slot.label.to_string(),
            icon: slot.icon.to_string(),
            completed,
            is_current: current_idx == Some(i),
        }
    }).collect();

    WaterStatus {
        enabled: config.enabled,
        total_ml: config.stats.total_ml,
        drink_count: config.stats.drink_count,
        daily_goal_ml: config.daily_goal_ml,
        progress_percent: config.progress_percent(),
        next_reminder_seconds: if is_in_active_hours { next } else { 0 },
        is_in_active_hours,
        schedule_enabled: true,
        current_slot_index,
        next_slot_minutes,
        next_slot_label,
        current_slot_amount,
        current_slot_message,
        schedule_completed_count: completed_count,
        schedule_total_slots: 6,
        schedule_slots,
    }
}

pub fn start_water_timer(app_handle: AppHandle) {
    if WATER_TIMER_RUNNING.load(Ordering::SeqCst) {
        return;
    }

    WATER_TIMER_RUNNING.store(true, Ordering::SeqCst);
    info!("Water reminder timer started");

    std::thread::spawn(move || {
        loop {
            let (enabled, schedule_enabled, interval_secs, start_hour, end_hour, stats_snapshot) = {
                if let Some(state) = app_handle.try_state::<crate::AppState>() {
                    if let Ok(config) = state.config.lock() {
                        (
                            config.water.enabled,
                            config.water.schedule_enabled,
                            config.water.interval_minutes as u64 * 60,
                            config.water.start_hour,
                            config.water.end_hour,
                            config.water.stats.schedule_completed.clone(),
                        )
                    } else {
                        (false, false, 1800, 8, 22, Vec::new())
                    }
                } else {
                    (false, false, 1800, 8, 22, Vec::new())
                }
            };

            if !enabled {
                NEXT_REMINDER_SECONDS.store(0, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(10));
                continue;
            }

            let now = chrono::Local::now();
            let current_hour = now.hour() as u32;
            let is_in_active_hours = current_hour >= start_hour && current_hour < end_hour;

            if !is_in_active_hours {
                NEXT_REMINDER_SECONDS.store(0, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }

            if schedule_enabled {
                handle_schedule_mode(&app_handle, &now, &stats_snapshot);
            } else {
                handle_interval_mode(&app_handle, interval_secs);
            }

            std::thread::sleep(Duration::from_secs(10));
        }
    });
}

/// 间隔模式（原有逻辑）
fn handle_interval_mode(app_handle: &AppHandle, interval_secs: u64) {
    let should_remind = {
        if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
            if let Some(last) = *last_time {
                last.elapsed().as_secs() >= interval_secs
            } else {
                true
            }
        } else {
            false
        }
    };

    if should_remind {
        send_water_notification(app_handle, "💧 喝水提醒", "该喝水啦！保持水分很重要哦～");
        if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
            *last_time = Some(Instant::now());
        }
        NEXT_REMINDER_SECONDS.store(interval_secs, Ordering::SeqCst);
    } else {
        let elapsed = if let Ok(last_time) = LAST_REMINDER_TIME.lock() {
            last_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
        } else {
            0
        };
        let remaining = interval_secs.saturating_sub(elapsed);
        NEXT_REMINDER_SECONDS.store(remaining, Ordering::SeqCst);
    }
}

/// 排班模式：按办公时段智能提醒
fn handle_schedule_mode(app_handle: &AppHandle, now: &chrono::DateTime<chrono::Local>, schedule_completed: &[bool]) {
    let minutes = now.hour() as u32 * 60 + now.minute() as u32;

    // 查找当前时段
    if let Some(idx) = find_current_slot(minutes) {
        let slot = &WATER_SCHEDULE[idx];
        let completed = schedule_completed.get(idx).copied().unwrap_or(false);

        if completed {
            // 当前时段已完成，计算到下一时段的倒计时
            let remaining_secs = if let Some(next_idx) = find_next_slot(minutes) {
                (WATER_SCHEDULE[next_idx].start_minutes - minutes) as u64 * 60
            } else {
                // 所有时段已过，60秒后重新检查
                60
            };
            NEXT_REMINDER_SECONDS.store(remaining_secs, Ordering::SeqCst);
        } else {
            // 当前时段未完成，每15分钟提醒一次（900秒）
            let schedule_interval: u64 = 900;
            let should_remind = {
                if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
                    if let Some(last) = *last_time {
                        last.elapsed().as_secs() >= schedule_interval
                    } else {
                        true
                    }
                } else {
                    false
                }
            };

            if should_remind {
                let title = format!("{} {} 喝水提醒", slot.icon, slot.label);
                let body = format!("建议饮水 {}ml — {}", slot.amount_ml, slot.message);
                send_water_notification(app_handle, &title, &body);
                if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
                    *last_time = Some(Instant::now());
                }
            }

            // 剩余时间 = 当前时段结束时间
            let remaining = (slot.end_minutes - minutes + 1) as u64 * 60;
            NEXT_REMINDER_SECONDS.store(remaining, Ordering::SeqCst);
        }
    } else {
        // 不在任何时段内，找到下一个时段
        let remaining_secs = if let Some(next_idx) = find_next_slot(minutes) {
            (WATER_SCHEDULE[next_idx].start_minutes - minutes) as u64 * 60
        } else {
            0
        };
        NEXT_REMINDER_SECONDS.store(remaining_secs, Ordering::SeqCst);
    }
}

fn send_water_notification(app_handle: &AppHandle, title: &str, body: &str) {
    info!("Sending water reminder: {} - {}", title, body);

    let _ = app_handle.emit("water-reminder", ());

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = app_handle.notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    }
}

pub fn record_drink(app_handle: &AppHandle, ml: u32) -> WaterStatus {
    if let Some(state) = app_handle.try_state::<crate::AppState>() {
        if let Ok(mut config) = state.config.lock() {
            config.water.record_drink(ml);

            // 排班模式：标记当前时段为已完成
            if config.water.schedule_enabled {
                let minutes = current_minutes_of_day();
                if let Some(idx) = find_current_slot(minutes) {
                    // 确保 vec 足够长
                    while config.water.stats.schedule_completed.len() <= idx {
                        config.water.stats.schedule_completed.push(false);
                    }
                    config.water.stats.schedule_completed[idx] = true;
                    info!("Schedule slot {} ({}) marked completed", idx, WATER_SCHEDULE[idx].label);
                }
            }

            // 重置提醒计时器
            if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
                *last_time = Some(Instant::now());
            }

            let status = get_water_status(&config.water);
            let _ = config.save();
            info!("Recorded drink: {}ml, total: {}ml", ml, config.water.stats.total_ml);
            return status;
        }
    }
    WaterStatus {
        enabled: false,
        total_ml: 0,
        drink_count: 0,
        daily_goal_ml: 2000,
        progress_percent: 0,
        next_reminder_seconds: 0,
        is_in_active_hours: true,
        schedule_enabled: false,
        current_slot_index: None,
        next_slot_minutes: None,
        next_slot_label: None,
        current_slot_amount: None,
        current_slot_message: None,
        schedule_completed_count: 0,
        schedule_total_slots: 6,
        schedule_slots: Vec::new(),
    }
}

pub fn reset_reminder_timer() {
    if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
        *last_time = Some(Instant::now());
    }
    info!("Water reminder timer reset");
}

pub fn update_next_reminder(interval_secs: u64) {
    NEXT_REMINDER_SECONDS.store(interval_secs, Ordering::SeqCst);
}
