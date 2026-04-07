# EyeCare - 护眼提醒助手

[English](#english) | [中文](#中文)

---

<a name="中文"></a>

## 中文

### 简介

EyeCare 是一款轻量级的护眼提醒桌面应用，基于 Tauri v2 开发。它通过智能监测您的连续工作时长，在适当的时候提醒您休息，帮助保护眼睛健康。支持 AI 智能生成个性化提醒文案，让休息提醒更加有趣和贴心。

### 功能特性

#### 核心功能

- **智能休息提醒** - 基于连续工作时长触发提醒，而非简单的定时器
- **眼睛生命值系统** - 直观显示眼睛疲劳程度，按工作时间衰减，休息后恢复
- **自然休息检测** - 检测用户空闲状态，自动暂停计时，智能识别有效休息
- **严重程度分级** - 根据跳过次数和眼睛健康值，动态调整提醒语气（1-5级）

#### 喝水提醒

- **上班排班模式** - 按办公时段智能提醒（6个时段，约1150ml/天）
- **固定间隔模式** - 自定义提醒间隔和每日目标
- **进度追踪** - 实时显示今日饮水量和完成进度

#### AI 智能增强

- **多服务商支持** - 硅基流动、OpenAI、DeepSeek、Ollama（本地）
- **个性化文案** - 根据工作时间、跳过次数、眼睛健康生成定制提醒
- **多种风格** - 温柔鼓励型、幽默俏皮型、平衡模式
- **本地消息兜底** - AI 不可用时自动使用本地消息库
- **智能缓存** - 减少重复 API 调用，提升响应速度

#### 宠物系统

- **养成互动** - 休息完成增加宠物心情，跳过休息降低心情
- **等级成长** - 累计休息次数提升宠物等级
- **成就系统** - 解锁各种成就徽章

#### 其他功能

- **系统托盘** - 托盘图标显示倒计时，右键菜单快捷操作
- **应用白名单** - 在会议软件等应用中自动暂停计时
- **开机自启** - 支持开机自动启动
- **全局快捷键** - `Ctrl+Shift+R` 立即休息
- **4K 背景图** - 全屏休息页面支持高清护眼背景图

### 技术栈

- **前端**: HTML5 + CSS3 + Vanilla JavaScript (ES Module)
- **后端**: Rust + Tauri v2
- **跨平台**: Windows / macOS / Linux

### 安装

#### Windows

下载安装包：
- `EyeCare_1.0.0_x64-setup.exe` (NSIS 安装包)
- `EyeCare_1.0.0_x64_en-US.msi` (MSI 安装包)

#### 从源码构建

```bash
# 克隆项目
git clone <repository-url>
cd eye-care

# 安装依赖
npm install

# 开发模式
npm run dev

# 构建发布版本
npm run build
```

### 配置

配置文件位于：
- Windows: `%APPDATA%\eye-care\config.json`
- macOS: `~/Library/Application Support/eye-care/config.json`
- Linux: `~/.config/eye-care/config.json`

#### AI 配置示例

```json
{
  "ai": {
    "enabled": true,
    "provider": "siliconflow",
    "api_base_url": "https://api.siliconflow.cn/v1",
    "api_key": "your-api-key",
    "model": "Qwen/Qwen2.5-7B-Instruct",
    "preferred_style": "balanced"
  }
}
```

### 默认设置

| 设置项 | 默认值 | 说明 |
|--------|--------|------|
| 休息间隔 | 40 分钟 | 连续工作多久后提醒休息 |
| 休息时长 | 60 秒 | 每次休息的时长 |
| 最大跳过次数 | 3 次/天 | 每天最多可跳过的次数 |
| 开机自启 | 开启 | 系统启动时自动运行 |

### 默认白名单应用

腾讯会议、Zoom、Teams、Skype、钉钉、飞书、腾讯QQ、微信、企业微信、Slack、Webex、GoTo Meeting、Google Meet

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+R` | 立即休息 |
| `Esc` | 关闭全屏休息页面 |

### 许可证

MIT License

---

<a name="english"></a>

## English

### Introduction

EyeCare is a lightweight eye protection reminder desktop application built with Tauri v2. It intelligently monitors your continuous work duration and reminds you to take breaks at appropriate times to help protect your eye health. It supports AI-powered personalized reminder messages, making break reminders more engaging and caring.

### Features

#### Core Features

- **Smart Break Reminders** - Triggered by continuous work duration, not simple timers
- **Eye Health System** - Visual display of eye fatigue level, decays with work time, recovers after breaks
- **Natural Break Detection** - Detects user idle state, automatically pauses timing, intelligently recognizes effective breaks
- **Severity Levels** - Dynamically adjusts reminder tone based on skip count and eye health (levels 1-5)

#### Water Reminder

- **Work Schedule Mode** - Smart reminders based on office hours (6 time slots, ~1150ml/day)
- **Fixed Interval Mode** - Customizable reminder interval and daily goal
- **Progress Tracking** - Real-time display of daily water intake and completion progress

#### AI Enhancement

- **Multiple Providers** - SiliconFlow, OpenAI, DeepSeek, Ollama (local)
- **Personalized Messages** - Generate customized reminders based on work time, skip count, and eye health
- **Multiple Styles** - Gentle encouraging, humorous playful, balanced mode
- **Local Message Fallback** - Automatically uses local message library when AI is unavailable
- **Smart Caching** - Reduces redundant API calls, improves response speed

#### Pet System

- **Interactive Companion** - Completing breaks increases pet mood, skipping decreases it
- **Level Growth** - Accumulated break count increases pet level
- **Achievement System** - Unlock various achievement badges

#### Other Features

- **System Tray** - Tray icon shows countdown, right-click menu for quick actions
- **App Whitelist** - Automatically pause timing in meeting apps and other whitelisted applications
- **Auto Start** - Support for automatic startup on system boot
- **Global Shortcuts** - `Ctrl+Shift+R` for immediate break
- **4K Backgrounds** - Fullscreen break page supports HD eye-friendly background images

### Tech Stack

- **Frontend**: HTML5 + CSS3 + Vanilla JavaScript (ES Module)
- **Backend**: Rust + Tauri v2
- **Cross-platform**: Windows / macOS / Linux

### Installation

#### Windows

Download the installer:
- `EyeCare_1.0.0_x64-setup.exe` (NSIS installer)
- `EyeCare_1.0.0_x64_en-US.msi` (MSI installer)

#### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd eye-care

# Install dependencies
npm install

# Development mode
npm run dev

# Build for release
npm run build
```

### Configuration

Configuration file location:
- Windows: `%APPDATA%\eye-care\config.json`
- macOS: `~/Library/Application Support/eye-care/config.json`
- Linux: `~/.config/eye-care/config.json`

#### AI Configuration Example

```json
{
  "ai": {
    "enabled": true,
    "provider": "siliconflow",
    "api_base_url": "https://api.siliconflow.cn/v1",
    "api_key": "your-api-key",
    "model": "Qwen/Qwen2.5-7B-Instruct",
    "preferred_style": "balanced"
  }
}
```

### Default Settings

| Setting | Default | Description |
|---------|---------|-------------|
| Break Interval | 40 min | Continuous work duration before break reminder |
| Break Duration | 60 sec | Duration of each break |
| Max Skips | 3/day | Maximum skips allowed per day |
| Auto Start | Enabled | Automatically run on system startup |

### Default Whitelist Apps

Tencent Meeting, Zoom, Teams, Skype, DingTalk, Feishu, Tencent QQ, WeChat, Enterprise WeChat, Slack, Webex, GoTo Meeting, Google Meet

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+R` | Take a break now |
| `Esc` | Close fullscreen break page |

### License

MIT License
