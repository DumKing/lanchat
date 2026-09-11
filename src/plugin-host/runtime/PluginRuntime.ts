import type { PluginBridgeHandler, PluginInstanceAccess } from "./BridgeDispatcher";
import { createBridgeDispatcher } from "./BridgeDispatcher";
import { PLUGIN_METHOD_SCHEMAS } from "./schemas";
import type { PluginBridgeRequest, PluginBridgeResponse } from "../contracts/bridge";

export interface PluginBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StartedPluginInstance extends PluginInstanceAccess {
  instanceToken: string;
  featureCode: string;
}

export interface PluginRuntimeAdapter {
  start(input: { pluginId: string; featureCode: string; bounds: PluginBounds }): Promise<StartedPluginInstance>;
  setBounds(instanceId: string, bounds: PluginBounds): Promise<void>;
  setVisible(instanceId: string, visible: boolean): Promise<void>;
  emit(instanceId: string, event: string, payload: unknown): Promise<void>;
  destroy(instanceId: string, reason: string): Promise<void>;
}

export class PluginRuntime {
  readonly #adapter: PluginRuntimeAdapter;
  readonly #instancesById = new Map<string, StartedPluginInstance>();
  readonly #instancesByToken = new Map<string, StartedPluginInstance>();
  readonly #dispatcher;

  constructor(adapter: PluginRuntimeAdapter, handlers: Readonly<Record<string, PluginBridgeHandler>>) {
    this.#adapter = adapter;
    this.#dispatcher = createBridgeDispatcher({
      resolveInstance: (token) => this.#instancesByToken.get(token) ?? null,
      handlers,
      validators: PLUGIN_METHOD_SCHEMAS,
    });
  }

  adopt(instance: StartedPluginInstance) {
    const known = this.#instancesById.get(instance.instanceId);
    if (known) return known;
    this.#instancesById.set(instance.instanceId, instance);
    this.#instancesByToken.set(instance.instanceToken, instance);
    return instance;
  }

  instances() {
    return [...this.#instancesById.values()];
  }

  async start(pluginId: string, featureCode: string, bounds: PluginBounds) {
    const existing = [...this.#instancesById.values()].find((item) => item.pluginId === pluginId);
    if (existing) {
      await this.#adapter.setBounds(existing.instanceId, bounds);
      await this.#adapter.setVisible(existing.instanceId, true);
      return existing;
    }
    const instance = await this.#adapter.start({ pluginId, featureCode, bounds });
    return this.adopt(instance);
  }

  dispatch(request: PluginBridgeRequest): Promise<PluginBridgeResponse> {
    return this.#dispatcher.dispatch(request);
  }

  async emit(instanceId: string, event: string, payload: unknown) {
    if (!this.#instancesById.has(instanceId)) return;
    await this.#adapter.emit(instanceId, event, payload);
  }

  async setBounds(instanceId: string, bounds: PluginBounds) {
    if (!this.#instancesById.has(instanceId)) return;
    await this.#adapter.setBounds(instanceId, bounds);
  }

  async setVisible(instanceId: string, visible: boolean) {
    if (!this.#instancesById.has(instanceId)) return;
    await this.#adapter.setVisible(instanceId, visible);
  }

  async destroy(instanceId: string, reason = "terminated") {
    const instance = this.#instancesById.get(instanceId);
    if (!instance) return;
    this.#instancesById.delete(instanceId);
    this.#instancesByToken.delete(instance.instanceToken);
    await this.#adapter.destroy(instanceId, reason);
  }

  async revokePlugin(pluginId: string, reason = "disabled") {
    const targets = [...this.#instancesById.values()].filter((item) => item.pluginId === pluginId);
    await Promise.all(targets.map((item) => this.destroy(item.instanceId, reason)));
  }
}
