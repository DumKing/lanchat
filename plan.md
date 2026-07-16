# LanChat 通用桌宠 Runtime 与资源包系统设计方案

> 文档状态：设计稿 V1  
> 适用项目：LanChat（Tauri 2 + Vue 3 + TypeScript + Rust）  
> 对接规范：`DesktopPet_Runtime_与_Skill_对接规范_V2.md`、`windows-desktop-pet-generator` Skill

## 1. 目标与边界

### 1.1 建设目标

将当前只服务于青蛙的原生桌宠实现改造成一个与角色无关的通用 DesktopPet Runtime：

1. 用户可通过 `windows-desktop-pet-generator` Skill 或其他方式自行生成桌宠图片资源。
2. 将完整角色资源包放入约定目录后，LanChat 自动发现、校验并注册该桌宠。
3. 设置界面展示所有可用桌宠，支持预览、选择、刷新、导入和删除自定义桌宠。
4. Runtime 只识别 `Idle / Alert / Move / Interact / Life` 五类状态，不识别青蛙、熊猫、猫咪等具体角色。
5. 每个状态、每个动作片段的帧数都以文件夹内实际可用图片数量为准，不使用 Skill 示例中的固定数量。
6. 保留现有告警、温度、可信度、闪烁、蹦迪、拖动、缩放、详情窗口和反馈等能力。
7. 单个资源包损坏、图片解码失败或热更新失败时，不得导致主界面白屏或整个应用退出。

### 1.2 非目标

第一阶段不包含以下能力：

- 桌宠资源包执行 JavaScript、Lua、Wasm 或原生插件。
- 用户资源包自定义网络协议、告警业务或超级管理员权限。
- 3D 模型实时渲染、骨骼动画和物理引擎。
- 在线桌宠商店、付费下载、账号云同步。
- Runtime 根据角色名称编写特殊分支。

资源包是纯数据包，只能包含清单、图片和可选配置，不能包含可执行代码。

## 2. 当前实现评估

LanChat 当前桌宠链路已经具备原生透明窗口和业务告警能力，但资源层与青蛙强绑定：

- `src-tauri/src/native_frog_pet.rs` 使用 `include_bytes!` 编译期嵌入固定青蛙图集。
- 图集采用固定行列和固定姿态索引，图片数量、动作顺序与源码耦合。
- `NativeFrogPetState`、`set_frog_pet_enabled`、`update_native_frog_pet` 等类型和命令均以 frog 命名。
- Vue 负责汇总告警、温度、反馈、蹦迪和主题状态，再同步给 Rust 原生桌宠窗口。
- 设置界面已有青蛙告警器、告警模式和快捷键等配置，可以迁移为通用桌宠设置。

改造策略不是重写告警系统，而是保留现有业务链路，只替换“角色资源发现、动作选择、帧播放和桌宠选择”这一层。

## 3. 总体架构

```text
Windows Desktop Pet Generator Skill / 用户自制资源
                         │
                         ▼
              Desktop Pet Package
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│ DesktopPet Runtime（Rust）                           │
│                                                     │
│  PackageRegistry ── PackageLoader ── ResourceWatcher│
│         │                 │                         │
│         ▼                 ▼                         │
│  StateMachine ─── AnimationPlayer ─── TextureCache  │
│         │                 │                         │
│         └──────── WindowController ────────────────┐│
└────────────────────────────────────────────────────┼┘
                                                     │
              Tauri Commands / Events                │
                                                     ▼
┌─────────────────────────────────────────────────────┐
│ Vue / Pinia                                         │
│ 设置、桌宠选择、告警业务、可信度、反馈、主题与日志  │
└─────────────────────────────────────────────────────┘
```

### 3.1 核心模块

