<template>
  <main class="lite-frame">
    <header class="titlebar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <span class="app-mark">L</span>
        <strong>LanChat Lite</strong>
        <small>桌宠报警器</small>
      </div>
      <div class="window-actions">
        <button aria-label="最小化" @click="api.minimizeMainWindow">－</button>
        <button aria-label="关闭到托盘" @click="api.hideToTray">×</button>
      </div>
    </header>

    <section class="lite-shell" :class="{ 'nav-collapsed': navCollapsed }">
      <aside class="lite-nav" :class="{ collapsed: navCollapsed }">
        <button class="nav-toggle" title="收起/展开侧栏" @click="navCollapsed = !navCollapsed">
          <span>{{ navCollapsed ? '›' : '‹' }}</span>
          <strong>收起</strong>
        </button>
        <button :class="{ active: activePage === 'leaderboard' }" @click="activePage = 'leaderboard'">
          <span>榜</span>
          <strong>狼来了</strong>
        </button>
        <button :class="{ active: activePage === 'settings' }" @click="activePage = 'settings'">
          <span>设</span>
          <strong>桌宠设置</strong>
        </button>
      </aside>

      <section class="lite-content">
        <div v-if="activePage === 'leaderboard'" class="page">
          <div class="page-head">
            <div>
              <h1>狼来了排行榜</h1>
              <p>根据告警反馈真实度排行，最近 200 条记录参与本地统计。</p>
            </div>
            <button class="primary danger" @click="sendAlert(settings.alertMode)">{{ alertText || "呱呱~呱~~" }}</button>
          </div>

          <section class="panel composer-panel">
            <label class="field">
              <span>告警文案</span>
              <input v-model="alertText" maxlength="60" placeholder="呱呱~呱~~" @change="saveAlertText" />
            </label>
            <div class="segmented">
              <button :class="{ active: settings.alertMode === 'normal' }" @click="updateSettings({ alertMode: 'normal' })">普通报警</button>
              <button :class="{ active: settings.alertMode === 'disco' }" @click="updateSettings({ alertMode: 'disco' })">蹦迪报警</button>
            </div>
          </section>

          <section class="panel">
            <div class="panel-title">
              <strong>排行</strong>
              <span>{{ rankingRows.length }} 人</span>
            </div>
            <div v-if="rankingRows.length === 0" class="empty">暂无告警反馈，收到或发送告警后会出现在这里</div>
            <div v-for="(row, index) in rankingRows" :key="row.deviceId" class="rank-row">
              <span class="rank">{{ index + 1 }}</span>
              <div class="rank-main">
                <strong>{{ row.nickname }}</strong>
                <small>{{ row.total }} 次告警 · {{ row.feedbackTotal }} 次反馈</small>
              </div>
              <div class="score" :class="{ cold: row.probability !== null && row.probability < 60 }">
                {{ row.probability === null ? "100°C" : `${row.probability}%` }}
              </div>
            </div>
          </section>

          <section class="panel">
            <div class="panel-title">
              <strong>最近告警</strong>
              <span>{{ pendingAlerts.length }} 条未处理</span>
            </div>
            <div v-if="alertRecords.length === 0" class="empty">暂无告警</div>
            <div v-for="alert in alertRecords.slice(0, 10)" :key="alert.alertId" class="alert-row" :class="{ pending: alert.incoming && !alert.handled }">
              <div>
                <strong>{{ alert.senderNickname }}</strong>
                <small>{{ formatTime(alert.createdAt) }} · {{ alert.senderAddress || "未知 IP" }}</small>
                <p>{{ alert.content }}</p>
              </div>
              <div class="alert-actions">
                <span>{{ alertScoreLabel(alert) }}</span>
                <template v-if="alert.incoming && !alert.handled && alert.senderDeviceId !== profile?.device_id">
                  <button @click="feedback(alert, 'real')">真实</button>
                  <button class="danger" @click="feedback(alert, 'false')">虚假</button>
                </template>
              </div>
            </div>
          </section>
        </div>

        <div v-else class="page">
          <div class="page-head">
            <div>
              <h1>桌宠设置</h1>
              <p>{{ profile?.nickname || "Lite 用户" }} · {{ profile?.device_id || "读取中" }}</p>
            </div>
            <button class="primary" @click="importPet">导入桌宠</button>
          </div>

          <section class="panel profile-panel">
            <div class="panel-title">
              <strong>本机资料</strong>
              <span>Lite 也会用此昵称发送告警</span>
            </div>
            <div class="profile-row">
              <label class="field">
                <span>昵称</span>
                <input v-model="nicknameDraft" maxlength="24" placeholder="输入昵称" @keydown.enter="saveProfile" />
              </label>
              <button class="primary" :disabled="profileSaving" @click="saveProfile">{{ profileSaving ? "保存中" : "保存昵称" }}</button>
            </div>
            <p class="hint">设备标识：{{ profile?.device_id || "读取中" }}</p>
          </section>

          <section class="panel">
            <div class="panel-title">
              <strong>桌宠资源</strong>
              <div class="actions">
                <button class="ghost" @click="refreshPets">刷新</button>
                <button class="ghost" @click="openPetFolder">目录</button>
              </div>
            </div>
            <div v-if="selectedPet" class="pet-preview-panel">
              <div class="pet-preview-image">
                <img v-if="petHeroPreview(selectedPet)" :src="petHeroPreview(selectedPet)" :alt="selectedPet.manifest.name" />
                <span v-else>{{ firstLetter(selectedPet.manifest.name) }}</span>
              </div>
              <div class="pet-preview-copy">
                <strong>{{ selectedPet.manifest.name }}</strong>
                <span>{{ selectedPet.manifest.description || "当前选中的桌宠资源" }}</span>
                <small>{{ selectedPet.manifest.id }} · {{ selectedPet.manifest.version }}</small>
              </div>
            </div>
            <div v-if="packages.length > 0" class="pet-grid">
              <button
                v-for="pet in packages"
                :key="pet.source + '-' + pet.manifest.id"
                class="pet-item"
                :class="{ active: pet.manifest.id === settings.selectedPetId }"
                @click="selectPet(pet.manifest.id)"
                @contextmenu.prevent="openPetEditor(pet)"
              >
                <img v-if="petIcon(pet)" :src="petIcon(pet)" alt="" />
                <span v-else>{{ firstLetter(pet.manifest.name) }}</span>
                <small>{{ pet.manifest.name }}</small>
                <em>右键设置</em>
              </button>
            </div>
            <div v-else class="empty">尚未发现可用桌宠资源包</div>
            <p v-if="petIssues.length" class="warning">{{ petIssues[0].root }}：{{ petIssues[0].error }}</p>
          </section>

          <section class="panel settings-grid">
            <div class="setting-row">
              <div>
                <strong>启用桌宠</strong>
                <span>关闭后仍可接收告警，但不显示桌面宠物。</span>
              </div>
              <label class="switch"><input v-model="settings.enabled" type="checkbox" @change="saveSettings" /><i></i></label>
            </div>
            <label class="field">
              <span>缩放比例</span>
              <input v-model.number="settings.scale" type="range" min="0.3" max="3" step="0.1" @change="saveSettings" />
              <small>{{ settings.scale.toFixed(1) }}x</small>
            </label>
            <div class="setting-row">
              <div>
                <strong>随机巡逻</strong>
                <span>空闲时低频播放移动动作。</span>
              </div>
              <label class="switch"><input v-model="settings.randomMoveEnabled" type="checkbox" @change="saveSettings" /><i></i></label>
            </div>
            <div class="setting-row">
              <div>
                <strong>随机生活动作</strong>
                <span>空闲时随机播放 Life 动作。</span>
              </div>
              <label class="switch"><input v-model="settings.randomLifeEnabled" type="checkbox" @change="saveSettings" /><i></i></label>
            </div>
            <label class="field">
              <span>蹦迪移动方式</span>
              <select v-model="settings.discoMovementMode" @change="saveSettings">
                <option value="jump">跳跃移动</option>
                <option value="linear">线性移动</option>
              </select>
            </label>
            <label class="field">
              <span>停止快捷键</span>
              <input v-model="settings.stopHotkey" placeholder="Ctrl+Alt+G" @keydown="captureHotkey" @change="saveHotkey" />
              <small>报警/蹦迪时停止；正常时快速发起蹦迪报警。</small>
            </label>
          </section>

          <section class="panel">
            <div class="panel-title">
              <strong>外部推送</strong>
              <label class="switch inline"><input v-model="settings.externalPushEnabled" type="checkbox" @change="saveSettings" /><i></i></label>
            </div>
            <div class="actions push-actions">
              <button class="ghost" @click="addExternalPush('wechat_work')">添加企业微信群</button>
              <button class="ghost" @click="addExternalPush('dingtalk')">添加钉钉群</button>
            </div>
            <p class="hint">只填写推送正文，来源会固定追加在最后一行，格式为 昵称（WLAN IP）。</p>
            <div v-if="settings.externalPushConfigs.length" class="push-list">
              <article v-for="config in settings.externalPushConfigs" :key="config.id" class="push-item">
                <div class="push-head">
                  <input :value="config.name" maxlength="30" @change="updatePush(config.id, { name: inputValue($event) })" />
                  <label class="switch inline"><input :checked="config.enabled" type="checkbox" @change="updatePush(config.id, { enabled: checkedValue($event) })" /><i></i></label>
                </div>
                <select :value="config.kind" @change="updatePush(config.id, { kind: inputValue($event) as ExternalPushKind })">
                  <option value="wechat_work">企业微信群机器人</option>
                  <option value="dingtalk">钉钉群机器人</option>
                </select>
                <input :value="config.webhook" type="password" placeholder="Webhook" @change="updatePush(config.id, { webhook: inputValue($event) })" />
                <textarea :value="config.template" rows="3" placeholder="推送内容" @change="updatePush(config.id, { template: inputValue($event) })"></textarea>
                <div class="push-foot">
                  <label class="check"><input :checked="config.mentionAll" type="checkbox" @change="updatePush(config.id, { mentionAll: checkedValue($event) })" />@所有人</label>
                  <button class="text-danger" @click="removeExternalPush(config.id)">删除</button>
                </div>
              </article>
            </div>
            <div v-else class="empty">还没有外部推送配置</div>
          </section>
        </div>
      </section>
    </section>

    <div v-if="petEditorOpen && petEditorTarget" class="modal-backdrop" @click.self="closePetEditor">
      <section class="pet-editor-modal" role="dialog" aria-modal="true">
        <header>
          <div>
            <strong>动作配置 · {{ petEditorTarget.manifest.name }}</strong>
            <span>{{ petEditorTarget.manifest.id }} · {{ petSourceLabel(petEditorTarget.source) }}</span>
          </div>
          <button class="icon-button" aria-label="关闭" @click="closePetEditor">×</button>
        </header>
        <div class="state-tabs">
          <button
            v-for="state in PET_STATE_ORDER"
            :key="state"
            :class="{ active: petEditorState === state }"
            @click="petEditorState = state"
          >
            {{ PET_STATE_LABELS[state] }}
          </button>
        </div>
        <div class="pet-editor-grid">
          <label class="field">
            <span>单动作最短持续（毫秒）</span>
            <input v-model.number="petPlaybackDraft[petEditorState].minDurationMs" type="number" min="0" max="300000" />
          </label>
          <label class="field">
            <span>单动作最长持续（毫秒）</span>
            <input v-model.number="petPlaybackDraft[petEditorState].maxDurationMs" type="number" min="0" max="300000" />
          </label>
          <label class="field">
            <span>最少随机动作数</span>
            <input v-model.number="petPlaybackDraft[petEditorState].minActionCount" type="number" min="1" max="20" />
          </label>
          <label class="field">
            <span>最多随机动作数</span>
            <input v-model.number="petPlaybackDraft[petEditorState].maxActionCount" type="number" min="1" max="20" />
          </label>
          <label class="field">
            <span>动作间最短停顿（毫秒）</span>
            <input v-model.number="petPlaybackDraft[petEditorState].minIntervalMs" type="number" min="0" max="60000" />
          </label>
          <label class="field">
            <span>动作间最长停顿（毫秒）</span>
            <input v-model.number="petPlaybackDraft[petEditorState].maxIntervalMs" type="number" min="0" max="60000" />
          </label>
        </div>
        <footer>
          <button v-if="petEditorTarget.source === 'user'" class="ghost danger-text" @click="removePet(petEditorTarget)">删除桌宠</button>
          <span v-else class="hint">内置桌宠不能删除，可以调整动作配置。</span>
          <div>
            <button class="ghost" @click="closePetEditor">取消</button>
            <button class="primary" :disabled="petEditorSaving" @click="savePetEditor">{{ petEditorSaving ? "保存中" : "保存并应用" }}</button>
          </div>
        </footer>
      </section>
    </div>
  </main>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { alertTemperature, alertTruthScore, senderCredibility } from "../src/utils/alertCredibility";
