<script setup lang="ts">
import { computed } from "vue";
import { NButton, NCard, NTag } from "naive-ui";
import type { FaceMonitorPolicy, FaceMonitorRuntimeStatus } from "../types/face-monitor";
import type { VisionRuntimeSnapshot } from "../types/vision";
import { t } from "../i18n";

const props = defineProps<{
  status: FaceMonitorRuntimeStatus | null;
  policy: FaceMonitorPolicy | null;
  snapshot?: VisionRuntimeSnapshot | null;
}>();

const emit = defineEmits<{ configure: [] }>();

const activeProfile = computed(() => props.snapshot?.activeProfileId || "baseline");
const activeVersion = computed(() => props.snapshot?.activeProfileVersion || props.status?.modelVersion || "-");
</script>

<template>
  <NCard class="vision-model-center" size="small" :title="t('vision.workspace.title')">
    <template #header-extra>
      <NTag size="small" :bordered="false" :type="status?.modelReady ? 'success' : 'warning'">
        {{ status?.modelReady ? t('vision.model.ready') : t('vision.model.unavailable') }}
      </NTag>
    </template>
    <p class="vision-model-intro">{{ t('vision.workspace.description') }}</p>
    <div class="vision-profile-list">
      <article class="vision-profile" :class="{ active: activeProfile === 'baseline' }">
        <div>
          <strong>{{ t('vision.profile.baseline') }}</strong>
          <span>{{ t('vision.profile.balanced.description') }}</span>
        </div>
        <NTag size="small" :bordered="false" :type="activeProfile === 'baseline' ? 'success' : 'default'">{{ activeVersion }}</NTag>
      </article>
      <article class="vision-profile">
        <div>
          <strong>{{ t('vision.profile.low_resource') }}</strong>
          <span>{{ t('vision.profile.low_resource.description') }}</span>
        </div>
        <NTag size="small" :bordered="false">{{ t('vision.profile.available_soon') }}</NTag>
      </article>
    </div>
    <div class="vision-model-meta">
      <span>{{ t('vision.model.policy') }}{{ policy?.version ?? '-' }}</span>
      <span>{{ t('vision.model.compatibility') }}{{ status?.modelAssetsReady ? t('vision.model.compatible') : t('vision.model.needs_check') }}</span>
    </div>
    <NButton size="small" secondary type="primary" @click="emit('configure')">{{ t('vision.workspace.configure') }}</NButton>
  </NCard>
</template>

<style scoped>
.vision-model-center { height: 100%; }
.vision-model-intro { margin: 0 0 12px; color: var(--n-text-color-3); font-size: 13px; line-height: 1.6; }
.vision-profile-list { display: grid; gap: 8px; }
.vision-profile { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 56px; padding: 10px 12px; border: 1px solid var(--n-border-color); border-radius: 7px; }
.vision-profile.active { border-color: var(--n-primary-color); background: color-mix(in srgb, var(--n-primary-color) 7%, transparent); }
.vision-profile strong, .vision-profile span { display: block; }
.vision-profile strong { font-size: 13px; }
.vision-profile span { margin-top: 3px; color: var(--n-text-color-3); font-size: 11px; }
.vision-model-meta { display: flex; flex-wrap: wrap; gap: 10px; margin: 12px 0; color: var(--n-text-color-3); font-size: 12px; }
</style>