| 模块 | 职责 |
| --- | --- |
| `PackageRegistry` | 扫描三类资源根目录，维护可用桌宠注册表，处理重复 ID 和版本 |
| `PackageLoader` | 解析 `manifest.json`，发现状态目录、动作片段和实际帧文件，执行校验 |
| `ResourceWatcher` | 监听用户目录与绿色版目录变化，防抖后触发增量重扫 |
| `StateMachine` | 接收运行时事件，按优先级切换五类状态，处理抢占、恢复和降级 |
| `AnimationPlayer` | 按动作片段配置播放真实帧序列，控制 FPS、循环、乒乓和随机策略 |
| `TextureCache` | 图片按需解码和 LRU 缓存，避免一次加载数百张 512×512 图片 |
| `WindowController` | 管理透明置顶窗口、位置、缩放、拖动、桌面移动和详情窗口 |
| `DesktopPetBridge` | 将 LanChat 告警业务状态转换为通用 Runtime 事件，并向 Vue 回传交互事件 |

## 4. 资源目录与发现规则

### 4.1 三类资源根目录

Runtime 同时扫描以下目录：

1. **内置资源目录**：`resource_dir()/desktop-pets/`
   - 随应用打包，只读。
   - 至少包含迁移后的内置青蛙。
   - 用于保证任何情况下都有可回退桌宠。

2. **用户资源目录**：`app_data_dir()/desktop-pets/`
   - Windows 通常对应 `%APPDATA%/com.lanchat.app/desktop-pets/`。
   - 设置界面的“导入桌宠”默认复制到这里。
   - 普通安装版的主要可写目录。

3. **绿色版资源目录**：`executable_dir()/desktop-pets/`
   - 适用于绿色包和需要随程序迁移的资源。
   - 目录不存在时自动忽略；可写时允许用户手工放入资源。

每个根目录下，一个一级子目录代表一个桌宠包：

```text
desktop-pets/
├── frog/
├── panda/
└── custom-cat/
```

### 4.2 同 ID 冲突规则

同一个 `id` 只能注册一个有效包，来源优先级为：

```text
用户资源目录 > 绿色版资源目录 > 内置资源目录
```

- 高优先级包覆盖低优先级包时，在设置界面显示来源和覆盖提示。
- 设置界面导入相同 ID 时必须明确提示“替换现有自定义桌宠”，不能静默覆盖。
- 自定义包损坏时不覆盖可用的内置同 ID 包，Runtime 自动回退并记录错误。
- 运行中的资源包使用不可变快照；重扫成功后只在下一个安全切换点替换，避免播放过程中帧文件消失。

### 4.3 自动发现时机

- 应用启动时全量扫描一次。
- 打开桌宠设置页时执行轻量校验和增量扫描。
- 点击“刷新桌宠”时立即扫描。
- 设置界面导入或删除完成后立即扫描。
- `ResourceWatcher` 监听用户目录和绿色版目录，文件变化稳定后自动扫描。

监听事件采用 800ms 防抖。同一资源包仍在复制时，不立即注册；连续两次检查文件数量和修改时间稳定后再校验，防止读取半成品。

## 5. 桌宠资源包协议

### 5.1 标准目录

```text
frog/
├── manifest.json
├── preview.png
├── icon.png
├── Idle/
│   ├── idle_001.png
│   ├── idle_002.png
│   └── ...
├── Alert/
├── Move/
├── Interact/
└── Life/
```

### 5.2 动作片段的两种组织方式

#### 兼容模式：状态目录平铺

```text
Idle/
├── idle_001.png
├── idle_002.png
└── idle_003.png
```

Runtime 将该目录中的所有有效 PNG 按自然数字顺序组成一个名为 `default` 的动作片段。

#### 推荐模式：一个子目录一个动作片段

```text
Idle/
├── breathe/
│   ├── idle_breathe_001.png
│   └── idle_breathe_002.png
├── blink/
│   ├── idle_blink_001.png
│   └── idle_blink_002.png
└── look_left/
    ├── idle_look_left_001.png
    └── idle_look_left_002.png
```

这样 Runtime 可以随机选择一个完整且连贯的动作，而不是把不同动作错误串成一段动画。

规则如下：

- 每个子目录形成一个独立 clip。
- 状态目录下同时存在平铺图片和子目录时，平铺图片形成 `default` clip，子目录形成具名 clip。
- 帧数永远来自当前 clip 实际扫描到的有效文件数。
- 文件使用不区分大小写的自然数字排序，例如 `2.png` 排在 `10.png` 前。
- 忽略隐藏文件、临时文件、非 PNG 文件和无法解码的图片。
- 单个坏帧只使该帧失效；clip 至少保留一帧才可用。

