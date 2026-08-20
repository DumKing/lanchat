<script setup lang="ts">
import { NAvatar, NButton, NCard, NEmpty, NTag } from "naive-ui";
import type { FacePersonPolicy } from "../types/face-monitor";
import { t } from "../i18n";

defineProps<{ people: FacePersonPolicy[] }>();
const emit = defineEmits<{
  add: [];
  detail: [person: FacePersonPolicy];
  remove: [person: FacePersonPolicy];
}>();

function initials(name: string) {
  return name.trim().slice(0, 1).toUpperCase() || "?";
}
</script>

<template>
  <NCard class="vision-people-panel" size="small" :title="t('vision.people.title')">
    <template #header-extra>
      <NButton size="small" type="primary" @click="emit('add')">{{ t('vision.people.add') }}</NButton>
    </template>
    <p class="vision-people-hint">{{ t('vision.people.hint') }}</p>
    <NEmpty v-if="people.length === 0" size="small" :description="t('vision.people.empty')" />
    <div v-else class="vision-people-list">
      <article v-for="person in people" :key="person.personId" class="vision-person-row">
        <NAvatar :size="32">{{ initials(person.displayName) }}</NAvatar>
        <div class="vision-person-main">
          <strong>{{ person.displayName }}</strong>
          <span>{{ t('vision.people.samples', { count: Math.max(0, person.sampleCount ?? 0) }) }}</span>
        </div>
        <NTag size="small" :bordered="false" :type="person.enabled && !person.deletedAt ? 'success' : 'default'">
          {{ person.enabled && !person.deletedAt ? t('vision.people.active') : t('vision.people.disabled') }}
        </NTag>
        <div class="vision-person-actions">
          <NButton size="tiny" quaternary @click="emit('detail', person)">{{ t('common.view') }}</NButton>
          <NButton size="tiny" quaternary type="error" @click="emit('remove', person)">{{ t('common.delete') }}</NButton>
        </div>
      </article>
    </div>
  </NCard>
</template>

<style scoped>
.vision-people-panel { height: 100%; }
.vision-people-hint { margin: 0 0 10px; color: var(--n-text-color-3); font-size: 12px; line-height: 1.55; }
.vision-people-list { display: grid; gap: 5px; }
.vision-person-row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; align-items: center; gap: 8px; padding: 7px 0; border-bottom: 1px solid var(--n-divider-color); }
.vision-person-row:last-child { border-bottom: 0; }
.vision-person-main { min-width: 0; }
.vision-person-main strong, .vision-person-main span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.vision-person-main strong { font-size: 13px; }
.vision-person-main span { margin-top: 2px; color: var(--n-text-color-3); font-size: 11px; }
.vision-person-actions { display: flex; gap: 2px; }
</style>
