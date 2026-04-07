use serde::{Deserialize, Serialize};
use reqwest::Client;
use log::{info, error};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::AiConfig;

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateContext {
    pub idle_minutes: u32,
    pub breaks_today: u32,
    pub total_breaks: u32,
    pub skip_count: u32,
    pub hour: u32,
    pub day_of_week: String,
    pub consecutive_work_minutes: u32,
    pub preferred_style: String,
    pub user_name: String,
    pub last_break_type: String,
    pub break_duration: u32,
    pub time_period: String,
    pub user_type: String,
    pub severity: u32,
    pub eye_health: u32,
    pub total_skipped_seconds: u64,
}

// ==================== Local Message Library ====================
// Severity-based messaging system:
//   1 = gentle/encouraging
//   2 = friendly reminder
//   3 = concerned/firm
//   4 = sarcastic/humorous
//   5 = dark humor / dramatic

const NOTIFICATION_MESSAGES: &[(&str, u32)] = &[
    // Severity 1-2: gentle
    ("程序员小伙伴，你已经专注30分钟啦～ 起身伸个懒腰，看看窗外的绿植吧～", 1),
    ("眼睛要放假啦！放下鼠标，喝口水，活动一下僵硬的肩膀呀～", 1),
    ("今日份休息提醒：久坐伤腰，站起来走两步，奖励自己一个深呼吸～", 1),
    ("辛苦啦！你已经很努力了，给眼睛放个小假吧～", 1),
    ("工作再忙也要记得休息哦～ 喝杯水，看看远方，元气满满继续冲！", 1),
    ("你的眼睛正在发送求救信号！护眼模式启动中～", 2),
    ("屏幕：我也需要休息一下下...", 2),
    ("码代码固然重要，但你的卡姿兰大眼睛更需要爱护呀～", 2),
    ("检测到长期盯屏幕行为，你的眼睛正在考虑罢工～", 2),
    ("你的键盘说：让我歇会儿吧，它也想让你去休息～", 2),
    // Severity 3: concerned
    ("亲爱的，休息一下吧，你已经很努力了～", 3),
    ("注意休息哦～身体是革命的本钱，眼睛更是～", 3),
    ("该休息啦～ 最棒的作品，来自于充分休息后的你！", 3),
    ("你已经连续工作好久了，眼睛的疲劳度正在飙升！", 3),
    ("Time's up! 你的睫状肌发来投诉信，请立即处理。", 3),
    // Severity 4: sarcastic
    ("你的眼睛刚刚给我发了一封辞职信，说它不想干了。", 4),
    ("如果你再不休息，你的眼睛就要在GitHub上开issue投诉你了。", 4),
    ("叮！您有一份来自眼科医生的「注意通知书」，请查收。", 4),
    ("你以为的「再写5分钟就休息」已经持续了2小时。", 4),
    ("你的眼睛正在群里吐槽：「这个人类从来不让我休息」。", 4),
    // Severity 5: dark humor
    ("据统计，99%的程序员在失明后才开始后悔没有使用EyeCare。你确定要继续？", 5),
    ("你的眼睛已经开始起草遗书了：「亲爱的主人，如果您再不让我休息……」", 5),
    ("如果眼睛有寿命，你刚才的2小时相当于偷走了它3天的生命。", 5),
    ("你的视网膜正在考虑连夜离职，并且不再给你写交接文档。", 5),
    ("温馨提示：继续盯着屏幕不会让bug自己消失，但会让你的视力消失。", 5),
];

