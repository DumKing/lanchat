import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const sourceDir = path.join(rootDir, "src-tauri", "resources", "object-models");
const profilesPath = path.join(scriptDir, "vision-model-profiles.json");
const outputDir = path.resolve(process.argv[2] ?? path.join(rootDir, "release-assets", "vision-models", "staging"));

const config = JSON.parse(await readFile(profilesPath, "utf8"));
if (config.schemaVersion !== 1 || !Array.isArray(config.profiles) || config.profiles.length === 0) {
  throw new Error("无效的视觉模型档位配置");
}

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });

for (const profile of config.profiles) {
  const profileDir = path.join(outputDir, profile.profileId);
  const modelsDir = path.join(profileDir, "object-models");
  await cp(sourceDir, modelsDir, { recursive: true, filter: (entry) => !entry.endsWith("README.md") });

  const legacyPath = path.join(modelsDir, "manifest.json");
  const legacyManifest = JSON.parse(await readFile(legacyPath, "utf8"));
  legacyManifest.modelVersion = profile.profileVersion;
  await writeFile(legacyPath, `${JSON.stringify(legacyManifest, null, 2)}\n`, "utf8");

  const v3Path = path.join(modelsDir, "manifest.v3.json");
  const v3Manifest = JSON.parse(await readFile(v3Path, "utf8"));
  v3Manifest.package = {
    id: `com.lanchat.vision.${profile.profileId}`,
    version: profile.profileVersion,
  };
  v3Manifest.profile = {
    id: profile.profileId,
    version: profile.profileVersion,
    tier: profile.tier,
  };
  // 运行时读此字段展示档位来源；模型安装校验仍只信任清单和组件摘要。
  v3Manifest.recommendedSettings = profile.recommendedSettings;
  await writeFile(v3Path, `${JSON.stringify(v3Manifest, null, 2)}\n`, "utf8");
}

console.log(`已生成 ${config.profiles.length} 个视觉模型档位目录：${outputDir}`);