import { api } from "../src/services/tauri-api";
import type { DesktopPetPackage, DesktopPetPackageIssue, DesktopPetSettings, ExternalPushConfig, ExternalPushKind, PetPackageSource, PetStateKind, PetStatePlaybackConfig } from "../src/types/desktop-pet";
import type { PetAlertMode, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset } from "../src/types/lanchat";

type PageKey = "leaderboard" | "settings";
type AlertFeedbackResult = "real" | "false";
type AlertFeedbackRecord = {
  responderDeviceId: string;
  responderNickname: string;
  result: AlertFeedbackResult;
  createdAt: number;
};
type AlertRecord = {
  alertId: string;
  senderDeviceId: string;
  senderNickname: string;
  senderAddress?: string | null;
  content: string;
  mode: PetAlertMode;
  createdAt: number;
  incoming: boolean;
  handled: boolean;
  localFeedback?: AlertFeedbackResult;
  feedbacks: AlertFeedbackRecord[];
};

const PET_STATE_ORDER: PetStateKind[] = ["Idle", "Alert", "Move", "Interact", "Life"];
const PET_STATE_LABELS: Record<PetStateKind, string> = {
  Idle: "待机",
  Alert: "告警",
  Move: "移动",
  Interact: "交互",
  Life: "生活",
};
const PET_PLAYBACK_DEFAULTS: Record<PetStateKind, PetStatePlaybackConfig> = {
  Idle: { minDurationMs: 3000, maxDurationMs: 7000, minActionCount: 1, maxActionCount: 2, minIntervalMs: 500, maxIntervalMs: 1200 },
  Alert: { minDurationMs: 2000, maxDurationMs: 4000, minActionCount: 1, maxActionCount: 2, minIntervalMs: 250, maxIntervalMs: 700 },
  Move: { minDurationMs: 1200, maxDurationMs: 2400, minActionCount: 2, maxActionCount: 4, minIntervalMs: 120, maxIntervalMs: 420 },
  Interact: { minDurationMs: 0, maxDurationMs: 0, minActionCount: 1, maxActionCount: 1, minIntervalMs: 0, maxIntervalMs: 0 },
  Life: { minDurationMs: 0, maxDurationMs: 0, minActionCount: 2, maxActionCount: 4, minIntervalMs: 800, maxIntervalMs: 2000 },
};
const QUICK_ALERT_TRUST_RESET_ALL_TARGET = "__all__";
const ALERT_SEND_COOLDOWN_MS = 20_000;
const PET_DISCO_ALERT_DURATION_MS = 60_000;

