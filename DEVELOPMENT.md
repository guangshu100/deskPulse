# EyeCare 开发与构建指南

## 环境要求

| 工具          | 版本要求    | 用途                        |
| ----------- | ------- | ------------------------- |
| Node.js     | >= 18   | 前端包管理                     |
| Rust        | >= 1.70 | 后端编译                      |
| Tauri CLI   | ^2      | 构建工具链                     |
| Windows SDK | 10+     | Windows 原生 API（仅 Windows） |

### 检查环境

```bash
node -v
npm -v
rustc --version
cargo --version
```

***

## 安装依赖

```bash
cd eye-care

# 安装前端依赖（Tauri CLI + API）
npm install

# Rust 依赖在首次构建时自动下载，无需手动操作
```

***

## 开发运行

```bash
# 开发模式（热重载，带调试日志）
npm run dev
# 等同于：npx tauri dev
```

启动后：

- 应用以**系统托盘图标**形式运行（主窗口默认隐藏）
- 终端输出 Rust `info!`/`error!` 日志
- 前端修改会自动刷新，Rust 修改会自动重新编译

### 退出开发模式

- 托盘右键 → 退出
- 或终端 `Ctrl+C`

***

## 生产打包

```bash
# 构建发布版本（会生成 .msi 和 .exe 安装包）
npm run build
# 等同于：npx tauri build
```

### 打包产物位置

```
eye-care/src-tauri/target/release/bundle/
├── msi/                    # Windows MSI 安装包
│   └── EyeCare_1.0.0_x64_en-US.msi
└── nsis/                   # Windows NSIS 安装包
    └── EyeCare_1.0.0_x64-setup.exe
```

可执行文件位置：`eye-care/src-tauri/target/release/eye-care.exe`

### 打包配置

在 `src-tauri/tauri.conf.json` 中：

- `bundle.targets`：当前配置为 `["nsis", "msis"]`
- `productName`：`EyeCare`
- `identifier`：`com.eyecare.app`

***

## 快捷键

| 快捷键            | 功能         |
| -------------- | ---------- |
| `Ctrl+Shift+R` | 立即触发一次护眼休息 |
| `Ctrl+Shift+P` | 暂停/恢复监控    |

***

## 项目结构

```
eye-care/
├── src/                        # 前端
│   ├── index.html              # 主窗口（设置页面）
│   ├── fullscreen.html         # 全屏休息页面
│   └── main.js                 # 前端逻辑
├── src-tauri/                  # 后端（Rust）
│   ├── src/
│   │   ├── lib.rs              # Tauri 入口 + 命令注册
│   │   ├── idle.rs             # 空闲检测 + 生命值 + 监控循环
│   │   ├── ai.rs               # AI 文案生成 + 本地消息库
│   │   ├── config.rs           # 配置持久化
│   │   ├── crypto.rs           # API 密钥加密存储
│   │   └── tray.rs             # 托盘图标 + 全屏窗口
│   ├── Cargo.toml              # Rust 依赖
│   ├── tauri.conf.json         # Tauri 应用配置
│   ├── capabilities/           # 权限声明
│   └── icons/                  # 应用图标
├── package.json                # Node.js 依赖
└── DEVELOPMENT.md              # 本文档
```

***

## C 盘空间占用问题

### 占用分析

Rust/Tauri 项目**不会在 C 盘产生项目级的大文件**。但 Rust 工具链本身会占用 C 盘空间：

| 路径                       | 用途                          | 典型大小     |
| ------------------------ | --------------------------- | -------- |
| `C:\Users\<用户>\.cargo\`  | Cargo 缓存（下载的 crate 源码+编译缓存） | \~1 GB   |
| `C:\Users\<用户>\.rustup\` | Rustup 工具链（编译器、标准库、目标平台）    | \~1-2 GB |

项目的 `target/` 目录（编译产物）默认在项目目录下，当前项目约为 **7 GB**（E 盘）。

### 如何将编译缓存移到其他盘

设置环境变量，让 Cargo 把缓存和 registry 放到其他盘：

```powershell
# 在 PowerShell 中设置（永久生效，写入用户环境变量）
[Environment]::SetEnvironmentVariable("CARGO_HOME", "E:\.cargo", "User")
[Environment]::SetEnvironmentVariable("RUSTUP_HOME", "E:\.rustup", "User")
```

然后**手动迁移已有文件**：

```powershell
# 迁移 Cargo 缓存
Move-Item "$env:USERPROFILE\.cargo" "E:\.cargo"
# 迁移 Rustup 工具链
Move-Item "$env:USERPROFILE\.rustup" "E:\.rustup"
```

迁移后重启终端，验证：

```bash
cargo env
rustc --version
```

### 如何清理编译缓存

```bash
# 清理当前项目的编译产物（约 7 GB）
cd eye-care/src-tauri
cargo clean

# 清理 Cargo 全局缓存（所有项目共享的下载+编译缓存）
# 谨慎执行：下次构建需要重新下载所有依赖
cargo cache -a        # 需要先安装：cargo install cargo-cache
# 或者手动删除
Remove-Item -Recurse -Force "$env:CARGO_HOME\registry\cache"
```

### Debug vs Release 体积差异

| 模式                     | 大小       | 说明                            |
| ---------------------- | -------- | ----------------------------- |
| Debug（`tauri dev`）     | \~7 GB   | 包含调试信息，无优化                    |
| Release（`tauri build`） | \~3-4 GB | LTO+strip 优化，最终 exe 约 5-10 MB |

项目 `Cargo.toml` 已配置 release 优化：

```toml
[profile.release]
strip = true      # 去除调试符号
lto = true        # 链接时优化
codegen-units = 1 # 单编译单元（更小体积）
panic = "abort"   # 缩小二进制
```

***

## 常见问题

### Q: `cargo build` 报网络错误 "Could not connect to server"

代理导致的。临时关闭代理或配置：

```bash
# 取消代理
set HTTP_PROXY=
set HTTPS_PROXY=

# 或设置镜像源（字节跳动 RsProxy）
# 在 ~/.cargo/config.toml 中添加：
# [source.crates-io]
# replace-with = 'rsproxy'
# [source.rsproxy]
# registry = "https://rsproxy.cn/crates.io-index"
```

### Q: 编译报 `windows` crate 相关错误

确保 `Cargo.toml` 的 `[target.'cfg(target_os = "windows")'.dependencies]` 中已包含所需 features：

- `Win32_System_SystemInformation`（GetTickCount）
- `Win32_UI_Input_KeyboardAndMouse`（GetLastInputInfo、LASTINPUTINFO）

### Q: 修改前端不生效

开发模式下前端文件会自动热更新。如果没有生效，手动刷新全屏页面或在 `index.html` 所在窗口按 F5。

### Q: 如何查看运行日志

开发模式下终端直接输出。打包后查看：

```
%APPDATA%/com.eyecare.app/logs/
```

