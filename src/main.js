// Autostart plugin - loaded dynamically with fallback
let enableAutostart = async () => {};
let disableAutostart = async () => {};
let isAutostartEnabled = async () => false;

try {
  const autostartModule = await import('@tauri-apps/plugin-autostart');
  enableAutostart = autostartModule.enable;
  disableAutostart = autostartModule.disable;
  isAutostartEnabled = autostartModule.isEnabled;
} catch (e) {
  console.warn('Autostart plugin not available:', e);
}

// Tauri API - with fallback
const getTauriApi = () => {
  if (!window.__TAURI__) {
    console.error('Tauri API not available');
    return null;
  }
  return window.__TAURI__;
};

const invoke = async (...args) => {
  const tauri = getTauriApi();
  if (!tauri) throw new Error('Tauri API not available');
  return tauri.core.invoke(...args);
};

const listen = async (...args) => {
  const tauri = getTauriApi();
  if (!tauri) throw new Error('Tauri API not available');
  return tauri.event.listen(...args);
};

// State
let config = null;
let isPaused = false;
let eyeHealth = 100;
let severity = 1;
let continuousWorkSeconds = 0;
let nextReminderSeconds = 0;

// Water state
let waterConfig = null;
let waterNextReminder = 0;

// Pet state
let petData = null;

// Initialize
window.addEventListener("DOMContentLoaded", async () => {
  try {
    await loadConfig();
    await loadWaterConfig();
    await loadPetState();
    setupEventListeners();
    setupIdleListener();
    setupNotificationListener();
    setupFullscreenListener();
    setupResumeListener();
    setupWaterNotificationListener();
    updateStatus();
    startLocalCountdown();
    startWaterCountdown();
  } catch (e) {
    console.error("Failed to initialize:", e);
  }
});

