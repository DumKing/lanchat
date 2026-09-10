import type {
  PluginBridgeErrorCode,
  PluginBridgeRequest,
  PluginBridgeResponse,
} from "../contracts/bridge";
import { PLUGIN_METHOD_CAPABILITIES } from "./capabilities";

export interface PluginInstanceAccess {
  pluginId: string;
  instanceId: string;
  capabilities: ReadonlySet<string>;
}

export interface PluginBridgeHandlerContext extends PluginInstanceAccess {}

export type PluginBridgeHandler = (
  params: unknown,
  context: PluginBridgeHandlerContext,
) => unknown | Promise<unknown>;

export interface BridgeDispatcherOptions {
  resolveInstance(token: string): PluginInstanceAccess | null;
  handlers: Readonly<Record<string, PluginBridgeHandler>>;
  validators?: Readonly<Record<string, (params: unknown) => boolean>>;
}

function failure(id: string, code: PluginBridgeErrorCode, message: string, retryable = false): PluginBridgeResponse {
  return { id, ok: false, error: { code, message, retryable } };
}

export function createBridgeDispatcher(options: BridgeDispatcherOptions) {
  return {
    async dispatch(request: PluginBridgeRequest): Promise<PluginBridgeResponse> {
      const requestId = typeof request?.id === "string" ? request.id : "";
      if (request?.apiVersion !== "1.0") {
        return failure(requestId, "UNSUPPORTED_API_VERSION", "宿主不支持此插件 API 版本");
      }

      const instance = options.resolveInstance(request.instanceToken);
      if (!instance) {
        return failure(requestId, "INVALID_INSTANCE_TOKEN", "插件实例已经失效");
      }

      const handler = options.handlers[request.method];
      if (!handler || !(request.method in PLUGIN_METHOD_CAPABILITIES)) {
        return failure(requestId, "UNKNOWN_METHOD", "宿主不支持此 API 方法");
      }

      const capability = PLUGIN_METHOD_CAPABILITIES[request.method];
      if (capability && !instance.capabilities.has(capability)) {
        return failure(requestId, "PERMISSION_DENIED", "插件未获得此能力授权");
      }

      const validator = options.validators?.[request.method];
      if (validator && !validator(request.params)) {
        return failure(requestId, "SCHEMA_VALIDATION_FAILED", "插件请求参数不符合 API 约定");
      }

      try {
        const result = await handler(request.params, instance);
        return { id: requestId, ok: true, result };
      } catch {
        return failure(requestId, "INTERNAL_ERROR", "宿主处理插件请求失败", true);
      }
    },
  };
}
