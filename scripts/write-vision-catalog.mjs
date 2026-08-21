import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [packagesDir, tag, outputPath] = process.argv.slice(2);
if (!packagesDir || !tag || !outputPath) {
  throw new Error("用法：node scripts/write-vision-catalog.mjs <packagesDir> <tag> <outputPath>");
}
if (!/^v\d+\.\d+\.\d+/.test(tag)) {
  throw new Error("发布标签必须是 v 开头的语义版本，例如 v0.5.2");
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const profilesPath = path.join(scriptDir, "vision-model-profiles.json");
const config = JSON.parse(await readFile(profilesPath, "utf8"));
const releaseBase = `https://github.com/DumKing/lanchat/releases/download/${tag}`;
const profiles = [];

for (const profile of config.profiles) {
  const archive = await readFile(path.join(packagesDir, profile.assetName));
  profiles.push({
    profileId: profile.profileId,
    profileVersion: profile.profileVersion,
    displayName: profile.displayName,
    tier: profile.tier,
    downloadUrl: `${releaseBase}/${profile.assetName}`,
    packageSha256: createHash("sha256").update(archive).digest("hex"),
    packageSizeBytes: archive.byteLength,
    recommendedSettings: profile.recommendedSettings,
  });
}

await writeFile(outputPath, `${JSON.stringify({ schemaVersion: 1, profiles }, null, 2)}\n`, "utf8");
console.log(`已写入待签名视觉模型目录：${outputPath}`);
