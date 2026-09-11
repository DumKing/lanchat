import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PluginBridgeRequest } from "../contracts/bridge";
import type { PluginRuntime } from "./PluginRuntime";
import { pluginRuntimeApi } from "../../services/plugin-runtime-api";

interface ForwardedBridgeRequest {
  instanceId: string;
  instanceToken: string;
  pluginId: string;
  featureCode: string;
  capabilities: string[];
  request: PluginBridgeRequest;
}

export async function installTauriPluginBridge(runtime: PluginRuntime): Promise<UnlistenFn> {
  return listen<ForwardedBridgeRequest>("plugin-bridge-requested", async ({ payload }) => {
    runtime.adopt({
      instanceId: payload.instanceId,
      instanceToken: payload.instanceToken,
      pluginId: payload.pluginId,
      featureCode: payload.featureCode,
      capabilities: new Set(payload.capabilities),
    });
    const response = await runtime.dispatch(payload.request);
    await pluginRuntimeApi.resolveBridgeRequest(payload.instanceToken, payload.request.id, response).catch(() => undefined);
  });
}
