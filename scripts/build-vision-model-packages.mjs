import { access, cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const profilesPath = path.join(scriptDir, "vision-model-profiles.json");
const sourcesPath = path.join(scriptDir, "vision-model-sources.json");
const outputDir = path.resolve(process.argv[2] ?? path.join(rootDir, "release-assets", "vision-models", "staging"));

const config = JSON.parse(await readFile(profilesPath, "utf8"));
if (config.schemaVersion !== 2 || !Array.isArray(config.profiles) || config.profiles.length === 0) {
  throw new Error("无效的视觉模型档位配置");
}
const sourceConfig = JSON.parse(await readFile(sourcesPath, "utf8"));
if (sourceConfig.schemaVersion !== 1 || !Array.isArray(sourceConfig.profiles)) {
  throw new Error("无效的视觉模型源配置");
}
const sourcesByProfileId = new Map(sourceConfig.profiles.map((source) => [source.profileId, source]));
if (sourcesByProfileId.size !== config.profiles.length) {
  throw new Error("视觉模型档位与模型源配置数量不一致");
}

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });

const builtProfiles = [];
const embeddingComponentCategories = new Set(["face_recognizer", "person_reid"]);

function digest(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function verifyAsset(modelsDir, component, asset = component) {
  const file = String(asset.file ?? "").trim();
  const expected = String(asset.sha256 ?? "").trim().toLowerCase();
  if (!file || !/^[a-f0-9]{64}$/.test(expected) || path.isAbsolute(file) || file.split(/[\\/]/).includes("..")) {
    throw new Error(`模型组件 ${component.id} 的资源声明无效`);
  }
  const actual = digest(await readFile(path.join(modelsDir, file)));
  if (actual !== expected) {
    throw new Error(`模型组件 ${component.id} 的资源摘要不匹配：${file}`);
  }
  return { file, sha256: actual };
}

function componentByFamily(manifest, category, family) {
  return manifest.components?.find((component) =>
    component.category === category && component.family === family,
  );
}

for (const profile of config.profiles) {
  const source = sourcesByProfileId.get(profile.profileId);
  if (!source?.sourceDir) {
    throw new Error(`模型 Profile ${profile.profileId} 缺少独立资源目录`);
  }
  if (!profile.modelStack || source.engine !== profile.modelStack.inferenceEngine
    || source.faceEngine?.family !== profile.modelStack.faceEngine
    || source.personReIdEngine?.family !== profile.modelStack.personReIdEngine) {
    throw new Error(`模型 Profile ${profile.profileId} 的模型组合与资源源配置不一致`);
  }
  const sourceDir = path.resolve(rootDir, source.sourceDir);
  try {
    await access(sourceDir);
  } catch {
    throw new Error(`模型 Profile ${profile.profileId} 的资源目录不存在：${sourceDir}`);
  }
  const sourceV4Path = path.join(sourceDir, "manifest.v4.json");
  try {
    await access(sourceV4Path);
  } catch {
    throw new Error(`模型 Profile ${profile.profileId} 的源资源缺少 manifest.v4.json：${sourceV4Path}`);
  }
  const profileDir = path.join(outputDir, profile.profileId);
  const modelsDir = path.join(profileDir, "object-models");
  // Windows Node 的 cp filter 在部分 Runner 上会遗漏根目录文件；先完整复制，再仅清理说明文件。
  await cp(sourceDir, modelsDir, { recursive: true });
  await rm(path.join(modelsDir, "README.md"), { force: true });

  const v4Path = path.join(modelsDir, "manifest.v4.json");
  try {
    await access(v4Path);
  } catch {
    throw new Error(`模型 Profile ${profile.profileId} 缺少 manifest.v4.json，不能再打包为旧 V3 模型`);
  }
  const v4Manifest = JSON.parse(await readFile(v4Path, "utf8"));
  if (v4Manifest.schemaVersion !== 4 || !Array.isArray(v4Manifest.components)) {
    throw new Error(`模型 Profile ${profile.profileId} 的 V4 清单无效`);
  }
  const faceComponent = componentByFamily(v4Manifest, "face_recognizer", source.faceEngine.family);
  const reidComponent = componentByFamily(v4Manifest, "person_reid", source.personReIdEngine.family);
  if (!faceComponent || !reidComponent
    || faceComponent.file !== source.faceEngine.source
    || reidComponent.file !== source.personReIdEngine.source) {
    throw new Error(`模型 Profile ${profile.profileId} 的 V4 清单与声明的识别模型来源不一致`);
  }
  const componentAssets = [];
  for (const component of v4Manifest.components) {
    componentAssets.push({
      id: component.id,
      category: component.category,
      family: component.family,
      adapterId: component.adapterId,
      engine: component.engine,
      ...(await verifyAsset(modelsDir, component)),
      auxiliaryFiles: await Promise.all(
        (component.auxiliaryFiles ?? []).map(async (asset) => verifyAsset(modelsDir, component, asset)),
      ),
      input: component.input ?? null,
      output: component.output ?? null,
    });
  }
  v4Manifest.package = {
    id: `com.lanchat.vision.${profile.profileId}`,
    version: profile.profileVersion,
  };
  v4Manifest.profile = {
    ...v4Manifest.profile,
    id: profile.profileId,
    version: profile.profileVersion,
    tier: profile.tier,
    displayName: profile.displayName,
    provider: profile.modelStack.provider,
    engine: profile.modelStack.inferenceEngine,
  };
  v4Manifest.recommendedSettings = profile.recommendedSettings;
  await writeFile(v4Path, `${JSON.stringify(v4Manifest, null, 2)}\n`, "utf8");
  builtProfiles.push({
    profileId: profile.profileId,
    profileVersion: profile.profileVersion,
    modelStack: profile.modelStack,
    components: componentAssets,
  });
}

const identitySignatures = new Map();
for (const profile of builtProfiles) {
  const signature = profile.components
    .filter((component) => embeddingComponentCategories.has(component.category))
    .map((component) => `${component.category}:${component.adapterId}:${component.sha256}`)
    .sort()
    .join("|");
  if (!signature) {
    throw new Error(`模型 Profile ${profile.profileId} 没有可识别的 Face/ReID 模型`);
  }
  const previous = identitySignatures.get(signature);
  if (previous) {
    throw new Error(`PROFILE_HAS_NO_DISTINCT_MODEL_ASSETS:${previous}:${profile.profileId}`);
  }
  identitySignatures.set(signature, profile.profileId);
}

const comparisons = [];
for (let left = 0; left < builtProfiles.length; left += 1) {
  for (let right = left + 1; right < builtProfiles.length; right += 1) {
    const source = builtProfiles[left];
    const target = builtProfiles[right];
    const sourceIdentity = source.components
      .filter((component) => embeddingComponentCategories.has(component.category))
      .map((component) => `${component.category}:${component.sha256}`)
      .sort();
    const targetIdentity = target.components
      .filter((component) => embeddingComponentCategories.has(component.category))
      .map((component) => `${component.category}:${component.sha256}`)
      .sort();
    comparisons.push({
      leftProfileId: source.profileId,
      rightProfileId: target.profileId,
      sameIdentityAssets: JSON.stringify(sourceIdentity) === JSON.stringify(targetIdentity),
      leftIdentityAssets: sourceIdentity,
      rightIdentityAssets: targetIdentity,
    });
  }
}
await writeFile(
  path.join(outputDir, "model-diff.json"),
  `${JSON.stringify({ schemaVersion: 1, profiles: builtProfiles, comparisons }, null, 2)}\n`,
  "utf8",
);

console.log(`已生成 ${config.profiles.length} 个视觉模型档位目录：${outputDir}`);
