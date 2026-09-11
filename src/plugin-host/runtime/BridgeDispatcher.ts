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
  timeoutMs?: number;
  maxRequestsPerWindow?: number;
  rateLimitWindowMs?: number;
}

function failure(id: string, code: PluginBridgeErrorCode, message: string, retryable = false): PluginBridgeResponse {
  return { id, ok: false, error: { code, message, retryable } };
}

export function createBridgeDispatcher(options: BridgeDispatcherOptions) {
  const requestWindows = new Map<string, { startedAt: number; count: number }>();

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

      const now = Date.now();
      const rateLimitWindowMs = options.rateLimitWindowMs ?? 1_000;
      const maxRequests = options.maxRequestsPerWindow ?? 120;
      const currentWindow = requestWindows.get(instance.instanceId);
      const requestWindow = !currentWindow || now - currentWindow.startedAt >= rateLimitWindowMs
        ? { startedAt: now, count: 0 }
        : currentWindow;
      requestWindow.count += 1;
      requestWindows.set(instance.instanceId, requestWindow);
      if (requestWindow.count > maxRequests) {
        return failure(requestId, "RATE_LIMITED", "插件调用过于频繁，请稍后重试", true);
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
        const timeoutMs = options.timeoutMs ?? 10_000;
        let timeoutHandle: ReturnType<typeof globalThis.setTimeout> | undefined;
        const timeout = new Promise<never>((_, reject) => {
          timeoutHandle = globalThis.setTimeout(() => reject(new Error("PLUGIN_HOST_TIMEOUT")), timeoutMs);
        });
        const result = await Promise.race([Promise.resolve(handler(request.params, instance)), timeout])
          .finally(() => {
            if (timeoutHandle !== undefined) globalThis.clearTimeout(timeoutHandle);
          });
        return { id: requestId, ok: true, result };
      } catch (error) {
        if (error instanceof Error && error.message === "PLUGIN_HOST_TIMEOUT") {
          return failure(requestId, "HOST_TIMEOUT", "宿主处理插件请求超时", true);
        }
        return failure(requestId, "INTERNAL_ERROR", "宿主处理插件请求失败", true);
      }
    },
  };
}
