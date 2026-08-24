import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, i18n, modelCenter, peoplePanel, runtimeStatus] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/i18n.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/components/VisionModelCenter.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/components/VisionPeoplePanel.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/components/VisionRuntimeStatus.vue", import.meta.url), "utf8"),
]);

assert.match(app, /VisionModelCenter/);
assert.match(app, /VisionPeoplePanel/);
assert.match(app, /VisionRuntimeStatus/);
assert.match(app, /openSection\('vision'\)/);
assert.match(i18n, /vision\.profile\.balanced/);
assert.match(i18n, /vision\.workspace\.title/);
assert.match(modelCenter, /vision\.profile\.low_resource/);
assert.match(modelCenter, /已启用/, "模型中心必须明确展示已选中的启用模型");
assert.match(modelCenter, /卸载/, "下载模型必须提供卸载入口");
assert.match(modelCenter, /uninstall/, "模型中心必须向上层触发卸载操作");
assert.match(peoplePanel, /vision\.people\.title/);
assert.match(runtimeStatus, /vision\.runtime\.pause/);

console.log("vision workspace UI contract passed");