async function loadConfig() {
  try {
    config = await invoke("get_config");
    isPaused = !config.run_in_tray;
    applyConfigToUI();
    
    // Initialize autostart based on saved config
    try {
      const currentlyEnabled = await isAutostartEnabled();
      if (config.auto_start && !currentlyEnabled) {
        await enableAutostart();
      } else if (!config.auto_start && currentlyEnabled) {
        await disableAutostart();
      }
    } catch (e) {
      console.error("Failed to sync autostart state:", e);
    }
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

function applyConfigToUI() {
  if (!config) return;
  
  document.getElementById("idle-threshold").value = config.idle_threshold;
  document.getElementById("idle-threshold-value").textContent = `${config.idle_threshold}分钟`;
  
  document.getElementById("break-duration").value = config.break_duration;
  document.getElementById("break-duration-value").textContent = `${config.break_duration}秒`;
  
  document.getElementById("max-skips").value = config.max_skips_per_day;
  document.getElementById("max-skips-value").textContent = `${config.max_skips_per_day}次`;
  
  document.getElementById("auto-start").checked = config.auto_start;
  
  // Theme
  document.querySelectorAll(".theme-btn").forEach(btn => {
    btn.classList.toggle("active", btn.dataset.color === config.theme_color);
  });
  
  // AI
  document.getElementById("ai-enabled").checked = config.ai?.enabled || false;
  toggleAiConfig(config.ai?.enabled || false);
  
  if (config.ai) {
    document.getElementById("ai-provider").value = config.ai.provider || "siliconflow";
    document.getElementById("api-base-url").value = config.ai.api_base_url || "";
    document.getElementById("api-key").value = config.ai.api_key || "";
    document.getElementById("model").value = config.ai.model || "";
    document.getElementById("preferred-style").value = config.ai.preferred_style || "balanced";
    updateApiBaseUrlVisibility(config.ai.provider || "siliconflow");
  }
  
  document.getElementById("break-count").textContent = config.break_count_today || 0;

  // 白名单
  whitelistApps = config.whitelist_apps || [];
  renderWhitelist();
}

async function loadPetState() {
  try {
    petData = await invoke("get_pet_state");
    updatePetUI();
  } catch (e) {
    console.error("Failed to load pet state:", e);
  }
}

function updatePetUI() {
  if (!petData) return;
  
  const moodEmoji = petData.mood > 70 ? "😊" : petData.mood > 40 ? "😐" : "😢";
  document.getElementById("header-pet-info").textContent = 
    `${petData.name || "小萌"} ${moodEmoji} ${petData.mood || 100}`;
}

function setupEventListeners() {
  // 基础设置 - 自动保存
  document.getElementById("idle-threshold").addEventListener("input", (e) => {
    document.getElementById("idle-threshold-value").textContent = `${e.target.value}分钟`;
  });
  document.getElementById("idle-threshold").addEventListener("change", saveConfigAuto);
  
  document.getElementById("break-duration").addEventListener("input", (e) => {
    document.getElementById("break-duration-value").textContent = `${e.target.value}秒`;
  });
  document.getElementById("break-duration").addEventListener("change", saveConfigAuto);
  
  document.getElementById("max-skips").addEventListener("input", (e) => {
    document.getElementById("max-skips-value").textContent = `${e.target.value}次`;
  });
  document.getElementById("max-skips").addEventListener("change", saveConfigAuto);
  
  document.getElementById("auto-start").addEventListener("change", saveConfigAuto);
  
  document.querySelectorAll(".theme-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".theme-btn").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      saveConfigAuto();
    });
  });
  
  // AI
  document.getElementById("ai-enabled").addEventListener("change", (e) => {
    toggleAiConfig(e.target.checked);
    saveConfigAuto();
  });
  
  document.getElementById("ai-provider").addEventListener("change", (e) => {
    updateApiBaseUrlVisibility(e.target.value);
    updateDefaultModel(e.target.value);
    saveConfigAuto();
  });
  
  document.getElementById("api-base-url").addEventListener("change", saveConfigAuto);
  document.getElementById("api-key").addEventListener("change", saveConfigAuto);
  document.getElementById("model").addEventListener("change", saveConfigAuto);
  document.getElementById("preferred-style").addEventListener("change", saveConfigAuto);
  
  // 测试AI连接
  document.getElementById("test-ai-btn").addEventListener("click", testAiConnection);
  
  // API密钥明文切换
  document.getElementById("toggle-api-key").addEventListener("click", () => {
    const input = document.getElementById("api-key");
    const btn = document.getElementById("toggle-api-key");
    if (input.type === "password") {
      input.type = "text";
      btn.textContent = "🙈";
    } else {
      input.type = "password";
      btn.textContent = "👁️";
    }
  });

  // 白名单管理
  initWhitelistSuggestions();

  // 按钮
  document.getElementById("toggle-pause").addEventListener("click", togglePause);
  document.getElementById("test-break").addEventListener("click", testBreak);
  document.getElementById("drink-water-btn").addEventListener("click", drinkWater);
  
  // 宠物互动
  document.getElementById("pet-avatar-header").addEventListener("click", petInteract);
  
  // 喝水设置
  document.getElementById("water-enabled").addEventListener("change", (e) => {
    toggleWaterConfig(e.target.checked);
    saveWaterConfig();
  });
  
  document.getElementById("water-interval").addEventListener("input", (e) => {
    document.getElementById("water-interval-value").textContent = `${e.target.value}分钟`;
  });
  document.getElementById("water-interval").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-goal").addEventListener("input", (e) => {
    document.getElementById("water-goal-value").textContent = `${e.target.value}ml`;
    document.getElementById("water-goal-display").textContent = e.target.value;
  });
  document.getElementById("water-goal").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-cup").addEventListener("input", (e) => {
    document.getElementById("water-cup-value").textContent = `${e.target.value}ml`;
  });
  document.getElementById("water-cup").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-cup-general").addEventListener("input", (e) => {
    document.getElementById("water-cup-general-value").textContent = `${e.target.value}ml`;
    document.getElementById("water-cup").value = e.target.value;
    document.getElementById("water-cup-value").textContent = `${e.target.value}ml`;
  });
  document.getElementById("water-cup-general").addEventListener("change", saveWaterConfig);
  
  document.getElementById("water-start-hour").addEventListener("change", saveWaterConfig);
  document.getElementById("water-end-hour").addEventListener("change", saveWaterConfig);
}

