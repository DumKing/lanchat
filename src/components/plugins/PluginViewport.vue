<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { PluginRuntime, StartedPluginInstance } from "../../plugin-host/runtime/PluginRuntime";
import type { PluginEntrySource } from "../../plugin-host/contracts/bridge";

const props = withDefaults(defineProps<{
  runtime: PluginRuntime;
  pluginId: string;
  featureCode: string;
  hostVersion: string;
  source?: PluginEntrySource;
  payload?: unknown;
  visible?: boolean;
}>(), {
  source: "navigation",
  payload: undefined,
  visible: true,
});

const emit = defineEmits<{
  started: [instance: StartedPluginInstance];
  error: [message: string];
}>();

const viewport = ref<HTMLElement | null>(null);
let observer: ResizeObserver | null = null;
let instance: StartedPluginInstance | null = null;
let stopped = false;

function bounds() {
  const rect = viewport.value?.getBoundingClientRect();
  return {
    x: Math.max(0, rect?.left ?? 0),
    y: Math.max(0, rect?.top ?? 0),
    width: Math.max(1, rect?.width ?? 1),
    height: Math.max(1, rect?.height ?? 1),
  };
}

async function updateBounds() {
  if (instance && !stopped) await props.runtime.setBounds(instance.instanceId, bounds());
}

async function start() {
  try {
    await nextTick();
    instance = await props.runtime.start(props.pluginId, props.featureCode, bounds());
    emit("started", instance);
    await props.runtime.emit(instance.instanceId, "plugin.enter", {
      featureCode: props.featureCode,
      payload: props.payload,
      source: props.source,
      instanceId: instance.instanceId,
      hostVersion: props.hostVersion,
    });
    await props.runtime.setVisible(instance.instanceId, props.visible);
  } catch (error) {
    emit("error", error instanceof Error ? error.message : String(error));
  }
}

watch(() => props.visible, async (visible) => {
  if (instance && !stopped) await props.runtime.setVisible(instance.instanceId, visible);
});

onMounted(() => {
  observer = new ResizeObserver(() => { void updateBounds(); });
  if (viewport.value) observer.observe(viewport.value);
  void start();
});

onUnmounted(() => {
  stopped = true;
  observer?.disconnect();
  if (instance) void props.runtime.destroy(instance.instanceId, "navigation");
});
</script>

<template>
  <div ref="viewport" class="plugin-viewport" aria-label="插件运行区域">
    <div class="plugin-viewport-loading">正在启动插件…</div>
  </div>
</template>

<style scoped>
.plugin-viewport { position: relative; width: 100%; height: 100%; min-width: 1px; min-height: 1px; overflow: hidden; background: var(--panel-bg, #fff); }
.plugin-viewport-loading { display: grid; height: 100%; place-items: center; color: var(--muted, #748096); font-size: 13px; }
</style>