### 5.3 `manifest.json` V2

兼容原 V2 草案的基础字段，并增加可选运行时配置：

```json
{
  "schemaVersion": 2,
  "id": "frog",
  "name": "青蛙",
  "version": "1.0.0",
  "author": "LanChat",
  "description": "LanChat 内置青蛙桌宠",
  "resolution": 512,
  "fps": 8,
  "transparent": true,
  "defaultState": "Idle",
  "states": {
    "Idle": {
      "loop": "repeat",
      "selection": "weighted-random",
      "minDurationMs": 3000,
      "maxDurationMs": 9000
    },
    "Alert": {
      "loop": "repeat",
      "minLoops": 1
    },
    "Move": {
      "loop": "repeat"
    },
    "Interact": {
      "loop": "once"
    },
    "Life": {
      "loop": "once"
    }
  },
  "clips": {
    "Move/jump_left": {
      "fps": 6,
      "loop": "repeat",
      "direction": "left",
      "weight": 1
    },
    "Move/jump_right": {
      "fps": 6,
      "loop": "repeat",
      "direction": "right",
      "weight": 1
    }
  }
}
```

说明：

- `schemaVersion` 用于协议演进。
- `resolution` 是资源设计尺寸，不限制窗口显示尺寸。
- `fps` 是全局默认值，状态和 clip 可覆盖。
- `states` 可兼容旧版字符串数组；旧包缺少扩展配置时使用 Runtime 默认值。
- `clips` 完全可选，键采用 `状态/子目录`。
- `weight` 控制随机选择权重。
- `direction` 供移动控制器选择方向匹配的动作。
- 清单不记录固定帧数，防止清单与实际文件不一致。

### 5.4 导入校验分级

#### 阻止导入的错误

- 缺少或无法解析 `manifest.json`。
- `id` 非法、为空或包含路径字符。
- `defaultState` 不存在或没有任何有效帧。
- `Idle` 没有任何有效 clip。
- 所有图片都无法解码。
- 图片路径逃逸出资源包目录。
- 清单或图片超过安全上限，可能造成内存或磁盘风险。

#### 允许导入但显示警告

- `Alert / Move / Interact / Life` 某个目录缺失或为空。
- 缺少 `preview.png` 或 `icon.png`。
- 图片分辨率与清单不一致。
- 图片没有 Alpha 通道或实际没有透明像素。
- 某些帧损坏、尺寸不一致或命名无法自然排序。
- manifest 中配置了不存在的 clip。

缺少非 Idle 状态时采用降级策略，而不是让桌宠完全不可用：

```text
目标状态无资源 → Interact（若可用）→ Idle
```

Alert 缺失时仍保留温度、角标、闪烁和详情窗口，仅使用 Idle 图片承载视觉效果。

## 6. 统一数据模型

Rust 侧建议采用以下核心模型：

```rust
enum PetStateKind {
    Idle,
    Alert,
    Move,
    Interact,
    Life,
}

struct PetPackage {
    manifest: PetManifest,
    source: PetPackageSource,
    root: PathBuf,
    states: HashMap<PetStateKind, Vec<PetClip>>,
    warnings: Vec<PetPackageWarning>,
}

struct PetClip {
    id: String,
    state: PetStateKind,
    frames: Vec<FrameAsset>,
    fps: f32,
    loop_mode: LoopMode,
    direction: Option<MoveDirection>,
    weight: u32,
}

struct FrameAsset {
    path: PathBuf,
    width: u32,
    height: u32,
}
```

运行时配置：

```rust
struct DesktopPetSettings {
    enabled: bool,
    selected_pet_id: String,
    scale: f32,
    position: Option<LogicalPosition>,
    alert_mode: AlertMode,
    stop_hotkey: Option<String>,
    random_move_enabled: bool,
    random_life_enabled: bool,
}
```

业务状态与资源状态分离：

```rust
struct DesktopPetBusinessState {
    pending_count: u32,
    temperature: Option<f32>,
    flashing: bool,
    disco: bool,
    alert_id: Option<String>,
    sender_name: Option<String>,
    sender_ip: Option<String>,
    alert_title: Option<String>,
    feedbackable: bool,
    is_self_alert: bool,
    theme: PetTheme,
}
```

