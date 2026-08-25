<script setup lang="ts">
import { NButton, NCard, NTag } from "naive-ui";
import type { FaceMonitorPolicy, FaceMonitorRuntimeStatus } from "../types/face-monitor";
import type { VisionProfileSummary } from "../types/vision";
import { t } from "../i18n";

const { status, policy, profiles, catalogLoading, installingKey } = defineProps<{
  status: FaceMonitorRuntimeStatus | null;
  policy: FaceMonitorPolicy | null;
  profiles: VisionProfileSummary[];
  catalogLoading?: boolean;
  installingKey?: string;
}>();

const emit = defineEmits<{
  refreshCatalog: [];
  install: [profile: VisionProfileSummary];
  activate: [profile: VisionProfileSummary];
  uninstall: [profile: VisionProfileSummary];
}>();

const profileKey = (profile: VisionProfileSummary) => `${profile.profileId}@${profile.profileVersion}`;
const isLowResource = (profile: VisionProfileSummary) => profile.tier === "lightweight" || String(profile.tier) === "low_resource";
const profileDescription = (profile: VisionProfileSummary) => isLowResource(profile)
  ? "低资源优先，适合普通办公笔记本。"
  : profile.tier === "experimental" ? "实验性模型，需要额外验证。" : "兼顾识别质量与运行负载。";
const recommendedSettings = (profile: VisionProfileSummary) => {
  const settings = profile.recommendedSettings;
  return settings
    ? `建议 ${settings.sampleFps} 帧/秒 · 人脸 ${settings.faceMinConfidence}% · 人体 ${settings.bodyMinConfidence}%`
    : "";
};
const modelStack = (profile: VisionProfileSummary) => {
  const models = [profile.faceEngine, profile.personReIdEngine].filter(Boolean).join(" + ");
  return models ? `${models}${profile.inferenceEngine ? ` / ${profile.inferenceEngine}` : ""}` : "旧版模型包";
};
const formatBytes = (bytes: number) => bytes > 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : bytes > 1024 ? `${Math.ceil(bytes / 1024)} KB` : "待发布";
// `active` 是持久化的下次启动选择；当前进程尚未重启时的运行时快照不能混入列表选中态。
const isSelected = (profile: VisionProfileSummary) => profile.active;
const isBuiltin = (profile: VisionProfileSummary) => profile.profileId === "baseline" && !profile.downloadable;
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
      <article v-for="profile in profiles" :key="profileKey(profile)" class="vision-profile" :class="{ active: isSelected(profile), builtin: isBuiltin(profile) }">
        <div>
          <strong>{{ profile.displayName }}</strong>
          <span>{{ modelStack(profile) }} · {{ isLowResource(profile) ? t('vision.profile.low_resource') : profileDescription(profile) }} {{ profile.packageSizeBytes ? `· ${formatBytes(profile.packageSizeBytes)}` : '' }} {{ recommendedSettings(profile) }}</span>
        </div>
        <div class="vision-profile-action">
          <NTag size="small" :bordered="false" :type="isSelected(profile) ? 'success' : !profile.compatible ? 'warning' : profile.installed ? 'info' : 'default'">{{ isSelected(profile) ? `已启用 · ${profile.profileVersion}` : !profile.compatible ? (profile.compatibilityReason || '待运行时') : profile.installed ? '已安装' : '可下载' }}</NTag>
          <NButton v-if="profile.downloadable && !profile.installed" size="tiny" secondary type="primary" :loading="installingKey === profileKey(profile)" @click="emit('install', profile)">下载</NButton>
          <NButton v-else-if="profile.installed && !isSelected(profile)" size="tiny" secondary type="primary" :disabled="!profile.compatible" @click="emit('activate', profile)">启用</NButton>
          <NButton v-if="profile.installed && !isSelected(profile) && !isBuiltin(profile)" size="tiny" tertiary type="error" @click="emit('uninstall', profile)">卸载</NButton>
        </div>
      </article>
    </div>
    <div class="vision-model-meta">
      <span>{{ t('vision.model.policy') }}{{ policy?.version ?? '-' }}</span>
      <span>{{ t('vision.model.compatibility') }}{{ status?.modelAssetsReady ? t('vision.model.compatible') : t('vision.model.needs_check') }}</span>
    </div>
    <div class="vision-model-actions">
      <NButton size="small" secondary :loading="catalogLoading" @click="emit('refreshCatalog')">检查模型</NButton>
    </div>
  </NCard>
</template>

<style scoped>
.vision-model-center { height: 100%; }
.vision-model-intro { margin: 0 0 12px; color: var(--n-text-color-3); font-size: 13px; line-height: 1.6; }
.vision-profile-list { display: grid; gap: 8px; }
.vision-profile { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 56px; padding: 10px 12px; border: 1px solid var(--n-border-color); border-radius: 7px; }
.vision-profile.active { border-color: transparent; box-shadow: inset 3px 0 0 var(--n-primary-color); background: color-mix(in srgb, var(--n-primary-color) 7%, transparent); }
.vision-profile.builtin:not(.active) { border-color: var(--n-border-color); }
.vision-profile strong, .vision-profile span { display: block; }
.vision-profile strong { font-size: 13px; }
.vision-profile span { margin-top: 3px; color: var(--n-text-color-3); font-size: 11px; }
.vision-model-meta { display: flex; flex-wrap: wrap; gap: 10px; margin: 12px 0; color: var(--n-text-color-3); font-size: 12px; }
.vision-profile-action,.vision-model-actions { display: flex; align-items: center; justify-content: flex-end; gap: 7px; flex-shrink: 0; }
.vision-model-actions { margin-top: 12px; }
</style>
