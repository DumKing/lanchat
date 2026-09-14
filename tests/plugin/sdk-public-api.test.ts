import { cp, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { PLUGIN_EVENTS, PLUGIN_HASH_CONTRACT, createLanChatPluginClient } from "../../packages/plugin-sdk/src/index";
import type { PluginBridgeTransport } from "../../packages/plugin-sdk/src/index";

interface PublicApiReport {
  schemaVersion: number;
  generatorVersion: string;
  apiVersion: string;
  declarations: {
    contentHash: string;
    graphHash: string;
    files: Array<{ path: string; contentHash: string }>;
  };
  exportedTypes: string[];
  runtimeExports: string[];
  runtimeConstants: { PLUGIN_EVENTS: Record<string, string>; PLUGIN_HASH_CONTRACT: Record<string, string> };
  clientMethods: Record<string, string[]>;
}

async function loadReport(): Promise<PublicApiReport> {
  return JSON.parse(await readFile(resolve("packages/plugin-sdk/public-api-report.json"), "utf8")) as PublicApiReport;
}

const generatorPath = resolve("packages/plugin-sdk/scripts/generate-public-api-report.mjs");

function generateReport(sdkRoot?: string) {
  const args = [generatorPath];
  if (sdkRoot) args.push("--sdk-root", sdkRoot);
  const result = spawnSync(process.execPath, args, { encoding: "utf8" });
  expect(result.status, `${result.stdout}\n${result.stderr}`).toBe(0);
  return result.stdout;
}

function replaceRequired(content: string, search: string, replacement: string) {
  expect(content).toContain(search);
  return content.replace(search, replacement);
}

describe("内置 SDK 公共 API 漂移护栏", () => {
  it("编译产物可重建已提交的规范化公共 API report", () => {
    const result = spawnSync(process.execPath, [
      resolve("packages/plugin-sdk/scripts/generate-public-api-report.mjs"),
      "--check",
      resolve("packages/plugin-sdk/public-api-report.json"),
    ], { encoding: "utf8" });

    expect(result.status, `${result.stdout}\n${result.stderr}`).toBe(0);
  });

  it("report 包含版本元数据、完整内容哈希和声明图哈希", async () => {
    const report = JSON.parse(generateReport()) as PublicApiReport;

    expect(report.schemaVersion).toBe(2);
    expect(report.generatorVersion).toBe("2.0.0");
    expect(report.apiVersion).toBe("1.0");
    expect(report.declarations.contentHash).toMatch(/^[a-f0-9]{64}$/);
    expect(report.declarations.graphHash).toMatch(/^[a-f0-9]{64}$/);
    const declarationPaths = report.declarations.files.map(({ path }) => path);
    expect(declarationPaths).toContain("index.d.ts");
    expect(declarationPaths).toContain("types.d.ts");
    expect(declarationPaths).toEqual([...declarationPaths].sort());
    expect(declarationPaths.every((path) => !path.includes("\\"))).toBe(true);
  });

  it("声明换行、行尾空白和终止换行差异会被规范化", async () => {
    const directory = await mkdtemp(resolve(".tmp-plugin-sdk-normalized-"));
    const sdkRoot = join(directory, "plugin-sdk");
    try {
      await cp(resolve("packages/plugin-sdk/dist"), join(sdkRoot, "dist"), { recursive: true });
      for (const entry of await readdir(join(sdkRoot, "dist"))) {
        if (!entry.endsWith(".d.ts")) continue;
        const declarationPath = join(sdkRoot, "dist", entry);
        const declaration = await readFile(declarationPath, "utf8");
        const withFormattingNoise = `${declaration.replace(/\r?\n/g, "  \r\n")}\r\n\r\n`;
        await writeFile(declarationPath, withFormattingNoise, "utf8");
      }

      expect(generateReport(sdkRoot)).toBe(generateReport());
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("任一声明字段漂移都会使 report 检查失败", async () => {
    const directory = await mkdtemp(resolve(".tmp-plugin-sdk-declaration-drift-"));
    const sdkRoot = join(directory, "plugin-sdk");
    try {
      await cp(resolve("packages/plugin-sdk/dist"), join(sdkRoot, "dist"), { recursive: true });
      const typesPath = join(sdkRoot, "dist/types.d.ts");
      const declarations = await readFile(typesPath, "utf8");
      await writeFile(typesPath, declarations.replace("roomId: string;", "roomId: string;\n    reviewDrift?: string;"), "utf8");
      const result = spawnSync(process.execPath, [
        resolve("packages/plugin-sdk/scripts/generate-public-api-report.mjs"),
        "--sdk-root",
        sdkRoot,
        "--check",
        resolve("packages/plugin-sdk/public-api-report.json"),
      ], { encoding: "utf8" });

      expect(result.status).not.toBe(0);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("签名、overload、联合类型、返回类型和嵌套声明变化都会改变 report", async () => {
    const mutations: Array<[string, string, string]> = [
      ["types.d.ts", "invite(input: {", "invite(input: {\n        source?: string;"],
      ["types.d.ts", "    submit(input: PluginLegacyLeaderboardSubmitInput): Promise<void>;\n", ""],
      ["types.d.ts", '"lobby" | "closed"', '"lobby" | "closed" | "review"'],
      ["types.d.ts", "Promise<PluginRoomRecovery>", "Promise<void>"],
    ];
    const baseline = generateReport();

    for (const [file, search, replacement] of mutations) {
      const directory = await mkdtemp(resolve(".tmp-plugin-sdk-api-drift-"));
      const sdkRoot = join(directory, "plugin-sdk");
      try {
        await cp(resolve("packages/plugin-sdk/dist"), join(sdkRoot, "dist"), { recursive: true });
        const declarationPath = join(sdkRoot, "dist", file);
        const declaration = await readFile(declarationPath, "utf8");
        await writeFile(declarationPath, replaceRequired(declaration, search, replacement), "utf8");
        expect(generateReport(sdkRoot)).not.toBe(baseline);
      } finally {
        await rm(directory, { recursive: true, force: true });
      }
    }

    const directory = await mkdtemp(resolve(".tmp-plugin-sdk-nested-declaration-"));
    const sdkRoot = join(directory, "plugin-sdk");
    try {
      await cp(resolve("packages/plugin-sdk/dist"), join(sdkRoot, "dist"), { recursive: true });
      await writeFile(join(sdkRoot, "dist", "nested.d.ts"), "export interface NestedContract { value: string; }\n", "utf8");
      expect(generateReport(sdkRoot)).not.toBe(baseline);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }, 15_000);

  it("以运行时结构校验常量与客户端方法", async () => {
    const report = await loadReport();
    const transport: PluginBridgeTransport = {
      request: async <TResult>() => undefined as TResult,
      subscribe: () => () => undefined,
    };
    const client = createLanChatPluginClient(transport) as unknown as Record<string, Record<string, unknown>>;

    expect(PLUGIN_EVENTS).toEqual(report.runtimeConstants.PLUGIN_EVENTS);
    expect(PLUGIN_HASH_CONTRACT).toEqual(report.runtimeConstants.PLUGIN_HASH_CONTRACT);
    for (const [namespace, methods] of Object.entries(report.clientMethods)) {
      expect(Object.keys(client[namespace]).sort()).toEqual(methods);
    }
  });

  it("以 tsc 夹具校验全部公共类型及旧、新调用", async () => {
    // 主程序内置 SDK 是插件契约源；独立 SDK 仓库发布它的镜像。
    const report = await loadReport();
    const directory = await mkdtemp(join(tmpdir(), "lanchat-plugin-sdk-source-"));
    const fixturePath = join(directory, "fixture.ts");
    const configPath = join(directory, "tsconfig.json");
    const importedTypes = report.exportedTypes.join(",\n  ");
    const fixture = `
import type {\n  ${importedTypes}\n} from "@lanchat/plugin-sdk";
import { PLUGIN_HASH_CONTRACT, canonicalizePluginValue, sha256PluginValue } from "@lanchat/plugin-sdk";
declare const api: LanChatPluginApiV1;
const legacyInvite: (input: { roomId: string }) => Promise<void> = api.rooms.invite;
api.leaderboard.submit({ gameId: "g", roomId: "r", idempotencyKey: "legacy", result: {} });
api.leaderboard.submit({ gameId: "g", roomId: "r", terminalRevision: 1, terminalCommitId: "c", stateHash: "s", result: {} });
const canonical: string = canonicalizePluginValue({ b: 1, a: 2 });
const digest: Promise<string> = sha256PluginValue({ b: 1, a: 2 });
api.events.onRoomEvent((event) => void event.sequence);
api.events.onAuthoritativeRoomEvent((event) => void event.kind);
api.rooms.invite({ roomId: "r" });
api.rooms.resolveInvite({ inviteId: "i", token: "opaque" });
api.rooms.resolveInvite({ inviteId: "i", token: "opaque" }).then((resolved) => resolved.ok && void resolved.room.revision);
api.rooms.recover({ roomId: "r", afterRevision: 0 }).then((recovered) => void recovered.headRevision);
api.rooms.setAdmission({ roomId: "r", admission: "closed" }).then((updated) => void updated.admission);
// @ts-expect-error resolveInvite 不信任插件声明的房间版本或过期时间
api.rooms.resolveInvite({ roomId: "r", roomVersion: 1, expiresAt: 2 });
const invite: PluginRoomInvite = { inviteId: "i", token: "opaque", roomId: "r", roomVersion: 1, expiresAt: 2, pluginId: "p" };
const room: PluginAuthoritativeRoomSummary = { roomId: "r", gameId: "g", ownerPeerId: "p", memberPeerIds: ["p"], schemaVersion: 1, admission: "lobby", roomVersion: 1, revision: 0 };
const recovery: PluginRoomRecovery = { room, headRevision: 0, checkpoint: null, commits: [] };
const resolvedInvite: PluginRoomInviteResolution = { ok: true, room };
type PublicTypes = [${report.exportedTypes.join(", ")}];
void [legacyInvite, invite, recovery, resolvedInvite, canonical, digest, PLUGIN_HASH_CONTRACT];
declare const publicTypes: PublicTypes;
void publicTypes;
`;
    const config = {
      compilerOptions: {
        strict: true,
        noEmit: true,
        target: "ES2020",
        module: "ESNext",
        moduleResolution: "Bundler",
        baseUrl: resolve("."),
        paths: { "@lanchat/plugin-sdk": ["packages/plugin-sdk/src/index.ts"] },
      },
      files: [fixturePath],
    };

    try {
      await writeFile(fixturePath, fixture, "utf8");
      await writeFile(configPath, JSON.stringify(config), "utf8");
      const result = spawnSync(process.execPath, [resolve("node_modules/typescript/bin/tsc"), "-p", configPath], { encoding: "utf8" });
      expect(`${result.stdout}\n${result.stderr}`).toBe("\n");
      expect(result.status).toBe(0);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