资源包不能修改 `DesktopPetBusinessState`，只能为 Runtime 提供图片和播放参数。

## 7. 状态机完整设计

### 7.1 状态优先级

```text
Alert > Interact > Move > Life > Idle
```

优先级只控制动画和移动状态，不阻塞业务命令。例如 Alert 播放期间仍可点击真实/虚假反馈、拖动桌宠或打开详情。

### 7.2 通用状态生命周期

每个状态都遵循：

```text
触发事件
  → 校验目标状态是否有可用 clip
  → 判断是否可抢占当前状态
  → 保存可恢复状态
  → onExit(当前状态)
  → 选择 clip
  → onEnter(目标状态)
  → 播放
  → 完成或被打断
  → 恢复仍有效的上一状态，否则进入 Idle
```

状态栈最多保留一层可恢复状态，避免 Alert、Interact 连续触发后形成无限嵌套。

### 7.3 Idle：基础待机

进入条件：

- 桌宠窗口创建完成。
- 其他动作结束且没有更高优先级事件。
- 资源包切换完成。
- 告警停止并且没有待处理告警动画。

行为：

- 从 Idle 可用 clips 中按权重随机选择。
- clip 完成后停留短暂随机间隔，再选择下一个。
- 禁止连续多次选择同一个 clip，除非只有一个可用 clip。
- 平铺目录只有一个 `default` clip 时按序循环。
- 无输入时允许调度 Life 或 Move。

退出条件：Alert、鼠标交互、随机生活动作、随机移动或禁用桌宠。

### 7.4 Alert：告警状态

触发来源：

- 收到他人告警。
- 自己双击桌宠发起告警。
- Ctrl + 双击发起全员蹦迪告警。
- 超管向本机下发告警模式。
- LanChat 后续新增的高优先级桌面提醒。

行为：

- 立即停止 Life 和普通 Move，切换 Alert clip。
- 多条告警按业务层队列管理，Runtime 只接收当前展示项和未处理数量。
- 普通告警在多个 Alert clips 中轮换，帧数按实际目录读取。
- 温度只影响告警期间的色彩映射和闪烁频率；无有效告警时显示资源原始颜色。
- 温度越高越偏红，温度越低越偏橙；染色使用图片 Alpha 遮罩，不产生角色背后的色块或圆形背景。
- 自己发出的告警也闪烁，但不显示反馈按钮，只能由自己点击桌宠本体停止闪烁。
- 别人的告警完成真实/虚假反馈后更新队列；全部未处理项处理完毕后自动关闭详情窗口。

蹦迪不是新增第六状态，而是 Alert 的运动策略：

```text
Alert + movementProfile = disco
```

此时动画优先选择带方向的 Move clip 或 Alert 中声明的方向 clip，窗口按屏幕工作区移动。向左移动使用左向动作，向右移动使用右向动作；跳跃节拍和坐标更新同步，确保能看清“蹲下 → 跃起 → 落点”的过程。

结束条件：

- 用户点击桌宠本体停止闪烁或蹦迪。
- 全局停止快捷键触发。
- 告警业务状态明确结束。
- 超管下发模式到期。

结束后若鼠标仍悬停则进入 Interact，否则回到 Idle。

### 7.5 Interact：用户交互

事件映射：

| 用户事件 | Runtime 行为 | LanChat 业务行为 |
| --- | --- | --- |
| 单击本体 | 播放 click clip；Alert 时优先执行停止闪烁/蹦迪 | 保留当前告警处理规则 |
| 双击本体 | 播放 double-click clip | 发起普通告警 |
| Ctrl + 双击 | 播放 double-click clip | 发起全员蹦迪告警 |
| 拖拽 | 播放 drag 或方向 clip，暂停自动移动 | 更新并保存窗口位置 |
| 滚轮 | 不强制切换动画 | 缩放桌宠本体及窗口命中区域 |
| 鼠标悬停 | 可播放 hover clip | 显示角标提示；不自动展开详情 |
| 点击角标 | 保持桌宠位置不变 | 在桌宠左侧或右侧打开独立置顶详情窗 |