function toggleAiConfig(show) {
  const aiConfig = document.getElementById("ai-config");
  aiConfig.classList.toggle("hidden", !show);
}

function updateApiBaseUrlVisibility(provider) {
  const row = document.getElementById("api-base-url-row");
  const defaultUrls = {
    siliconflow: "https://api.siliconflow.cn/v1",
    openai: "https://api.openai.com/v1",
    deepseek: "https://api.deepseek.com/v1",
    ollama: "http://localhost:11434/v1",
    custom: ""
  };
  
  if (provider === "custom") {
    row.style.display = "flex";
    document.getElementById("api-base-url").placeholder = "输入API地址";
  } else {
    row.style.display = "none";
    document.getElementById("api-base-url").value = defaultUrls[provider] || "";
  }
}

// 白名单管理
let whitelistApps = [];

// 应用图标映射表
const APP_ICON_MAP = {
  "腾讯会议": "📹",
  "zoom": "🎥",
  "teams": "💬",
  "skype": "📞",
  "钉钉": "📌",
  "飞书": "📄",
  "腾讯QQ": "🐧",
  "微信": "💚",
  "企业微信": "🏢",
  "slack": "💬",
  "webex": "🌐",
  "go to meeting": "🤝",
  "google meet": "📅",
  "notion": "📓",
  "obsidian": "💜",
  "figma": "🎨",
  "photoshop": "🖼️",
  "illustrator": "✏️",
  "premiere": "🎬",
  "after effects": "✨",
  "blender": "🧊",
  "unity": "🎮",
  "unreal": "🎯",
  "visual studio": "💻",
  "visual studio code": "📝",
  "vs code": "📝",
  "code": "📝",
  "intellij": "🧠",
  "idea": "🧠",
  "pycharm": "🐍",
  "webstorm": "🌊",
  "clion": "🔧",
  "goland": "🐹",
  "rustrover": "🦀",
  "cursor": "🖱️",
  "vim": "📏",
  "neovim": "📝",
  "terminal": "⬛",
  "powershell": "⚡",
  "cmd": "⬛",
  "chrome": "🌐",
  "edge": "🔵",
  "firefox": "🦊",
  "safari": "🧭",
  "bilibili": "📺",
  "youtube": "▶️",
  "netflix": "🎬",
  "spotify": "🎵",
  "qq音乐": "🎵",
  "网易云音乐": "🎵",
  " steam": "🎮",
  "epic games": "🎮",
};

// 推荐应用列表（按分类）
const SUGGESTED_APPS = [
  { category: "视频会议", apps: [
    { name: "腾讯会议", icon: "📹" },
    { name: "Zoom", icon: "🎥" },
    { name: "Teams", icon: "💬" },
    { name: "Skype", icon: "📞" },
    { name: "钉钉", icon: "📌" },
    { name: "飞书", icon: "📄" },
    { name: "Webex", icon: "🌐" },
    { name: "Google Meet", icon: "📅" },
  ]},
  { category: "即时通讯", apps: [
    { name: "微信", icon: "💚" },
    { name: "腾讯QQ", icon: "🐧" },
    { name: "企业微信", icon: "🏢" },
    { name: "Slack", icon: "💬" },
  ]},
  { category: "开发工具", apps: [
    { name: "Visual Studio Code", icon: "📝" },
    { name: "IntelliJ IDEA", icon: "🧠" },
    { name: "PyCharm", icon: "🐍" },
    { name: "Cursor", icon: "🖱️" },
    { name: "Terminal", icon: "⬛" },
  ]},
  { category: "设计创作", apps: [
    { name: "Figma", icon: "🎨" },
    { name: "Photoshop", icon: "🖼️" },
    { name: "Premiere", icon: "🎬" },
    { name: "Blender", icon: "🧊" },
  ]},
  { category: "影音娱乐", apps: [
    { name: "Bilibili", icon: "📺" },
    { name: "YouTube", icon: "▶️" },
    { name: "Spotify", icon: "🎵" },
    { name: "网易云音乐", icon: "🎵" },
    { name: "Steam", icon: "🎮" },
  ]},
  { category: "浏览器", apps: [
    { name: "Chrome", icon: "🌐" },
    { name: "Edge", icon: "🔵" },
    { name: "Firefox", icon: "🦊" },
  ]},
];

