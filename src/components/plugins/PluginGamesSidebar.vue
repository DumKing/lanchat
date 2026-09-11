<script setup lang="ts">
import { NEmpty, NInput, NLayoutSider, NScrollbar, NTag } from "naive-ui";

export interface PluginGameNavigationItem {
  pluginId: string;
  pluginName: string;
  gameId: string;
  minPlayers: number;
  maxPlayers: number;
}

defineProps<{
  width: number;
  games: PluginGameNavigationItem[];
  activeGameId?: string;
}>();

const emit = defineEmits<{
  select: [gameId: string];
  beginResize: [event: MouseEvent];
}>();
</script>

<template>
  <NLayoutSider class="plugin-games-sidebar" :width="width" bordered>
    <div class="pane-header">
      <div class="pane-title-row"><strong>游戏插件</strong></div>
      <NInput size="small" clearable placeholder="搜索游戏插件" />
    </div>
    <NScrollbar class="list-scroll">
      <div class="section-label">已启用</div>
      <button
        v-for="game in games"
        :key="`${game.pluginId}:${game.gameId}`"
        class="game-list-card"
        :class="{ active: activeGameId === game.gameId }"
        type="button"
        @click="emit('select', game.gameId)"
      >
        <span class="game-list-icon">🎮</span>
        <span class="game-list-copy">
          <strong>{{ game.pluginName }}</strong>
          <small>{{ game.minPlayers }}-{{ game.maxPlayers }} 人 · 房间和规则由插件提供</small>
        </span>
        <NTag size="small" :bordered="false" type="success">插件</NTag>
      </button>
      <NEmpty v-if="games.length === 0" description="尚未启用游戏插件" class="list-empty" />
    </NScrollbar>
    <button class="pane-resize-handle" type="button" aria-label="拖动调整列表宽度" title="拖动调整宽度" @mousedown="emit('beginResize', $event)"></button>
  </NLayoutSider>
</template>

<style scoped>
.plugin-games-sidebar { position: relative; background: var(--list-bg, #f7f9fc); }
.plugin-games-sidebar :deep(.n-layout-sider-scroll-container) { display: flex; min-height: 0; flex-direction: column; }
.pane-header { display: grid; gap: 10px; padding: 16px 14px 12px; }
.pane-title-row { display: flex; align-items: center; justify-content: space-between; }
.list-scroll { min-height: 0; flex: 1; }
.section-label { padding: 10px 14px 8px; color: var(--text-secondary, #748096); font-size: 12px; }
.game-list-card { display: grid; width: calc(100% - 20px); grid-template-columns: 38px minmax(0, 1fr) auto; align-items: center; gap: 10px; margin: 0 10px 8px; padding: 11px; border: 1px solid transparent; border-radius: 10px; background: transparent; color: inherit; text-align: left; cursor: pointer; }
.game-list-card:hover { background: color-mix(in srgb, var(--accent, #1677ff) 8%, transparent); }
.game-list-card.active { border-color: color-mix(in srgb, var(--accent, #1677ff) 36%, transparent); background: color-mix(in srgb, var(--accent, #1677ff) 12%, transparent); }
.game-list-icon { display: grid; width: 38px; height: 38px; place-items: center; border-radius: 10px; background: color-mix(in srgb, var(--accent, #1677ff) 10%, #fff); }
.game-list-copy { display: grid; min-width: 0; gap: 3px; }
.game-list-copy strong, .game-list-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.game-list-copy small { color: var(--text-secondary, #748096); font-size: 11px; }
.list-empty { margin-top: 28px; }
.pane-resize-handle { position: absolute; z-index: 2; top: 0; right: -3px; width: 6px; height: 100%; border: 0; background: transparent; cursor: col-resize; }
</style>
