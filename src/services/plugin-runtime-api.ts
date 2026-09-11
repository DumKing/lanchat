import { invoke } from "@tauri-apps/api/core";
import type { PluginBounds, PluginRuntimeAdapter, StartedPluginInstance } from "../plugin-host/runtime/PluginRuntime";

interface NativeStartedPluginInstance {
  pluginId: string;
  instanceId: string;
  instanceToken: string;
  featureCode: string;
  capabilities: string[];
}

export const pluginRuntimeApi = {
  start: (input: { pluginId: string; featureCode: string; bounds: PluginBounds }) =>
    invoke<NativeStartedPluginInstance>("start_plugin_instance", input),
  setBounds: (instanceId: string, bounds: PluginBounds) =>
    invoke<void>("set_plugin_instance_bounds", { instanceId, bounds }),
  setVisible: (instanceId: string, visible: boolean) =>
    invoke<void>("set_plugin_instance_visible", { instanceId, visible }),
  emit: (instanceId: string, event: string, payload: unknown) =>
    invoke<number>("emit_plugin_runtime_event", { instanceId, event, payload }),
  destroy: (instanceId: string, reason: string) =>
    invoke<void>("destroy_plugin_instance", { instanceId, reason }),
  resolveBridgeRequest: (instanceToken: string, requestId: string, response: unknown) =>
    invoke<void>("resolve_plugin_bridge_request", { instanceToken, requestId, response }),
};

export function createTauriPluginRuntimeAdapter(): PluginRuntimeAdapter {
  return {
    async start(input): Promise<StartedPluginInstance> {
      const instance = await pluginRuntimeApi.start(input);
      return { ...instance, capabilities: new Set(instance.capabilities) };
    },
    setBounds: pluginRuntimeApi.setBounds,
    setVisible: pluginRuntimeApi.setVisible,
    emit: async (instanceId, event, payload) => { await pluginRuntimeApi.emit(instanceId, event, payload); },
    destroy: pluginRuntimeApi.destroy,
  };
}
