import type { PluginManifestV1 } from "../contracts/manifest";

export interface CoreNavigationItem {
  id: string;
  title: string;
  order: number;
  source: "core";
  icon?: string;
}

export interface PluginNavigationItem {
  id: string;
  title: string;
  order: number;
  source: "plugin";
  pluginId: string;
  icon?: string;
}

export type NavigationRegistryItem = CoreNavigationItem | PluginNavigationItem;

export interface InstalledPluginContribution {
  manifest: PluginManifestV1;
  enabled: boolean;
}

export interface ContributionConflict {
  contributionId: string;
  pluginId: string;
}

export function buildNavigationRegistry(
  coreItems: readonly CoreNavigationItem[],
  plugins: readonly InstalledPluginContribution[],
): { items: NavigationRegistryItem[]; conflicts: ContributionConflict[] } {
  const items: NavigationRegistryItem[] = [...coreItems];
  const claimedIds = new Set(coreItems.map((item) => item.id));
  const conflicts: ContributionConflict[] = [];

  for (const plugin of plugins) {
    if (!plugin.enabled) continue;
    for (const contribution of plugin.manifest.contributes?.navigation ?? []) {
      if (claimedIds.has(contribution.id)) {
        conflicts.push({ contributionId: contribution.id, pluginId: plugin.manifest.id });
        continue;
      }
      claimedIds.add(contribution.id);
      items.push({
        id: contribution.id,
        title: contribution.title,
        order: contribution.order ?? 500,
        source: "plugin",
        pluginId: plugin.manifest.id,
        icon: contribution.icon,
      });
    }
  }

  items.sort((left, right) => left.order - right.order || left.title.localeCompare(right.title) || left.id.localeCompare(right.id));
  return { items, conflicts };
}