const FULLSCREEN_MAIN_TEXTS: &[(&str, u32)] = &[
    // Severity 1-2
    ("暂时告别屏幕，让眼睛休息一下", 1),
    ("放下鼠标，享受片刻宁静", 1),
    ("眼睛辛苦了，给它们放个假", 1),
    ("休息一下，效率更高哦", 1),
    ("该休息啦～ 闭眼享受这一刻", 1),
    ("你的眼睛需要充电了 🔋", 2),
    ("放下键盘，拯救你的卡姿兰大眼睛", 2),
    // Severity 3
    ("再不休息，眼睛真的要罢工了", 3),
    ("眼睛疲劳度已过载，请立即执行休息程序", 3),
    ("这不是建议，这是你眼睛的紧急请求", 3),
    // Severity 4
    ("你的眼睛正在罢工中...", 4),
    ("恭喜你，成功把眼睛逼到了崩溃边缘", 4),
    ("代码不会跑，但你的眼睛会废", 4),
    // Severity 5
    ("💀 你的眼睛正在死去", 5),
    ("最终警告：你的视力值已跌破安全线", 5),
    ("紧急抢救：请立即闭眼，否则后果自负", 5),
];

const FULLSCREEN_SUB_TEXTS: &[(&str, u32)] = &[
    ("远眺6米外的物体，睫状肌会放松哦～", 1),
    ("试试20-20-20法则：看20英尺外20秒", 1),
    ("闭眼休息30秒，眼睛会感谢你的", 1),
    ("轻轻眨眼10次，缓解眼睛干涩", 1),
    ("站起来活动一下，肩颈也需要休息", 1),
    ("喝口水，补充水分也是护眼的一部分", 1),
    ("你的眼睛需要急救，请配合治疗", 3),
    ("睫状肌疲劳度已达临界值，远眺是唯一的解药", 3),
    ("研究表明：继续盯屏幕将导致不可逆的视疲劳", 4),
    ("你现在休息的每一秒，都在挽救未来的视力", 4),
    ("闭眼是你现在唯一能做的一件正确的事", 5),
    ("每多看屏幕1分钟，你就离近视加深0.001mm更近一步", 5),
];

const FULLSCREEN_INTERACTIONS: &[&str] = &[
    "轻轻闭眼休息一下～",
    "转动眼珠，顺时针3圈，逆时针3圈",
    "远眺6米外的物体，让眼睛放松",
    "用力眨眨眼，活动一下眼肌",
    "双手搓热，轻轻敷在眼睛上",
];

const WELCOME_MESSAGES: &[&str] = &[
    "休息时间结束～ 满血复活继续加油！",
    "眼睛亮晶晶啦！接下来的工作效率翻倍哦～",
    "欢迎回来！继续保持健康节奏吧～",
    "眼睛充满电啦！继续冲鸭！",
    "休息完毕～ 你现在状态绝佳！",
    "元气满满回来了！今天也要棒棒哒～",
    "眼睛休息好了，灵感马上就到！",
    "调整完毕，战斗力 +100！",
];

const TIPS_MESSAGES: &[&str] = &[
    "眨眼一次只要0.3秒！现在多眨几下吧～",
    "眼睛每天眨眼的次数超过10000次哦！",
    "远眺可以让晶状体放松，预防近视～",
    "维生素A对眼睛好，多吃胡萝卜呀～",
    "绿色植物可以有效缓解眼疲劳哦～",
    "多喝水也能帮助缓解眼睛干涩～",
];

const TRAY_MESSAGES: &[(&str, u32)] = &[
    ("今天也要好好爱护眼睛呀～", 1),
    ("加油！注意适当休息哦～", 1),
    ("眼睛是心灵的窗户，要好好保护哦～", 1),
    ("适度休息，效率更高！", 1),
    ("你的眼睛也需要放假呀～", 1),
    ("眼睛正在低电量运行...请充电", 3),
    ("警告：眼睛疲劳度正在上升", 3),
    ("你的眼睛状态：不太妙", 4),
    ("视力存亡之际，请立即休息", 5),
];

const FORCED_REST_TIPS: &[&str] = &[
    "今日跳过次数已达上限，眼睛真的需要休息了！",
    "检测到你频繁跳过休息，这次请一定要坚持哦～",
    "过度用眼会降低工作效率，休息是对自己最好的投资！",
    "坚持完这次休息，你的眼睛会感谢你的！",
    "为明天更好的状态，现在请放下屏幕休息一下～",
    "身体是革命的本钱，眼睛更是！这次不能跳过啦～",
    "研究表明：规律休息可以提高30%的工作效率，忍一下！",
];

