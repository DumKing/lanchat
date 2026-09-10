# LanChat v0.8.0 插件平台实施计划

> 对应设计：`docs/superpowers/specs/2026-09-10-lanchat-plugin-platform-v080-design.md`

## 目标与迁移纪律

交付版本化插件契约、独立 SDK、插件仓库、隔离子 WebView、插件中心、应用外壳拆分和五子棋参考插件。每项迁移验收后，同一批次删除原入口和旧实现，不保留兼容页、双入口或内置回退；后续游戏与视觉能力全部通过标准 API 接入。

## 任务 1：测试基础、独立 SDK 与清单契约

**文件：**

- 在宿主开发阶段修改根 `package.json`，临时启用 `packages/*` workspace；SDK 发布后改为正式包依赖
- 新建 `vitest.config.ts`
- 新建 `packages/plugin-sdk/{package.json,tsconfig.json,src/index.ts,src/types.ts}`
- 新建 `src/plugin-host/contracts/{manifest,bridge}.ts`
- 新建 `src/plugin-host/manifest/validateManifest.ts`
- 新建 `tests/plugin/manifest.test.ts`

**步骤：**

1. 引入 Vitest；组件交互测试使用 Vue Test Utils 与 jsdom，安全与契约测试执行真实函数。
2. 先写失败测试，覆盖合法清单、非法 ID、绝对路径、目录穿越、未知 capability、不支持 API 主版本和重复贡献 ID。
3. 定义 manifest、capability、contribution、生命周期、桥信封和稳定错误码。
4. `@dumking/lanchat-plugin-sdk` 只导出公共类型和客户端，构建声明文件并测试 API 版本兼容。
5. 运行契约测试、SDK build 和根构建。

## 任务 2：从 App.vue 抽取通用游戏宿主服务

**文件：**

- 新建 `src/features/games/{roomHostService,gameTransport,leaderboardService}.ts`
- 新建 `tests/games/{room-host-service,game-transport,leaderboard-service}.test.ts`
- 修改 `src/App.vue`、`src/stores/lanchat.ts` 及相关游戏测试

**步骤：**

1. 用测试固定房间成员、准备、快照、帧幂等、版本协商、多人计榜和扫雷例外。
2. 把通用房间状态所有权、网络帧和排行榜持久化抽成无 UI 服务。
3. 尚未迁移的内置游戏改为调用服务，保持协议和数据格式。
4. 删除 `App.vue` 中已替代的通用分支，并运行全部房间与排行榜测试。

## 任务 3：安全插件仓库、权限与签名

**文件：**

- 新建 `src-tauri/src/plugin/{mod,manifest,package,repository,storage,permissions,signature}.rs`
- 新建 `src-tauri/resources/{plugin-trusted-keys,plugin-catalog}.json`
- 修改 `src-tauri/src/{lib,storage}.rs`、`src-tauri/tauri.conf.json`
- 新建 `src/services/plugin-api.ts`

**步骤：**

1. 先写 Rust 测试：ZIP 路径穿越、解压体积上限、哈希、canonical 签名输入、不可信/撤销 key、宿主版本、原子回滚和存储隔离。
2. 增加安装、版本、启用、授权、上一版本、错误状态与插件私有数据表。
3. 实现暂存、校验、原子切换、启停、回滚、卸载与保留/删除数据。
4. 增加权限 grant/revoke 和权限差异命令；撤权立即通知运行时销毁令牌。
5. 读取签名官方目录；下载经宿主受控请求，并二次校验哈希与签名。

## 任务 4：插件打包脚手架

**文件：**

- 新建 `scripts/{build-plugin,generate-plugin-integrity}.mjs`
- 新建 `scripts/package-plugin.ps1`
- 新建 `tests/plugin/package.test.ts`
- 修改根 `package.json`

**步骤：**

1. 先写测试，固定 canonical JSON、可重复哈希、完整性清单、缺签名失败和 `.lcp` 目录结构。
2. 增加 `plugin:check`、`plugin:build`、`plugin:package` 命令。
3. 开发包带开发标识；正式包从环境变量读取 `keyId` 和签名私钥。

## 任务 5：正式 Tauri 子 WebView 运行时与标准桥

**文件：**

- 新建 `src-tauri/src/plugin/{runtime,protocol,commands}.rs`
- 新建 `src/plugin-host/runtime/{PluginRuntime,BridgeDispatcher,capabilities,schemas}.ts`
- 新建 `src/components/plugins/PluginViewport.vue`
- 新建 `src/services/plugin-runtime-api.ts`
- 新建 `packages/plugin-sdk/src/events.ts`
- 新建 `tests/plugin/{bridge,runtime}.test.ts`
- 修改 `src-tauri/capabilities/default.json`、`src-tauri/tauri.conf.json`

**步骤：**

