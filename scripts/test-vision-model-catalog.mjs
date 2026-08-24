import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const profiles = JSON.parse(
  await readFile(new URL("./vision-model-profiles.json", import.meta.url), "utf8"),
);
const sources = JSON.parse(
  await readFile(new URL("./vision-model-sources.json", import.meta.url), "utf8"),
);
const packageBuilder = await readFile(
  new URL("./build-vision-model-packages.mjs", import.meta.url),
  "utf8",
);

assert.equal(profiles.schemaVersion, 2, "Catalog 源必须使用 schemaVersion 2");
assert.equal(sources.schemaVersion, 1, "模型源必须使用 schemaVersion 1");

const sourceById = new Map(sources.profiles.map((source) => [source.profileId, source]));
assert.equal(sourceById.size, profiles.profiles.length, "每个官方 Profile 必须有唯一模型源");

for (const profile of profiles.profiles) {
  assert.ok(profile.profileId && profile.profileVersion, "Profile 必须有稳定 ID 与版本");
  assert.ok(profile.assetName?.endsWith(".zip"), `${profile.profileId} 必须声明 ZIP 资产名`);
  assert.ok(profile.modelStack?.inferenceEngine, `${profile.profileId} 缺少推理后端`);
  assert.ok(profile.modelStack?.faceEngine, `${profile.profileId} 缺少人脸引擎`);
  assert.ok(profile.modelStack?.personReIdEngine, `${profile.profileId} 缺少人体 ReID 引擎`);
  assert.ok(profile.modelStack?.provider, `${profile.profileId} 缺少模型来源`);
  assert.ok(profile.modelStack?.license, `${profile.profileId} 缺少许可证声明`);
  assert.notEqual(profile.modelStack.license, "TO_BE_VERIFIED", `${profile.profileId} 许可证尚未核验`);
  assert.ok(!profile.profileId.startsWith("custom-"), "受限自定义模型不得进入官方 Catalog");

  const source = sourceById.get(profile.profileId);
  assert.ok(source, `${profile.profileId} 未找到模型源`);
  assert.equal(source.engine, profile.modelStack.inferenceEngine, `${profile.profileId} 后端声明不一致`);
  assert.equal(source.faceEngine.family, profile.modelStack.faceEngine, `${profile.profileId} 人脸引擎声明不一致`);
  assert.equal(source.personReIdEngine.family, profile.modelStack.personReIdEngine, `${profile.profileId} 人体引擎声明不一致`);
}

assert.match(packageBuilder, /model-diff\.json/, "发布必须产出模型差异审计文件");
assert.match(packageBuilder, /PROFILE_HAS_NO_DISTINCT_MODEL_ASSETS/, "发布必须拦截同权重改名");
assert.match(packageBuilder, /auxiliaryFiles/, "发布必须校验 OpenVINO 等辅助模型文件");

console.log("vision catalog source guards passed");
