import { invoke } from "@tauri-apps/api/core";
import type { PluginManifestV1 } from "../plugin-host/contracts/manifest";

export interface InstalledPluginRecord {
  pluginId: string;
  displayName: string | null;
  activeVersion: string;
  previousVersion: string | null;
  enabled: boolean;
  grantedCapabilities: string[];
  source: "official" | "development" | string;
  signatureKeyId: string | null;
  installedAt: number;
  updatedAt: number;
  lastError: string | null;
}

export const pluginApi = {
  listInstalled: () => invoke<InstalledPluginRecord[]>("list_installed_plugins"),
  listEnabledManifests: () => invoke<PluginManifestV1[]>("list_enabled_plugin_manifests"),
  installPackage: (packagePath: string, allowUnsignedDevelopment = false) =>
    invoke<InstalledPluginRecord>("install_plugin_package", {
      packagePath,
      allowUnsignedDevelopment,
    }),
  setEnabled: (pluginId: string, enabled: boolean) =>
    invoke<InstalledPluginRecord>("set_plugin_enabled", { pluginId, enabled }),
  setPermissions: (pluginId: string, capabilities: string[]) =>
    invoke<InstalledPluginRecord>("set_plugin_permissions", { pluginId, capabilities }),
  rollback: (pluginId: string) =>
    invoke<InstalledPluginRecord>("rollback_plugin", { pluginId }),
  uninstall: (pluginId: string, deletePrivateData: boolean) =>
    invoke<void>("uninstall_plugin", { pluginId, deletePrivateData }),
};
