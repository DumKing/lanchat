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
assert.match(peoplePanel, /vision\.people\.title/);
assert.match(runtimeStatus, /vision\.runtime\.pause/);

console.log("vision workspace UI contract passed");