1. 先写行为测试：请求配对、超时、取消订阅、伪 token、越权、schema、限流、销毁和撤权。
2. Rust runtime 通过 `lanchat-plugin://` 只读协议加载已校验资源，设置每插件 CSP，禁止外部导航和通用 Tauri API。
3. 实现子 WebView 创建、bounds/resize、显隐、主题变化、关闭、崩溃计数和禁用销毁。
4. 每实例生成能力令牌；专用 IPC 转发到 dispatcher，再绑定任务 2 的宿主服务。
5. SDK 注入 `window.lanchat`，实现 enter/out、主题、网络、房间和可见性事件；注册均返回取消函数。

## 任务 6：插件 registry、权限交互与插件中心

**文件：**

- 新建 `src/plugin-host/stores/pluginRegistry.ts`
- 新建 `src/plugin-host/registry/contributions.ts`
- 新建 `src/pages/PluginCenterPage.vue`
- 新建 `src/components/plugins/{PluginCard,PluginPermissionList,PluginPermissionDialog}.vue`
- 新建 `tests/plugin/plugin-center.test.ts`
- 修改 `src/App.vue`

**步骤：**

1. registry 合并核心页面与插件贡献，稳定排序并拒绝 ID 冲突。
2. 插件中心实现已安装、签名官方目录、搜索、下载、安装、启停、更新、回滚和卸载。
3. 首次启用及更新新增权限时确认差异；撤权后立即停止插件。
4. UI 测试覆盖状态、空态、错误、权限确认和操作禁用。

## 任务 7：拆分 AppShell 与统一导航

**文件：**

- 新建 `src/app/AppShell.vue`
- 新建 `src/app/navigation/{AppNavigationRail.vue,types.ts}`
- 新建 `src/app/layout/AppWorkspace.vue`
- 新建 `src/pages/{ChatPage,DevicesPage,GamesPage,AlertsPage,SettingsPage}.vue`
- 修改 `src/App.vue` 与现有 UI 测试

**步骤：**

1. 固定聊天、设备、游戏、狼来了、视觉、设置和插件导航行为。
2. 迁出标题栏、导航、收起逻辑、内容容器和插件视口。
3. 先迁出不依赖游戏状态的页面，再按 feature/service 继续拆状态与事件。
4. 本任务验收时 `App.vue` 不含插件业务实现且少于 9,500 行；全部能力迁移完成后少于 300 行。

## 任务 8：五子棋插件直接替换

**文件：**

- 在 `lanchat-official-plugins` 新建 `plugins/com.lanchat.gomoku/{package.json,plugin.json,vite.config.ts,src/*,tests/*}`
- 修改 `src/App.vue`、`src/games/{registry,rules,gameLeaderboard}.ts`
- 修改/删除内置五子棋组件、状态、动作、模板和样式
- 修改 `scripts/test-gomoku-rules.mjs`、`scripts/test-game-catalog-view.mjs` 及房间测试

**步骤：**

1. 把现有规则测试迁入插件，先得到失败测试。
2. 插件只依赖 SDK，实现创建/加入、准备、落子、认输、再来一局、断线快照、主题和排行榜。
3. 打出签名 `.lcp`，经插件仓库安装并由子 WebView 运行。
4. 功能一致后，同批删除内置五子棋入口、规则、状态、帧分支、模板、样式和源码。
5. 旧测试改为插件测试，并加入“宿主无内置 gomoku 分支、只存在插件贡献”的负断言。

## 任务 9：GitHub 流水线与独立发布

**文件：**

- 修改 `.github/workflows/release.yml`
- 修改发布文档

**步骤：**

1. `lanchat` 增加 `core-check`；`lanchat-plugin-sdk` 增加 SDK build；`lanchat-official-plugins` 增加插件矩阵构建和 `.lcp` 签名任务。
2. 私钥只由 GitHub secret 注入；上传 `.lcp`、SHA-256、签名和 keyId。
3. 核心安装包不读取或内嵌 `plugins/*` 业务源码；官方插件作为独立 Release 附件。
4. 审计核心和插件产物清单，验证独立安装和更新。

## 后续里程碑

1. 按象棋、扫雷、大富翁顺序直接迁移；每项验收后同批删除内置实现。
2. 视觉识别迁移为 Web UI + 签名 sidecar，同时建立 Cargo/Vite feature 和核心构建裁剪。
3. 核心包不得包含视觉页面、识别代码、ORT/OpenVINO、模型打包脚本和模型资源。

## 全量验收

- Vitest 插件契约、仓库、签名、桥、runtime、权限和 UI 测试
- 现有全部游戏、聊天、设备、桌宠与升级测试
- `npm run build` 与 `lanchat-plugin-sdk` 仓库的 `npm run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `.lcp` 安装/启用/进入/退出/撤权/禁用/回滚/卸载冒烟
- 两客户端完成五子棋房间、重连与排行榜测试
- 核心与插件 Release 附件内容审计
