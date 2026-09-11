// @vitest-environment jsdom
import { flushPromises, shallowMount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PluginCenterPage from "../../src/pages/PluginCenterPage.vue";

const mocks = vi.hoisted(() => ({
  open: vi.fn(),
  listInstalled: vi.fn(),
  installPackage: vi.fn(),
  setEnabled: vi.fn(),
  setPermissions: vi.fn(),
  rollback: vi.fn(),
  uninstall: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.open }));
vi.mock("../../src/services/plugin-api", () => ({
  pluginApi: {
    listInstalled: mocks.listInstalled,
    installPackage: mocks.installPackage,
    setEnabled: mocks.setEnabled,
    setPermissions: mocks.setPermissions,
    rollback: mocks.rollback,
    uninstall: mocks.uninstall,
  },
}));

const installedPlugin = {
  pluginId: "com.lanchat.gomoku",
  activeVersion: "0.8.0",
  previousVersion: null,
  enabled: false,
  grantedCapabilities: ["rooms.read", "rooms.write"],
  source: "official",
  signatureKeyId: "lanchat-official-2026-v1",
  installedAt: 1,
  updatedAt: 1,
  lastError: null,
};

const passthrough = { template: "<div><slot /></div>" };
const button = { emits: ["click"], template: "<button @click=\"$emit('click')\"><slot /></button>" };

function mountPage() {
  return shallowMount(PluginCenterPage, {
    global: {
      stubs: {
        NAlert: passthrough,
        NButton: button,
        NCard: passthrough,
        NCheckbox: passthrough,
        NEmpty: passthrough,
        NModal: passthrough,
        NSpin: passthrough,
        NSwitch: passthrough,
        NTag: passthrough,
      },
    },
  });
}

describe("插件中心", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalled.mockResolvedValue([]);
  });

  it("选择本地 lcp 后调用正式签名安装并等待权限确认", async () => {
    mocks.open.mockResolvedValue("D:\\plugins\\gomoku.lcp");
    mocks.installPackage.mockResolvedValue(installedPlugin);
    mocks.listInstalled.mockResolvedValueOnce([]).mockResolvedValueOnce([installedPlugin]);
    const wrapper = mountPage();
    await flushPromises();

    await (wrapper.vm as unknown as { installLocalPackage: () => Promise<void> }).installLocalPackage();
    await flushPromises();

    expect(mocks.installPackage).toHaveBeenCalledWith("D:\\plugins\\gomoku.lcp", false);
    expect(mocks.setEnabled).not.toHaveBeenCalled();
    const state = wrapper.vm as unknown as { successMessage: string; permissionTarget: typeof installedPlugin | null };
    expect(state.successMessage).toContain("请确认权限后启用");
    expect(state.permissionTarget?.signatureKeyId).toBe("lanchat-official-2026-v1");
  });

  it("确认权限后授权并启用插件", async () => {
    mocks.open.mockResolvedValue("D:\\plugins\\gomoku.lcp");
    mocks.installPackage.mockResolvedValue(installedPlugin);
    mocks.setPermissions.mockResolvedValue(installedPlugin);
    mocks.setEnabled.mockResolvedValue({ ...installedPlugin, enabled: true });
    mocks.listInstalled
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([installedPlugin])
      .mockResolvedValueOnce([{ ...installedPlugin, enabled: true }]);
    const wrapper = mountPage();
    await flushPromises();

    await (wrapper.vm as unknown as { installLocalPackage: () => Promise<void> }).installLocalPackage();
    await flushPromises();
    await (wrapper.vm as unknown as { enableAfterPermissionReview: () => Promise<void> }).enableAfterPermissionReview();
    await flushPromises();

    expect(mocks.setPermissions).toHaveBeenCalledWith(installedPlugin.pluginId, installedPlugin.grantedCapabilities);
    expect(mocks.setEnabled).toHaveBeenCalledWith(installedPlugin.pluginId, true);
    expect(wrapper.emitted("changed")).toHaveLength(1);
  });
});
