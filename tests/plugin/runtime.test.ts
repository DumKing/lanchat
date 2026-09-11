import { describe, expect, it, vi } from "vitest";
import { PluginRuntime, type PluginRuntimeAdapter } from "../../src/plugin-host/runtime/PluginRuntime";

function setup() {
  let serial = 0;
  const adapter: PluginRuntimeAdapter = {
    start: vi.fn(async ({ pluginId, featureCode }) => {
      serial += 1;
      return {
        pluginId,
        featureCode,
        instanceId: `instance-${serial}`,
        instanceToken: `token-${serial}`,
        capabilities: new Set(["devices.read"]),
      };
    }),
    setBounds: vi.fn(async () => undefined),
    setVisible: vi.fn(async () => undefined),
    emit: vi.fn(async () => undefined),
    destroy: vi.fn(async () => undefined),
  };
  const runtime = new PluginRuntime(adapter, { "devices.list": async () => [] });
  return { adapter, runtime };
}

describe("PluginRuntime", () => {
  it("单例插件再次打开时复用实例并更新视口", async () => {
    const { adapter, runtime } = setup();
    const first = await runtime.start("com.lanchat.gomoku", "gomoku", { x: 0, y: 0, width: 500, height: 400 });
    const second = await runtime.start("com.lanchat.gomoku", "gomoku", { x: 5, y: 6, width: 700, height: 500 });

    expect(second.instanceId).toBe(first.instanceId);
    expect(adapter.start).toHaveBeenCalledTimes(1);
    expect(adapter.setBounds).toHaveBeenCalledWith(first.instanceId, { x: 5, y: 6, width: 700, height: 500 });
  });

  it("销毁实例后立即撤销令牌", async () => {
    const { adapter, runtime } = setup();
    const instance = await runtime.start("com.lanchat.gomoku", "gomoku", { x: 0, y: 0, width: 500, height: 400 });
    await runtime.destroy(instance.instanceId, "disabled");
    const response = await runtime.dispatch({
      id: "request-1",
      apiVersion: "1.0",
      instanceToken: instance.instanceToken,
      method: "devices.list",
      params: {},
    });

    expect(response).toMatchObject({ ok: false, error: { code: "INVALID_INSTANCE_TOKEN" } });
    expect(adapter.destroy).toHaveBeenCalledWith(instance.instanceId, "disabled");
  });

  it("可以接管由原生层先创建的受信实例", async () => {
    const { runtime } = setup();
    runtime.adopt({
      pluginId: "com.lanchat.gomoku",
      featureCode: "gomoku",
      instanceId: "native-instance",
      instanceToken: "native-token",
      capabilities: new Set(["devices.read"]),
    });
    await expect(runtime.dispatch({
      id: "request-1",
      apiVersion: "1.0",
      instanceToken: "native-token",
      method: "devices.list",
      params: {},
    })).resolves.toMatchObject({ ok: true });
  });
});
