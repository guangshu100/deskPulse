use log::info;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub idle_threshold: u32,
    pub check_interval: u32,
    pub break_duration: u32,
    pub notify_before_fullscreen: u32,
    pub max_skips_per_day: u32,
    pub theme_color: String,
    pub sound_enabled: bool,
    pub auto_start: bool,
    pub run_in_tray: bool,
    pub break_interval: u32,
    pub break_count_today: u32,
    pub skip_count_today: u32,
    pub last_date: String,
    pub continuous_work_seconds: u64,
    pub pet: PetData,
    pub ai: AiConfig,
    pub water: WaterConfig,
    #[serde(default)]
    pub whitelist_apps: Vec<String>,
}

// ==================== 喝水提醒配置 ====================

#[derive(Clone, Serialize, Deserialize)]
pub struct WaterConfig {
    pub enabled: bool,
    pub interval_minutes: u32,
    pub daily_goal_ml: u32,
    pub cup_size_ml: u32,
    pub sound_enabled: bool,
    pub start_hour: u32,
    pub end_hour: u32,
    pub stats: WaterStats,
    /// 上班模式：启用后按办公时段排班提醒（9:00-17:30 六个时段）
    #[serde(default)]
    pub schedule_enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WaterStats {
    pub today_date: String,
    pub total_ml: u32,
    pub drink_count: u32,
    pub last_drink_time: Option<String>,
    /// 排班模式：6个时段的完成状态（true=已喝水）
    #[serde(default)]
    pub schedule_completed: Vec<bool>,
}

impl Default for WaterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 30,
            daily_goal_ml: 2000,
            cup_size_ml: 250,
            sound_enabled: true,
            start_hour: 8,
            end_hour: 22,
            stats: WaterStats::default(),
            schedule_enabled: false,
        }
    }
}

impl Default for WaterStats {
    fn default() -> Self {
        Self {
            today_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            total_ml: 0,
            drink_count: 0,
            last_drink_time: None,
            schedule_completed: Vec::new(),
        }
    }
}

impl WaterConfig {
    pub fn record_drink(&mut self, ml: u32) {
        self.stats.total_ml += ml;
        self.stats.drink_count += 1;
        self.stats.last_drink_time = Some(chrono::Local::now().format("%H:%M:%S").to_string());
    }
    
    pub fn check_and_reset_daily(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if self.stats.today_date != today {
            info!("Water stats reset for new day: {}", today);
            self.stats = WaterStats {
                today_date: today,
                total_ml: 0,
                drink_count: 0,
                last_drink_time: None,
                schedule_completed: Vec::new(),
            };
        }
    }
    