// 获取应用图标
function getAppIcon(appName) {
  const lower = appName.toLowerCase();
  // 先精确匹配
  if (APP_ICON_MAP[appName]) return APP_ICON_MAP[appName];
  if (APP_ICON_MAP[lower]) return APP_ICON_MAP[lower];
  // 模糊匹配
  for (const [key, icon] of Object.entries(APP_ICON_MAP)) {
    if (lower.includes(key) || key.includes(lower)) return icon;
  }
  return "📱";
}

function renderWhitelist() {
  const container = document.getElementById("whitelist-apps");
  if (whitelistApps.length === 0) {
    container.innerHTML = '<span class="whitelist-empty-hint">暂无白名单应用，推荐添加会议和通讯类应用</span>';
    return;
  }
  container.innerHTML = whitelistApps.map((app, index) => `
    <span class="whitelist-tag" data-index="${index}" title="${escapeHtml(app)}">
      <span class="app-name">${escapeHtml(app)}</span>
      <button class="remove-btn" data-remove="${index}">×</button>
    </span>
  `).join("");
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function escapeAttr(text) {
  return text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/'/g, "&#39;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function addWhitelistApp(appName) {
  const input = document.getElementById("whitelist-input");
  const name = (appName || input.value).trim();
  if (name && !whitelistApps.some(a => a.toLowerCase() === name.toLowerCase())) {
    whitelistApps.push(name);
    renderWhitelist();
    saveConfigAuto();
  }
  input.value = "";
  hideSuggestions();
}

function removeWhitelistApp(index) {
  whitelistApps.splice(index, 1);
  renderWhitelist();
  saveConfigAuto();
}

// 推荐应用下拉
function showSuggestions(filter = "") {
  const container = document.getElementById("whitelist-suggestions");
  const filterLower = filter.toLowerCase();

  // 过滤掉已添加的
  const addedLower = whitelistApps.map(a => a.toLowerCase());

  let html = "";
  let hasAny = false;

  for (const group of SUGGESTED_APPS) {
    const filteredApps = group.apps.filter(app =>
      !addedLower.includes(app.name.toLowerCase()) &&
      (!filterLower || app.name.toLowerCase().includes(filterLower))
    );
    if (filteredApps.length === 0) continue;
    hasAny = true;
    html += `<div class="suggestions-category-label">${escapeHtml(group.category)}</div>`;
    for (const app of filteredApps) {
      html += `<div class="whitelist-suggestion-item" data-app-name="${escapeAttr(app.name)}">
        <span class="suggestion-icon">${app.icon}</span>
        <span class="suggestion-name">${escapeHtml(app.name)}</span>
        <span class="suggestion-category">${escapeHtml(group.category)}</span>
      </div>`;
    }
  }

  if (!hasAny) {
    html = '<div class="suggestions-category-label" style="padding:12px;text-align:center;">无匹配应用，直接回车添加</div>';
  }

  container.innerHTML = html;
  container.classList.remove("hidden");
}

function hideSuggestions() {
  document.getElementById("whitelist-suggestions").classList.add("hidden");
}

function initWhitelistSuggestions() {
  const input = document.getElementById("whitelist-input");
  const suggestionsEl = document.getElementById("whitelist-suggestions");

  // 点击输入框时显示推荐
  input.addEventListener("focus", () => {
    showSuggestions(input.value.trim());
  });

  // 输入时过滤
  input.addEventListener("input", () => {
    const val = input.value.trim();
    showSuggestions(val);
  });

  // 回车添加
  input.addEventListener("keypress", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addWhitelistApp();
    }
  });

  // 事件委托：点击推荐项添加
  suggestionsEl.addEventListener("mousedown", (e) => {
    const item = e.target.closest(".whitelist-suggestion-item");
    if (item) {
      e.preventDefault(); // 阻止 input 失焦
      const appName = item.getAttribute("data-app-name");
      if (appName) addWhitelistApp(appName);
    }
  });

  // 点击外部关闭
  document.addEventListener("mousedown", (e) => {
    if (!e.target.closest(".whitelist-input-wrap")) {
      hideSuggestions();
    }
  });

  // 事件委托：删除白名单标签
  document.getElementById("whitelist-apps").addEventListener("click", (e) => {
    const btn = e.target.closest(".remove-btn");
    if (btn) {
      const index = parseInt(btn.getAttribute("data-remove"), 10);
      if (!isNaN(index)) removeWhitelistApp(index);
    }
  });
}

