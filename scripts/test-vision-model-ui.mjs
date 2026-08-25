import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const modelCenter = await readFile(
  new URL("../src/components/VisionModelCenter.vue", import.meta.url),
  "utf8",
);
const runtimeStatus = await readFile(
  new URL("../src/components/VisionRuntimeStatus.vue", import.meta.url),
  "utf8",
);

assert.match(
  modelCenter,
  /const isSelected = \(profile: VisionProfileSummary\) => profile\.active;/,
  "模型列表的已启用状态必须只使用持久化选择，不能把当前旧运行时快照同时标为已启用",
);
assert.match(
  modelCenter,
  /isSelected\(profile\) \? `已启用 · \$\{profile\.profileVersion\}`/,
  "已启用标签必须显示被选中模型自身版本，不能沿用旧运行时版本",
);
assert.doesNotMatch(
  modelCenter,
  /profile\.active \|\| activeProfile\.value === profile\.profileId/,
  "旧运行时快照不能参与模型列表选中态判断",
);

assert.match(
  runtimeStatus,
  /status\?\.modelProfileId \|\| '-'/,
  "运行状态必须展示识别引擎当前实际加载的模型 Profile",
);
assert.match(
  runtimeStatus,
  /status\?\.modelVersion \|\| '-'/,
  "运行状态必须展示识别引擎当前实际加载的模型版本",
);
assert.doesNotMatch(
  runtimeStatus,
  /snapshot\?\.activeProfileId \|\| status\?\.modelProfileId/,
  "持久化快照不能覆盖实际运行模型状态",
);

console.log("vision model UI selection guards passed");
