import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const config = JSON.parse(
  await readFile(new URL("./vision-model-sources.json", import.meta.url), "utf8"),
);
const packageBuilder = await readFile(
  new URL("./build-vision-model-packages.mjs", import.meta.url),
  "utf8",
);
const profileConfig = JSON.parse(
  await readFile(new URL("./vision-model-profiles.json", import.meta.url), "utf8"),
);

assert.equal(config.schemaVersion, 1, "模型源配置必须使用 schemaVersion 1");
assert.equal(profileConfig.schemaVersion, 2, "模型目录必须使用 schemaVersion 2，声明真实模型组合");
assert.ok(Array.isArray(config.profiles), "模型源配置必须包含 profiles");
assert.ok(config.profiles.length >= 3, "官方目录至少应包含基线和两套真实独立的视觉模型组合");

const profileIds = new Set();
const recognitionSources = new Set();
for (const profile of config.profiles) {
  assert.ok(profile.profileId, "模型 Profile 必须有 profileId");
  assert.ok(profile.sourceDir, `${profile.profileId} 必须声明独立 sourceDir`);
  assert.ok(profile.faceEngine, `${profile.profileId} 必须声明 FaceEngine`);
  assert.ok(profile.personReIdEngine, `${profile.profileId} 必须声明 PersonReIdEngine`);
  assert.ok(profile.engine, `${profile.profileId} 必须声明推理后端`);
  assert.ok(profileIds.add(profile.profileId), `重复的模型 Profile：${profile.profileId}`);

  const identity = `${profile.faceEngine.source}|${profile.personReIdEngine.source}`;
  assert.ok(
    recognitionSources.add(identity),
    `模型 Profile ${profile.profileId} 与其他 Profile 使用同一组识别权重来源`,
  );
}

for (const profile of profileConfig.profiles) {
  assert.ok(profile.modelStack?.inferenceEngine, `${profile.profileId} 必须声明推理引擎`);
  assert.ok(profile.modelStack?.faceEngine, `${profile.profileId} 必须声明人脸引擎`);
  assert.ok(profile.modelStack?.personReIdEngine, `${profile.profileId} 必须声明人体特征引擎`);
}

assert.match(
  packageBuilder,
  /vision-model-sources\.json/,
  "模型构建必须读取独立模型源配置",
);
assert.match(
  packageBuilder,
  /sourceDir/,
  "模型构建必须按 Profile 的 sourceDir 取资源",
);
assert.doesNotMatch(
  packageBuilder,
  /const sourceDir = path\.join\(rootDir, "src-tauri", "resources", "object-models"\)/,
  "模型构建不得固定复制单一内置模型目录",
);
assert.match(
  packageBuilder,
  /model-diff\.json/,
  "模型构建必须输出模型差异清单",
);
assert.match(
  packageBuilder,
  /PROFILE_HAS_NO_DISTINCT_MODEL_ASSETS/,
  "模型构建必须拒绝同一识别权重的改名 Profile",
);

console.log("vision package distinctness guards passed");
