<script setup lang="ts">
import { computed } from "vue";
import { NButton, NCard, NTag } from "naive-ui";
import type { FaceMonitorRuntimeStatus } from "../types/face-monitor";
import type { VisionRuntimeDiagnostics, VisionRuntimeSnapshot } from "../types/vision";
import { t } from "../i18n";

const props = defineProps<{
  status: FaceMonitorRuntimeStatus | null;
  diagnostics?: VisionRuntimeDiagnostics | null;
  snapshot?: VisionRuntimeSnapshot | null;
}>();

const emit = defineEmits<{
  refresh: [];
}>();

const runtimeLabel = computed(() => {
  if (props.snapshot?.sampling === "pausedByUser") return t("vision.runtime.paused");
  if (props.snapshot?.lifecycle === "rebuildingSession") return t("vision.runtime.recovering");
  if (props.snapshot?.performance === "degraded") return t("vision.runtime.degraded");
  if (!props.status?.enabled) return t("vision.runtime.paused");
  if (!props.status?.modelReady) return t("vision.runtime.starting");
  if (props.status.queueBusy) return t("vision.runtime.busy");
  return t("vision.runtime.running");
});

const runtimeType = computed(() => {
  if (props.snapshot?.lifecycle === "rebuildingSession") return "error";
  if (props.snapshot?.performance === "degraded") return "warning";
  if (!props.status?.enabled) return "default";
  if (!props.status?.modelReady) return "warning";
  return props.status.queueBusy ? "warning" : "success";
});
</script>

<template>
  <NCard class="vision-runtime-status" size="small" :title="t('vision.runtime.title')">
    <div class="vision-runtime-main">
      <div>
        <strong>{{ runtimeLabel }}</strong>
        <p>{{ snapshot?.reasonCode || status?.lastError || t('vision.runtime.hint') }}</p>
      </div>
      <NTag size="small" :bordered="false" :type="runtimeType">{{ runtimeLabel }}</NTag>
    </div>
    <div class="vision-runtime-metrics">
      <span>{{ t('vision.runtime.frames') }}<b>{{ diagnostics?.acceptedFrames ?? status?.acceptedFrames ?? 0 }}</b></span>
      <span>{{ t('vision.runtime.dropped') }}<b>{{ diagnostics?.droppedFrames ?? status?.droppedFrames ?? 0 }}</b></span>
      <span>{{ t('vision.runtime.model') }}<b>{{ snapshot?.activeProfileVersion || status?.modelVersion || '-' }}</b></span>
    </div>
    <div v-if="diagnostics" class="vision-runtime-detail">
      {{ t('vision.runtime.latency', { p50: diagnostics.p50ProcessingMs, p95: diagnostics.p95ProcessingMs }) }}
      · {{ t('vision.runtime.queue', { depth: diagnostics.workerQueueDepth }) }}
    </div>
    <div class="vision-runtime-actions">
      <NButton size="small" secondary @click="emit('refresh')">{{ t('common.refresh') }}</NButton>
    </div>
  </NCard>
</template>

<style scoped>
.vision-runtime-status { height: 100%; }
.vision-runtime-main { display: flex; justify-content: space-between; gap: 12px; align-items: flex-start; }
.vision-runtime-main strong { display: block; font-size: 15px; }
.vision-runtime-main p { margin: 6px 0 0; color: var(--n-text-color-3); font-size: 12px; line-height: 1.55; }
.vision-runtime-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-top: 14px; }
.vision-runtime-metrics span { display: flex; flex-direction: column; gap: 4px; padding: 8px; border-radius: 6px; background: var(--n-color-embedded); color: var(--n-text-color-3); font-size: 11px; }
.vision-runtime-metrics b { overflow: hidden; color: var(--n-text-color); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.vision-runtime-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.vision-runtime-detail { margin-top: 9px; color: var(--n-text-color-3); font-size: 11px; }
</style>