function updateDefaultModel(provider) {
  const modelInput = document.getElementById("model");
  const defaults = {
    siliconflow: "Qwen/Qwen2.5-7B-Instruct",
    openai: "gpt-3.5-turbo",
    deepseek: "deepseek-chat",
    ollama: "llama3",
    custom: ""
  };
  modelInput.value = defaults[provider] || "";
}

async function testAiConnection() {
  const resultEl = document.getElementById("ai-test-result");
  const btn = document.getElementById("test-ai-btn");
  
  btn.disabled = true;
  btn.style.opacity = "0.5";
  resultEl.classList.remove("hidden", "success", "error");
  resultEl.textContent = "正在保存配置并连接...";
  
  try {
    // 先保存当前配置
    await saveConfigAuto();
    // 再测试连接（从 state 读取最新配置）
    await invoke("test_api_connection");
    resultEl.classList.add("success");
    resultEl.textContent = "✅ 连接成功！模型响应正常";
  } catch (e) {
    resultEl.classList.add("error");
    resultEl.textContent = `❌ 连接失败: ${e}`;
  }
  
  btn.disabled = false;
  btn.style.opacity = "1";
}

function toggleWaterConfig(show) {
  const waterConfigEl = document.getElementById("water-config");
  waterConfigEl.classList.toggle("hidden", !show);
}

async function saveConfigAuto() {
  if (!config) return;
  
  const autoStartEnabled = document.getElementById("auto-start").checked;
  
  const newConfig = {
    ...config,
    idle_threshold: parseInt(document.getElementById("idle-threshold").value),
    break_duration: parseInt(document.getElementById("break-duration").value),
    max_skips_per_day: parseInt(document.getElementById("max-skips").value),
    auto_start: autoStartEnabled,
    theme_color: document.querySelector(".theme-btn.active")?.dataset.color || "#E8F4F8",
    whitelist_apps: whitelistApps,
    ai: {
      ...config.ai,
      enabled: document.getElementById("ai-enabled").checked,
      provider: document.getElementById("ai-provider").value,
      api_base_url: document.getElementById("api-base-url").value,
      api_key: document.getElementById("api-key").value,
      model: document.getElementById("model").value,
      preferred_style: document.getElementById("preferred-style").value,
    },
  };
  
  try {
    await invoke("save_config", { newConfig });
    config = newConfig;
    
    // Apply autostart setting to system
    try {
      if (autoStartEnabled) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
    } catch (e) {
      console.error("Failed to set autostart:", e);
    }
  } catch (e) {
    console.error("Failed to save config:", e);
  }
}

async function togglePause() {
  isPaused = !isPaused;
  try {
    await invoke("toggle_pause");
  } catch (e) {
    console.error("Failed to toggle pause:", e);
  }
  updateStatus();
}

