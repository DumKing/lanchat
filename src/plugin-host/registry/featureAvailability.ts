import type { PluginManifestV1 } from "../contracts/manifest";
import {
  buildNavigationRegistry,
  type InstalledPluginContribution,
  type PluginNavigationItem,
} from "./contributions";

export interface PluginFeatureAvailability {
  navigation: PluginNavigationItem[];
  gameIds: string[];
  visionPluginId: string | null;
}

export function resolvePluginFeatures(
  plugins: readonly InstalledPluginContribution[],
): PluginFeatureAvailability {
  const enabledPlugins = plugins.filter((plugin) => plugin.enabled);
  const navigation = buildNavigationRegistry([], enabledPlugins).items.filter(
    (item): item is PluginNavigationItem => item.source === "plugin",
  );
  const gameIds = uniqueGameIds(enabledPlugins.map((plugin) => plugin.manifest));
  const visionPlugin = enabledPlugins.find(
    (plugin) => plugin.manifest.id === "com.lanchat.vision"
      && plugin.manifest.contributes?.navigation?.some((item) => item.id === "vision"),
  );

  return {
    navigation,
    gameIds,
    visionPluginId: visionPlugin?.manifest.id ?? null,
  };
}

function uniqueGameIds(manifests: readonly PluginManifestV1[]): string[] {
  const ids = new Set<string>();
  for (const manifest of manifests) {
    for (const game of manifest.contributes?.games ?? []) {
      ids.add(game.id);
    }
  }
  return [...ids].sort((left, right) => left.localeCompare(right));
}