重要规则：

- Alert 优先级高于 Interact，但交互命令仍必须被处理。
- 拖动期间不允许自动移动；新 Alert 先更新业务状态，松开鼠标后立即显示 Alert 动画。
- 详情窗口位置不能挤动桌宠本体，窗口尺寸也不能改变桌宠缩放比例。
- 详情窗口使用固定字号和标准尺寸，根据屏幕空间选择桌宠左侧或右侧。

### 7.6 Move：桌面移动

触发来源：

- 空闲达到随机时间后巡逻。
- 用户开启自动巡逻。
- Alert 的 disco 运动策略请求方向移动。

行为：

- 在当前显示器工作区内计算安全路径。
- 不跨越任务栏和不可用工作区。
- 根据水平移动方向选择 `direction=left/right` 的 clip。
- 缺少方向资源时允许镜像同一动作；manifest 可用 `allowMirror: false` 禁止镜像带文字或不对称角色。
- 每个跳跃周期包含准备、移动、落地三个阶段，窗口坐标采用缓动插值而不是每帧瞬移。
- 普通巡逻到达目标后返回 Idle；蹦迪移动由 Alert 状态控制结束。

抢占规则：Alert 和 Interact 可立即抢占；Life 不可抢占 Move。

### 7.7 Life：随机生活动作

用于表现伸懒腰、打哈欠、整理衣服、喝水、思考等低频动作。

触发条件：

- 无告警。
- 最近没有鼠标交互。
- 当前处于 Idle。
- 距上一次 Life 已达到随机冷却时间。

默认调度为 30～120 秒随机一次，实际范围允许由 manifest 覆盖。

行为：随机选择一个 Life clip，完整播放一次后回到 Idle。Alert 和 Interact 可以抢占，Move 不在 Life 播放过程中主动触发。

### 7.8 状态流转总表

| 当前状态 | 事件 | 下一状态 | 说明 |
| --- | --- | --- | --- |
| 任意 | `Disable` | 停止 | 保存位置并隐藏窗口 |
| 启动/停止 | `Enable` | Idle | 恢复选择的桌宠和上次位置 |
| Idle | `LifeTimer` | Life | 满足空闲条件才执行 |
| Idle | `MoveTimer` | Move | 自动巡逻开启时执行 |
| Idle/Life/Move | `AlertRaised` | Alert | 立即抢占 |
| Idle/Life/Move | `PointerInteract` | Interact | Life/Move 可被打断 |
| Alert | `PointerInteract` | Alert | 处理交互，但保持告警语义 |
| Alert | `StopAlert` | Idle/Interact | 根据当前鼠标状态恢复 |
| Interact | `AlertRaised` | Alert | 告警抢占交互动画 |
| Interact | `AnimationFinished` | Idle | 不恢复已过期的随机动作 |
| Move | `AnimationFinished` | Idle | 巡逻到达目标 |
| Life | `AnimationFinished` | Idle | 生活动作结束 |
| 任意 | `PackageChanged` | Idle | 在安全点重建播放器 |

## 8. 动画播放与性能策略

### 8.1 实际帧发现

播放器不读取清单中的 `frameCount`，也不假设 `001～040` 连续存在：

1. 扫描 clip 目录下所有有效 PNG。
2. 自然排序。
3. 解码图片头并过滤坏帧。
4. 使用剩余实际文件组成 `frames`。
5. 播放边界始终使用 `frames.len()`。

因此删除、增加或重新生成某些图片后，不需要修改源码和固定帧数。

### 8.2 播放模式

支持以下模式：

- `once`：播放一次后触发状态完成。
- `repeat`：循环播放。
- `ping-pong`：正序再倒序播放，首尾不重复。
- `random-frame`：按保持时间随机切换单帧姿态。
- `shuffle-clips`：一个状态内随机选择 clip，避免连续重复。

旧版平铺资源默认 `repeat`，具名 Interact/Life clip 默认 `once`。

### 8.3 图片缩放与命中区域