const activePage = ref<PageKey>("leaderboard");
const navCollapsed = ref(readStored("lanchat-lite-nav-collapsed") === "1");
const profile = ref<Profile | null>(null);
const nicknameDraft = ref("");
const profileSaving = ref(false);
const packages = ref<DesktopPetPackage[]>([]);
const petIssues = ref<DesktopPetPackageIssue[]>([]);
const alertText = ref(readStored("lanchat-pet-alert-text") || "呱呱~呱~~");
const alertRecords = ref<AlertRecord[]>(readAlertRecords());
const nowTick = ref(Date.now());
const petEditorOpen = ref(false);
const petEditorSaving = ref(false);
const petEditorTarget = ref<DesktopPetPackage | null>(null);
const petEditorState = ref<PetStateKind>("Idle");
const ownAlertFlashUntil = ref(0);
const lastOwnAlertSentAt = ref(0);
const discoModeUntil = ref(0);
const visuallyStoppedAlertIds = ref<Set<string>>(new Set());
const petPlaybackDraft = reactive<Record<PetStateKind, PetStatePlaybackConfig>>({
  Idle: { ...PET_PLAYBACK_DEFAULTS.Idle },
  Alert: { ...PET_PLAYBACK_DEFAULTS.Alert },
  Move: { ...PET_PLAYBACK_DEFAULTS.Move },
  Interact: { ...PET_PLAYBACK_DEFAULTS.Interact },
  Life: { ...PET_PLAYBACK_DEFAULTS.Life },
});
const settings = reactive<DesktopPetSettings>({
  enabled: true,
  selectedPetId: null,
  scale: 1,
  positionX: null,
  positionY: null,
  monitorId: null,
  alertMode: "normal",
  stopHotkey: "Ctrl+Alt+G",
  randomMoveEnabled: true,
  randomLifeEnabled: true,
  discoMovementMode: "jump",
  externalPushEnabled: false,
  externalPushConfigs: [],
});

