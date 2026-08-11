# LanChat 原生 UI 主题与国际化设计

## 目标

在 Slint 原生迁移过程中，采用官方 Fluent 控件风格构建企业聊天软件界面，并提供简体中文与英文切换能力。主题和语言必须不依赖 Vue/WebView，不改变既有 SQLite 聊天数据、局域网协议或桌宠资源。

## 视觉方案

### Fluent 基础

- 编译期固定启用 Slint `fluent` 控件样式，Windows 下保持与系统原生控件的协调感。
- 自定义聊天区域使用 `Palette` 与 `StyleMetrics`，避免将颜色、边距散落在页面中。
- 主题为 `system`、`light`、`dark` 三种模式。`system` 由 Slint Fluent 跟随系统配色；显式模式在启动时选定 `fluent-light` 或 `fluent-dark`。

### LanChat 语义令牌

定义少量稳定的界面语义：

- `surface`：窗口与内容背景。
- `sidebar`：左侧导航和会话列表背景。
- `accent`：LanChat 青绿色主操作色。
- `message-own`、`message-peer`：发送与接收消息气泡。
- `muted`、`danger`、`online`：次要文字、危险动作、在线状态。

聊天页面使用紧凑三栏：72px 功能导航、可调整的会话列表、主聊天区。输入工具条与发送区遵循企业聊天软件的密度，不使用嵌套卡片。

## 国际化架构

### 范围

- 第一批提供 `zh-CN` 与 `en-US`。
- 默认 `zh-CN`，系统语言仅在无本地设置时作为初始建议，不自动覆盖用户选择。
- 聊天内容、昵称、频道名称、文件名等用户数据不翻译。

### 模块边界

- 新增 `native_app/i18n.rs`，提供 `Locale`、`TextKey` 与 `Translator`。
- Slint 页面只接收已解析的本地化文本属性，不在 `.slint` 内散落语言条件判断。
- 新增原生外观设置文件 `native-ui-settings.json`，与现有 `lanchat.sqlite3` 位于同一应用数据目录；仅保存 `locale`、`theme` 与未来的原生 UI 偏好。
- 文件缺失、内容不合法或版本升级时回退到安全默认值，不阻断主窗口启动。

## 数据流

1. 原生启动读取 `native-ui-settings.json`。
2. 创建 `Translator`，将基础导航、会话、输入区文案映射为 Slint 属性。
3. 用户在原生设置页切换语言或主题时，更新内存状态、保存设置，并重建需要编译期样式变化的主窗口。
4. 聊天、设备、频道等服务继续通过 `NativeAppServices -> Storage` 读取现有 SQLite。

## 迁移顺序

1. 加入原生外观设置与国际化单元测试。
2. 将主窗口现有静态文案与硬编码基础颜色替换为主题/语言属性。
3. 迁移会话列表、设备页与设置页时全部复用同一文本与主题入口。
4. 在默认入口切换前，验证 `zh-CN`、`en-US` 和明暗主题均能启动、切换且不损坏旧数据库。

## 验收

- 原生窗口在中文和英文下不出现截断或重叠。
- 设置值重启后保持一致，损坏设置文件自动回退。
- 主题切换不改变聊天、设备、桌宠和游戏的本地数据。
- `cargo test --lib` 与 `cargo check` 均通过。
