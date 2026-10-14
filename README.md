# LanChat

LanChat 是基于 **Tauri 2、Vue 3、TypeScript 和 Rust** 的局域网桌面聊天应用。设备在局域网内自动发现并直接通信，无需部署聊天服务器，同时提供文件传输、音视频通话、多人游戏、桌宠和本地视觉识别告警。

当前仓库版本：**0.7.2**。现有发布流水线提供 Windows 和 macOS 构建。

## 目录

- [功能概览](#功能概览)
- [下载安装与开始使用](#下载安装与开始使用)
- [网络与端口](#网络与端口)
- [本地开发](#本地开发)
- [构建与发布](#构建与发布)
- [验证与测试](#验证与测试)
- [项目结构](#项目结构)
- [桌宠资源](#桌宠资源)
- [本地视觉识别](#本地视觉识别)
- [数据与隐私](#数据与隐私)
- [常见问题](#常见问题)
- [相关文档](#相关文档)

## 功能概览

| 模块 | 能力 |
| --- | --- |
| 局域网通信 | mDNS 服务发现、UDP 在线广播、TCP 消息通信、联系人在线状态 |
| 聊天与频道 | 单聊、公共频道、私有频道、成员管理、公告、消息撤回、@ 提及、本机单聊记录删除 |
| 文件与媒体 | 文件传输、图片预览、音视频通话、摄像头与麦克风管理 |
| 桌面体验 | 系统托盘、未读提醒、主题切换、多语言、开机自启动设置 |
| 告警 | 快捷告警、反馈与可信度、本地摄像头识别告警、桌宠联动 |
| 桌宠 | 内置角色、自定义资源导入、状态动画、拖动与缩放、动作节奏设置 |
| 游戏 | 斗地主、五子棋、中国象棋、扫雷竞速、大富翁、游戏排行榜 |
| 视觉模型 | 模型中心、签名目录下载、资源完整性校验、模型切换、ONNX Runtime 与可选 OpenVINO 后端 |
| 管理与更新 | 管理员通知、远程更新分发、版本检测、更新进度与强制更新策略 |

### 局域网游戏

| 游戏 | 房间人数 | 说明 |
| --- | --- | --- |
| 斗地主 | 3 人 | 局域网房间与实时同步 |
| 五子棋 | 2 人 | 双人棋盘对战 |
| 中国象棋 | 2 人 | 双人象棋对局 |
| 扫雷竞速 | 1～6 人 | 单人挑战或多人同图竞速 |
| 大富翁 | 2～4 人 | 地产经营、道具、神明、随机事件、机器人补位与观战 |

大富翁默认使用伪 3D 棋盘，可在房间中切换平面棋盘。排行榜保存在 SQLite 中；普通多人游戏仅记录至少三名真人参与的有效对局，扫雷按单人成绩统计。

## 下载安装与开始使用

从 [GitHub Releases](https://github.com/DumKing/lanchat/releases) 选择所需版本及平台附件：

- **Windows 安装版**：下载安装程序并运行。
- **Windows 绿色版**：解压 `lanchat-windows-portable.zip`，保留资源目录结构，运行 `lanchat.exe`。系统需要 WebView2 Runtime。
- **macOS**：下载 DMG 并安装应用。

首次使用建议：

1. 将需要通信的设备连接到可互通的局域网，启动 LanChat。
2. 在个人设置中配置昵称和头像，允许应用通过系统防火墙。
3. 在联系人或公共频道中确认其他设备已上线，开始聊天或传输文件。
4. 按需开启麦克风、摄像头权限，使用通话或视觉识别。
5. 在设置中选择桌宠、配置告警与模型；在游戏入口创建或加入房间。

聊天通信无需中心服务器。版本检查、官方下载模型和资源包等功能需要访问对应下载地址。

## 网络与端口

| 用途 | 协议与端口 | 说明 |
| --- | --- | --- |
| 聊天与业务消息 | TCP，默认 `18145` | 监听端口保存在个人配置中 |
| 在线状态广播 | UDP `18146` | 用于局域网设备在线发现 |
| mDNS | UDP `5353` | 服务类型为 `_lanchat._tcp.local.` |
| 文件服务 | TCP，动态端口 | 文件服务启动时由系统分配 |
| 前端开发服务 | TCP `1420` | Vite 固定端口，仅用于开发 |
| 远程开发 HMR | TCP `1421` | 配置 `TAURI_DEV_HOST` 时使用 |

客户端需要能互相访问。访客 Wi-Fi、AP 隔离、跨网段限制或 VPN 路由可能影响发现与连接；只放行默认聊天端口也可能不足以支持文件传输和音视频。

Windows 可在应用设置中使用防火墙修复入口。仓库也提供 `scripts/Allow-LanChat-Firewall.ps1` 和 `.bat`：使用前将脚本放到实际 `LanChat.exe` 所在目录并以管理员身份运行，因为脚本按自身目录定位程序。脚本会为 Domain、Private、Public 网络配置入站规则。

## 本地开发

### 环境准备

- **Node.js 20** 与 npm：与现有 CI 使用的版本保持一致。
- **Rust stable** 与 Cargo。
- 当前操作系统所需的 Tauri 2 原生构建依赖：Windows 需要 MSVC C++ 构建工具与 WebView2；macOS 需要 Xcode Command Line Tools。
- Linux 开发需要 WebKitGTK、GTK、系统托盘等原生依赖；当前发布矩阵没有提供 Linux 安装包。

首次安装 Rust 依赖和 ONNX Runtime 相关构建资源时可能需要网络。

### 获取代码与启动

```bash
git clone https://github.com/DumKing/lanchat.git
cd lanchat
npm ci
npm run tauri -- dev
```

Tauri 会自动启动 Vite，然后编译并打开桌面应用。

仅调试前端布局时可以运行：

```bash
npm run dev
```

访问 `http://localhost:1420`。完整聊天、存储、文件服务和桌宠等能力依赖 Tauri 原生后端，普通浏览器不能替代桌面运行环境。

### 常用命令

| 命令 | 用途 |
| --- | --- |
| `npm ci` | 按锁文件安装前端依赖 |
| `npm run dev` | 启动 Vite |
| `npm run tauri -- dev` | 启动桌面开发环境 |
| `npm run build` | 写入构建时间、执行 TypeScript 检查并构建前端 |
| `npm run preview` | 预览前端构建产物 |
| `npm run tauri -- build` | 编译并打包桌面应用 |
| `npm run vision:package -- -Tag vX.Y.Z` | 使用 PowerShell 打包视觉模型，需先准备资源和签名环境 |

## 构建与发布

### 本地打包

```bash
npm run tauri -- build
```

前端产物位于 `dist/`，桌面打包产物通常位于 `src-tauri/target/release/bundle/`。

默认 Cargo features 不启用 OpenVINO。准备好对应原生运行时资源后，可显式启用：

```bash
npm run tauri -- build --features openvino-runtime
```

启用 feature 本身不会自动准备完整 OpenVINO 资源。Windows 正式发布流水线会准备运行时、校验 DLL 并测试模型加载。

### GitHub Actions

- `build-windows.yml`、`build-macos.yml`：平台构建。
- `release.yml`：推送 `v*` 标签或手动触发后，构建安装包、Windows 绿色包、外置桌宠与视觉模型资源，再发布 Release。
- 发布前同步 `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 中的项目版本，并准备 `docs/releases/vX.Y.Z.md`。
- 可通过同版本 JSON 配置或手动触发参数指定强制更新、最低支持版本等发布策略。

正式发布使用以下 Secrets：

| Secret | 用途 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Windows 更新包签名 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 更新签名私钥密码 |
| `VISION_CATALOG_SIGNING_KEY` | Ed25519 视觉模型目录签名 |
| `WECHAT_RELEASE_BOT_WEBHOOK` | 可选企业微信发布通知，需同时开启对应仓库变量 |

当前流水线生成的 `latest.json` 自动更新平台条目为 `windows-x86_64`；macOS 提供下载包，不能据此认为已配置同等自动更新支持。应用更新地址与模型信任公钥绑定项目配置，维护 fork 时需要配套调整。

## 验证与测试

前端构建与 Rust 测试：

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

按改动范围选择已有回归脚本，例如：

```bash
npm run test:gomoku
npm run test:monopoly
npm run test:monopoly-room
npm run test:xiangqi
npm run test:minesweeper
npm run test:desktop-pet-runtime
npm run test:face-recognition-alert
npm run test:vision-model-ui
npm run test:i18n
```

全部 npm 命令见 `package.json`；`scripts/` 中还包含可直接用 Node.js 运行的视觉兼容性、模型目录、资源包区分和其他回归脚本。部分脚本检查源码约束，不能替代真实设备联调。

涉及网络、音视频、桌宠原生窗口或模型后端的改动，还应在对应平台进行实际验证，例如双设备通信、文件接收、权限授予、模型加载与更新包安装。

## 项目结构

```text
lanchat/
├── src/
│   ├── App.vue                 # 主界面与业务交互
│   ├── components/             # 聊天输入、模型中心、人员面板、游戏棋盘等
│   ├── games/                  # 游戏目录、规则、房间和排行榜
│   ├── services/               # Tauri 调用、事件、摄像头与帧传输
│   ├── stores/                 # 聊天与桌宠状态
│   ├── types/                  # TypeScript 类型
│   ├── utils/                  # 通用工具
│   ├── styles/                 # 样式
│   └── i18n.ts                 # 多语言资源
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # 原生初始化与命令接口
│   │   ├── network.rs          # 发现、连接与局域网业务通信
│   │   ├── protocol.rs         # 消息协议
│   │   ├── storage.rs          # SQLite 存储
│   │   ├── file_server.rs      # 文件服务
│   │   ├── desktop_pet*.rs     # 原生桌宠与动画运行时
│   │   ├── face_monitor.rs     # 摄像头识别告警
│   │   ├── vision/             # 模型、推理、特征与运行时管理
│   │   └── bin/                # 模型目录签名和验签工具
│   ├── resources/              # 内置桌宠、模型与运行时资源
│   └── tauri.conf.json         # 桌面窗口、打包与更新配置
├── desktop-pet-packages/       # 外置桌宠资源
├── scripts/                    # 回归校验、资源打包与发布辅助脚本
├── docs/                       # 规范、设计、发布说明
└── .github/workflows/          # 构建与发布流水线
```

## 桌宠资源

桌宠通过资源目录和 `manifest.json` 定义，使用透明 PNG 序列帧。运行时识别 `Idle`、`Alert`、`Move`、`Interact`、`Life` 五种状态，可针对动作配置帧率、循环方式与移动方向。

自定义资源的基本要求：

- 目录名与 manifest 的 `id` 一致。
- `Idle` 至少有一张可解码的透明 PNG；缺失的其他状态会回退到待机。
- 同一动作内的帧按文件名自然排序，建议使用连续编号。
- 资源包用于提供清单和图片，不用于执行脚本或原生插件。

完整字段、目录结构、默认节奏和示例见 [桌宠资源规范](docs/desktop-pet-manifest.md)。

## 本地视觉识别

视觉识别用于在本机摄像头画面中识别已录入人员，并按策略触发告警。人脸用于确认身份，人体外观重识别提供远距离、侧身或背身场景的疑似身份线索。

基本流程：在设置中检查摄像头权限与运行状态，录入人员参考资料，选择模型组合，再开启识别策略并配置采样、阈值与连续命中条件。

| 模型组合 | 识别与重识别 | 后端 |
| --- | --- | --- |
| `baseline` | SFace + YoutuReID | ONNX Runtime |
| `office-arcface-buffalo-sc` | ArcFace Buffalo-SC + YoutuReID | ONNX Runtime |
| `office-omz-retail-0288` | OMZ retail-0095 + retail-0288 | OpenVINO Runtime |
| `office-omz-retail-0286` | OMZ retail-0095 + retail-0286 | OpenVINO Runtime |

官方模型目录使用 Ed25519 签名；安装时校验资源包与文件摘要、Manifest V4、适配器和后端。不同组合的特征向量按独立空间保存。OpenVINO 模型需要匹配的运行时和成对 XML/BIN 资源，后端不可用时会拒绝激活。

ArcFace Buffalo-SC 预训练权重仅作为非商用学习/研究可选资源；商业使用或二次分发前需取得对应授权。具体资源来源、许可证和打包步骤见 [视觉模型发布规范](docs/vision-model-publishing.md)。

## 数据与隐私

- SQLite 数据库 `lanchat.sqlite3` 位于 Tauri 的应用数据目录，用于保存本机配置、聊天记录、排行榜及识别相关记录。具体路径由平台和应用标识 `com.lanchat.desktop` 决定。
- 摄像头采样帧进入本机内存推理流程，推理结束后释放；人员特征向量在本机存储与比对。
- 视觉模型不通过聊天消息或局域网文件服务自动传播；识别触发后的告警属于局域网业务通信。
- 删除本机单聊记录不等于删除对方设备上的记录。
- 备份数据库前应退出应用；需迁移完整配置或模型时，应一并保留应用数据目录中的配套资源。

## 常见问题

**其他设备没有出现，或消息无法送达？**

确认双方应用都在运行、网络可以互通、没有 AP 隔离或 VPN 干扰，再检查防火墙是否允许程序以及 TCP `18145`、UDP `18146` 和 UDP `5353`。修改监听端口后，防火墙规则也需要对应调整。

**文字可发，但文件或通话不可用？**

文件服务使用动态 TCP 端口，应检查应用级防火墙规则；音视频还需要检查系统摄像头、麦克风权限及媒体设备状态。

**网页开发预览无法调用原生功能？**

请使用 `npm run tauri -- dev` 启动桌面应用。`npm run dev` 只提供前端开发服务。

**启动开发环境时报端口占用？**

Vite 使用固定端口 `1420` 且不自动切换。停止占用该端口的进程后重试。

**摄像头识别不可用，或模型无法激活？**

检查模型中心和运行状态中的具体错误，包括权限、资源缺失、摘要校验失败、模型后端缺失或 OpenVINO XML/BIN 不完整。切换设置不能替代缺失的模型与运行时文件。

**桌宠导入失败或动画异常？**

按资源规范检查目录名、manifest、透明 PNG 和待机帧，并确认编号及帧率设置。可先切换内置桌宠确认运行时是否正常。

## 相关文档

- [版本更新记录](docs/releases/v0.7.2.md)
- [全部发布说明](docs/releases/)
- [桌宠资源与 Manifest 规范](docs/desktop-pet-manifest.md)
- [视觉模型发布规范](docs/vision-model-publishing.md)
- [大富翁玩法设计](docs/monopoly-game-design.md)
- [大富翁 3D 集成说明](docs/monopoly-3d-integration.md)
- [企业微信发布通知配置](docs/release-wecom-notification.md)
- [插件平台路线图](docs/plugin-platform-roadmap.md)

`docs/superpowers/` 和 `plan.md` 保留阶段性设计与实施记录；其中的规划不代表当前版本已交付的功能。项目当前未包含根目录 LICENSE 文件，使用或分发时还需分别核对代码与第三方资源的授权。