const pendingAlerts = computed(() => alertRecords.value.filter((item) => item.incoming && !item.handled && item.senderDeviceId !== profile.value?.device_id));
const latestPendingAlert = computed(() =>
  [...alertRecords.value]
    .filter((item) => item.incoming && !item.handled && item.senderDeviceId !== profile.value?.device_id)
    .sort((a, b) => b.createdAt - a.createdAt)[0] ?? null,
);
const latestOwnAlert = computed(() =>
  [...alertRecords.value]
    .filter((item) => !item.incoming && item.senderDeviceId === profile.value?.device_id)
    .sort((a, b) => b.createdAt - a.createdAt)[0] ?? null,
);
const activeAlert = computed(() =>
  (latestPendingAlert.value && !visuallyStoppedAlertIds.value.has(latestPendingAlert.value.alertId) ? latestPendingAlert.value : null)
  ?? (latestOwnAlert.value && ownAlertFlashUntil.value > 0 && !visuallyStoppedAlertIds.value.has(latestOwnAlert.value.alertId) ? latestOwnAlert.value : null),
);
const discoModeActive = computed(() => discoModeUntil.value > nowTick.value);
const selectedPet = computed(() => packages.value.find((pet) => pet.manifest.id === settings.selectedPetId) ?? packages.value[0] ?? null);
const rankingRows = computed(() => {
  const rows = new Map<string, { deviceId: string; nickname: string; total: number; feedbackTotal: number; real: number; falseCount: number; lastAt: number }>();
  for (const alert of alertRecords.value) {
    const row = rows.get(alert.senderDeviceId) ?? { deviceId: alert.senderDeviceId, nickname: alert.senderNickname, total: 0, feedbackTotal: 0, real: 0, falseCount: 0, lastAt: 0 };
    row.total += 1;
    row.lastAt = Math.max(row.lastAt, alert.createdAt);
    for (const feedback of alert.feedbacks) {
      row.feedbackTotal += 1;
      if (feedback.result === "real") row.real += 1;
      if (feedback.result === "false") row.falseCount += 1;
    }
    rows.set(row.deviceId, row);
  }
  return [...rows.values()]
    .map((row) => ({ ...row, probability: senderCredibility(alertRecords.value, row.deviceId, nowTick.value) }))
    .sort((left, right) => (right.probability ?? -1) - (left.probability ?? -1) || right.feedbackTotal - left.feedbackTotal || right.lastAt - left.lastAt);
});