async function testBreak() {
  try {
    await invoke("show_fullscreen", { 
      forced: false, 
      severity: 1, 
      eye_health: 100,
      skip_history_json: null,
      total_skipped_seconds: 0
    });
  } catch (e) {
    console.error("Failed to test break:", e);
  }
}

async function petInteract() {
  try {
    petData = await invoke("pet_interact");
    updatePetUI();
  } catch (e) {
    console.error("Failed to interact with pet:", e);
  }
}

function updateStatus() {
  const statusText = document.getElementById("status-text");
  const statusDot = document.getElementById("status-dot");
  const toggleBtn = document.getElementById("toggle-pause");

  if (isPaused) {
    statusText.textContent = "已暂停";
    statusDot.classList.remove("active");
    toggleBtn.textContent = "开始";
  } else {
    statusText.textContent = "运行中";
    statusDot.classList.add("active");
    toggleBtn.textContent = "暂停";
  }
}

// ==================== 空闲状态监听 ====================

async function setupIdleListener() {
  await listen("idle-status", (event) => {
    const status = event.payload;
    eyeHealth = status.eye_health || 100;
    severity = status.severity || 1;
    continuousWorkSeconds = status.continuous_work_seconds || 0;
    nextReminderSeconds = status.next_reminder_seconds || 0;

    document.getElementById("eye-health").textContent = `${eyeHealth}%`;
    document.getElementById("break-count").textContent = status.skip_count_today || 0;
    
    // 更新宠物状态
    if (status.pet) {
      petData = status.pet;
      updatePetUI();
    }
  });

  try {
    const status = await invoke("get_status");
    eyeHealth = status.eye_health || 100;
    continuousWorkSeconds = status.continuous_work_seconds || 0;
    nextReminderSeconds = status.next_reminder_seconds || 0;
    
    // 更新 UI
    document.getElementById("eye-health").textContent = `${eyeHealth}%`;
    document.getElementById("break-count").textContent = status.skip_count_today || 0;
  } catch (e) {
    console.error("Failed to get status:", e);
  }
}

function startLocalCountdown() {
  setInterval(() => {
    if (!isPaused) {
      continuousWorkSeconds++;
      const workMinutes = Math.floor(continuousWorkSeconds / 60);
      const workSeconds = continuousWorkSeconds % 60;
      document.getElementById("continuous-work-time").textContent = `${workMinutes}分${workSeconds}秒`;
      
      if (nextReminderSeconds > 0) {
        nextReminderSeconds--;
        const remMin = Math.floor(nextReminderSeconds / 60);
        const remSec = nextReminderSeconds % 60;
        document.getElementById("next-reminder").textContent = `${remMin}分${remSec}秒`;
      } else {
        document.getElementById("next-reminder").textContent = "即将提醒";
      }
    }
  }, 1000);
}

// ==================== 喝水提醒 ====================

let currentWaterMode = "schedule"; // "schedule" | "interval"

async function loadWaterConfig() {
  try {
    waterConfig = await invoke("get_water_config");
    applyWaterConfigToUI();
    updateWaterStats();
  } catch (e) {
    console.error("Failed to load water config:", e);
  }
}

function applyWaterConfigToUI() {
  if (!waterConfig) return;
  
  document.getElementById("water-enabled").checked = waterConfig.enabled;
  toggleWaterConfig(waterConfig.enabled);
  
  // 设置模式
  currentWaterMode = waterConfig.schedule_enabled ? "schedule" : "interval";
  setWaterModeUI(currentWaterMode);
  
  // 通用设置
  const cupVal = waterConfig.cup_size_ml || 250;
  document.getElementById("water-cup-general").value = cupVal;
  document.getElementById("water-cup-general-value").textContent = `${cupVal}ml`;
  
  // 间隔模式设置
  document.getElementById("water-interval").value = waterConfig.interval_minutes;
  document.getElementById("water-interval-value").textContent = `${waterConfig.interval_minutes}分钟`;
  document.getElementById("water-goal").value = waterConfig.daily_goal_ml;
  document.getElementById("water-goal-value").textContent = `${waterConfig.daily_goal_ml}ml`;
  document.getElementById("water-goal-display").textContent = waterConfig.daily_goal_ml;
  document.getElementById("water-cup").value = cupVal;
  document.getElementById("water-cup-value").textContent = `${cupVal}ml`;
  document.getElementById("water-start-hour").value = waterConfig.start_hour;
  document.getElementById("water-end-hour").value = waterConfig.end_hour;
}

