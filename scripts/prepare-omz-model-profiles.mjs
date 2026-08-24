import { cp, mkdir, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const modelCacheRoot = process.argv[2] ? path.resolve(process.argv[2]) : "";

if (!modelCacheRoot) {
  throw new Error("用法: node scripts/prepare-omz-model-profiles.mjs <open-model-zoo-download-dir>");
}

const profileRoot = path.join(projectRoot, "src-tauri", "resources", "model-profiles");

async function findFile(directory, fileName) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const candidate = path.join(directory, entry.name);
    if (entry.isFile() && entry.name === fileName) return candidate;
    if (entry.isDirectory()) {
      const found = await findFile(candidate, fileName);
      if (found) return found;
    }
  }
  return undefined;
}

async function sha256(file) {
  const hash = createHash("sha256");
  const handle = await (await import("node:fs/promises")).open(file, "r");
  try {
    for await (const chunk of handle.createReadStream()) hash.update(chunk);
  } finally {
    await handle.close();
  }
  return hash.digest("hex");
}

async function copyIr(modelName, targetDirectory, targetBaseName) {
  const xmlSource = await findFile(modelCacheRoot, `${modelName}.xml`);
  if (!xmlSource) throw new Error(`未找到 Open Model Zoo 模型: ${modelName}.xml`);
  const binSource = path.join(path.dirname(xmlSource), `${modelName}.bin`);
  if (!(await stat(binSource)).isFile()) throw new Error(`未找到 Open Model Zoo 权重: ${modelName}.bin`);

  const xmlTarget = path.join(targetDirectory, `${targetBaseName}.xml`);
  const binTarget = path.join(targetDirectory, `${targetBaseName}.bin`);
  await cp(xmlSource, xmlTarget);
  await cp(binSource, binTarget);
  return {
    file: path.basename(xmlTarget),
    sha256: await sha256(xmlTarget),
    auxiliaryFiles: [{ file: path.basename(binTarget), sha256: await sha256(binTarget) }],
  };
}

function irComponent({ id, category, family, adapterId, asset, input, output }) {
  return {
    id,
    category,
    family,
    file: asset.file,
    sha256: asset.sha256,
    auxiliaryFiles: asset.auxiliaryFiles,
    adapterId,
    engine: "openvino",
    ...(input ? { input } : {}),
    ...(output ? { output } : {}),
  };
}

const profiles = [
  {
    id: "office-omz-retail-0288",
    displayName: "OMZ Retail-0288",
    tier: "balanced",
    reidModel: "person-reidentification-retail-0288",
    reidFamily: "omz-person-reid-0288",
    reidAdapter: "builtin.person-reid.omz.v1",
    reidFile: "person-reid-0288",
  },
  {
    id: "office-omz-retail-0286",
    displayName: "OMZ Retail-0286",
    tier: "experimental",
    reidModel: "person-reidentification-retail-0286",
    reidFamily: "omz-person-reid-0286",
    reidAdapter: "builtin.person-reid.omz.0286.v1",
    reidFile: "person-reid-0286",
  },
];

for (const profile of profiles) {
  const targetDirectory = path.join(profileRoot, profile.id);
  await rm(targetDirectory, { recursive: true, force: true });
  await mkdir(targetDirectory, { recursive: true });

  const faceDetector = await copyIr("face-detection-retail-0004", targetDirectory, "face-detector");
  const faceRecognizer = await copyIr("face-reidentification-retail-0095", targetDirectory, "face-recognizer");
  const personDetector = await copyIr("person-detection-0203", targetDirectory, "person-detector");
  const personReid = await copyIr(profile.reidModel, targetDirectory, profile.reidFile);

  const manifest = {
    schemaVersion: 4,
    package: { id: `com.lanchat.vision.${profile.id}`, version: "1.0.0" },
    profile: {
      id: profile.id,
      version: "1.0.0",
      tier: profile.tier,
      displayName: profile.displayName,
      provider: "Open Model Zoo",
      engine: "openvino",
      supportedBackends: ["openvino_cpu"],
    },
    pipeline: {
      personDetector: "person-detector",
      faceEngine: "face-recognizer",
      personReIdEngine: "person-reid",
      fusionPolicy: "quality-temporal-v1",
    },
    components: [
      irComponent({
        id: "face-detector",
        category: "face_detector",
        family: "omz-face-detector",
        adapterId: "builtin.face-detector.omz.v1",
        asset: faceDetector,
      }),
      irComponent({
        id: "face-recognizer",
        category: "face_recognizer",
        family: "omz-face-reid",
        adapterId: "builtin.face-recognizer.omz.v1",
        asset: faceRecognizer,
        input: { colorOrder: "BGR", resizeMode: "128x128", normalization: "openvino_retail" },
        output: { embeddingDimension: 256, distanceMetric: "cosine" },
      }),
      irComponent({
        id: "person-detector",
        category: "person_detector",
        family: "omz-person-detection",
        adapterId: "builtin.person-detector.omz.v1",
        asset: personDetector,
      }),
      irComponent({
        id: "person-reid",
        category: "person_reid",
        family: profile.reidFamily,
        adapterId: profile.reidAdapter,
        asset: personReid,
        input: { colorOrder: "BGR", resizeMode: "128x256", normalization: "openvino_retail" },
        output: { embeddingDimension: 256, distanceMetric: "cosine" },
      }),
    ],
  };
  await writeFile(path.join(targetDirectory, "manifest.v4.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`已装配 Open Model Zoo Profile: ${profile.id}`);
}