// ==================== AI Cache ====================

struct AiCache {
    entries: HashMap<String, (String, Instant)>,
}

impl AiCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.entries.get(key).and_then(|(value, time)| {
            if time.elapsed() < Duration::from_secs(3600) {
                Some(value)
            } else {
                None
            }
        })
    }

    fn set(&mut self, key: String, value: String) {
        self.entries.insert(key, (value, Instant::now()));
        if self.entries.len() > 50 {
            self.entries.retain(|_, (_, time)| time.elapsed() < Duration::from_secs(1800));
        }
    }
}

static AI_CACHE: Mutex<Option<AiCache>> = Mutex::new(None);

fn get_cache() -> std::sync::MutexGuard<'static, Option<AiCache>> {
    let mut cache = AI_CACHE.lock().unwrap();
    if cache.is_none() {
        *cache = Some(AiCache::new());
    }
    cache
}

// ==================== Rate Limiter ====================

static LAST_API_CALL: Mutex<Option<Instant>> = Mutex::new(None);

fn should_rate_limit() -> bool {
    let mut last = LAST_API_CALL.lock().unwrap();
    if let Some(instant) = *last {
        if instant.elapsed() < Duration::from_secs(30) {
            info!("AI rate limited (last call {}s ago)", instant.elapsed().as_secs());
            return true;
        }
    }
    *last = Some(Instant::now());
    false
}

// ==================== Prompt Templates ====================

const NOTIFICATION_PROMPT: &str = r#"你是一个{role}健康助手。请根据以下信息生成一条休息提醒：

当前时间：{hour}点 ({day_of_week})
累计空闲：{idle_minutes}分钟
今日休息次数：{breaks_today}次
今日跳过次数：{skip_count}次
眼睛生命值：{eye_health}/100
用户昵称：{user_name}
未休息总时长：{total_skipped_minutes}分钟

{severity_instruction}

要求：
1. 30字以内，适合系统通知显示
2. 可以适当加入emoji
3. 根据时间段调整内容"#;

const FULLSCREEN_PROMPT: &str = r#"你是一个{role}护眼顾问。请为用户生成一个护眼休息页面的文案：

场景：用户需要休息 {break_duration} 秒
当前时间段：{time_period}
用户类型：{user_type}
眼睛生命值：{eye_health}/100
今日跳过次数：{skip_count}次
严重程度：{severity}/5

{severity_instruction}

要求：
1. 主文案：15字以内
2. 副文案：30字以内
3. 互动文案：20字以内
4. 三段文案用 | 分隔
5. 适当使用emoji"#;

const WELCOME_PROMPT: &str = r#"你是一个{role}能量补给站。请为用户生成回归欢迎文案：

休息时长：{break_duration}秒
连续工作时长：{consecutive_work_minutes}分钟

要求：
1. 简短有力，20字以内
2. 正向激励，让用户感到被鼓励
3. 语气活泼，元气满满"#;

fn get_severity_role(severity: u32) -> &'static str {
    match severity {
        1 => "温柔可爱的",
        2 => "活泼有趣的",
        3 => "关切认真的",
        4 => "幽默讽刺的",
        5 => "黑色幽默的",
        _ => "温暖可爱的",
    }
}

fn get_severity_instruction(severity: u32) -> &'static str {
    match severity {
        1 => "语气非常温柔轻松，像朋友间随口的关心，不要有任何压力感。",
        2 => "语气轻松友好，带一点小幽默，让用户愿意主动休息。",
        3 => "语气关切甚至有些严肃，明确告诉用户眼睛已经很累了，需要立即休息。",
        4 => "用幽默讽刺的方式指出用户不爱惜眼睛的行为，用拟人化手法（眼睛辞职信、投诉、罢工等），让人会心一笑后主动休息。可以引用程序员梗。",
        5 => "用黑色幽默和夸张手法制造冲击感。把「不休息」的后果戏剧化（眼睛写遗书、视力寿命、视网膜离职等）。要让人又想笑又真的害怕。风格参考：「死了么」类产品。",
        _ => "语气温柔、轻松，避免说教",
    }
}

