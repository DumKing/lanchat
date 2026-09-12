<script setup lang="ts">
import PluginViewport from "../components/plugins/PluginViewport.vue";
import type { PluginRuntime } from "../plugin-host/runtime/PluginRuntime";

defineProps<{
  runtime: PluginRuntime;
  pluginId: string;
  gameId: string;
  payload?: unknown;
}>();

const emit = defineEmits<{ error: [message: string] }>();
</script>

<template>
  <section class="game-workspace plugin-game-workspace">
    <PluginViewport
      :key="pluginId"
      :runtime="runtime"
      :plugin-id="pluginId"
      :feature-code="gameId"
      host-version="0.8.0"
      :payload="payload"
      @error="emit('error', $event)"
    />
  </section>
</template>

<style scoped>
.plugin-game-workspace {
  display: block;
  height: 100%;
  min-height: 0;
}
</style>
