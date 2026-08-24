import { cp, mkdir, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const buffaloRoot = process.argv[2] ? path.resolve(process.argv[2]) : "";
const baselineRoot = path.join(projectRoot, "src-tauri", "resources", "object-models");
const targetRoot = path.join(projectRoot, "src-tauri", "resources", "model-profiles", "office-arcface-buffalo-sc");

if (!buffaloRoot) {
  throw new Error("用法: node scripts/prepare-insightface-arcface-profile.mjs <buffalo_sc-expand-dir>");
}

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

async function copyWithHash(source, targetName) {
  const target = path.join(targetRoot, targetName);
  await cp(source, target);
  return { file: targetName, sha256: await sha256(target) };
}

await rm(targetRoot, { recursive: true, force: true });
await mkdir(targetRoot, { recursive: true });

const arcFaceSource = await findFile(buffaloRoot, "w600k_mbf.onnx");
if (!arcFaceSource || !(await stat(arcFaceSource)).isFile()) {
  throw new Error("未在 buffalo_sc 资源中找到 w600k_mbf.onnx");
}

const faceDetector = await copyWithHash(path.join(baselineRoot, "presence-detector.onnx"), "presence-detector.onnx");
const personDetector = await copyWithHash(path.join(baselineRoot, "person-detector.onnx"), "person-detector.onnx");
const personReid = await copyWithHash(path.join(baselineRoot, "person-recognizer.onnx"), "person-recognizer.onnx");
const faceRecognizer = await copyWithHash(arcFaceSource, "face-recognizer-arcface.onnx");

const manifest = {
  schemaVersion: 4,
  package: { id: "com.lanchat.vision.office-arcface-buffalo-sc", version: "1.0.0" },
  profile: {
    id: "office-arcface-buffalo-sc",
    version: "1.0.0",
    tier: "experimental",
    displayName: "ArcFace Buffalo-SC",
    provider: "InsightFace buffalo_sc",
    engine: "onnxruntime",
    supportedBackends: ["cpu", "directml"],
  },
  pipeline: {
    personDetector: "person-detector",
    faceEngine: "face-recognizer",
    personReIdEngine: "person-reid",
    fusionPolicy: "quality-temporal-v1",
  },
  components: [
    {
      id: "face-detector",
      category: "face_detector",
      family: "yunet",
      ...faceDetector,
      adapterId: "builtin.face-detector.yunet.v1",
      engine: "onnxruntime",
    },
    {
      id: "face-recognizer",
      category: "face_recognizer",
      family: "arcface",
      ...faceRecognizer,
      adapterId: "builtin.face-recognizer.arcface.v1",
      engine: "onnxruntime",
      input: { colorOrder: "RGB", resizeMode: "aligned_112", normalization: "arcface_127_5" },
      output: { embeddingDimension: 512, distanceMetric: "cosine" },
    },
    {
      id: "person-detector",
      category: "person_detector",
      family: "yolox",
      ...personDetector,
      adapterId: "builtin.person-detector.yolox.v1",
      engine: "onnxruntime",
    },
    {
      id: "person-reid",
      category: "person_reid",
      family: "youtureid",
      ...personReid,
      adapterId: "builtin.person-reid.youtu.v1",
      engine: "onnxruntime",
      input: { colorOrder: "RGB", resizeMode: "128x256", normalization: "imagenet" },
      output: { embeddingDimension: 768, distanceMetric: "cosine" },
    },
  ],
};

await writeFile(path.join(targetRoot, "manifest.v4.json"), `${JSON.stringify(manifest, null, 2)}\n`);
await writeFile(
  path.join(targetRoot, "MODEL-LICENSE-NOTICE.txt"),
  "InsightFace buffalo_sc public pretrained weights are limited to non-commercial research use.\n"
    + "LanChat publishes this optional profile only for learning and non-commercial research.\n"
    + "Do not redistribute or use it commercially unless you obtain an appropriate license.\n",
);
console.log("已装配 InsightFace ArcFace Profile: office-arcface-buffalo-sc");
