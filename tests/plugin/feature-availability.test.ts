import { describe, expect, it } from "vitest";
import type { PluginManifestV1 } from "../../src/plugin-host/contracts/manifest";
import { resolvePluginFeatures } from "../../src/plugin-host/registry/featureAvailability";

function manifest(input: {
  id: string;
  navigation?: PluginManifestV1["contributes"] extends infer _T ? Array<{ id: string; title: string }> : never;
  games?: Array<{ id: string; minPlayers: number; maxPlayers: number }>;
}): PluginManifestV1 {
  return {
    manifestVersion: 1,
    id: input.id,
    name: input.id,
    version: "0.8.0",
    apiVersion: "1.0",
    minHostVersion: "0.8.0",
    type: "web",
    entry: "dist/index.html",
    icon: "icon.png",
    singleton: true,
    capabilities: [],
    contributes: {
      navigation: input.navigation,
      games: input.games,
    },
  };
}

describe("插件能力可见性", () => {
  it("没有已启用插件时不提供扩展能力", () => {
    const result = resolvePluginFeatures([]);

    expect(result.gameIds).toEqual([]);
    expect(result.navigation).toEqual([]);
  });

  it("只公开已启用插件声明的能力", () => {
    const result = resolvePluginFeatures([
      {
        enabled: true,
        manifest: manifest({
          id: "com.lanchat.gomoku",
          navigation: [{ id: "gomoku", title: "五子棋" }],
          games: [{ id: "gomoku", minPlayers: 2, maxPlayers: 2 }],
        }),
      },
      {
        enabled: false,
        manifest: manifest({
          id: "com.example.disabled",
          navigation: [{ id: "disabled-page", title: "未启用页面" }],
        }),
      },
    ]);

    expect(result.gameIds).toEqual(["gomoku"]);
    expect(result.navigation.map((item) => item.id)).toEqual(["gomoku"]);
  });

  it("启用插件可以贡献标准导航入口", () => {
    const result = resolvePluginFeatures([
      {
        enabled: true,
        manifest: manifest({
          id: "com.example.toolbox",
          navigation: [{ id: "toolbox", title: "工具箱" }],
        }),
      },
    ]);

    expect(result.navigation.map((item) => item.id)).toEqual(["toolbox"]);
  });
});
