import { invoke } from "@tauri-apps/api/core";
import type { PluginBridgeHandler } from "./BridgeDispatcher";
import type { PluginThemeSnapshot } from "../contracts/bridge";

export interface PluginHostHandlerDependencies {
  hostVersion: string;
  listDevices(): unknown | Promise<unknown>;
  sendChat(input: { conversationId: string; text: string; metadata?: Record<string, unknown> }): unknown | Promise<unknown>;
  currentTheme(): PluginThemeSnapshot;
  close(instanceId: string): unknown | Promise<unknown>;
  notify(input: { title: string; body: string; level?: string }): unknown | Promise<unknown>;
  confirm(input: { title: string; body: string; confirmText?: string; cancelText?: string }): boolean | Promise<boolean>;
  pickFile(input: { title?: string; extensions?: string[]; multiple?: boolean }): string[] | Promise<string[]>;
  additionalHandlers?: Readonly<Record<string, PluginBridgeHandler>>;
}

export function createPluginHostHandlers(dependencies: PluginHostHandlerDependencies): Readonly<Record<string, PluginBridgeHandler>> {
  const handlers: Record<string, PluginBridgeHandler> = {
    "app.getVersion": () => dependencies.hostVersion,
    "app.getInstance": (_params, context) => ({ pluginId: context.pluginId, instanceId: context.instanceId }),
    "app.close": (_params, context) => dependencies.close(context.instanceId),
    "devices.list": () => dependencies.listDevices(),
    "chat.send": (params) => dependencies.sendChat(params as { conversationId: string; text: string; metadata?: Record<string, unknown> }),
    "storage.get": (params, context) => invoke("plugin_private_storage_get", { pluginId: context.pluginId, key: (params as { key: string }).key }),
    "storage.set": (params, context) => invoke("plugin_private_storage_set", { pluginId: context.pluginId, ...(params as { key: string; value: unknown }) }),
    "storage.delete": (params, context) => invoke("plugin_private_storage_delete", { pluginId: context.pluginId, key: (params as { key: string }).key }),
    "storage.keys": (_params, context) => invoke("plugin_private_storage_keys", { pluginId: context.pluginId }),
    "theme.current": () => dependencies.currentTheme(),
    "ui.notify": (params) => dependencies.notify(params as { title: string; body: string; level?: string }),
    "ui.confirm": (params) => dependencies.confirm(params as { title: string; body: string; confirmText?: string; cancelText?: string }),
    "ui.pickFile": (params) => dependencies.pickFile(params as { title?: string; extensions?: string[]; multiple?: boolean }),
    "logger.write": (params, context) => {
      const entry = params as { level: "debug" | "info" | "warn" | "error"; message: string; details?: unknown };
      const writer = console[entry.level] ?? console.info;
      writer(`[plugin:${context.pluginId}] ${entry.message}`, entry.details ?? "");
    },
    ...dependencies.additionalHandlers,
  };
  return Object.freeze(handlers);
}