function setWaterMode(mode) {
  currentWaterMode = mode;
  setWaterModeUI(mode);
  saveWaterConfig();
}

function setWaterModeUI(mode) {
  const scheduleBtn = document.getElementById("mode-schedule");
  const intervalBtn = document.getElementById("mode-interval");
  const schedulePanel = document.getElementById("schedule-panel");
  const intervalPanel = document.getElementById("interval-panel");
  
  scheduleBtn.classList.toggle("active", mode === "schedule");
  intervalBtn.classList.toggle("active", mode === "interval");
  schedulePanel.classList.toggle("hidden", mode !== "schedule");
  intervalPanel.classList.toggle("hidden", mode !== "interval");
}
window.setWaterMode = setWaterMode;

function renderScheduleSlots(slots) {
  const container = document.getElementById("schedule-list");
  if (!container || !slots || slots.length === 0) {
    if (container) container.innerHTML = '<div class="schedule-empty">暂无排班数据</div>';
    return;
  }
  
  container.innerHTML = slots.map(slot => {
    const stateClass = slot.completed ? 'slot-completed' : (slot.is_current ? 'slot-current' : 'slot-pending');
    const stateIcon = slot.completed ? '✅' : (slot.is_current ? '⏰' : '⬜');
    const currentBadge = slot.is_current ? '<span class="slot-badge">当前</span>' : '';
    return `<div class="schedule-slot ${stateClass}">
      <span class="slot-icon">${slot.icon}</span>
      <div class="slot-info">
        <div class="slot-header">
          <span class="slot-label">${slot.label} ${currentBadge}</span>
          <span class="slot-amount">${slot.amount_ml}ml</span>
        </div>
        <span class="slot-time">${slot.time_range}</span>
      </div>
      <span class="slot-status">${stateIcon}</span>
    </div>`;
  }).join('');
}

async function saveWaterConfig() {
  if (!waterConfig) return;
  
  const cupSize = parseInt(document.getElementById("water-cup-general").value);
  const newConfig = {
    enabled: document.getElementById("water-enabled").checked,
    interval_minutes: parseInt(document.getElementById("water-interval").value),
    daily_goal_ml: parseInt(document.getElementById("water-goal").value),
    cup_size_ml: cupSize,
    sound_enabled: waterConfig.sound_enabled,
    start_hour: parseInt(document.getElementById("water-start-hour").value),
    end_hour: parseInt(document.getElementById("water-end-hour").value),
    stats: waterConfig.stats,
    schedule_enabled: currentWaterMode === "schedule",
  };
  
  try {
    await invoke("save_water_config", { newConfig });
    waterConfig = newConfig;
  } catch (e) {
    console.error("Failed to save water config:", e);
  }
}

async function drinkWater() {
  try {
    const status = await invoke("drink_one_cup");
    waterConfig.stats = {
      today_date: waterConfig.stats.today_date,
      total_ml: status.total_ml,
      drink_count: status.drink_count,
      last_drink_time: status.last_drink_time,
      schedule_completed: status.schedule_slots ? 
        status.schedule_slots.map(s => s.completed) : 
        (waterConfig.stats.schedule_completed || []),
    };
    updateWaterStats();
  } catch (e) {
    console.error("Failed to record drink:", e);
  }
}

