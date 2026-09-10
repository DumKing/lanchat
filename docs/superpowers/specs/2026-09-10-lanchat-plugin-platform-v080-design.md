# LanChat v0.8.0 插件平台与项目骨架设计

## 1. 背景与目标

LanChat 当前把聊天、设备、桌宠、游戏、视觉识别、设置和大部分业务状态集中在 `src/App.vue` 与单个 Pinia store 中。功能已经可用，但新游戏、视觉能力和后续扩展继续进入主程序，会同时放大安装包、回归范围、权限面和维护成本。

v0.8.0 先建立可运行的插件宿主骨架，并用五子棋验证一条完整链路。设计参考 uTools 的独立清单、预先注册生命周期回调和宿主注入 API，但 API、权限、房间模型与 LanChat 的局域网业务保持独立。参考资料：[uTools 生命周期事件](https://www.u-tools.cn/docs/developer/utools-api/events.html)、[插件目录结构](https://www.u-tools.cn/docs/developer/information/file-structure.html)、[plugin.json](https://www.u-tools.cn/docs/developer/information/plugin-json.html)。

本阶段目标：

- 提供版本化的 `window.lanchat` 标准 API，插件不直接访问 Pinia、Tauri `invoke` 或内部模块。
- 提供插件清单校验、安装、启停、卸载、回滚和私有存储的宿主基础。
- 提供插件中心页面和统一导航扩展点。
- 把应用外壳、页面路由和业务模块从 `App.vue` 逐步拆开。
- 将五子棋迁移成第一个官方 Web 插件，验证房间、消息、主题和排行榜能力。
- 按能力逐项直接迁移；每个插件达到验收标准后，在同一批次删除对应内置入口、页面和业务实现，不保留双入口或回退实现。

本阶段不迁移视觉识别 sidecar，也不开放任意系统命令、任意文件系统或任意网络访问。尚未开始迁移的游戏暂时维持现状；一旦开始迁移，就以标准 API 插件为唯一实现。视觉代码与模型资源的构建裁剪随视觉插件迁移一起完成。

## 2. 总体结构

```text
LanChat Core
├─ AppShell：标题栏、导航、页面容器、全局弹层
├─ Core Services：聊天、设备发现、桌宠、升级
├─ Plugin Manager：安装、校验、版本、启停、回滚
├─ Plugin Runtime：隔离 WebView、生命周期、消息桥、能力令牌
├─ Host API：设备、聊天、房间、存储、主题、UI、日志
├─ Contribution Registry：导航、命令、游戏、设置页
└─ Plugin Center：已安装、可用更新、权限、错误与操作

Official Plugins
├─ com.lanchat.gomoku
├─ com.lanchat.xiangqi（后续）
├─ com.lanchat.minesweeper（后续）
├─ com.lanchat.monopoly（后续）
└─ com.lanchat.vision（后续，Web UI + 签名 sidecar）
```

正式运行环境使用 Tauri 子 WebView 承载插件页面。Rust `PluginRuntime` 负责创建、定位、显隐、缩放和销毁子 WebView，通过只读 `lanchat-plugin://<plugin-id>/<version>/...` 协议加载已校验资源，并为每个插件设置禁止外部导航、禁止任意脚本源和禁止通用 Tauri IPC 的独立 CSP。宿主只注入最小引导脚本，所有调用通过带实例令牌的专用 IPC 通道进入桥分发器。浏览器开发与单元测试使用 iframe 测试适配器，但发布包不以同源 iframe 作为安全边界。

## 3. 插件包格式

官方扩展名为 `.lcp`，内容为 ZIP：

```text
plugin.json
icon.png
integrity.json
signature.ed25519
dist/
  index.html
  assets/*
sidecar/                 # 可选，v0.8.0 游戏插件不使用
  windows-x86_64/*
```

`plugin.json` 第一版结构：

```json
{
  "manifestVersion": 1,
  "id": "com.lanchat.gomoku",
  "name": "五子棋",
  "version": "0.8.0",
  "apiVersion": "1.0",
  "minHostVersion": "0.8.0",
  "type": "web",
  "entry": "dist/index.html",
  "icon": "icon.png",
  "singleton": true,
  "capabilities": [
    "rooms.read",
    "rooms.write",
    "leaderboard.read",
    "leaderboard.write",
    "storage.private",
    "theme.read",
    "logger.write"
  ],
  "contributes": {
    "navigation": [{ "id": "gomoku", "title": "五子棋", "icon": "icon.png", "order": 210 }],
    "games": [{ "id": "gomoku", "minPlayers": 2, "maxPlayers": 2, "ranking": { "type": "win-loss", "minHumanPlayers": 2 } }]
  }
}
```

约束：

- `id` 使用反向域名格式，安装后不可修改。
- `entry`、`icon` 和完整性清单中的路径必须是相对路径，规范化后仍位于插件目录内。
- `apiVersion` 按主版本决定兼容边界；宿主拒绝不支持的主版本。
- 每项 API 调用必须同时满足清单声明、用户授权和当前实例能力令牌。
- `integrity.json` 列出包内文件的 SHA-256；官方包必须通过 Ed25519 签名。开发模式可安装未签名包，但页面持续显示开发标识。

签名输入是 UTF-8 编码的 canonical JSON：键按字典序排列、无额外空白，内容覆盖 `plugin.json` 的 SHA-256、`integrity.json` 的 SHA-256、插件 ID 和版本。宿主内嵌带 `keyId` 的官方公钥 keyring；流水线从 GitHub secret 注入私钥，只输出签名和 `keyId`。keyring 可同时保留新旧公钥完成轮换，撤销列表随宿主更新。

## 4. 标准 API

插件通过 `window.lanchat` 访问 API。根对象只读，并包含 `apiVersion` 与以下命名空间：

```ts
interface LanChatPluginApiV1 {
  readonly apiVersion: "1.0";
  app: AppApi;
  events: EventApi;
  devices: DeviceApi;
  chat: ChatApi;
  rooms: RoomApi;
  leaderboard: LeaderboardApi;
  storage: StorageApi;
  theme: ThemeApi;
  ui: UiApi;
  logger: LoggerApi;
}
```

### 4.1 生命周期与事件

事件注册沿用“插件预先注册、宿主主动回调”的模式。每个注册函数返回取消订阅函数，避免页面反复进入后累积监听器。

```ts
type Unsubscribe = () => void;

events.onPluginEnter((context) => {}): Unsubscribe;
events.onPluginOut((context) => {}): Unsubscribe;
events.onThemeChanged((theme) => {}): Unsubscribe;
events.onNetworkChanged((network) => {}): Unsubscribe;
events.onRoomEvent((event) => {}): Unsubscribe;
events.onVisibilityChanged((visible) => {}): Unsubscribe;
```

进入上下文包含 `featureCode`、`payload`、`source`、`instanceId` 和当前宿主版本。退出上下文包含 `reason` 与 `isTerminated`，插件可区分隐藏、切换页面、禁用和进程终止。

宿主事件采用单调递增序号；房间事件同时携带 `roomId`、`gameId`、`senderPeerId`、`schemaVersion` 和幂等键。插件重连后通过房间快照恢复状态，不依赖完整事件历史重放。

### 4.2 业务能力

- `app`：读取宿主版本、插件实例信息、退出当前插件页面。
- `devices`：读取已发现设备的脱敏快照；不暴露内部连接对象。
- `chat`：在授权频道发送文本或结构化卡片、订阅与插件有关的消息。
- `rooms`：创建、加入、离开游戏房间，发送房主权威事件，读取成员和快照。
- `leaderboard`：按宿主管理的游戏类型读取或提交结算；计榜真人阈值由插件清单声明。多人游戏默认 2 人，`maxPlayers: 1` 的单机游戏默认 1 人，插件可提高阈值。机器人始终不计入阈值，也不产生排行记录。
- `storage`：插件私有键值与 JSON 文档存储，按 `pluginId` 自动隔离。
- `theme`：读取当前主题 token，并订阅主题变化。
- `ui`：通知、确认框、文件选择等受控宿主 UI。
- `logger`：分级日志，自动附加插件、版本和实例信息。

API 请求使用统一信封：

```ts
interface BridgeRequest {
  id: string;
  apiVersion: "1.0";
  instanceToken: string;
  method: string;
  params: unknown;
}

interface BridgeResponse {
  id: string;
  ok: boolean;
  result?: unknown;
  error?: { code: string; message: string; retryable: boolean };
}
```

所有参数和返回值都经过 schema 校验。单次消息、频率、存储空间和日志数量均设配额；超限返回稳定错误码，不把 Rust/SQLite 内部错误直接暴露给插件。

## 5. 权限与隔离

第一版权限按最小能力拆分：

| 能力 | 用途 | 默认 |
| --- | --- | --- |
| `devices.read` | 读取局域网设备摘要 | 询问 |
| `chat.read` / `chat.send` | 读取授权消息、发送消息 | 询问 |
| `rooms.read` / `rooms.write` | 游戏房间与事件 | 游戏插件默认允许 |
| `leaderboard.read` / `leaderboard.write` | 排行榜读取与结算提交 | 游戏插件默认允许 |
| `storage.private` | 插件私有数据 | 默认允许 |
| `theme.read` | 主题 token | 默认允许 |
| `ui.notify` / `ui.filePicker` | 宿主通知、文件选择 | 分项询问 |
| `network.lan` / `network.internet` | 受控网络访问 | 询问 |
| `logger.write` | 写插件诊断日志 | 默认允许 |
| `native.sidecar` | 启动签名子进程 | 高风险，后续开放 |

插件 WebView 不拥有通用 Tauri capability，不加载宿主前端源码，也不能读取其他插件目录。插件安装目录只读，数据写入 `plugins-data/<pluginId>/` 对应的 SQLite 命名空间。禁用插件时立即撤销实例令牌、关闭 WebView、取消事件订阅并停止 sidecar。

## 6. 插件管理与 UI

插件状态：`staged -> installed -> enabled -> running -> stopped`，异常进入 `failed`，更新失败可回滚到 `previousVersion`。

插件中心包含：

- “已安装”和“发现插件”两个页签；第一版读取随宿主发布的签名官方目录 `plugin-catalog.json`，目录只保存元数据、下载地址、哈希和签名，后续可切换远程目录。
- 搜索、类型筛选、版本、来源、签名状态、权限摘要和运行状态。
- 安装、启用、禁用、更新、回滚、卸载；卸载时单独选择是否删除插件数据。
- 错误详情、最近日志与重新加载入口。
- 权限变化对比；更新新增权限时暂停自动启用并要求用户确认。

插件贡献的导航项进入统一 registry，AppShell 根据 registry 渲染。内置页面也使用同一导航模型，避免插件入口再次硬编码进 `App.vue`。

## 7. 项目骨架重构

### 7.0 仓库边界

插件平台采用核心与扩展分仓，当前仓库及地址如下：

| 仓库 | 地址 | 职责 |
| --- | --- | --- |
| `lanchat` | https://github.com/DumKing/lanchat | 核心宿主、标准桥、插件中心与核心能力 |
| `lanchat-plugin-sdk` | https://github.com/DumKing/lanchat-plugin-sdk | `@dumking/lanchat-plugin-sdk` 类型、运行时客户端与契约 |
| `lanchat-official-plugins` | https://github.com/DumKing/lanchat-official-plugins | 官方游戏插件的独立构建与发布 |
| `lanchat-vision-plugin` | https://github.com/DumKing/lanchat-vision-plugin | 视觉 Web UI、签名 sidecar 与模型打包 |
| `lanchat-plugin-catalog` | https://github.com/DumKing/lanchat-plugin-catalog | 签名目录、可信公钥与撤销列表 |

SDK 在主仓库中的 workspace 副本只用于宿主契约落地阶段，独立仓库发布首个可用版本后，主程序改为依赖发布包并删除副本。官方插件和视觉插件不进入核心仓库，也不随核心安装包编译。

目标目录：

```text
src/
├─ app/
│  ├─ AppShell.vue
│  ├─ navigation/AppNavigationRail.vue
│  └─ layout/AppWorkspace.vue
├─ pages/
│  ├─ ChatPage.vue
│  ├─ DevicesPage.vue
│  ├─ GamesPage.vue
│  ├─ AlertsPage.vue
│  ├─ SettingsPage.vue
│  └─ PluginCenterPage.vue
├─ features/
│  ├─ chat/
│  ├─ devices/
│  ├─ games/
│  ├─ pet/
│  └─ updater/
├─ plugin-host/
│  ├─ contracts/
│  ├─ manifest/
│  ├─ runtime/
│  ├─ registry/
│  └─ stores/

packages/
└─ plugin-sdk/
   ├─ package.json
   ├─ tsconfig.json
   └─ src/
      ├─ index.ts
      └─ types.ts

plugins/
└─ com.lanchat.gomoku/
   ├─ plugin.json
   ├─ src/
   ├─ tests/
   └─ vite.config.ts
```

独立插件仓库可使用自己的 npm workspaces 管理多个插件。插件只能依赖公开的 `@dumking/lanchat-plugin-sdk`，不得使用指向宿主 `src/` 的相对路径。

### 7.1 宿主业务服务边界

在桥接 `rooms` 和 `leaderboard` 前，先把现有游戏网络与持久化逻辑从 `App.vue` 抽为宿主服务：

- `roomHostService`：房间成员、准备状态、进入/离开和快照所有权。
- `gameTransport`：局域网帧发送、接收、幂等键、版本协商和重连。
- `leaderboardService`：结算校验、多人计榜规则、持久化与同步。

内置游戏与插件迁移期间都调用这些服务，但插件只能通过标准桥间接访问。迁移某个游戏时删除它在 `App.vue` 中的专用状态机和帧分支，不复制到新的宿主服务。

拆分原则：

- `App.vue` 最终只挂载 Provider、ErrorBoundary 与 AppShell，目标少于 300 行。
- 第一阶段先抽出导航、插件中心、插件宿主和页面容器；每迁移一个业务模块，就删除 `App.vue` 中对应模板、状态和样式。
- 跨页面共享状态进入职责单一的 store 或 service；只在页面内部使用的状态留在页面 composable。
- 不做一次性全量重写。尚未迁移的游戏仍由现有游戏页承载；某个游戏迁移完成后，registry 只保留插件贡献，并在同一提交中删除该游戏的内置入口和实现。

## 8. 五子棋参考插件

五子棋是首个端到端参考实现，必须验证：

- 插件安装、启用、导航贡献和生命周期回调。
- 创建/加入房间、准备、落子、认输、再来一局、断线快照恢复。
- 当前主题 token 实时同步。
- 两人完成的对局提交排行榜；无效或重复结算由宿主拒绝。
- 插件私有设置保存。
- 禁用或卸载插件后入口、事件监听和 WebView 全部移除。

五子棋插件达到规则、房间、排行榜和 UI 验收后，在同一批次删除内置入口与实现，并通过一次数据库迁移保留历史排行。发布版本只显示插件入口，不提供内置回退。

## 9. 构建与分发

流水线增加三个层次：

1. `core-check`：主程序类型检查、Rust 测试、插件契约测试，不打包任何官方插件。
2. `plugin-build`：每个官方插件独立构建、生成完整性清单、签名并产出 `.lcp`。
3. `bundle-release`：核心安装包与官方插件作为独立附件发布；可按发行策略选择预装插件，但核心产物本身不含插件业务源码。

视觉识别迁移时，`core-check` 和核心安装包不编译视觉模块、OpenVINO/ORT、模型打包脚本、视觉页面或模型资源；视觉插件任务负责建立对应 Cargo/Vite feature 和 sidecar 构建矩阵。

## 10. 失败处理

- 安装先解压到暂存目录，校验路径、大小、哈希、签名、清单和兼容性后原子切换。
- 启动失败保留上个可运行版本，并在插件中心提供回滚。
- 插件桥请求超时后返回 `HOST_TIMEOUT`；宿主不因插件异常关闭聊天主进程。
- 插件连续崩溃达到阈值后自动暂停，并保留诊断日志。
- 房间中的插件版本或协议版本不兼容时禁止加入，明确提示需要升级的插件版本。

## 11. 验收标准

- 一个独立 `.lcp` 可以完成安装、启用、进入、退出、禁用、回滚和卸载。
- 插件只能调用清单声明且用户授权的 API；伪造 token、越权方法和目录穿越测试均被拒绝。
- `window.lanchat` v1 由独立仓库包 `@dumking/lanchat-plugin-sdk` 发布，包含 TypeScript 类型、运行时客户端、示例和契约测试。
- 五子棋插件完成完整双人对局，并能通过宿主房间和排行榜 API 工作。
- 主题切换后插件无需刷新即可更新颜色。
- App 导航和插件中心不再写在 `App.vue` 内；后续页面可按同一骨架继续迁移。
- 已迁移能力只存在插件入口和标准 API 实现；未迁移能力保持可用，排行数据和房间协议不中断。
- 核心包与插件产物可在流水线中独立构建；视觉能力最终迁移后，核心包不包含视觉代码、运行库和模型资源。