    pub fn progress_percent(&self) -> u32 {
        if self.daily_goal_ml == 0 { return 0; }
        (self.stats.total_ml * 100 / self.daily_goal_ml).min(100)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PetData {
    pub name: String,
    pub level: u32,
    pub total_breaks: u64,
    pub mood: u32,
    pub achievements: Vec<String>,
}

impl Default for PetData {
    fn default() -> Self {
        Self {
            name: "小瞳".to_string(),
            level: 1,
            total_breaks: 0,
            mood: 80,
            achievements: Vec::new(),
        }
    }
}

impl PetData {
    /// Calculate pet level based on total breaks
    pub fn calculate_level(total_breaks: u64) -> u32 {
        (1 + (total_breaks as f64).sqrt().floor() as u32).min(99)
    }

    /// Exp needed for current level
    pub fn exp_for_level(level: u32) -> u64 {
        if level <= 1 { 3 }
        else { (level as u64) * (level as u64) }
    }

    /// Get (current_exp_in_level, exp_needed_for_next_level)
    pub fn exp_progress(&self) -> (u64, u64) {
        let next_level = self.level + 1;
        let current_threshold = Self::exp_for_level(self.level);
        let next_threshold = Self::exp_for_level(next_level);
        let current_in_level = if self.total_breaks > current_threshold {
            self.total_breaks - current_threshold
        } else {
            0
        };
        let needed = next_threshold - current_threshold;
        (current_in_level.min(needed), needed)
    }

    /// Update mood on break completion
    pub fn on_break_completed(&mut self) {
        self.total_breaks += 1;
        self.level = Self::calculate_level(self.total_breaks);
        self.mood = (self.mood as u32 + 15).min(100);
        self.check_achievements();
    }

    /// Update mood on break skipped
    pub fn on_break_skipped(&mut self) {
        self.mood = self.mood.saturating_sub(20);
    }

    fn check_achievements(&mut self) {
        let add_if_new = |achievements: &mut Vec<String>, id: &str| {
            if !achievements.contains(&id.to_string()) {
                achievements.push(id.to_string());
            }
        };

        if self.total_breaks >= 1 {
            add_if_new(&mut self.achievements, "first_break");
        }
        if self.total_breaks >= 10 {
            add_if_new(&mut self.achievements, "ten_breaks");
        }
        if self.total_breaks >= 50 {
            add_if_new(&mut self.achievements, "fifty_breaks");
        }
        if self.total_breaks >= 100 {
            add_if_new(&mut self.achievements, "hundred_breaks");
        }
        if self.level >= 5 {
            add_if_new(&mut self.achievements, "level_5");
        }
        if self.level >= 10 {
            add_if_new(&mut self.achievements, "level_10");
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: String,
    pub api_key: String,
    pub api_base_url: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub preferred_style: String,
    pub user_name: String,
    pub user_profession: String,
    pub cache_enabled: bool,
    pub fallback_enabled: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "siliconflow".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.siliconflow.cn/v1".to_string(),
            model: "Qwen/Qwen2.5-7B-Instruct".to_string(),
            temperature: 0.8,
            max_tokens: 200,
            preferred_style: "balanced".to_string(),
            user_name: String::new(),
            user_profession: String::new(),
            cache_enabled: true,
            fallback_enabled: true,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            idle_threshold: 40,
            check_interval: 2,
            break_duration: 60,
            notify_before_fullscreen: 5,
            max_skips_per_day: 3,
            theme_color: "#E8F4F8".to_string(),
            sound_enabled: false,
            auto_start: true,
            run_in_tray: true,
            break_interval: 1,
            break_count_today: 0,
            skip_count_today: 0,
            last_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            continuous_work_seconds: 0,
            pet: PetData::default(),
            ai: AiConfig::default(),
            water: WaterConfig::default(),
            whitelist_apps: vec![
                "腾讯会议".to_string(),
                "zoom".to_string(),
                "teams".to_string(),
                "skype".to_string(),
                "钉钉".to_string(),
                "飞书".to_string(),
                "腾讯QQ".to_string(),
                "微信".to_string(),
                "企业微信".to_string(),
                "Slack".to_string(),
                "Webex".to_string(),
                "GoTo Meeting".to_string(),
                "Google Meet".to_string(),
            ],
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        // 1. 环境变量优先: EYECARE_CONFIG_PATH 或 EYECARE_CONFIG_DIR
        if let Ok(custom_path) = env::var("EYECARE_CONFIG_PATH") {
            let path = PathBuf::from(&custom_path);
            info!("Using custom config path from EYECARE_CONFIG_PATH: {:?}", path);
            return path;
        }
        if let Ok(custom_dir) = env::var("EYECARE_CONFIG_DIR") {
            let path = PathBuf::from(&custom_dir).join("config.json");
            info!("Using custom config dir from EYECARE_CONFIG_DIR: {:?}", path);
            return path;
        }

        // 2. 便携模式: 检测 exe 同目录下是否有 config.json 或 portable 标记文件
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_marker = exe_dir.join(".portable");
                let portable_config = exe_dir.join("config.json");
                
                // 如果存在 .portable 标记文件或已有 config.json，使用便携模式
                if portable_marker.exists() || portable_config.exists() {
                    info!("Portable mode detected, using exe directory: {:?}", exe_dir);
                    return portable_config;
                }
            }
        }

        // 3. 系统默认路径: %APPDATA%\eye-care\config.json
        let app_data = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        app_data.join("eye-care").join("config.json")
    }

    /// Get the directory where config is stored (for logging)
    pub fn config_dir() -> PathBuf {
        Self::config_path().parent().unwrap_or(&PathBuf::from(".")).to_path_buf()
    }

    /// Prepare config for disk storage: encrypt API key
    fn prepare_for_save(&self) -> Self {
        let mut config = self.clone();
        if !config.ai.api_key.is_empty() && !crate::crypto::is_encrypted(&config.ai.api_key) {
            match crate::crypto::encrypt_for_storage(&config.ai.api_key) {
                Ok(encrypted) => config.ai.api_key = encrypted,
                Err(e) => log::error!("Failed to encrypt API key: {}", e),
            }
        }
        config
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: AppConfig = serde_json::from_str(&content)?;

            // Decrypt API key after loading (graceful on failure)
            config.ai.api_key = crate::crypto::decrypt_from_storage(&config.ai.api_key)
                .unwrap_or_else(|e| {
                    log::error!("Failed to decrypt API key (machine changed?): {}", e);
                    log::info!("API key cleared, please re-enter in settings");
                    String::new()
                });

            // Check day rollover and reset daily counters
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if config.last_date != today {
                info!(
                    "Day rollover detected ({} -> {}), resetting daily counters",
                    config.last_date, today
                );
                config.break_count_today = 0;
                config.skip_count_today = 0;
                config.last_date = today;
                config.water.check_and_reset_daily();
                let _ = config.save_silent();
            }

            info!("Config loaded from {:?}", path);
            Ok(config)
        } else {
            info!("No config file found, using defaults");
            Ok(AppConfig::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = self.prepare_for_save();
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&path, content)?;
        info!("Config saved to {:?}", path);
        Ok(())
    }

    fn save_silent(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let config = self.prepare_for_save();
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&path, content)?;
        Ok(())
    }
}