// ==================== API Functions ====================

pub async fn test_connection(config: &AiConfig) -> Result<String, String> {
    if config.api_key.is_empty() {
        return Err("API密钥不能为空".to_string());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        temperature: 0.7,
        max_tokens: 10,
    };

    let url = format!("{}/chat/completions", config.api_base_url.trim_end_matches('/'));

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if response.status().is_success() {
        Ok("连接成功！".to_string())
    } else {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        Err(format!("连接失败: {} - {}", status, text))
    }
}

pub async fn generate(context_type: &str, context: GenerateContext, config: &AiConfig) -> Result<String, String> {
    if !config.enabled || config.api_key.is_empty() {
        return get_local_message(context_type, context.severity);
    }

    // Check cache first (include severity in key)
    if config.cache_enabled {
        let cache_key = format!("{}:{:?}:s{}", context_type, context.hour, context.severity);
        if let Some(cached) = get_cache().as_mut().and_then(|c| c.get(&cache_key)) {
            info!("Using cached AI message for {} (severity {})", context_type, context.severity);
            return Ok(cached.clone());
        }
    }

    // Rate limiting: max 1 call per minute
    if should_rate_limit() {
        info!("Rate limited, using local message");
        return get_local_message(context_type, context.severity);
    }

    let prompt = match context_type {
        "notification" => build_severity_prompt(NOTIFICATION_PROMPT, &context),
        "fullscreen" => build_severity_prompt(FULLSCREEN_PROMPT, &context),
        "welcome" => build_severity_prompt(WELCOME_PROMPT, &context),
        _ => return get_local_message(context_type, context.severity),
    };

    let system_prompt = match config.preferred_style.as_str() {
        "gentle" => format!("你是一个温柔鼓励的健康助手，用中文回答，语气温暖柔和。{}", get_severity_instruction(context.severity)),
        "humor" => format!("你是一个幽默俏皮的健康助手，用中文回答。{}", get_severity_instruction(context.severity)),
        "caring" => format!("你是一个暖心关怀的健康助手，用中文回答。{}", get_severity_instruction(context.severity)),
        "scientific" => format!("你是一个专业的健康顾问，用中文回答。{}", get_severity_instruction(context.severity)),
        _ => format!("你是一个{}健康助手，用中文回答。{}", get_severity_role(context.severity), get_severity_instruction(context.severity)),
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ],
        temperature: config.temperature,
        max_tokens: config.max_tokens,
    };

    let url = format!("{}/chat/completions", config.api_base_url.trim_end_matches('/'));

    match tokio::time::timeout(
        Duration::from_secs(8),
        client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send(),
    )
    .await
    {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                match response.json::<ChatResponse>().await {
                    Ok(result) => {
                        if let Some(choice) = result.choices.first() {
                            let content = choice.message.content.trim().to_string();
                            if config.cache_enabled {
                                let cache_key = format!("{}:{:?}:s{}", context_type, context.hour, context.severity);
                                if let Some(cache) = get_cache().as_mut() {
                                    cache.set(cache_key, content.clone());
                                }
                            }
                            info!("AI generated message (severity {}): {}", context.severity, content);
                            Ok(content)
                        } else {
                            get_local_message(context_type, context.severity)
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse API response: {}", e);
                        get_local_message(context_type, context.severity)
                    }
                }
            } else {
                error!("API error: {}", response.status());
                get_local_message(context_type, context.severity)
            }
        }
        Ok(Err(e)) => {
            error!("Request error: {}", e);
            get_local_message(context_type, context.severity)
        }
        Err(_) => {
            error!("API timeout (2s)");
            get_local_message(context_type, context.severity)
        }
    }
}

// ==================== Local Message Functions ====================

