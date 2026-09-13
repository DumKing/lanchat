import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { PLUGIN_EVENTS, PLUGIN_HASH_CONTRACT, createLanChatPluginClient } from "../../packages/plugin-sdk/src/index";
import type { PluginBridgeTransport } from "../../packages/plugin-sdk/src/index";

interface PublicApiSnapshot {
  exportedTypes: string[];
  runtimeConstants: { PLUGIN_EVENTS: Record<string, string>; PLUGIN_HASH_CONTRACT: Record<string, string> };
  clientMethods: Record<string, string[]>;
}

async function loadSnapshot(): Promise<PublicApiSnapshot> {
  return JSON.parse(await readFile(resolve("tests/plugin-sdk-public-api.snapshot.json"), "utf8")) as PublicApiSnapshot;
}

describe("内置 SDK 公共 API 漂移护栏", () => {
  it("以运行时结构校验常量与客户端方法", async () => {
    const snapshot = await loadSnapshot();
    const transport: PluginBridgeTransport = {
      request: async <TResult>() => undefined as TResult,
      subscribe: () => () => undefined,
    };
    const client = createLanChatPluginClient(transport) as unknown as Record<string, Record<string, unknown>>;

    expect(PLUGIN_EVENTS).toEqual(snapshot.runtimeConstants.PLUGIN_EVENTS);
    expect(PLUGIN_HASH_CONTRACT).toEqual(snapshot.runtimeConstants.PLUGIN_HASH_CONTRACT);
    for (const [namespace, methods] of Object.entries(snapshot.clientMethods)) {
      expect(Object.keys(client[namespace]).sort()).toEqual(methods);
    }
  });

  it("以 tsc 夹具校验全部公共类型及旧、新调用", async () => {
    // 主程序内置 SDK 是插件契约源；独立 SDK 仓库发布它的镜像。
    const snapshot = await loadSnapshot();
    const directory = await mkdtemp(join(tmpdir(), "lanchat-plugin-sdk-source-"));
    const fixturePath = join(directory, "fixture.ts");
    const configPath = join(directory, "tsconfig.json");
    const importedTypes = snapshot.exportedTypes.join(",\n  ");
    const fixture = `
import type {\n  ${importedTypes}\n} from "@lanchat/plugin-sdk";
import { PLUGIN_HASH_CONTRACT } from "@lanchat/plugin-sdk";
declare const api: LanChatPluginApiV1;
api.leaderboard.submit({ gameId: "g", roomId: "r", idempotencyKey: "legacy", result: {} });
api.leaderboard.submit({ gameId: "g", roomId: "r", terminalRevision: 1, terminalCommitId: "c", stateHash: "s", result: {} });
api.events.onRoomEvent((event) => void event.sequence);
api.events.onAuthoritativeRoomEvent((event) => void event.kind);
api.rooms.invite({ roomId: "r" }).then((issued) => void issued.token);
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
type PublicTypes = [${snapshot.exportedTypes.join(", ")}];
void [invite, recovery, resolvedInvite, PLUGIN_HASH_CONTRACT];
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
