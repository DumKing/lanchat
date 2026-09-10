import { describe, expect, it } from "vitest";
import { createBridgeDispatcher } from "../../src/plugin-host/runtime/BridgeDispatcher";
import type { PluginBridgeRequest } from "../../src/plugin-host/contracts/bridge";

const request = (overrides: Partial<PluginBridgeRequest> = {}): PluginBridgeRequest => ({
  id: "request-1",
  apiVersion: "1.0",
  instanceToken: "valid-token",
  method: "devices.list",
  params: {},
  ...overrides,
});

function dispatcher(capabilities = ["devices.read"] as const) {
  return createBridgeDispatcher({
    resolveInstance: (token) => token === "valid-token"
      ? { pluginId: "com.lanchat.test", instanceId: "instance-1", capabilities: new Set<string>(capabilities) }
      : null,
    handlers: {
      "devices.list": async () => [{ id: "peer-a", name: "A", online: true }],
      "chat.send": async (params) => ({ messageId: String((params as { text: string }).text) }),
      "logger.write": async () => {
        throw new Error("数据库连接字符串不能暴露");
      },
    },
    validators: {
      "chat.send": (params) => typeof (params as { text?: unknown })?.text === "string",
    },
  });
}

describe("BridgeDispatcher", () => {
  it("执行已授权方法并保留请求 ID", async () => {
    await expect(dispatcher().dispatch(request())).resolves.toEqual({
      id: "request-1",
      ok: true,
      result: [{ id: "peer-a", name: "A", online: true }],
    });
  });

  it("拒绝伪造的实例令牌", async () => {
    const response = await dispatcher().dispatch(request({ instanceToken: "fake" }));

    expect(response).toMatchObject({ id: "request-1", ok: false, error: { code: "INVALID_INSTANCE_TOKEN" } });
  });

  it("拒绝未声明的能力", async () => {
    const response = await dispatcher().dispatch(request({ method: "chat.send", params: { text: "hello" } }));

    expect(response).toMatchObject({ ok: false, error: { code: "PERMISSION_DENIED" } });
  });

  it("在调用 handler 前校验参数", async () => {
    const response = await dispatcher(["chat.send"]).dispatch(request({ method: "chat.send", params: {} }));

    expect(response).toMatchObject({ ok: false, error: { code: "SCHEMA_VALIDATION_FAILED" } });
  });

  it("不向插件暴露宿主内部异常", async () => {
    const response = await dispatcher(["logger.write"]).dispatch(request({ method: "logger.write" }));

    expect(response).toMatchObject({ ok: false, error: { code: "INTERNAL_ERROR", message: "宿主处理插件请求失败" } });
    expect(JSON.stringify(response)).not.toContain("数据库连接字符串");
  });

  it("拒绝不兼容的 API 版本和未知方法", async () => {
    await expect(dispatcher().dispatch(request({ apiVersion: "2.0" as "1.0" })))
      .resolves.toMatchObject({ ok: false, error: { code: "UNSUPPORTED_API_VERSION" } });
    await expect(dispatcher().dispatch(request({ method: "system.shell" })))
      .resolves.toMatchObject({ ok: false, error: { code: "UNKNOWN_METHOD" } });
  });
});
