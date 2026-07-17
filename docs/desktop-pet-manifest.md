# LanChat 桌宠资源与 manifest.json 规范

## 目录结构

```text
frog-buddy/
├─ manifest.json
├─ icon.png
├─ preview.png                 # 可选
├─ Idle/
│  ├─ breathe/
│  │  ├─ idle_breathe_001.png
│  │  └─ idle_breathe_002.png
│  └─ blink/
├─ Alert/
├─ Move/
├─ Interact/
└─ Life/
```

- 资源包目录名必须与 `manifest.json` 的 `id` 完全一致。
- `icon.png` 是设置页桌宠图标；缺失时回退 `preview.png`，再回退 `Idle` 第一帧。
- `Idle` 至少包含一张可解码的透明 PNG，其他状态缺失时回退 `Idle`。
- 状态目录下每个子目录代表一个独立动作。直接放在状态目录中的 PNG 会组成名为 `default` 的动作。
- 每个动作内的 PNG 按文件名自然排序，建议使用三位连续编号。
- 图片宽高建议与 `resolution` 一致；不一致时会产生资源警告，但仍会按实际尺寸加载。

## 根字段

| 字段 | 类型 | 必填 | 可选值或限制 | 含义 |
|---|---|---:|---|---|
| `schemaVersion` | number | 否 | `1`、`2`，默认 `1` | manifest 结构版本；大于 `2` 会拒绝导入。 |
| `id` | string | 是 | 小写字母、数字、`-`、`_` | 桌宠唯一标识，必须与目录名一致。 |
| `name` | string | 是 | 非空 | 设置页展示名称。 |
| `version` | string | 是 | 建议语义化版本 | 资源包版本。 |
| `author` | string | 否 | 任意文本 | 作者。 |
| `description` | string | 否 | 任意文本 | 资源说明。 |
| `resolution` | number | 是 | 大于 `0` | 每帧 PNG 的标准宽高，例如 `512` 表示 `512x512`。 |
| `fps` | number | 是 | 大于 `0` | 未在动作中单独配置时使用的默认帧率。 |
| `transparent` | boolean | 是 | 必须为 `true` | 声明资源使用透明背景。 |
| `defaultState` | string | 是 | 必须为 `Idle` | 默认状态。 |
| `states` | object | 是 | 见下表 | 五种状态的播放规则。 |
| `clips` | object | 否 | 见动作字段 | 针对单个动作覆盖帧率、循环和方向。 |

## states 状态字段

状态键固定为 `Idle`、`Alert`、`Move`、`Interact`、`Life`。所有节奏字段都可在设置页右键桌宠图标编辑。

| 字段 | 类型 | 可选值或范围 | 含义 |
|---|---|---|---|
| `loop` | string | `repeat`、`once`、`ping-pong` | 该状态动作未单独声明循环方式时的默认值。 |
| `minDurationMs` | number | `0..300000` | 单个动作最短持续时间。`0` 表示至少完整播放一轮。 |
| `maxDurationMs` | number | `0..300000` | 单个动作最长持续时间；小于最短值时会自动交换。 |
| `minActionCount` | number | `1..20` | 每组最少随机动作数。 |
| `maxActionCount` | number | `1..20` | 每组最多随机动作数；小于最少值时会自动交换。 |
| `minIntervalMs` | number | `0..60000` | 组内两个动作之间的最短停顿。 |
| `maxIntervalMs` | number | `0..60000` | 组内两个动作之间的最长停顿；小于最短值时会自动交换。 |

未配置时使用以下默认值：

| 状态 | 持续时间 | 动作数 | 动作间停顿 | 运行语义 |
|---|---:|---:|---:|---|
| `Idle` | 3000～7000 ms | 1～2 | 500～1200 ms | 动作组持续随机循环。 |
| `Alert` | 2000～4000 ms | 1～2 | 250～700 ms | 告警有效期间持续随机循环。 |
| `Move` | 1200～2400 ms | 2～4 | 120～420 ms | 自动巡逻、拖动和蹦迪时使用，按方向过滤。 |
| `Interact` | 完整一轮 | 1 | 0 | 默认随机一个动作播放后回到待机。 |
| `Life` | 每个动作完整一轮 | 2～4 | 800～2000 ms | 随机播放一组生活动作后回到待机。 |

动作选择始终为等概率随机，不使用权重。状态变化、资源切换或移动方向变化会立即中断并重建动作组。

## clips 动作字段

动作键格式为 `状态目录/动作子目录`，例如 `Move/jump_left`。状态目录中的直属 PNG 使用 `状态目录/default`。

| 字段 | 类型 | 可选值或限制 | 含义 |
|---|---|---|---|
| `fps` | number | 大于 `0` | 覆盖根级默认帧率。 |
| `loop` | string | `repeat`、`once`、`ping-pong` | `repeat` 循环；`once` 播放到末帧；`ping-pong` 正放后倒放。 |
| `direction` | string | `left`、`right` | 移动方向。拖动或蹦迪时优先选择同方向动作。 |
| `weight` | number | 正整数 | 兼容旧资源保留，但当前运行时忽略，所有动作等概率。 |

旧字段 `states.*.selection`、`states.*.minLoops` 可被 JSON 解析器保留，但当前运行时不读取。

## 完整示例

```json
{
  "schemaVersion": 2,
  "id": "frog-buddy",
  "name": "治愈小青蛙",
  "version": "1.0.0",
  "author": "LanChat",
  "description": "透明背景桌宠",
  "resolution": 512,
  "fps": 8,
  "transparent": true,
  "defaultState": "Idle",
  "states": {
    "Idle": {
      "loop": "repeat",
      "minDurationMs": 4000,
      "maxDurationMs": 9000,
      "minActionCount": 1,
      "maxActionCount": 2,
      "minIntervalMs": 600,
      "maxIntervalMs": 1400
    },
    "Alert": { "loop": "repeat", "minDurationMs": 2000, "maxDurationMs": 4000, "minActionCount": 1, "maxActionCount": 2, "minIntervalMs": 300, "maxIntervalMs": 700 },
    "Move": { "loop": "repeat", "minDurationMs": 1200, "maxDurationMs": 2200, "minActionCount": 2, "maxActionCount": 4, "minIntervalMs": 150, "maxIntervalMs": 450 },
    "Interact": { "loop": "once", "minDurationMs": 0, "maxDurationMs": 0, "minActionCount": 1, "maxActionCount": 1, "minIntervalMs": 0, "maxIntervalMs": 0 },
    "Life": { "loop": "once", "minDurationMs": 0, "maxDurationMs": 0, "minActionCount": 2, "maxActionCount": 4, "minIntervalMs": 900, "maxIntervalMs": 1800 }
  },
  "clips": {
    "Idle/breathe": { "fps": 6, "loop": "repeat" },
    "Move/jump_left": { "fps": 7, "loop": "repeat", "direction": "left" },
    "Move/jump_right": { "fps": 7, "loop": "repeat", "direction": "right" },
    "Interact/head_pat": { "fps": 8, "loop": "once" }
  }
}
```