async function updateWaterStats() {
  try {
    const status = await invoke("get_water_status");
    
    document.getElementById("water-total").textContent = status.total_ml;
    document.getElementById("water-total-display").textContent = status.total_ml;
    
    // 更新 header
    if (status.schedule_enabled) {
      document.getElementById("header-water-info").textContent = 
        `💧 ${status.schedule_completed_count}/${status.schedule_total_slots}`;
    } else {
      document.getElementById("header-water-info").textContent = `💧 ${status.total_ml}ml`;
    }
    
    // 进度条
    const progressFill = document.getElementById("water-progress-fill");
    if (status.schedule_enabled) {
      const percent = status.schedule_total_slots > 0 
        ? (status.schedule_completed_count / status.schedule_total_slots * 100) : 0;
      progressFill.style.width = `${percent}%`;
      document.getElementById("schedule-progress-text").textContent = 
        `${status.schedule_completed_count}/${status.schedule_total_slots} 时段`;
    } else {
      progressFill.style.width = `${status.progress_percent}%`;
      document.getElementById("water-goal-display").textContent = status.daily_goal_ml;
    }
    
    // 渲染排班列表
    if (status.schedule_enabled && status.schedule_slots) {
      renderScheduleSlots(status.schedule_slots);
    }
    
    waterNextReminder = status.next_reminder_seconds || 0;
  } catch (e) {
    console.error("Failed to get water status:", e);
  }
}

function startWaterCountdown() {
  setInterval(() => {
    if (waterConfig && waterConfig.enabled && waterNextReminder > 0) {
      waterNextReminder--;
    }
  }, 1000);
}

async function setupWaterNotificationListener() {
  await listen("water-reminder", async () => {
    await updateWaterStats();
  });
}

// ==================== 其他监听 ====================

async function setupNotificationListener() {
  await listen("trigger-notification", async () => {
    console.log("Notification triggered");
  });
}

async function setupFullscreenListener() {
  await listen("trigger-fullscreen", async (event) => {
    const payload = event.payload;
    console.log("Fullscreen triggered", payload);
    
    // 构建 URL 参数，包含 AI 内容
    const params = new URLSearchParams({
      duration: 30,
      theme: '#E8F4F8',
      forced: payload.forced,
      severity: payload.severity,
      eye_health: payload.eye_health,
      skip_history: btoa(JSON.stringify(payload.skip_history || [])),
      total_skipped: payload.total_skipped_seconds || 0,
    });
    
    // 添加 AI 内容（如果存在）
    if (payload.ai_title) params.set('ai_title', payload.ai_title);
    if (payload.ai_main_text) params.set('ai_main_text', payload.ai_main_text);
    if (payload.ai_sub_text) params.set('ai_sub_text', payload.ai_sub_text);
    if (payload.ai_interaction) params.set('ai_interaction', payload.ai_interaction);
    
    try {
      await invoke("show_fullscreen", { 
        forced: payload.forced, 
        severity: payload.severity, 
        eye_health: payload.eye_health,
        skip_history_json: JSON.stringify(payload.skip_history || []),
        total_skipped_seconds: payload.total_skipped_seconds || 0,
        ai_title: payload.ai_title,
        ai_main_text: payload.ai_main_text,
        ai_sub_text: payload.ai_sub_text,
        ai_interaction: payload.ai_interaction,
      });
    } catch (e) {
      console.error("Failed to show fullscreen:", e);
    }
  });
}

async function setupResumeListener() {
  await listen("system-resumed", (event) => {
    console.log("System resumed from sleep, gap:", event.payload);
  });
}

// 全局函数：折叠区块
function toggleSection(id) {
  const content = document.getElementById(id);
  if (!content) return;
  const header = content.previousElementSibling;
  content.classList.toggle("expanded");
  header.classList.toggle("expanded");
}

// 使用事件委托统一处理折叠
document.addEventListener("DOMContentLoaded", () => {
  document.addEventListener("click", (e) => {
    const header = e.target.closest(".collapsible-header");
    if (header) {
      const content = header.nextElementSibling;
      if (content && content.classList.contains("collapsible-content")) {
        content.classList.toggle("expanded");
        header.classList.toggle("expanded");
      }
    }
  });
});