/// Get a random local message matching the given severity level
fn get_local_message(context_type: &str, severity: u32) -> Result<String, String> {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as usize)
        .unwrap_or(0);

    // Find messages matching severity (allow ±1 tolerance)
    let pick_matching = |messages: &[(&str, u32)]| -> Option<String> {
        // First try exact severity match
        let matches: Vec<_> = messages.iter()
            .filter(|(_, s)| *s == severity)
            .collect();
        if !matches.is_empty() {
            return Some(matches[seed % matches.len()].0.to_string());
        }
        // Fallback: allow ±1 severity
        let nearby: Vec<_> = messages.iter()
            .filter(|(_, s)| (*s as i32 - severity as i32).abs() <= 1)
            .collect();
        if !nearby.is_empty() {
            return Some(nearby[seed % nearby.len()].0.to_string());
        }
        // Final fallback: random
        Some(messages[seed % messages.len()].0.to_string())
    };

    match context_type {
        "notification" => pick_matching(NOTIFICATION_MESSAGES).ok_or_else(|| "no msg".into()),
        "fullscreen" => {
            let main = pick_matching(FULLSCREEN_MAIN_TEXTS).unwrap_or_else(|| "休息一下吧".into());
            let sub = pick_matching(FULLSCREEN_SUB_TEXTS).unwrap_or_else(|| "远眺放松".into());
            let inter_idx = seed % FULLSCREEN_INTERACTIONS.len();
            Ok(format!("{}|{}|{}", main, sub, FULLSCREEN_INTERACTIONS[inter_idx]))
        }
        "welcome" => {
            let idx = seed % WELCOME_MESSAGES.len();
            Ok(WELCOME_MESSAGES[idx].to_string())
        }
        "tips" => {
            let idx = seed % TIPS_MESSAGES.len();
            Ok(TIPS_MESSAGES[idx].to_string())
        }
        "tray" => {
            pick_matching(TRAY_MESSAGES).ok_or_else(|| "no msg".into())
        }
        "forced" => {
            let idx = seed % FORCED_REST_TIPS.len();
            Ok(FORCED_REST_TIPS[idx].to_string())
        }
        _ => Ok("休息一下吧！".to_string()),
    }
}

/// Get a tray tooltip message based on eye health
pub fn get_tray_message(eye_health: u32) -> String {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as usize)
        .unwrap_or(0);

    let severity = match eye_health {
        h if h > 65 => 1,
        h if h > 45 => 2,
        h if h > 25 => 3,
        h if h > 10 => 4,
        _ => 5,
    };

    let matches: Vec<_> = TRAY_MESSAGES.iter()
        .filter(|(_, s)| (*s as i32 - severity as i32).abs() <= 1)
        .collect();
    if !matches.is_empty() {
        matches[seed % matches.len()].0.to_string()
    } else {
        TRAY_MESSAGES[seed % TRAY_MESSAGES.len()].0.to_string()
    }
}

fn build_severity_prompt(template: &str, context: &GenerateContext) -> String {
    template
        .replace("{role}", get_severity_role(context.severity))
        .replace("{severity_instruction}", get_severity_instruction(context.severity))
        .replace("{hour}", &context.hour.to_string())
        .replace("{day_of_week}", &context.day_of_week)
        .replace("{idle_minutes}", &context.idle_minutes.to_string())
        .replace("{breaks_today}", &context.breaks_today.to_string())
        .replace("{skip_count}", &context.skip_count.to_string())
        .replace("{eye_health}", &context.eye_health.to_string())
        .replace("{total_skipped_minutes}", &(context.total_skipped_seconds / 60).to_string())
        .replace("{user_name}", if context.user_name.is_empty() { "朋友" } else { &context.user_name })
        .replace("{break_duration}", &context.break_duration.to_string())
        .replace("{time_period}", &context.time_period)
        .replace("{user_type}", if context.user_type.is_empty() { "用户" } else { &context.user_type })
        .replace("{consecutive_work_minutes}", &context.consecutive_work_minutes.to_string())
        .replace("{severity}", &context.severity.to_string())
}
