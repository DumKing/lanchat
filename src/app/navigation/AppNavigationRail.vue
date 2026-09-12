<script setup lang="ts">
import { NAvatar, NLayoutSider, NTooltip } from "naive-ui";
import { t } from "../../i18n";

export type AppNavigationSection = "chat" | "devices" | "games" | "alerts" | "settings";

defineProps<{
  expanded: boolean;
  activeSection: string;
  profileNickname: string;
  profileAvatarSrc: string | null;
  profileAvatarLabel: string;
  totalUnread: number;
  gamesAvailable: boolean;
  gameAttentionCount: number;
  showGameAttention: boolean;
  petAlertEnabled: boolean;
  pendingNotificationCount: number;
  updateAvailable: boolean;
  updateBadgeLabel: string;
}>();

const emit = defineEmits<{
  select: [section: AppNavigationSection];
  toggle: [];
  openNotificationHistory: [];
}>();
</script>

<template>
  <NLayoutSider class="rail" :class="{ expanded }" :width="expanded ? 176 : 64" bordered>
    <div class="rail-inner">
      <button class="rail-action profile-entry" title="个人资料" @click="emit('select', 'settings')">
        <img v-if="profileAvatarSrc" class="avatar-image self-avatar" :src="profileAvatarSrc" alt="本机头像" />
        <NAvatar v-else class="self-avatar">{{ profileAvatarLabel }}</NAvatar>
        <span v-if="expanded" class="nav-label">{{ profileNickname }}</span>
      </button>
      <button class="rail-collapse-toggle" :title="expanded ? '收起侧栏' : '展开侧栏'" @click="emit('toggle')">
        {{ expanded ? "‹" : "›" }}
      </button>
      <button class="rail-action" :class="{ active: activeSection === 'chat' }" :title="t('nav.chat')" @click="emit('select', 'chat')">
        <span class="nav-icon">💬</span>
        <span v-if="expanded" class="nav-label">{{ t("nav.chat") }}</span>
        <span v-if="totalUnread > 0" class="nav-unread">{{ totalUnread > 99 ? "99+" : totalUnread }}</span>
      </button>
      <button class="rail-action" :class="{ active: activeSection === 'devices' }" :title="t('nav.devices')" @click="emit('select', 'devices')">
        <span class="nav-icon">🖥</span>
        <span v-if="expanded" class="nav-label">{{ t("nav.devices") }}</span>
      </button>
      <button v-if="gamesAvailable" class="rail-action" :class="{ active: activeSection === 'games' }" :title="t('nav.games')" @click="emit('select', 'games')">
        <span class="nav-icon">🎮</span>
        <span v-if="expanded" class="nav-label">{{ t("nav.games") }}</span>
        <span v-if="showGameAttention" class="nav-unread">{{ gameAttentionCount > 9 ? "9+" : gameAttentionCount }}</span>
      </button>
      <button v-if="petAlertEnabled" class="rail-action" :class="{ active: activeSection === 'alerts' }" :title="t('nav.alerts')" @click="emit('select', 'alerts')">
        <span class="nav-icon">🐸</span>
        <span v-if="expanded" class="nav-label">{{ t("nav.alerts") }}</span>
      </button>
      <button class="rail-action add" title="添加设备" @click="emit('select', 'devices')">
        <span class="nav-icon">＋</span>
        <span v-if="expanded" class="nav-label">添加设备</span>
      </button>
      <div class="rail-spacer"></div>
      <NTooltip trigger="hover" placement="right">
        <template #trigger>
          <button class="rail-action rail-notification-bell" title="历史公告" @click="emit('openNotificationHistory')">
            <span class="nav-icon nav-bell-icon" aria-hidden="true"><svg viewBox="0 0 24 24" focusable="false"><path d="M18 10a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 22h4" /></svg></span>
            <span v-if="expanded" class="nav-label">{{ t("nav.notifications") }}</span>
            <span v-if="pendingNotificationCount > 0" class="nav-notification-dot"></span>
          </button>
        </template>
        历史公告
      </NTooltip>
      <NTooltip trigger="hover" placement="right">
        <template #trigger>
          <button class="rail-action" :class="{ active: activeSection === 'settings' }" :title="t('nav.settings')" @click="emit('select', 'settings')">
            <span class="nav-icon">⚙</span>
            <span v-if="expanded" class="nav-label">{{ t("nav.settings") }}</span>
            <span v-if="updateAvailable" class="nav-upgrade-badge">{{ updateBadgeLabel }}</span>
          </button>
        </template>
        设置
      </NTooltip>
    </div>
  </NLayoutSider>
</template>