function readStored(key: string) {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(key);
}

function saveAlertText() {
  window.localStorage.setItem("lanchat-pet-alert-text", alertText.value.trim() || "呱呱~呱~~");
}

function normalizeAlertMode(value: unknown): PetAlertMode {
  return value === "disco" ? "disco" : "normal";
}

function normalizeAlertRecords(records: AlertRecord[]) {
  return records
    .filter((item) => item.alertId && item.senderDeviceId)
    .map((item) => ({
      ...item,
      senderAddress: item.senderAddress ?? null,
      content: item.content || "呱呱~呱~~",
      mode: normalizeAlertMode(item.mode),
      feedbacks: Array.isArray(item.feedbacks) ? item.feedbacks : [],
      handled: Boolean(item.handled),
      incoming: Boolean(item.incoming),
    }))
    .sort((a, b) => b.createdAt - a.createdAt)
    .slice(0, 200);
}

function readAlertRecords() {
  try {
    return normalizeAlertRecords(JSON.parse(readStored("lanchat-pet-alert-records-v1") || "[]") as AlertRecord[]);
  } catch {
    return [];
  }
}

function saveAlertRecords() {
  window.localStorage.setItem("lanchat-pet-alert-records-v1", JSON.stringify(normalizeAlertRecords(alertRecords.value)));
}

function firstLetter(value: string) {
  return value.trim().slice(0, 1).toUpperCase() || "L";
}