- 使用 Alpha 包围盒计算角色本体区域，缩放以角色本体为基准，而不是以原始透明画布为基准。
- 窗口命中区域至少覆盖 Alpha 包围盒并保留角标空间。
- 缩放只改变桌宠窗口，不改变详情窗口字号和尺寸。
- 保持图片宽高比，禁止将圆形角色拉伸成椭圆。
- 最小尺寸下温度和未读角标使用独立 UI 层，保证仍然可见。

### 8.4 缓存

一张 512×512 RGBA 图片解码后约占 1MB。若一次性加载 200 张图片，单个角色可能占用约 200MB，因此采用：

- 启动时只预载选中桌宠的 Idle 首帧、Alert 首帧、icon 和 preview。
- 当前 clip 提前解码当前帧和后续 2～4 帧。
- 使用 LRU 纹理缓存，默认保留最近 16～32 帧。
- 切换桌宠后异步释放旧资源。
- 注册表扫描只读取图片元数据，不全量上传 GPU 纹理。

## 9. 设置界面设计

在现有设置页增加“桌宠”区域，替代只针对青蛙的资源选择逻辑，但保留“青蛙告警器”业务名称也可作为告警功能入口。

### 9.1 设置项

- 启用桌宠。
- 当前桌宠选择。
- 本机报警模式：普通 / 蹦迪。
- 自动巡逻开关。
- 随机生活动作开关。
- 停止告警/蹦迪快捷键。
- 恢复默认大小和位置。
- 打开桌宠资源目录。
- 导入桌宠。
- 刷新桌宠列表。

### 9.2 桌宠列表

列表项展示：

- `preview.png` 预览图。
- 名称、版本、作者。
- 来源：内置 / 用户目录 / 绿色版。
- 可用、警告或损坏状态。
- 选中标识。
- 自定义包删除按钮；内置包不可删除。

选择桌宠后立即预加载并切换。加载失败时保留当前桌宠并显示中文错误，不允许主界面白屏。

### 9.3 导入流程

```text
选择资源包目录
  → 复制到 desktop-pets/.staging/<uuid>
  → 完整校验
  → 处理同 ID 替换确认
  → 原子重命名到正式目录
  → 刷新注册表
  → 设置页显示新桌宠
```

用户也可以直接把资源包放入用户目录或绿色版目录，Watcher 会自动发现。

## 10. Tauri 接口设计

### 10.1 Commands

| 命令 | 说明 |
| --- | --- |
| `list_desktop_pets` | 返回全部有效包和无效包摘要 |
| `refresh_desktop_pets` | 主动重扫资源目录 |
| `import_desktop_pet` | 校验并原子导入资源包 |
| `remove_desktop_pet` | 删除用户包，不允许删除内置包 |
| `select_desktop_pet` | 切换并持久化当前桌宠 |
| `get_desktop_pet_settings` | 获取通用桌宠配置 |
| `update_desktop_pet_settings` | 更新开关、缩放、模式、快捷键等 |
| `set_desktop_pet_enabled` | 显示或隐藏原生桌宠窗口 |
| `update_desktop_pet_state` | Vue 向 Runtime 同步告警业务状态 |
| `open_desktop_pet_folder` | 打开用户资源目录 |

### 10.2 Events

| 事件 | 方向 | 说明 |
| --- | --- | --- |
| `desktop_pet_registry_changed` | Rust → Vue | 自动发现、删除或更新资源包 |
| `desktop_pet_selected` | Rust → Vue | 选中桌宠切换完成 |
| `desktop_pet_package_error` | Rust → Vue | 包损坏或热更新失败 |
| `desktop_pet_action` | Rust → Vue | 点击、双击、反馈、停止蹦迪等交互 |
| `desktop_pet_runtime_state` | Rust → Vue | 当前状态、clip 和诊断信息 |

现有 `set_frog_pet_enabled`、`update_native_frog_pet` 和 `native_frog_action` 保留一个版本作为兼容别名，内部转发到通用接口，前端迁移完成后再删除。

## 11. 持久化与启动顺序

桌宠选择和关键运行时配置应由 Rust 可直接读取，不能只保存在 Vue `localStorage`，否则主窗口尚未加载时无法可靠恢复桌宠。

建议持久化字段：

