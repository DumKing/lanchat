import { describe, expect, it } from "vitest";
import type { PluginManifestV1 } from "../../src/plugin-host/contracts/manifest";
import { buildNavigationRegistry } from "../../src/plugin-host/registry/contributions";

const manifest = (id: string, navigationId: string, order: number): PluginManifestV1 => ({
  manifestVersion: 1,
  id,
  name: id,
  version: "1.0.0",
  apiVersion: "1.0",
  minHostVersion: "0.8.0",
  type: "web",
  entry: "dist/index.html",
  icon: "icon.png",
  singleton: true,
  capabilities: [],
  contributes: { navigation: [{ id: navigationId, title: navigationId, order }] },
});

describe("contribution registry", () => {
  it("合并核心与已启用插件导航并按 order 排序", () => {
    const result = buildNavigationRegistry(
      [{ id: "chat", title: "聊天", order: 10, source: "core" }],
      [
        { manifest: manifest("com.lanchat.gomoku", "gomoku", 30), enabled: true },
        { manifest: manifest("com.lanchat.disabled", "disabled", 20), enabled: false },
      ],
    );

    expect(result.items.map((item) => item.id)).toEqual(["chat", "gomoku"]);
    expect(result.items[1]).toMatchObject({ source: "plugin", pluginId: "com.lanchat.gomoku" });
    expect(result.conflicts).toEqual([]);
  });

  it("插件不能覆盖核心导航 ID", () => {
    const result = buildNavigationRegistry(
      [{ id: "chat", title: "聊天", order: 10, source: "core" }],
      [{ manifest: manifest("com.lanchat.fake", "chat", 1), enabled: true }],
    );

    expect(result.items.map((item) => item.id)).toEqual(["chat"]);
    expect(result.conflicts).toEqual([{ contributionId: "chat", pluginId: "com.lanchat.fake" }]);
  });

  it("多个插件声明同一导航 ID 时保留先注册项并报告冲突", () => {
    const result = buildNavigationRegistry([], [
      { manifest: manifest("com.lanchat.first", "board", 10), enabled: true },
      { manifest: manifest("com.lanchat.second", "board", 20), enabled: true },
    ]);

    expect(result.items).toHaveLength(1);
    expect(result.items[0].pluginId).toBe("com.lanchat.first");
    expect(result.conflicts).toEqual([{ contributionId: "board", pluginId: "com.lanchat.second" }]);
  });
});