function assetUrl(path?: string | null) {
  if (!path) return "";
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

function petHeroPreview(pet: DesktopPetPackage) {
  return assetUrl(pet.preview_path ?? pet.icon_path ?? pet.states.Idle?.[0]?.frames[0]?.path);
}

function petIcon(pet: DesktopPetPackage) {
  return assetUrl(pet.icon_path ?? pet.preview_path ?? pet.states.Idle?.[0]?.frames[0]?.path);
}

function petSourceLabel(source: PetPackageSource) {
  if (source === "built_in") return "内置";
  if (source === "portable") return "绿色版";
  return "用户导入";
}

function petPlaybackConfig(pet: DesktopPetPackage, state: PetStateKind): PetStatePlaybackConfig {
  return { ...PET_PLAYBACK_DEFAULTS[state], ...(pet.manifest.states?.[state] ?? {}) };
}

function openPetEditor(pet: DesktopPetPackage) {
  petEditorTarget.value = pet;
  petEditorState.value = "Idle";
  for (const state of PET_STATE_ORDER) {
    Object.assign(petPlaybackDraft[state], petPlaybackConfig(pet, state));
  }
  petEditorOpen.value = true;
}

function closePetEditor() {
  petEditorOpen.value = false;
  petEditorTarget.value = null;
}

async function savePetEditor() {
  if (!petEditorTarget.value) return;
  petEditorSaving.value = true;
  try {
    const configs = Object.fromEntries(PET_STATE_ORDER.map((state) => [state, { ...petPlaybackDraft[state] }])) as Record<PetStateKind, PetStatePlaybackConfig>;
    await api.updateDesktopPetPlaybackConfig(petEditorTarget.value.manifest.id, configs);
    await refreshPets();
    closePetEditor();
  } finally {
    petEditorSaving.value = false;
  }
}

async function removePet(pet: DesktopPetPackage) {
  if (pet.source !== "user") return;
  if (!window.confirm(`确定删除桌宠“${pet.manifest.name}”吗？`)) return;
  await api.removeDesktopPet(pet.manifest.id);
  await refreshPets();
  applySettings(await api.getDesktopPetSettings());
  closePetEditor();
  syncPet();
}

function applySettings(next: DesktopPetSettings) {
  Object.assign(settings, next, {
    enabled: next.enabled,
    scale: Number.isFinite(next.scale) ? next.scale : 1,
    externalPushConfigs: next.externalPushConfigs ?? [],
  });
}

async function refreshPets() {
  const registry = await api.refreshDesktopPets();
  packages.value = registry.packages;
  petIssues.value = registry.issues;
}

async function load() {
  profile.value = await api.getProfile();
  nicknameDraft.value = profile.value.nickname;
  const [registry, nextSettings] = await Promise.all([api.listDesktopPets(), api.getDesktopPetSettings()]);
  packages.value = registry.packages;
  petIssues.value = registry.issues;
  applySettings(nextSettings);
  await saveHotkey();
  syncPet();
}

async function saveProfile() {
  const nickname = nicknameDraft.value.trim() || profile.value?.nickname || "Lite 用户";
  const listenPort = profile.value?.listen_port ?? 18145;
  profileSaving.value = true;
  try {
    profile.value = await api.updateProfile(nickname, listenPort, profile.value?.avatar ?? null);
    nicknameDraft.value = profile.value.nickname;
  } finally {
    profileSaving.value = false;
  }
}

async function saveSettings() {
  applySettings(await api.updateDesktopPetSettings({ ...settings }));
  syncPet();
}

async function updateSettings(patch: Partial<DesktopPetSettings>) {
  Object.assign(settings, patch);
  await saveSettings();
}

async function saveHotkey() {
  await api.registerDesktopPetStopHotkey(settings.stopHotkey || "");
  await saveSettings();
}

async function importPet() {
  const selected = await open({ directory: true, multiple: false, title: "选择桌宠资源目录" });
  if (typeof selected !== "string") return;
  const pet = await api.importDesktopPet(selected);
  await selectPet(pet.manifest.id);
}

async function selectPet(id: string) {
  applySettings(await api.selectDesktopPet(id));
  await refreshPets();
  syncPet();
}

async function openPetFolder() {
  await api.openDesktopPetFolder();
}

function createExternalPush(kind: ExternalPushKind): ExternalPushConfig {
  return {
    id: crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    name: kind === "dingtalk" ? "钉钉群" : "企业微信群",
    kind,
    webhook: "",
    enabled: true,
    mentionAll: false,
    template: "",
  };
}

async function addExternalPush(kind: ExternalPushKind) {
  settings.externalPushEnabled = true;
  settings.externalPushConfigs = [...settings.externalPushConfigs, createExternalPush(kind)];
  await saveSettings();
}

async function updatePush(id: string, patch: Partial<ExternalPushConfig>) {
  settings.externalPushConfigs = settings.externalPushConfigs.map((item) => item.id === id ? { ...item, ...patch } : item);
  await saveSettings();
}

async function removeExternalPush(id: string) {
  settings.externalPushConfigs = settings.externalPushConfigs.filter((item) => item.id !== id);
  await saveSettings();
}

function inputValue(event: Event) {
  return (event.target as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
}

function checkedValue(event: Event) {
  return (event.target as HTMLInputElement).checked;
}

function hotkeyFromEvent(event: KeyboardEvent) {
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return "";
  return [event.ctrlKey ? "Ctrl" : "", event.altKey ? "Alt" : "", event.shiftKey ? "Shift" : "", event.metaKey ? "Meta" : "", key].filter(Boolean).join("+");
}

function captureHotkey(event: KeyboardEvent) {
  const hotkey = hotkeyFromEvent(event);
  if (!hotkey) return;
  event.preventDefault();
  settings.stopHotkey = hotkey;
  void saveHotkey();
}

function alertRecordFromFrame(alert: QuickAlert): AlertRecord {
  const incoming = alert.sender_device_id !== profile.value?.device_id;
  return {
    alertId: alert.alert_id,
    senderDeviceId: alert.sender_device_id,
    senderNickname: alert.sender_nickname,
    senderAddress: alert.sender_address ?? null,
    content: alert.content || "呱呱~呱~~",
    mode: normalizeAlertMode(alert.mode),
    createdAt: alert.created_at,
    incoming,
    handled: !incoming,
    feedbacks: [],
  };
}

function applyAlert(alert: QuickAlert) {
  const nextStopped = new Set(visuallyStoppedAlertIds.value);
  nextStopped.delete(alert.alert_id);
  visuallyStoppedAlertIds.value = nextStopped;
  const current = alertRecords.value.find((item) => item.alertId === alert.alert_id);
  if (current) {
    alertRecords.value = normalizeAlertRecords(alertRecords.value.map((item) =>
      item.alertId === alert.alert_id
        ? {
            ...item,
            senderNickname: alert.sender_nickname,
            senderAddress: alert.sender_address ?? item.senderAddress ?? null,
            content: alert.content || item.content,
            mode: normalizeAlertMode(alert.mode || item.mode),
            createdAt: alert.created_at || item.createdAt,
          }
        : item,
    ));
    syncPet();
    return;
  } else {
    alertRecords.value = normalizeAlertRecords([alertRecordFromFrame(alert), ...alertRecords.value]);
  }
  if (normalizeAlertMode(alert.mode) === "disco") {
    discoModeUntil.value = Math.max(discoModeUntil.value, Date.now() + PET_DISCO_ALERT_DURATION_MS);
  }
  syncPet();
}

function applyFeedback(feedback: QuickAlertFeedback) {
  const result = feedback.result === "real" ? "real" : "false";
  alertRecords.value = normalizeAlertRecords(alertRecords.value.map((alert) => {
    if (alert.alertId !== feedback.alert_id) return alert;
    const nextFeedbacks = alert.feedbacks.filter((item) => item.responderDeviceId !== feedback.responder_device_id);
    nextFeedbacks.push({
      responderDeviceId: feedback.responder_device_id,
      responderNickname: feedback.responder_nickname,
      result,
      createdAt: feedback.created_at,
    });
    return { ...alert, feedbacks: nextFeedbacks };
  }));
  syncPet();
}

function applyTrustReset(reset: QuickAlertTrustReset) {
  if (reset.target_device_id === QUICK_ALERT_TRUST_RESET_ALL_TARGET) {
    alertRecords.value = [];
    visuallyStoppedAlertIds.value = new Set();
    ownAlertFlashUntil.value = 0;
    discoModeUntil.value = 0;
    syncPet(false);
    return;
  }
  alertRecords.value = normalizeAlertRecords(alertRecords.value.filter((alert) => alert.senderDeviceId !== reset.target_device_id));
  syncPet();
}

async function sendAlert(mode: string) {
  saveAlertText();
  const now = Date.now();
  if (now - lastOwnAlertSentAt.value < ALERT_SEND_COOLDOWN_MS) return;
  const sent = await api.sendQuickAlert(alertText.value.trim() || "呱呱~呱~~", normalizeAlertMode(mode));
  applyAlert(sent);
  lastOwnAlertSentAt.value = now;
  ownAlertFlashUntil.value = Date.now();
  syncPet();
}

async function feedback(alert: AlertRecord, result: AlertFeedbackResult) {
  alertRecords.value = alertRecords.value.map((item) => item.alertId === alert.alertId ? { ...item, handled: true, localFeedback: result } : item);
  const feedbackFrame = await api.sendQuickAlertFeedback(alert.alertId, alert.senderDeviceId, result);
  applyFeedback(feedbackFrame);
}

function stopVisuals() {
  if (activeAlert.value) {
    visuallyStoppedAlertIds.value = new Set([...visuallyStoppedAlertIds.value, activeAlert.value.alertId]);
  }
  ownAlertFlashUntil.value = 0;
  lastOwnAlertSentAt.value = 0;
  discoModeUntil.value = 0;
  syncPet(false);
}

function alertDisplayTemperature(alert?: AlertRecord | null) {
  if (!alert) return 0;
  return alertTemperature(senderCredibility(alertRecords.value, alert.senderDeviceId, nowTick.value));
}

function alertScoreLabel(alert: AlertRecord) {
  const score = alertTruthScore(alert, nowTick.value);
  return score.feedbackCount === 0 ? `${alertDisplayTemperature(alert)}°C` : `${score.probability}%`;
}

function formatTime(value: number) {
  return new Date(value).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}

function syncPet(flashing = Boolean(activeAlert.value)) {
  const latest = activeAlert.value;
  api.updateDesktopPetState({
    enabled: settings.enabled,
    pending_count: pendingAlerts.value.length,
    temperature: alertDisplayTemperature(latest),
    latest_alert_id: latest?.alertId ?? null,
    latest_sender: latest?.senderNickname ?? null,
    latest_sender_address: latest?.senderAddress ?? null,
    latest_content: latest?.content ?? null,
    latest_created_at: latest?.createdAt ?? null,
    feedbackable: Boolean(latestPendingAlert.value),
    flashing,
    disco: discoModeActive.value && Boolean(latest),
    theme_accent: "#159A8C",
    random_move_enabled: settings.randomMoveEnabled,
    random_life_enabled: settings.randomLifeEnabled,
    disco_movement_mode: settings.discoMovementMode,
  }).catch(() => {});
}

watch(alertRecords, saveAlertRecords, { deep: true });
watch([activeAlert, discoModeActive, pendingAlerts], () => {
  syncPet();
});
watch(navCollapsed, (collapsed) => {
  window.localStorage.setItem("lanchat-lite-nav-collapsed", collapsed ? "1" : "0");
});
setInterval(() => {
  nowTick.value = Date.now();
}, 1000);

onMounted(async () => {
  try {
    await load();
  } catch (error) {
    console.error("lite app initialization failed", error);
  }
  try {
    await listen<QuickAlert>("quick_alert_received", (event) => applyAlert(event.payload));
    await listen<QuickAlertFeedback>("quick_alert_feedback_received", (event) => applyFeedback(event.payload));
    await listen<QuickAlertTrustReset>("quick_alert_trust_reset_received", (event) => applyTrustReset(event.payload));
    await listen<{ action: string; alert_id?: string | null }>("desktop_pet_action", async (event) => {
    try {
      if (event.payload.action === "quick_alert") {
        await sendAlert(settings.alertMode);
      } else if (event.payload.action === "broadcast_disco_alert") {
        await sendAlert("disco");
      } else if (event.payload.action === "stop_visuals") {
        stopVisuals();
      } else if (event.payload.action === "feedback_real" || event.payload.action === "feedback_false") {
        const target = alertRecords.value.find((item) => item.alertId === event.payload.alert_id) ?? latestPendingAlert.value;
        if (target) await feedback(target, event.payload.action === "feedback_real" ? "real" : "false");
      } else if (event.payload.action === "configure_pet" || event.payload.action === "open_main_window") {
        activePage.value = "settings";
        await api.showFromTray();
      }
    } catch (error) {
      console.error("desktop pet action failed", error);
    }
    });
    await listen("desktop_pet_stop_hotkey_received", async () => {
      if (activeAlert.value || discoModeActive.value) stopVisuals();
      else await sendAlert("disco");
    });
    await listen<{ packages: DesktopPetPackage[]; issues: DesktopPetPackageIssue[] }>("desktop_pet_registry_changed", (event) => {
      packages.value = event.payload.packages;
      petIssues.value = event.payload.issues;
    });
  } catch (error) {
    console.error("lite event registration failed", error);
  }
});
</script>