- `enabled`
- `selected_pet_id`
- `scale`
- `position_x / position_y`
- `monitor_id`
- `alert_mode`
- `stop_hotkey`
- `random_move_enabled`
- `random_life_enabled`

启动顺序：

```text
Tauri setup
  → 读取 DesktopPetSettings
  → 扫描三类资源目录
  → 解析选中桌宠
  → 失败则回退内置 frog
  → 创建透明置顶桌宠窗口
  → 恢复位置并校正到可见工作区
  → 启动资源监听
  → Vue 加载后同步业务告警状态
```

切换显示器、分辨率变化或旧坐标不可见时，将桌宠校正到当前主显示器右下角。

## 12. 错误隔离与白屏防护

- 资源扫描、图片解码和 Watcher 在线程中运行，错误转换为结构化结果，不允许 `panic!` 传播到主线程。
- Rust 桌宠线程崩溃时记录日志并尝试重建窗口，不影响 LanChat 主窗口。
- Vue 设置组件使用局部错误边界；桌宠列表接口失败只显示错误状态，不替换整个应用页面。
- 资源热更新失败继续使用上一份成功快照。
- 所有 Runtime 命令返回中文可展示错误，同时 Debug 日志保留包路径、状态、clip 和坏帧原因。
- 内置 frog 是最终兜底；若内置资源也失败，则关闭桌宠并保持主应用正常运行。

## 13. 安全策略

- 只读取 `manifest.json`、PNG、preview 和 icon，不加载脚本、动态库和可执行文件。
- 对 manifest 大小、路径长度、图片尺寸、单文件大小和资源包总大小设置安全上限。
- 导入复制时拒绝符号链接、绝对路径和 `..` 路径逃逸。
- 删除操作只能作用于用户资源根目录下经过规范化确认的包路径。
- 资源包 ID 仅允许 ASCII 小写字母、数字、短横线和下划线。
- Watcher 事件必须重新走完整校验，不能因为是本地变更就跳过安全检查。

## 14. 代码组织建议

Rust：

```text
src-tauri/src/desktop_pet/
├── mod.rs
├── types.rs
├── manifest.rs
├── package_loader.rs
├── registry.rs
├── watcher.rs
├── state_machine.rs
├── animation_player.rs
├── texture_cache.rs
├── window_controller.rs
└── bridge.rs
```

前端：

```text
src/
├── components/settings/
│   ├── DesktopPetSettings.vue
│   ├── DesktopPetPicker.vue
│   └── DesktopPetPackageStatus.vue
├── stores/
│   └── desktopPet.ts
└── types/
    └── desktop-pet.ts
```

内置资源：

```text
src-tauri/resources/desktop-pets/frog/
```

在 `tauri.conf.json` 的 bundle resources 中声明该目录，替代 Rust 源码中的固定 `include_bytes!` 图集。

## 15. 实施阶段

### 阶段一：资源协议与注册表

- 定义 manifest V2 数据模型和兼容解析。
- 实现三类目录扫描、自然排序和动态帧发现。
- 实现导入校验、错误分级、重复 ID 处理和内置回退。
- 将当前青蛙资源整理为首个标准资源包。

交付标准：不修改桌宠业务状态，也能从外部目录读取青蛙并显示首帧。

### 阶段二：通用播放器与状态机

- 抽离 `native_frog_pet.rs` 中窗口和渲染能力。
- 实现五状态、优先级、抢占、恢复和资源降级。
- 实现平铺目录与多 clip 两种播放方式。
- 加入按需解码、纹理缓存、方向动作和窗口移动同步。

交付标准：更换资源目录即可切换角色，无青蛙专用姿态索引。

### 阶段三：LanChat 告警能力迁移

- 将 `NativeFrogPetState` 改为通用 `DesktopPetBusinessState`。
- 保留温度、可信度、角标、闪烁、详情、反馈和蹦迪逻辑。
- 将蹦迪建模为 Alert 的 movement profile。
- 保留旧命令兼容转发。

交付标准：现有告警体验不回退，换任意有效角色包仍可报警和反馈。

### 阶段四：设置、导入和热发现

- 增加桌宠选择和包状态列表。
- 实现原子导入、删除、打开目录和手动刷新。
- 接入目录 Watcher 与防抖重扫。
- 将选择、大小、位置和快捷键迁移到 Rust 可读取配置。

交付标准：手工放入资源包或在设置中导入后，无需重启即可识别和选择。

### 阶段五：稳定性与兼容清理

- 完成异常隔离、资源更新快照和崩溃恢复。
- 删除前端和 Rust 中已无调用的 frog 专用接口。
- 补充 Debug 日志、资源诊断和设置页错误展示。
- 验证安装版、绿色版、双屏、多 DPI 和托盘状态。

## 16. 测试方案

### 16.1 Rust 单元测试

- manifest 旧格式和 V2 格式解析。
- 平铺目录识别为 `default` clip。
- 子目录识别为多个 clips。
- 实际图片数量为 1、2、17、空目录和编号不连续时的帧结果。
- 自然排序：`1.png`、`2.png`、`10.png`。
- 坏帧过滤、混合尺寸、无 Alpha 和缺少状态目录。
- 三目录同 ID 的优先级与损坏回退。
- 状态优先级和抢占恢复。
- Alert 期间点击、拖动、停止和新告警合并。
- Watcher 连续文件事件防抖和半成品包保护。
- LRU 缓存淘汰和切换角色后资源释放。

### 16.2 前端测试

- 桌宠列表加载、来源和警告展示。
- 选择、导入、替换和删除确认。
- 选中资源损坏后的回退提示。
- 设置持久化和 Rust 状态同步。
- 桌宠接口失败时设置页局部降级，不产生白屏。

### 16.3 集成测试

1. 安装版启动，加载内置 frog。
2. 将新角色放入用户目录，运行中自动出现。
3. 绿色版同级放入角色，自动发现并可选择。
4. 每个状态目录使用不同数量图片，确认按实际数量完整播放且不越界。
5. 播放时增加、删除或替换资源，确认安全热切换。
6. 收到普通告警，进入 Alert、显示温度和未处理角标。
7. 收到蹦迪告警，角色方向动作与全屏移动同步，点击本体和全局快捷键均可停止。
8. 拖动和滚轮缩放保持可用，详情窗口不改变桌宠位置和比例。
9. 主窗口最小化到托盘后，桌宠、告警和停止快捷键继续工作。
10. 双屏和 DPI 缩放环境下，桌宠始终位于可见工作区。
11. 故意放入坏 manifest、损坏 PNG 和超大图片，确认主应用不白屏、不崩溃。

## 17. 验收标准

- 新角色不需要修改 Rust 或 Vue 源码即可被发现和选择。
- 帧数完全由实际目录内容决定，不依赖 Skill 推荐数量和固定图集索引。
- 五类状态均有明确触发、抢占、结束和降级规则。
- Alert 始终优先，现有告警、反馈、温度和蹦迪能力完整保留。
- 资源缺失或损坏不会导致 LanChat 主界面白屏。
- 安装版、绿色版和用户目录三种资源来源都可正常工作。
- 设置中可以查看桌宠来源、版本、校验状态并完成选择、导入、刷新和删除。
- 资源目录变化后无需重启应用即可识别。
- 桌宠主窗口关闭到托盘后继续常驻，退出托盘才终止 Runtime。
- 自定义资源包始终是纯数据，不具备执行代码和越权访问能力。

## 18. 最终设计决策

1. 使用“五类通用状态 + 可选具名 clip”，不增加角色专用状态枚举。
2. 同时兼容平铺图片和按动作子目录组织，保证旧 Skill 产物可直接使用。
3. 不在 manifest 保存固定帧数，Runtime 每次以实际有效图片为准。
4. 告警、可信度和反馈属于 LanChat 业务层，角色资源只提供视觉素材。
5. 蹦迪属于 Alert 的移动策略，不额外制造与五类协议冲突的新状态。
6. 使用内置、用户数据和绿色版三个资源根目录，兼顾安装安全与便携使用。
7. 使用按需解码和 LRU 缓存，避免多桌宠、多帧资源造成高内存占用。
8. 采用原子导入、不可变运行快照和错误隔离，保证资源热更新不影响主应用稳定性。

