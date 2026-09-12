<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from "@tauri-apps/plugin-autostart";
import { check as checkNativeUpdate } from "@tauri-apps/plugin-updater";
import CryptoJS from "crypto-js";
import {
  NAlert,
  NAvatar,
  NBadge,
  NButton,
  NCard,
  NCheckbox,
  NConfigProvider,
  NDropdown,
  NEmpty,
  NFormItem,
  NInput,
  NInputNumber,
  NLayout,
  NLayoutSider,
  NList,
  NListItem,
  NMessageProvider,
  NModal,
  NPagination,
  NProgress,
  NRadioButton,
  NRadioGroup,
  NScrollbar,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  NText,
  NThing,
  NTooltip,
} from "naive-ui";
import { storeToRefs } from "pinia";
import { api } from "../services/tauri-api";
import { pluginApi } from "../services/plugin-api";
import { callMediaCoordinator } from "../services/callMediaCoordinator";
import ChatComposerInput from "../components/ChatComposerInput.vue";
import PluginCenterPage from "../pages/PluginCenterPage.vue";
import PluginGamesSidebar from "../components/plugins/PluginGamesSidebar.vue";
import GamesPage from "../pages/GamesPage.vue";
import AlertsPage from "../pages/AlertsPage.vue";
import DevicesPage from "../pages/DevicesPage.vue";
import BasicSystemSettings from "../pages/settings/BasicSystemSettings.vue";
import AppNavigationRail from "./navigation/AppNavigationRail.vue";
import { DEFAULT_GROUP_ID, useLanChatStore } from "../stores/lanchat";
import { useDesktopPetStore } from "../stores/desktopPet";
import type { DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings, ExternalPushConfig, ExternalPushKind, PetPackageSource, PetStateKind, PetStatePlaybackConfig } from "../types/desktop-pet";
import type { AdminAlertMode, AdminAlertPushPolicy, AdminDiscoMode, AdminNotification, AdminRemoteUpdate, AdminRemoteUpdateDispatchTarget, AdminRemoteUpdateProgress, AppVersionInfo, CallSignal, ChannelMember, Conversation, DesktopPetRuntimeState, Message, Nudge, Peer, PetAlertMode, PlatformInfo, PreviewMediaCacheInfo, PrivateChannelInvitePayload, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset, SimulationMeta, TrayAttentionItem, UpdateCheckResult, UpdateGithubTokenInfo } from "../types/lanchat";
import { alertTemperature, alertTruthScore, senderCredibility } from "../utils/alertCredibility";
import { detectMentionKind, trayConversationTitle, type MentionKind } from "../utils/messageMentions";
import { peerDisplayName, peerOriginalName, sameDeviceId, sortPeersForDisplay } from "../utils/peerPresentation";
import { dateLocale, effectiveLocale, installUiTranslation, languagePreference, naiveLocale, setLanguagePreference, t } from "../i18n";
import type { PluginManifestV1 } from "../plugin-host/contracts/manifest";
import { resolvePluginFeatures } from "../plugin-host/registry/featureAvailability";
import { PluginRuntime } from "../plugin-host/runtime/PluginRuntime";
import { createTauriPluginRuntimeAdapter } from "../services/plugin-runtime-api";
import { installTauriPluginBridge } from "../plugin-host/runtime/tauriBridge";
import { createPluginHostHandlers } from "../plugin-host/runtime/createHostHandlers";
import { PluginRoomService } from "../plugin-host/services/PluginRoomService";
import { PluginLeaderboardService } from "../plugin-host/services/PluginLeaderboardService";
import type { AlertFeedbackResult, AlertRecord } from "../features/alerts/types";

type UiThemeKey = "theme-dingtalk" | "theme-work" | "theme-lan" | "theme-light";
type MainSection = "chat" | "devices" | "games" | "alerts" | "settings";
type RecipientPickerMode = "privateChannelCreate" | "privateChannelInvite";
type SimulationKind = "direct" | "channel" | "alert" | "disco";
const PRIVATE_CHANNEL_INVITE_PREFIX = "LANCHAT_PRIVATE_CHANNEL_INVITE:";
const DEFAULT_CHANNEL_NOTICE = "欢迎来到频道，公告可以由超管维护。";
const QUICK_ALERT_TRUST_RESET_ALL_TARGET = "__all__";
const store = useLanChatStore();
const desktopPetStore = useDesktopPetStore();
const {
  settings: desktopPetSettings,
  packages: desktopPetPackages,
  issues: desktopPetIssues,
  selectedPackage: selectedDesktopPetPackage,
  loading: desktopPetLoading,
  error: desktopPetError,
} = storeToRefs(desktopPetStore);
const desktopPetPackagesExpanded = ref(true);
const desktopPetManifestEditorOpen = ref(false);
const desktopPetManifestEditorTarget = ref<DesktopPetPackage | null>(null);
const desktopPetPlaybackDraft = ref<Record<PetStateKind, PetStatePlaybackConfig>>({} as Record<PetStateKind, PetStatePlaybackConfig>);
const {
  profile,
  peers,
  conversations,
  activeConversationId,
  activeConversation,
  activeMessages,
  messagesByConversation,
  channelMembersByConversation,
  channelMutedByConversation,
  chatCapablePeers,
  onlinePeers,
  activePeer,
  canSendActive,
  loading,
  error,
  networkRepairing,
  networkRepairStatus,
  debugEnabled,
  debugLogs,
  unreadByConversation,
  totalUnread,
  latestIncomingMessage,
  latestGameFrame,
  latestChannelNotice,
  latestQuickAlert,
  latestQuickAlertFeedback,
  latestQuickAlertTrustReset,
  latestAdminDiscoMode,
  latestAdminAlertMode,
  latestCallSignal,
  latestNudge,
  latestAdminAlertPushPolicy,
  adminNotifications,
  latestAdminRemoteUpdate,
  latestAdminRemoteUpdateProgress,
  manualAddress,
  manualPort,
  draft,
} = storeToRefs(store);
const messagePane = ref<HTMLElement | null>(null);
const mentionPickerOpen = ref(false);
const mentionSearch = ref("");
type MentionNotice = { messageId: string; kind: MentionKind; createdAt: number };
const mentionNoticesByConversation = ref<Record<string, MentionNotice[]>>({});
const highlightedMentionMessageId = ref("");
let mentionHighlightTimer: number | null = null;
const platformInfo = ref<PlatformInfo | null>(null);
const AUTOSTART_INITIALIZED_KEY = "lanchat.autostart-initialized.v1";
const autostartEnabled = ref(true);
const autostartLoading = ref(false);
const autostartError = ref("");
const nicknameDraft = ref("");
const portDraft = ref(18145);
const avatarDraft = ref("");
async function initializeAutostart() {
  try {
    const initialized = window.localStorage.getItem(AUTOSTART_INITIALIZED_KEY) === "1";
    let enabled = await isAutostartEnabled();
    if (!initialized && import.meta.env.PROD) {
      if (!enabled) await enableAutostart();
      window.localStorage.setItem(AUTOSTART_INITIALIZED_KEY, "1");
      enabled = true;
    }
    autostartEnabled.value = enabled;
    autostartError.value = "";
  } catch (error) {
    autostartEnabled.value = false;
    autostartError.value = `读取开机自启状态失败：${stringifyError(error)}`;
  }
}
async function updateAutostart(enabled: boolean) {
  autostartLoading.value = true;
  autostartError.value = "";
  try {
    if (enabled) await enableAutostart();
    else await disableAutostart();
    autostartEnabled.value = await isAutostartEnabled();
    window.localStorage.setItem(AUTOSTART_INITIALIZED_KEY, "1");
  } catch (error) {
    autostartError.value = `修改开机自启失败：${stringifyError(error)}`;
    autostartEnabled.value = await isAutostartEnabled().catch(() => !enabled);
  } finally {
    autostartLoading.value = false;
  }
}
const profileAvatarInput = ref<HTMLInputElement | null>(null);
const adminNotificationImageInput = ref<HTMLInputElement | null>(null);
const adminNotificationHistoryOpen = ref(false);
const adminNotificationReviewOpen = ref(false);
const adminNotificationReviewPage = ref(1);
const ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE = 8;
const AVATAR_MAX_BYTES = 5 * 1024 * 1024;
const canRepairWindowsNetwork = computed(() => platformInfo.value?.windowsFirewallRepairSupported ?? true);
const networkRepairDescription = computed(() => {
  if (canRepairWindowsNetwork.value) {
    return "当 Windows 专用网络或公用网络下发现不到局域网设备时，可一键放行 LanChat 的局域网通信。";
  }
  if (platformInfo.value?.os === "macos") {
    return "macOS 下请在系统设置中允许 LanChat 访问本地网络，并确认防火墙没有阻止传入连接。";
  }
  return "当前平台不支持 Windows 网络修复，请检查系统防火墙和本地网络权限。";
});
const preferredUpdateUrl = computed(() => {
  const info = updateInfo.value;
  if (!info) return "";
  if (platformInfo.value?.os === "windows") {
    return info.downloads.windowsInstaller || info.downloads.windowsPortable || info.downloads.releasePage || info.releaseUrl;
  }
  if (platformInfo.value?.os === "macos") {
    return info.downloads.macosDmg || info.downloads.releasePage || info.releaseUrl;
  }
  return info.downloads.releasePage || info.releaseUrl;
});
const updateStatusLabel = computed(() => {
  const info = updateInfo.value;
  if (updateChecking.value) return "正在检查更新";
  if (!info) return "尚未检查";
  if (info.updateAvailable) return `发现新版本 ${info.latestVersion}`;
  return "已是最新版本";
});
const updateStatusType = computed(() => {
  if (updateInfo.value?.updateAvailable) return "warning";
  return "success";
});
const localVersionLabel = computed(() => appVersionInfo.value?.buildVersion ?? updateInfo.value?.current.buildVersion ?? "未知");
const visibleUpdateAvailable = computed(() => updateInfo.value?.updateAvailable === true);
const updateBadgeLabel = computed(() => (visibleUpdateAvailable.value ? "升级" : ""));
const isRecording = ref(false);
const recordingStartedAt = ref(0);
let mediaRecorder: MediaRecorder | null = null;
let recordingChunks: BlobPart[] = [];
let recordingTimer: number | null = null;
let turnTicker: number | null = null;
let updateCheckTimer: number | null = null;
let stopUiTranslation: (() => void) | null = null;
let unlistenTrayOpenTarget: (() => void) | null = null;
let unlistenDesktopPetAction: (() => void) | null = null;
let unlistenDesktopPetStopHotkey: (() => void) | null = null;
let unlistenDesktopPetSendHotkey: (() => void) | null = null;
let unlistenDesktopPetRegistry: (() => void) | null = null;
let unlistenPluginBridge: (() => void) | null = null;
const desktopPetFeedbackInFlight = new Set<string>();
const conversationSearch = ref("");
const deviceSearch = ref("");
const selectedPeerId = ref("");
const peerNoteDraft = ref("");
const previewMediaPaths = ref<Record<string, string>>({});
const avatarBlobUrls = ref<Record<string, string>>({});
const avatarBlobUrlOrder = new Map<string, number>();
const avatarBlobUrlPending = new Set<string>();
const imagePreviewBlobUrls = new Map<string, string>();
const previewMediaCacheInfo = ref<PreviewMediaCacheInfo | null>(null);
const previewMediaCacheClearing = ref(false);
const imagePreviewMessage = ref<Message | null>(null);
const imagePreviewScale = ref(1);
const VISIBLE_MESSAGE_WINDOW = 60;
const visibleMessageEnd = ref(0);
const hasMoreEarlierMessages = ref(true);
const messagePaneFollowingLatest = ref(true);
const memoryDiagnostic = ref({
  jsHeapBytes: null as number | null,
  jsHeapLimitBytes: null as number | null,
  cachedConversations: 0,
  cachedMessages: 0,
  visibleMessages: 0,
  avatarBytes: 0,
  previewCacheBytes: 0,
});
const memoryDiagnosticConversationRows = computed(() => Object.entries(messagesByConversation.value)
  .map(([conversationId, items]) => ({
    conversationId,
    title: conversations.value.find((conversation) => conversation.id === conversationId)?.title ?? conversationId,
    count: items.length,
  }))
  .sort((left, right) => right.count - left.count)
  .slice(0, 5));
const memoryDiagnosticAvatarRows = computed(() => [
  ...(profile.value ? [{ name: profile.value.nickname || "本机", avatar: profile.value.avatar }] : []),
  ...peers.value.map((peer) => ({ name: peerDisplayName(peer), avatar: peer.avatar })),
]
  .map((entry) => ({ name: entry.name, bytes: base64ByteLength(entry.avatar) }))
  .filter((entry) => entry.bytes > 0)
  .sort((left, right) => right.bytes - left.bytes)
  .slice(0, 5));
const operationNotice = ref("");
const deleteDirectConversationOpen = ref(false);
const pendingDeleteDirectConversation = ref<Conversation | null>(null);
let operationNoticeTimer: ReturnType<typeof setTimeout> | undefined;
let desktopPetRuntimeRevision = 0;
const selectedDeviceChannelId = ref("");
const adminNicknameDraft = ref("");
const adminNicknameLockAfterIssue = ref(false);
const superAdminEnabled = ref(false);
const superAdminTapCount = ref(0);
const superAdminAuthOpen = ref(false);
const superAdminPasswordDraft = ref("");
const superAdminPasswordError = ref("");
const SUPER_ADMIN_PASSWORD_MD5 = "D7B9AF919901FA1598BDC21465E3EB3F";
const alertTrustResetTargetId = ref<string | null>(null);
const adminAlertModeTargetId = ref<string | null>(null);
const adminAlertModeDraft = ref<PetAlertMode>("normal");
const adminAlertPushPolicyTargetId = ref<string | null>("*");
const adminAlertPushPolicyDraft = ref(50);
const adminAlertPushPolicyLockAfterIssue = ref(false);
type CallMedia = "audio" | "video";
type CallSession = { callId: string; peerDeviceId: string; peerNickname: string; media: CallMedia; status: "incoming" | "outgoing" | "connected" | "failed"; error?: string };
type DetachedCallWindow = {
  window: Window;
  title: HTMLElement;
  status: HTMLElement;
  remoteVideo: HTMLVideoElement | null;
  localVideo: HTMLVideoElement | null;
  remoteAudio: HTMLAudioElement | null;
};
const callSession = ref<CallSession | null>(null);
const callPanelExpanded = ref(false);
const callPanelPosition = ref<{ left: number; top: number } | null>(null);
let callPanelDrag: { offsetX: number; offsetY: number; width: number; height: number } | null = null;
const callPanelStyle = computed(() => callPanelPosition.value
  ? { left: `${callPanelPosition.value.left}px`, top: `${callPanelPosition.value.top}px`, right: "auto" }
  : {});
const incomingCallSignal = ref<CallSignal | null>(null);
const localCallVideo = ref<HTMLVideoElement | null>(null);
const remoteCallVideo = ref<HTMLVideoElement | null>(null);
const remoteCallAudio = ref<HTMLAudioElement | null>(null);
const callMuted = ref(false);
const callCameraOn = ref(true);
const callActionInProgress = ref(false);
let callPeerConnection: RTCPeerConnection | null = null;
let callLocalStream: MediaStream | null = null;
let callRemoteStream: MediaStream | null = null;
let detachedCallWindow: DetachedCallWindow | null = null;
let detachedCallWindowUnavailable = false;
let queuedCallCandidates: RTCIceCandidateInit[] = [];
const pendingCallCandidatesById = new Map<string, RTCIceCandidateInit[]>();
let callDisconnectTimer: ReturnType<typeof setTimeout> | undefined;
const simulationModalOpen = ref(false);
const simulationSending = ref(false);
const simulationKind = ref<SimulationKind>("channel");
const simulationTargetId = ref("");
const simulationContent = ref("");
const simulationDisplayLabel = ref(true);
const adminNotificationModalOpen = ref(false);
const adminNotificationSending = ref(false);
const adminNotificationScope = ref<"device" | "all_online">("device");
const adminNotificationTargetId = ref<string | null>(null);
const adminNotificationTitle = ref("通知");
const adminNotificationContent = ref("");
const adminNotificationTemplate = ref("announcement");
const adminNotificationSupportUrl = ref("");
const adminNotificationDisplayMode = ref<"dismissible" | "requires_confirmation">("dismissible");
const adminNotificationDeadline = ref("");
const adminNotificationTimeoutPolicy = ref("manual_review");
const adminNotificationForceOpenMainWindow = ref(false);
const adminNotificationDetail = ref<AdminNotification | null>(null);
const adminNotificationDetailOpen = ref(false);
const adminNotificationBulkProcessing = ref(false);
const dismissedAdminNotificationIds = ref<string[]>(readDismissedAdminNotificationIds());
const appVersionInfo = ref<AppVersionInfo | null>(null);
const updateInfo = ref<UpdateCheckResult | null>(readSavedUpdateInfo());
const updateChecking = ref(false);
const updateError = ref("");
const operationErrorMessage = computed(() => (
  error.value
  || desktopPetError.value
  || autostartError.value
  || updateError.value
));
const updateGithubTokenInfo = ref<UpdateGithubTokenInfo | null>(null);
const updateGithubTokenDraft = ref("");
const updateGithubTokenSaving = ref(false);
const adminRemoteUpdateTargetId = ref<string | null>(null);
const adminRemoteUpdateScope = ref<"device" | "all_online_windows">("device");
const adminRemoteUpdateVersion = ref("");
const adminRemoteUpdatePackagePath = ref("");
const adminRemoteUpdateSignaturePath = ref("");
const adminRemoteUpdateForce = ref(false);
const adminRemoteUpdateSending = ref(false);
const adminRemoteUpdateResults = ref<AdminRemoteUpdateDispatchTarget[]>([]);
const processedRemoteUpdateCommandIds = new Set<string>();
const pendingAdminRemoteUpdate = ref<AdminRemoteUpdate | null>(null);
const updateReminderOpen = ref(false);
const nativeUpdateInstalling = ref(false);
const nativeUpdateProgress = ref({ downloaded: 0, total: 0, phase: "idle" as "idle" | "downloading" | "installing" });
const forceUpdateRequired = computed(() => updateInfo.value?.forceRequired === true);
const visibleMessages = computed(() => {
  const end = visibleMessageEnd.value || activeMessages.value.length;
  return activeMessages.value.slice(Math.max(0, end - VISIBLE_MESSAGE_WINDOW), end);
});
const hasLaterMessages = computed(() => (visibleMessageEnd.value || activeMessages.value.length) < activeMessages.value.length);
const nativeUpdateProgressPercent = computed(() => {
  if (nativeUpdateProgress.value.total <= 0) return nativeUpdateInstalling.value ? 0 : 100;
  return Math.min(100, Math.round((nativeUpdateProgress.value.downloaded / nativeUpdateProgress.value.total) * 100));
});
const nativeUpdateProgressLabel = computed(() => {
  const progress = nativeUpdateProgress.value;
  if (!nativeUpdateInstalling.value && progress.phase === "idle") return "";
  if (progress.phase === "installing") return "下载完成，正在安装更新...";
  if (progress.total > 0) return `正在下载更新 ${formatFileSize(progress.downloaded)} / ${formatFileSize(progress.total)}`;
  return progress.downloaded > 0 ? `正在下载更新 ${formatFileSize(progress.downloaded)}` : "正在准备下载更新...";
});
const blockingAdminNotification = computed(() => {
  const deviceId = profile.value?.device_id;
  if (!deviceId) return null;
  return adminNotifications.value.find((item) => item.target_device_id === deviceId && item.display_mode === "requires_confirmation" && ["pending", "rejected", "expired_locked"].includes(item.status)) ?? null;
});
const visibleAdminAnnouncement = computed(() => {
  const deviceId = profile.value?.device_id;
  if (!deviceId) return null;
  return adminNotifications.value.find((item) => item.target_device_id === deviceId && item.display_mode === "dismissible" && !dismissedAdminNotificationIds.value.includes(item.notification_id)) ?? null;
});
const pendingAdminNotificationCount = computed(() => {
  const deviceId = profile.value?.device_id;
  if (!deviceId) return 0;
  return adminNotifications.value.filter((item) => {
    if (item.target_device_id !== deviceId) return false;
    if (item.display_mode === "requires_confirmation") {
      return ["pending", "rejected", "expired_locked"].includes(item.status);
    }
    return item.display_mode === "dismissible" && !dismissedAdminNotificationIds.value.includes(item.notification_id);
  }).length;
});
const recipientAdminNotifications = computed(() => {
  const deviceId = profile.value?.device_id;
  if (!deviceId) return [];
  return adminNotifications.value
    .filter((item) => item.target_device_id === deviceId)
    .sort((left, right) => right.created_at - left.created_at);
});
const adminNotificationTargetOptions = computed(() => onlinePeers.value.map((peer) => ({
  label: `${peerDisplayName(peer)} · ${peer.address} · ${peer.device_id}`,
  value: peer.device_id,
})));
const UPDATE_CHECK_INTERVAL_MS = 12 * 60 * 60 * 1000;
const activeSection = ref<MainSection>("chat");
const settingsCategory = ref<"basic" | "pet" | "plugins" | "admin">("basic");
const listPaneCollapsed = ref(false);
type ResizePaneKind = "list" | "group";
type PaneResizeState = { kind: ResizePaneKind; startX: number; startWidth: number };
const listPaneWidth = ref(readSavedPaneWidth("lanchat-list-pane-width", 292, 240, 380));
const groupInspectorWidth = ref(readSavedPaneWidth("lanchat-group-inspector-width", 252, 210, 340));
const paneResizeState = ref<PaneResizeState | null>(null);
const chatEmojiOpen = ref(false);
const nowTick = ref(Date.now());
const channelNoticeEditing = ref(false);
const channelNoticeDraft = ref("");
const channelNotices = ref<Record<string, string>>(readSavedChannelNotices());
const publicChannelMutedIds = ref<Record<string, boolean>>(readSavedPublicChannelMutedIds());
const recipientPickerOpen = ref(false);
const recipientPickerMode = ref<RecipientPickerMode>("privateChannelCreate");
const selectedRecipientPeerIds = ref<string[]>([]);
const privateChannelTitleDraft = ref("私有频道");
const handledPrivateChannelInvites = ref<Record<string, "accepted" | "rejected">>(readSavedPrivateChannelInviteStates());
const messageContextMenuOpen = ref(false);
const messageContextMenuX = ref(0);
const messageContextMenuY = ref(0);
const messageContextMessage = ref<Message | null>(null);
const petAlertEnabled = ref(readSavedPetAlertEnabled());
const quickAlertDraft = ref(readSavedQuickAlertText());
const petAlertMode = ref<PetAlertMode>(readSavedPetAlertMode());
const petSendHotkey = ref(readSavedPetSendHotkey());
const petStopHotkey = ref(readSavedPetStopHotkey());
const alertRecords = ref<AlertRecord[]>(readSavedAlertRecords());
const ownAlertFlashUntil = ref(0);
const lastOwnAlertSentAt = ref(0);
const discoModeUntil = ref(0);
const visuallyStoppedAlertIds = ref<Set<string>>(new Set());
const ALERT_SEND_COOLDOWN_MS = 20_000;
const petDiscoDurationMs = computed(() =>
  Math.max(10, Math.min(3_600, desktopPetSettings.value?.discoDurationSeconds ?? 60)) * 1_000,
);
const enabledPluginManifests = ref<PluginManifestV1[]>([]);
const activePluginGameId = ref("");
const pluginFeatures = computed(() => resolvePluginFeatures(
  enabledPluginManifests.value.map((manifest) => ({ manifest, enabled: true })),
));
const gamesFeatureAvailable = computed(() => pluginFeatures.value.gameIds.length > 0);
const enabledGamePlugins = computed(() => enabledPluginManifests.value.flatMap((manifest) =>
  (manifest.contributes?.games ?? []).map((game) => ({
    pluginId: manifest.id,
    pluginName: manifest.name,
    gameId: game.id,
    minPlayers: game.minPlayers,
    maxPlayers: game.maxPlayers,
  })),
));
const activePluginGame = computed(() => enabledGamePlugins.value.find((item) => item.gameId === activePluginGameId.value) ?? enabledGamePlugins.value[0] ?? null);
const emojiOptions = ["😀", "😄", "😂", "😉", "👍", "👏", "🎉", "🔥", "❤️", "👌", "😎", "🤝", "🍵", "🃏", "💣", "🚀"];
const navExpanded = ref(readSavedNavExpanded());
const themeOptions: Array<{ label: string; key: UiThemeKey; accent: string; hover: string; pressed: string }> = [
  { label: "钉钉商务蓝", key: "theme-dingtalk", accent: "#1677ff", hover: "#4096ff", pressed: "#0958d9" },
  { label: "企业灰白", key: "theme-work", accent: "#2f6fed", hover: "#5287f2", pressed: "#1f55bf" },
  { label: "局域网设备感", key: "theme-lan", accent: "#0f8f83", hover: "#14a99a", pressed: "#0b746c" },
  { label: "轻量清新", key: "theme-light", accent: "#2a9df4", hover: "#55b5fb", pressed: "#177ec7" },
];
const languageOptions = computed(() => [
  { label: t("language.system"), key: "system" },
  { label: t("language.chinese"), key: "zh-CN" },
  { label: t("language.english"), key: "en-US" },
]);
const selectedLanguage = languagePreference;
const themeMenuOptions = themeOptions.map((item) => ({ label: item.label, key: item.key }));
const messageContextOptions = computed(() => {
  const message = messageContextMessage.value;
  return canRecallMessage(message) ? [{ label: "撤回", key: "recall" }] : [];
});
const selectedTheme = ref<UiThemeKey>(readSavedTheme());
const currentTheme = computed(() => themeOptions.find((item) => item.key === selectedTheme.value) ?? themeOptions[0]);
const selectedThemeLabel = computed(() => currentTheme.value.label);
const selectedLanguageLabel = computed(
  () => languageOptions.value.find((item) => item.key === selectedLanguage.value)?.label ?? t("language.system"),
);
const themeOverrides = computed(() => ({
  common: {
    primaryColor: currentTheme.value.accent,
    primaryColorHover: currentTheme.value.hover,
    primaryColorPressed: currentTheme.value.pressed,
    borderRadius: "8px",
    borderRadiusSmall: "6px",
  },
  Button: {
    borderRadiusMedium: "7px",
    borderRadiusLarge: "7px",
  },
  Card: {
    borderRadius: "8px",
  },
  Input: {
    borderRadius: "7px",
  },
}));
let pluginRuntime: PluginRuntime;
const pluginRoomService = new PluginRoomService({
  profile: () => profile.value,
  send: async (frame) => { await store.sendGameFrame(null, frame, true); },
  emit: async (gameId, event) => {
    const pluginIds = enabledPluginManifests.value
      .filter((manifest) => manifest.contributes?.games?.some((game) => game.id === gameId))
      .map((manifest) => manifest.id);
    await Promise.all(pluginRuntime.instances()
      .filter((instance) => pluginIds.includes(instance.pluginId))
      .map((instance) => pluginRuntime.emit(instance.instanceId, "room.event", event)));
  },
});
const pluginLeaderboardService = new PluginLeaderboardService({
  profile: () => profile.value,
  peers: () => peers.value,
  rooms: pluginRoomService,
  policy: (gameId) => {
    const game = enabledPluginManifests.value.flatMap((manifest) => manifest.contributes?.games ?? [])
      .find((item) => item.id === gameId);
    return game ? { maxPlayers: game.maxPlayers, minHumanPlayers: game.ranking?.minHumanPlayers } : undefined;
  },
});
const pluginThemeSnapshot = () => ({
  mode: "light" as const,
  tokens: {
    accent: currentTheme.value.accent,
    accentHover: currentTheme.value.hover,
    accentPressed: currentTheme.value.pressed,
    panelBg: "#ffffff",
    textColor: "#1f2d3d",
    mutedColor: "#748096",
  },
});
pluginRuntime = new PluginRuntime(
  createTauriPluginRuntimeAdapter(),
  createPluginHostHandlers({
    hostVersion: "0.8.0",
    listDevices: () => peers.value.map((peer) => ({
      id: peer.device_id,
      name: peerDisplayName(peer),
      online: peer.online,
      capabilities: { chat: peer.supports_chat !== false },
    })),
    sendChat: async (input) => {
      const message = await api.sendMessage(input.conversationId, input.text);
      return { messageId: message.id };
    },
    currentTheme: pluginThemeSnapshot,
    close: async (instanceId): Promise<void> => { await pluginRuntime.destroy(instanceId, "navigation"); },
    notify: (input) => { error.value = input.level === "error" ? input.body : ""; },
    confirm: (input) => window.confirm(`${input.title}\n\n${input.body}`),
    pickFile: async (input) => {
      const result = await openFileDialog({
        title: input.title,
        multiple: input.multiple ?? false,
        filters: input.extensions?.length ? [{ name: "允许的文件", extensions: input.extensions }] : undefined,
      });
      return result == null ? [] : Array.isArray(result) ? result : [result];
    },
    additionalHandlers: { ...pluginRoomService.handlers(), ...pluginLeaderboardService.handlers() },
  }),
);
const sortedConversations = computed(() => {
  const keyword = conversationSearch.value.trim().toLowerCase();
  return [...conversations.value]
    .filter((conversation) => !keyword || `${conversationDisplayName(conversation)} ${conversation.title}`.toLowerCase().includes(keyword))
    .sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "group" ? -1 : 1;
      return b.updated_at - a.updated_at;
    });
});
const pickerPeerOptions = computed(() => {
  const existingPrivateMembers = activeConversation.value?.is_private
    ? new Set((channelMembersByConversation.value[activeConversation.value.id] ?? []).map((member) => member.device_id))
    : new Set<string>();
  return sortPeersForDisplay(peers.value).filter((peer) => {
    if (!peer.online) return false;
    if (!peerSupportsFullFeatures(peer)) return false;
    if (recipientPickerMode.value === "privateChannelInvite" && existingPrivateMembers.has(peer.device_id)) return false;
    return true;
  });
});
const activeMentionNotices = computed(() => mentionNoticesByConversation.value[activeConversationId.value] ?? []);
const activeMentionLabel = computed(() => activeMentionNotices.value[0]?.kind === "all" ? "@所有人" : "有人@我");
const recipientPickerTitle = computed(() => {
  if (recipientPickerMode.value === "privateChannelCreate") return "创建私有频道";
  return "邀请频道成员";
});
const recipientConfirmDisabled = computed(() => {
  if (recipientPickerMode.value === "privateChannelCreate") return !privateChannelTitleDraft.value.trim();
  return selectedRecipientPeerIds.value.length === 0;
});
const deviceChannelConversations = computed(() => conversations.value
  .filter((conversation) => conversation.kind === "group")
  .sort((a, b) => Number(a.is_private) - Number(b.is_private) || b.updated_at - a.updated_at));
const filteredPeers = computed(() => {
  const keyword = deviceSearch.value.trim().toLowerCase();
  return sortPeersForDisplay(peers.value).filter((peer) => {
    const text = `${peerDisplayName(peer)} ${peer.nickname} ${peer.address} ${peer.port}`.toLowerCase();
    return !keyword || text.includes(keyword);
  });
});
const selectedPeerDetail = computed(() => peers.value.find((peer) => sameDeviceId(peer.device_id, selectedPeerId.value)) ?? null);
const selectedDeviceChannelDetail = computed(() => deviceChannelConversations.value.find((conversation) => conversation.id === selectedDeviceChannelId.value) ?? null);
const publicChannelMembers = computed<Array<ChannelMember | Peer>>(() => {
  const remoteMembers = chatCapablePeers.value.filter((peer) => !sameDeviceId(peer.device_id, profile.value?.device_id));
  if (!profile.value) return sortChannelMembers(remoteMembers);
  const selfMember: ChannelMember = {
    channel_id: DEFAULT_GROUP_ID,
    device_id: profile.value.device_id,
    nickname: profile.value.nickname,
    avatar: profile.value.avatar,
    online: true,
    last_seen_at: Date.now(),
    is_owner: false,
    muted: false,
  };
  return sortChannelMembers([selfMember, ...remoteMembers]);
});
const issuedAdminNotifications = computed(() => {
  const deviceId = profile.value?.device_id;
  if (!deviceId) return [];
  return adminNotifications.value
    .filter((item) => item.issued_by_device_id === deviceId)
    .sort((left, right) => right.created_at - left.created_at);
});
const adminNotificationReviewPageCount = computed(() => Math.max(1, Math.ceil(issuedAdminNotifications.value.length / ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE)));
const pagedIssuedAdminNotifications = computed(() => {
  const start = (adminNotificationReviewPage.value - 1) * ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE;
  return issuedAdminNotifications.value.slice(start, start + ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE);
});
const selectedDeviceChannelMembers = computed<Array<ChannelMember | Peer>>(() => {
  const channel = selectedDeviceChannelDetail.value;
  if (!channel) return [];
  return sortChannelMembers(channel.is_private ? channelMembersByConversation.value[channel.id] ?? [] : publicChannelMembers.value);
});
const selectedDeviceChannelOwnerName = computed(() => {
  const channel = selectedDeviceChannelDetail.value;
  const ownerId = channel?.owner_device_id;
  if (!channel) return "";
  if (!ownerId) return channel.is_private ? "未知" : "局域网公开频道";
  if (sameDeviceId(ownerId, profile.value?.device_id)) return profile.value?.nickname ?? "我";
  const owner = peers.value.find((peer) => sameDeviceId(peer.device_id, ownerId));
  return owner ? peerDisplayName(owner) : ownerId;
});
const canManageSelectedDeviceChannel = computed(() => !!selectedDeviceChannelDetail.value?.is_private && (sameDeviceId(selectedDeviceChannelDetail.value.owner_device_id, profile.value?.device_id) || superAdminEnabled.value));
const activePrivateChannelMembers = computed(() => activeConversation.value?.is_private ? channelMembersByConversation.value[activeConversation.value.id] ?? [] : []);
const channelMembers = computed<Array<ChannelMember | Peer>>(() => activeConversation.value?.is_private ? activePrivateChannelMembers.value : publicChannelMembers.value);
const normalizedChannelMembers = computed<Array<ChannelMember | Peer>>(() => {
  const normalized = channelMembers.value.map((member) => {
    if (!sameDeviceId(member.device_id, profile.value?.device_id)) return member;
    return {
    ...member,
    nickname: profile.value?.nickname ?? member.nickname,
    avatar: profile.value?.avatar ?? member.avatar,
    online: true,
    last_seen_at: Date.now(),
    };
  });
  return [...normalized].sort((left, right) => {
    const leftSelf = sameDeviceId(left.device_id, profile.value?.device_id);
    const rightSelf = sameDeviceId(right.device_id, profile.value?.device_id);
    if (leftSelf !== rightSelf) return leftSelf ? -1 : 1;
    if (left.online !== right.online) return left.online ? -1 : 1;
    return memberDisplayName(left).localeCompare(memberDisplayName(right), "zh-CN");
  });
});
const canMentionInActiveConversation = computed(() => activeConversation.value?.kind === "group" && canSendActive.value);
const mentionPickerMembers = computed<Array<ChannelMember | Peer>>(() => {
  if (!canMentionInActiveConversation.value) return [];
  const keyword = mentionSearch.value.trim().toLowerCase();
  const seen = new Set<string>();
  return normalizedChannelMembers.value
    .filter((member) => {
      if (seen.has(member.device_id)) return false;
      seen.add(member.device_id);
      const text = `${member.nickname} ${member.device_id}`.toLowerCase();
      return !keyword || text.includes(keyword);
    })
    .slice(0, 80);
});
const channelMembersOnlineCount = computed(() => normalizedChannelMembers.value.filter((member) => sameDeviceId(member.device_id, profile.value?.device_id) || member.online).length);
const canManageActivePrivateChannel = computed(() => !!activeConversation.value?.is_private && (sameDeviceId(activeConversation.value.owner_device_id, profile.value?.device_id) || superAdminEnabled.value));
const canInviteActivePrivateChannel = computed(() => !!activeConversation.value?.is_private && (
  superAdminEnabled.value
  || activePrivateChannelMembers.value.some((member) => sameDeviceId(member.device_id, profile.value?.device_id))
));
const groupInspectorAvailable = computed(() => activeSection.value === "chat" && activeConversation.value?.kind === "group");
const canManageActivePublicChannel = computed(() => !!superAdminEnabled.value && activeConversation.value?.id === DEFAULT_GROUP_ID);
const canEditActiveChannelNotice = computed(() => {
  const conversation = activeConversation.value;
  if (!conversation || conversation.kind !== "group") return false;
  if (!conversation.is_private) return superAdminEnabled.value;
  return sameDeviceId(conversation.owner_device_id, profile.value?.device_id) || superAdminEnabled.value;
});
const activeChannelNotice = computed(() => {
  const conversation = activeConversation.value;
  if (!conversation?.id) return DEFAULT_CHANNEL_NOTICE;
  return channelNotices.value[conversation.id] ?? (conversation.is_private ? "这是私有频道，只有受邀成员可以接收消息。" : DEFAULT_CHANNEL_NOTICE);
});
const activeSelfMuted = computed(() => {
  const selfId = profile.value?.device_id;
  const conversation = activeConversation.value;
  if (!selfId || conversation?.kind !== "group") return false;
  if (conversation.is_private) {
    return activePrivateChannelMembers.value.some((member) => member.device_id === selfId && member.muted);
  }
  return channelMutedByConversation.value[conversation.id] === true;
});
const activePeerStatusLabel = computed(() => (activePeer.value?.online ? "在线" : "离线"));
const activePeerStatusType = computed(() => (activePeer.value?.online ? "success" : "default"));
const canStartPrivateCall = computed(() => Boolean(
  activeConversation.value?.kind === "direct"
  && activePeer.value?.online
  && activePeer.value?.supports_chat !== false,
));
const composerPlaceholder = computed(() => {
  if (canSendActive.value) return "输入消息";
  if (activeSelfMuted.value) return "你已被禁言，暂不能发言";
  const peer = activePeer.value;
  if (activeConversation.value?.kind === "direct" && peer && !peerSupportsFullFeatures(peer)) return "该设备不支持聊天发送";
  return activeConversation.value?.kind === "direct" ? "对方已离线，暂不能发送私聊消息" : "当前不可发送消息";
});
const pendingAlertCount = computed(() => alertRecords.value.filter((item) => item.incoming && !item.handled && item.senderDeviceId !== profile.value?.device_id).length);
const adminDeviceOptions = computed(() => {
  const local = profile.value
    ? [{
        label: `我 · ${profile.value.nickname}`,
        value: profile.value.device_id,
      }]
    : [];
  return [
    ...local,
    ...peers.value.map((peer) => ({
      label: `${peer.nickname} · ${peer.online ? "在线" : "离线"}`,
      value: peer.device_id,
    })),
  ];
});
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
const activePetAlert = computed(() =>
  latestPendingAlert.value
  ?? (latestOwnAlert.value && ownAlertFlashUntil.value > 0 ? latestOwnAlert.value : null),
);
const alertRankingRows = computed(() => {
  const map = new Map<string, {
    deviceId: string;
    nickname: string;
    total: number;
    feedbackTotal: number;
    real: number;
    falseCount: number;
    lastAt: number;
  }>();
  for (const alert of alertRecords.value) {
    const row = map.get(alert.senderDeviceId) ?? {
      deviceId: alert.senderDeviceId,
      nickname: alert.senderNickname,
      total: 0,
      feedbackTotal: 0,
      real: 0,
      falseCount: 0,
      lastAt: 0,
    };
    row.total += 1;
    row.lastAt = Math.max(row.lastAt, alert.createdAt);
    for (const feedback of alert.feedbacks) {
      row.feedbackTotal += 1;
      if (feedback.result === "real") row.real += 1;
      if (feedback.result === "false") row.falseCount += 1;
    }
    map.set(alert.senderDeviceId, row);
  }
  return [...map.values()]
    .map((row) => ({
      ...row,
      probability: senderCredibility(alertRecords.value, row.deviceId, nowTick.value),
    }))
    .sort((a, b) => (b.probability ?? -1) - (a.probability ?? -1) || b.feedbackTotal - a.feedbackTotal || b.lastAt - a.lastAt);
});
const adminRemoteUpdateTargetOptions = computed(() => peers.value
  .filter((peer) => peer.online && peer.platform_os === "windows" && !sameDeviceId(peer.device_id, profile.value?.device_id))
  .map((peer) => ({
    label: `${peerDisplayName(peer)} · ${peer.address}`,
    value: peer.device_id,
  })));
const adminRemoteUpdateWindowsCount = computed(() => adminRemoteUpdateTargetOptions.value.length);
const canIssueAdminRemoteUpdate = computed(() => (
  !!adminRemoteUpdateVersion.value.trim()
  && (!adminRemoteUpdatePackagePath.value.trim() || !!adminRemoteUpdateSignaturePath.value.trim())
  && !adminRemoteUpdateSending.value
  && (adminRemoteUpdateScope.value === "all_online_windows"
    ? adminRemoteUpdateWindowsCount.value > 0
    : !!adminRemoteUpdateTargetId.value)
));
function adminRemoteUpdatePhaseLabel(phase: string) {
  return ({
    received: "已送达，等待目标处理",
    downloading: "下载中",
    verifying: "正在校验",
    installing: "正在安装并重启",
    skipped_version: "已跳过：本机版本不低于目标版本",
    skipped_platform: "已跳过：非 Windows",
    undelivered: "未送达",
    failed: "失败",
  } as Record<string, string>)[phase] ?? phase;
}
function adminRemoteUpdateProgressPercent(row: AdminRemoteUpdateDispatchTarget) {
  const progress = row.downloaded;
  const total = row.total;
  if (row.phase !== "downloading" || !total || total <= 0) return 0;
  return Math.min(100, Math.round(((progress ?? 0) / total) * 100));
}
const petAlertProbability = computed(() => alertDisplayTemperature(activePetAlert.value));
const discoModeActive = computed(() => discoModeUntil.value > nowTick.value);
// Local presentation only: changing the view never sends a room action.
const listPaneAvailable = computed(() => ["chat", "devices", "games"].includes(activeSection.value));
const listPaneToggleTitle = computed(() => listPaneCollapsed.value ? "展开列表栏" : "收起列表栏");
function buildTrayAttentionItems(): TrayAttentionItem[] {
  const chatItems = sortedConversations.value
    .map((conversation) => ({
      id: conversation.id,
      kind: "chat",
      title: trayConversationTitle(conversation.title, conversation.kind),
      count: unreadByConversation.value[conversation.id] ?? 0,
    }))
    .filter((item) => item.count > 0);
  return chatItems.slice(0, 12);
}
async function syncTrayAttention() {
  try {
    await api.updateTrayAttention(buildTrayAttentionItems());
  } catch {
    // 浏览器预览时没有 Tauri 后端。
  }
}
async function scrollActiveChatToBottom() {
  await nextTick();
  if (typeof window !== "undefined") {
    window.requestAnimationFrame(() => {
      if (messagePane.value) {
        messagePane.value.scrollTop = messagePane.value.scrollHeight;
      }
    });
    return;
  }
  if (messagePane.value) {
    messagePane.value.scrollTop = messagePane.value.scrollHeight;
  }
}
async function openTrayTarget(target: TrayAttentionItem) {
  if (target.kind !== "chat") return;
  activeSection.value = "chat";
  await store.selectConversation(target.id);
  await syncTrayAttention();
}
const shortDeviceId = computed(() => {
  const id = profile.value?.device_id ?? "";
  if (id.length <= 18) return id;
  return `${id.slice(0, 10)}...${id.slice(-6)}`;
});
const DESKTOP_PET_STATE_ORDER: PetStateKind[] = ["Idle", "Alert", "Move", "Interact", "Life"];
const DESKTOP_PET_STATE_LABELS: Record<PetStateKind, string> = {
  Idle: "待机",
  Alert: "告警",
  Move: "移动",
  Interact: "交互",
  Life: "生活",
};
const DESKTOP_PET_PLAYBACK_DEFAULTS: Record<PetStateKind, PetStatePlaybackConfig> = {
  Idle: { minDurationMs: 3000, maxDurationMs: 7000, minActionCount: 1, maxActionCount: 2, minIntervalMs: 500, maxIntervalMs: 1200 },
  Alert: { minDurationMs: 2000, maxDurationMs: 4000, minActionCount: 1, maxActionCount: 2, minIntervalMs: 250, maxIntervalMs: 700 },
  Move: { minDurationMs: 1200, maxDurationMs: 2400, minActionCount: 2, maxActionCount: 4, minIntervalMs: 120, maxIntervalMs: 420 },
  Interact: { minDurationMs: 0, maxDurationMs: 0, minActionCount: 1, maxActionCount: 1, minIntervalMs: 0, maxIntervalMs: 0 },
  Life: { minDurationMs: 0, maxDurationMs: 0, minActionCount: 2, maxActionCount: 4, minIntervalMs: 800, maxIntervalMs: 2000 },
};
const EXTERNAL_PUSH_DEFAULT_TEMPLATE = "";
const externalPushKindOptions: Array<{ label: string; value: ExternalPushKind }> = [
  { label: "企业微信群机器人", value: "wechat_work" },
  { label: "钉钉群机器人", value: "dingtalk" },
];
function externalPushKindLabel(kind: ExternalPushKind) {
  return externalPushKindOptions.find((item) => item.value === kind)?.label ?? "企业微信群机器人";
}
function desktopPetSourceLabel(source: PetPackageSource) {
  if (source === "built_in") return "内置";
  if (source === "portable") return "绿色版";
  return "用户导入";
}
function desktopPetFrameCount(pet: DesktopPetPackage, state: PetStateKind) {
  return (pet.states[state] ?? []).reduce((total, clip) => total + clip.frames.length, 0);
}
function desktopPetPreview(pet: DesktopPetPackage) {
  const path = pet.icon_path ?? pet.preview_path ?? pet.states.Idle?.[0]?.frames[0]?.path;
  if (!path) return "";
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}
function desktopPetPlaybackConfig(pet: DesktopPetPackage, state: PetStateKind): PetStatePlaybackConfig {
  const source = pet.manifest.states?.[state] ?? {};
  return { ...DESKTOP_PET_PLAYBACK_DEFAULTS[state], ...source };
}
function openDesktopPetManifestEditor(pet: DesktopPetPackage) {
  desktopPetManifestEditorTarget.value = pet;
  desktopPetPlaybackDraft.value = Object.fromEntries(
    DESKTOP_PET_STATE_ORDER.map((state) => [state, desktopPetPlaybackConfig(pet, state)]),
  ) as Record<PetStateKind, PetStatePlaybackConfig>;
  desktopPetManifestEditorOpen.value = true;
}
async function saveDesktopPetManifestConfig() {
  const pet = desktopPetManifestEditorTarget.value;
  if (!pet) return;
  await desktopPetStore.updatePlaybackConfig(pet.manifest.id, desktopPetPlaybackDraft.value).catch(() => undefined);
  desktopPetManifestEditorOpen.value = false;
  desktopPetManifestEditorTarget.value = null;
}
async function importDesktopPetPackage() {
  const selected = await openFileDialog({ directory: true, multiple: false, title: "选择桌宠资源包目录" });
  if (typeof selected !== "string") return;
  await desktopPetStore.importPackage(selected).catch(() => undefined);
}
async function selectDesktopPetPackage(pet: DesktopPetPackage) {
  await desktopPetStore.selectPackage(pet.manifest.id).catch(() => undefined);
}
async function removeDesktopPetPackage(pet: DesktopPetPackage) {
  if (pet.source !== "user") return false;
  if (typeof window !== "undefined" && !window.confirm(`确定删除桌宠“${pet.manifest.name}”吗？`)) return false;
  try {
    await desktopPetStore.removePackage(pet);
    return true;
  } catch {
    return false;
  }
}
async function removeDesktopPetFromEditor() {
  const pet = desktopPetManifestEditorTarget.value;
  if (!pet || !(await removeDesktopPetPackage(pet))) return;
  desktopPetManifestEditorOpen.value = false;
  desktopPetManifestEditorTarget.value = null;
}
async function updateDesktopPetBehavior<K extends keyof DesktopPetSettings>(key: K, value: DesktopPetSettings[K]) {
  if (!desktopPetSettings.value) return;
  await desktopPetStore.updateSettings({ ...desktopPetSettings.value, [key]: value }).catch(() => undefined);
  await syncDesktopPetRuntime();
}
async function updateDesktopPetSettingsPatch(patch: Partial<DesktopPetSettings>) {
  if (!desktopPetSettings.value) return;
  await desktopPetStore.updateSettings({ ...desktopPetSettings.value, ...patch }).catch(() => undefined);
}
function createExternalPushConfig(kind: ExternalPushKind): ExternalPushConfig {
  return {
    id: crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    name: kind === "dingtalk" ? "钉钉群" : "企业微信群",
    kind,
    webhook: "",
    enabled: true,
    mentionAll: true,
    template: EXTERNAL_PUSH_DEFAULT_TEMPLATE,
  };
}
async function addExternalPushConfig(kind: ExternalPushKind = "wechat_work") {
  const configs = desktopPetSettings.value?.externalPushConfigs ?? [];
  await updateDesktopPetSettingsPatch({
    externalPushEnabled: true,
    externalPushConfigs: [...configs, createExternalPushConfig(kind)],
  });
}
async function updateExternalPushConfig(id: string, patch: Partial<ExternalPushConfig>) {
  const settings = desktopPetSettings.value;
  if (!settings) return;
  await updateDesktopPetSettingsPatch({
    externalPushConfigs: (settings.externalPushConfigs ?? []).map((config) =>
      config.id === id ? { ...config, ...patch } : config,
    ),
  });
}
async function removeExternalPushConfig(id: string) {
  const settings = desktopPetSettings.value;
  if (!settings) return;
  await updateDesktopPetSettingsPatch({
    externalPushConfigs: (settings.externalPushConfigs ?? []).filter((config) => config.id !== id),
  });
}
function registerIncomingMention(message: Message) {
  const conversation = conversations.value.find((item) => item.id === message.conversation_id);
  if (conversation?.kind !== "group" || message.message_type !== "text") return;
  const kind = detectMentionKind(message.content, profile.value?.nickname ?? "");
  if (!kind) return;
  const current = mentionNoticesByConversation.value[message.conversation_id] ?? [];
  if (current.some((item) => item.messageId === message.id)) return;
  mentionNoticesByConversation.value = {
    ...mentionNoticesByConversation.value,
    [message.conversation_id]: [...current, { messageId: message.id, kind, createdAt: message.created_at }],
  };
}
function conversationMentionLabel(conversation: Conversation) {
  const notices = mentionNoticesByConversation.value[conversation.id] ?? [];
  const latest = notices[notices.length - 1];
  if (!latest) return "";
  return latest.kind === "all" ? "@所有人" : "有人@我";
}
async function jumpToActiveMention() {
  const target = activeMentionNotices.value[0];
  if (!target) return;
  await nextTick();
  const element = document.getElementById(`message-${target.messageId}`);
  element?.scrollIntoView({ behavior: "smooth", block: "center" });
  highlightedMentionMessageId.value = target.messageId;
  if (mentionHighlightTimer !== null) window.clearTimeout(mentionHighlightTimer);
  mentionHighlightTimer = window.setTimeout(() => {
    highlightedMentionMessageId.value = "";
    mentionHighlightTimer = null;
  }, 1800);
  mentionNoticesByConversation.value = {
    ...mentionNoticesByConversation.value,
    [activeConversationId.value]: activeMentionNotices.value.filter((item) => item.messageId !== target.messageId),
  };
}
async function checkUpdates(manual = false) {
  if (pendingAdminRemoteUpdate.value && !manual) return;
  updateChecking.value = true;
  updateError.value = "";
  try {
    const result = await api.checkForUpdate();
    updateInfo.value = result;
    saveUpdateInfo(result);
    maybeOpenUpdateReminder(result, manual);
    if (result.forceRequired) {
      void installNativeUpdate(true);
    }
    if (manual && !result.updateAvailable) {
      store.error = "";
    }
  } catch (err) {
    updateError.value = stringifyError(err);
    if (manual) {
      store.error = updateError.value;
    }
  } finally {
    updateChecking.value = false;
  }
}
function scheduleAutomaticUpdateChecks() {
  if (typeof window === "undefined") return;
  void checkUpdates(false);
  if (updateCheckTimer !== null) window.clearInterval(updateCheckTimer);
  updateCheckTimer = window.setInterval(() => {
    void checkUpdates(false);
  }, UPDATE_CHECK_INTERVAL_MS);
}
async function openUpdateReminderForAdminRemoteUpdate(command: AdminRemoteUpdate) {
  const current = await api.getAppVersionInfo();
  const packageUrl = command.package?.url ?? "";
  const packageName = command.package?.name?.toLowerCase() ?? "";
  const isPortablePackage = packageName.endsWith(".zip");
  const fallbackReleaseUrl = `https://github.com/DumKing/lanchat/releases/tag/v${command.target_version}`;
  pendingAdminRemoteUpdate.value = command;
  updateInfo.value = {
    repository: "LAN",
    current,
    latestVersion: command.target_version,
    latestBuild: null,
    title: `${command.issued_by_nickname} 下发了 LanChat ${command.target_version}`,
    notes: command.package
      ? "更新包将从超管设备的局域网文件服务下载，下载完成后沿用当前自动更新的安装与重启流程。"
      : "未携带局域网安装包，将从 GitHub Release 下载并沿用当前自动更新流程。",
    releaseUrl: packageUrl || fallbackReleaseUrl,
    downloads: {
      windowsPortable: isPortablePackage ? packageUrl : null,
      windowsPortableSha256: isPortablePackage ? command.package_sha256 ?? null : null,
      windowsInstaller: !isPortablePackage ? packageUrl : null,
      macosDmg: null,
      releasePage: packageUrl || fallbackReleaseUrl,
    },
    updateAvailable: true,
    force: command.force,
    minSupportedVersion: null,
    forceRequired: command.force,
    checkedAt: Date.now(),
  };
  updateError.value = "";
  updateReminderOpen.value = true;
  if (command.force) void installNativeUpdate(true);
}
function isMessagePaneAtBottom() {
  const pane = messagePane.value;
  if (!pane) return true;
  return pane.scrollHeight - pane.scrollTop - pane.clientHeight < 32;
}

async function saveUpdateGithubToken() {
  const token = updateGithubTokenDraft.value.trim();
  if (!token) return;
  updateGithubTokenSaving.value = true;
  try {
    updateGithubTokenInfo.value = await api.saveUpdateGithubToken(token);
    updateGithubTokenDraft.value = "";
    updateError.value = "";
  } catch (err) {
    updateError.value = `保存 GitHub Token 失败：${stringifyError(err)}`;
  } finally {
    updateGithubTokenSaving.value = false;
  }
}

async function clearUpdateGithubToken() {
  updateGithubTokenSaving.value = true;
  try {
    updateGithubTokenInfo.value = await api.clearUpdateGithubToken();
    updateError.value = "";
  } catch (err) {
    updateError.value = `清除 GitHub Token 失败：${stringifyError(err)}`;
  } finally {
    updateGithubTokenSaving.value = false;
  }
}
async function loadEarlierMessages() {
  if (!hasMoreEarlierMessages.value) return;
  const loaded = await store.loadEarlierMessages(activeConversationId.value);
  if (loaded === 0) {
    hasMoreEarlierMessages.value = false;
    return;
  }
  visibleMessageEnd.value = Math.min(activeMessages.value.length, visibleMessageEnd.value + loaded);
  await nextTick();
  if (messagePane.value) messagePane.value.scrollTop = messagePane.value.scrollHeight;
}
async function jumpToLatestMessages() {
  visibleMessageEnd.value = activeMessages.value.length;
  await scrollActiveChatToBottom();
}
function handleMessagePaneScroll() {
  messagePaneFollowingLatest.value = isMessagePaneAtBottom();
  if (messagePane.value?.scrollTop !== undefined && messagePane.value.scrollTop < 24) {
    void loadEarlierMessages();
  }
}
async function installNativeUpdate(force = false) {
  if (nativeUpdateInstalling.value) return;
  nativeUpdateInstalling.value = true;
  nativeUpdateProgress.value = { downloaded: 0, total: 0, phase: "downloading" };
  try {
    const remoteCommand = pendingAdminRemoteUpdate.value;
    if (remoteCommand) {
      await api.executeAdminRemoteUpdate(remoteCommand);
      pendingAdminRemoteUpdate.value = null;
      if (!(force || remoteCommand.force)) updateReminderOpen.value = false;
      return;
    }
    await api.refreshUpdateProxy().catch(() => undefined);
    if (await api.isPortableRuntime()) {
      const url = updateInfo.value?.downloads.windowsPortable;
      const sha256 = updateInfo.value?.downloads.windowsPortableSha256;
      if (!url || !sha256) throw new Error("当前绿色版更新包尚未提供完整性校验信息");
      nativeUpdateProgress.value = { downloaded: 0, total: 0, phase: "installing" };
      await api.installPortableUpdate(url, sha256);
      return;
    }
    const update = await checkNativeUpdate();
    if (!update) {
      if (force) updateError.value = "已发现强制更新，但签名更新包尚未就绪，请从 Release 页面完成更新。";
      return;
    }
    let downloaded = 0;
    let total = 0;
    await update.download((event) => {
      if (event.event === "Started") {
        downloaded = 0;
        total = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      } else if (event.event === "Finished") {
        downloaded = total > 0 ? total : downloaded;
      }
      nativeUpdateProgress.value = { downloaded, total, phase: "downloading" };
    });
    nativeUpdateProgress.value = { downloaded, total, phase: "installing" };
    await update.install();
    await api.quitApp();
  } catch (err) {
    updateError.value = `自动更新失败：${stringifyError(err)}`;
    if (force) updateReminderOpen.value = true;
  } finally {
    nativeUpdateInstalling.value = false;
    nativeUpdateProgress.value = { downloaded: 0, total: 0, phase: "idle" };
  }
}
async function openPreferredUpdateUrl() {
  const url = preferredUpdateUrl.value;
  if (!url) return;
  try {
    if (!forceUpdateRequired.value) dismissUpdateReminder();
    await api.openUpdateUrl(url);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function openReleasePage() {
  const url = updateInfo.value?.downloads.releasePage || updateInfo.value?.releaseUrl;
  if (!url) return;
  try {
    if (!forceUpdateRequired.value) dismissUpdateReminder();
    await api.openUpdateUrl(url);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function initializePluginFeatures() {
  enabledPluginManifests.value = await pluginApi.listEnabledManifests().catch(() => []);
  if (!enabledGamePlugins.value.some((item) => item.gameId === activePluginGameId.value)) {
    activePluginGameId.value = enabledGamePlugins.value[0]?.gameId ?? "";
  }
  if (activeSection.value === "games" && !gamesFeatureAvailable.value) {
    activeSection.value = "chat";
  }
}
function openPluginGame(gameId: string) {
  if (!enabledGamePlugins.value.some((item) => item.gameId === gameId)) return;
  activePluginGameId.value = gameId;
  activeSection.value = "games";
  listPaneCollapsed.value = false;
}
function handlePluginViewportError(message: string) {
  error.value = message;
}
onMounted(async () => {
  stopUiTranslation = installUiTranslation();
  unlistenPluginBridge = await installTauriPluginBridge(pluginRuntime).catch(() => null);
  void initializeAutostart();
  platformInfo.value = await api.getPlatformInfo().catch(() => null);
  appVersionInfo.value = await api.getAppVersionInfo().catch(() => null);
  updateGithubTokenInfo.value = await api.getUpdateGithubTokenInfo().catch(() => null);
  await initializePluginFeatures();
  await store.initialize();
  await restoreSavedSuperAdminSession();
  previewMediaCacheInfo.value = await api.getPreviewMediaCacheInfo().catch(() => null);
  await refreshMemoryDiagnostic();
  await desktopPetStore.initialize();
  if (desktopPetSettings.value) {
    petAlertEnabled.value = desktopPetSettings.value.enabled;
    petSendHotkey.value = desktopPetSettings.value.sendHotkey || petSendHotkey.value;
    petStopHotkey.value = desktopPetSettings.value.stopHotkey || petStopHotkey.value;
  }
  nicknameDraft.value = profile.value?.nickname ?? "";
  portDraft.value = profile.value?.listen_port ?? 18145;
  avatarDraft.value = profile.value?.avatar ?? "";
  scheduleAutomaticUpdateChecks();
  await api.setDesktopPetEnabled(petAlertEnabled.value).catch(() => undefined);
  await registerDesktopPetSendHotkey();
  await registerDesktopPetStopHotkey();
  await syncDesktopPetRuntime();
  try {
    unlistenTrayOpenTarget = await listen<TrayAttentionItem>("tray_open_target", (event) => {
      void openTrayTarget(event.payload);
    });
    unlistenDesktopPetAction = await listen<{ action: string; alert_id?: string | null; alert_kind?: string | null }>("desktop_pet_action", (event) => {
      if (event.payload.action === "quick_alert") {
        void sendPetQuickAlert(petAlertMode.value);
      } else if (event.payload.action === "open_main_window") {
        void api.showFromTray();
      } else if (event.payload.action === "broadcast_disco_alert") {
        void sendPetQuickAlert("disco");
      } else if (event.payload.action === "stop_visuals") {
        stopPetAlertVisuals();
      } else if (event.payload.action === "feedback_real" || event.payload.action === "feedback_false") {
        void handleDesktopPetFeedbackAction(event.payload);
      } else if (event.payload.action === "accept_call" || event.payload.action === "reject_call") {
        void handleDesktopPetCallAction(event.payload.action, event.payload.alert_id);
      }
    });
    unlistenDesktopPetStopHotkey = await listen("desktop_pet_stop_hotkey_received", () => {
      stopPetAlertVisuals();
    });
    unlistenDesktopPetSendHotkey = await listen("desktop_pet_send_hotkey_received", () => {
      void sendPetQuickAlert("disco");
    });
    unlistenDesktopPetRegistry = await listen<DesktopPetRegistrySnapshot>("desktop_pet_registry_changed", (event) => {
      desktopPetStore.applySnapshot(event.payload);
    });
  } catch {
    // 浏览器预览时没有 Tauri 事件通道。
  }
  await syncTrayAttention();
  if (typeof window !== "undefined") {
    window.addEventListener("keydown", handleDesktopPetSendHotkey);
    window.addEventListener("keydown", handleDesktopPetStopHotkey);
    turnTicker = window.setInterval(() => {
      nowTick.value = Date.now();
    }, 1000);
  }
});
onUnmounted(() => {
  stopUiTranslation?.();
  stopUiTranslation = null;
  store.stopRuntime();
  Object.values(avatarBlobUrls.value).forEach((url) => URL.revokeObjectURL(url));
  imagePreviewBlobUrls.forEach((url) => URL.revokeObjectURL(url));
  imagePreviewBlobUrls.clear();
  clearCallSession();
  callMediaCoordinator.dispose();
  stopCallPanelDrag();
  stopPaneResize();
  unlistenTrayOpenTarget?.();
  unlistenTrayOpenTarget = null;
  unlistenDesktopPetAction?.();
  unlistenDesktopPetAction = null;
  unlistenDesktopPetStopHotkey?.();
  unlistenDesktopPetStopHotkey = null;
  unlistenDesktopPetSendHotkey?.();
  unlistenDesktopPetSendHotkey = null;
  unlistenDesktopPetRegistry?.();
  unlistenDesktopPetRegistry = null;
  unlistenPluginBridge?.();
  unlistenPluginBridge = null;
  if (turnTicker !== null && typeof window !== "undefined") {
    window.clearInterval(turnTicker);
    turnTicker = null;
  }
  if (updateCheckTimer !== null && typeof window !== "undefined") {
    window.clearInterval(updateCheckTimer);
    updateCheckTimer = null;
  }
  if (typeof window !== "undefined") {
    window.removeEventListener("keydown", handleDesktopPetSendHotkey);
    window.removeEventListener("keydown", handleDesktopPetStopHotkey);
  }
  if (mentionHighlightTimer !== null && typeof window !== "undefined") {
    window.clearTimeout(mentionHighlightTimer);
    mentionHighlightTimer = null;
  }
});
watch(profile, (next) => {
  nicknameDraft.value = next?.nickname ?? "";
  portDraft.value = next?.listen_port ?? 18145;
  avatarDraft.value = next?.avatar ?? "";
});
const simulationDirectTargetOptions = computed(() => peers.value
  .filter((peer) => peer.online && peer.supports_chat !== false)
  .map((peer) => ({ label: `${peerDisplayName(peer)} · ${peer.address}`, value: peer.device_id })));
const simulationChannelOptions = computed(() => conversations.value
  .filter((conversation) => conversation.kind === "group")
  .filter((conversation) => !conversation.is_private || (channelMembersByConversation.value[conversation.id] ?? []).some((member) => sameDeviceId(member.device_id, profile.value?.device_id)))
  .map((conversation) => ({ label: conversation.is_private ? `${conversation.title} · 私有频道` : conversation.title, value: conversation.id })));
watch([peers, profile], () => {
  let changed = false;
  const next = alertRecords.value.map((record) => {
    const sender = resolveAlertSender({
      sender_device_id: record.senderDeviceId,
      sender_nickname: record.senderNickname,
      sender_address: record.senderAddress,
    });
    const nickname = record.senderNickname?.trim() && record.senderNickname !== "未知设备"
      ? record.senderNickname
      : sender.nickname;
    const address = record.senderAddress?.trim() || sender.address;
    if (nickname === record.senderNickname && address === record.senderAddress) return record;
    changed = true;
    return { ...record, senderNickname: nickname, senderAddress: address };
  });
  if (changed) alertRecords.value = next;
}, { deep: true });
watch(activeMessages, (messages) => {
  if (messagePaneFollowingLatest.value) {
    visibleMessageEnd.value = messages.length;
    void scrollActiveChatToBottom();
  }
});
watch(() => activeConversationId.value, () => {
  hasMoreEarlierMessages.value = true;
  messagePaneFollowingLatest.value = true;
  visibleMessageEnd.value = activeMessages.value.length;
  void scrollActiveChatToBottom();
});
watch(activeSection, (section) => {
  if (section === "chat") {
    void scrollActiveChatToBottom();
  }
});
watch(selectedTheme, (next) => {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-ui-theme", next);
  }
  void Promise.all(pluginRuntime.instances().map((instance) =>
    pluginRuntime.emit(instance.instanceId, "theme.changed", pluginThemeSnapshot())));
  void syncDesktopPetRuntime();
});
watch(() => onlinePeers.value.length, () => {
  const snapshot = { online: Boolean(profile.value), lanAvailable: onlinePeers.value.length > 0 };
  void Promise.all(pluginRuntime.instances().map((instance) =>
    pluginRuntime.emit(instance.instanceId, "network.changed", snapshot)));
});
watch(selectedLanguage, (next) => {
  setLanguagePreference(next);
});
watch(effectiveLocale, (next) => {
  if (typeof document !== "undefined") document.documentElement.lang = next;
});
watch(latestIncomingMessage, async (message) => {
  if (!message || message.sender_device_id === profile.value?.device_id) return;
  const isUnreadContext = activeSection.value !== "chat" || message.conversation_id !== activeConversationId.value;
  if (isUnreadContext) registerIncomingMention(message);
  if (activeSection.value !== "chat" && message.conversation_id === activeConversationId.value) {
    unreadByConversation.value = {
      ...unreadByConversation.value,
      [message.conversation_id]: (unreadByConversation.value[message.conversation_id] ?? 0) + 1,
    };
  }
  await notifyIncomingActivity();
});
watch(latestGameFrame, (frame) => {
  if (!frame) return;
  void pluginRoomService.receive(frame);
});
watch(latestChannelNotice, (payload) => {
  if (!payload) return;
  channelNotices.value = {
    ...channelNotices.value,
    [payload.conversation_id]: payload.notice || DEFAULT_CHANNEL_NOTICE,
  };
});
watch(latestQuickAlert, async (alert) => {
  if (!alert || !petAlertEnabled.value) return;
  applyQuickAlert(alert);
  if (alert.sender_device_id !== profile.value?.device_id) {
    await notifyIncomingActivity();
  }
});
watch(latestQuickAlertFeedback, (feedback) => {
  if (!feedback || !petAlertEnabled.value) return;
  applyQuickAlertFeedback(feedback);
});
watch(latestQuickAlertTrustReset, (reset) => {
  if (!reset || !petAlertEnabled.value) return;
  void applyQuickAlertTrustReset(reset).catch((err) => {
    store.error = stringifyError(err);
  });
});
watch(latestAdminDiscoMode, (mode) => {
  if (!mode || !petAlertEnabled.value) return;
  applyAdminDiscoMode(mode);
});
watch(latestAdminAlertMode, (mode) => {
  if (!mode || !petAlertEnabled.value) return;
  applyAdminAlertMode(mode);
});
watch(latestCallSignal, (signal) => {
  if (signal) {
    void handleCallSignal(signal).catch((err) => {
      store.error = `通话信令处理失败：${stringifyError(err)}`;
    });
  }
});
watch(latestNudge, (nudge) => {
  if (nudge) void handleIncomingNudge(nudge);
});
watch(latestAdminAlertPushPolicy, (policy: AdminAlertPushPolicy | null) => {
  if (!policy || (policy.target_device_id !== "*" && !sameDeviceId(policy.target_device_id, profile.value?.device_id))) return;
  adminAlertPushPolicyDraft.value = policy.min_credibility;
  adminAlertPushPolicyLockAfterIssue.value = policy.min_credibility_locked;
  void desktopPetStore.refreshSettings();
});
watch(callSession, (session, previous) => {
  if (session?.callId !== previous?.callId) {
    callPanelExpanded.value = false;
  }
  syncDetachedCallWindow();
  void syncDesktopPetRuntime();
});
watch(callPanelExpanded, (expanded) => {
  if (expanded) void attachCallStreams();
});
watch([petAlertEnabled, pendingAlertCount, activePetAlert, petAlertProbability, discoModeActive, latestPendingAlert], () => {
  void syncDesktopPetRuntime();
});
watch(latestAdminRemoteUpdate, (command: AdminRemoteUpdate | null) => {
  if (!command
    || processedRemoteUpdateCommandIds.has(command.command_id)
    || !sameDeviceId(command.target_device_id, profile.value?.device_id)) return;
  processedRemoteUpdateCommandIds.add(command.command_id);
  adminRemoteUpdateResults.value = [{
    target_device_id: command.target_device_id,
    nickname: profile.value?.nickname || "本机",
    address: "本机",
    command_id: command.command_id,
    delivery_id: command.delivery_id,
    phase: "received",
    error: null,
  }];
  void openUpdateReminderForAdminRemoteUpdate(command).catch((err) => {
    store.error = `打开远程更新提醒失败：${stringifyError(err)}`;
  });
});
watch(latestAdminRemoteUpdateProgress, (progress: AdminRemoteUpdateProgress | null) => {
  if (!progress) return;
  const index = adminRemoteUpdateResults.value.findIndex((row) =>
    row.command_id === progress.command_id || (
      row.target_device_id === progress.target_device_id && row.delivery_id === progress.delivery_id
    ));
  if (index < 0) return;
  const next = [...adminRemoteUpdateResults.value];
  next[index] = {
    ...next[index],
    phase: progress.phase,
    error: progress.error,
    downloaded: progress.downloaded,
    total: progress.total,
  };
  adminRemoteUpdateResults.value = next;
  if (pendingAdminRemoteUpdate.value?.command_id === progress.command_id) {
    if (progress.phase === "downloading" || progress.phase === "verifying") {
      nativeUpdateProgress.value = {
        downloaded: progress.downloaded,
        total: progress.total ?? 0,
        phase: "downloading",
      };
    } else if (progress.phase === "installing") {
      nativeUpdateProgress.value = {
        downloaded: progress.downloaded,
        total: progress.total ?? 0,
        phase: "installing",
      };
    } else if (progress.phase === "failed") {
      nativeUpdateInstalling.value = false;
      updateError.value = progress.error || "局域网更新失败";
      updateReminderOpen.value = true;
    }
  }
});
watch(navExpanded, (next) => {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-nav-expanded", String(next));
  }
});
watch(listPaneWidth, (next) => {
  savePaneWidth("lanchat-list-pane-width", next);
});
watch(groupInspectorWidth, (next) => {
  savePaneWidth("lanchat-group-inspector-width", next);
});
watch(() => activeConversation.value?.id, () => {
  channelNoticeEditing.value = false;
  channelNoticeDraft.value = activeChannelNotice.value;
});
watch(channelNotices, saveChannelNotices, { deep: true });
watch(publicChannelMutedIds, savePublicChannelMutedIds, { deep: true });
watch(handledPrivateChannelInvites, savePrivateChannelInviteStates, { deep: true });
watch(petAlertEnabled, (next) => {
  savePetAlertEnabled(next);
  void desktopPetStore.setEnabled(next).catch(() => undefined);
  if (!next && activeSection.value === "alerts") {
    activeSection.value = "settings";
  }
});
watch(quickAlertDraft, (next) => {
  saveQuickAlertText(next);
});
watch(petAlertMode, (next) => {
  savePetAlertMode(next);
});
watch(petSendHotkey, (next) => {
  savePetSendHotkey(next);
  void updateDesktopPetSettingsPatch({ sendHotkey: next });
  void registerDesktopPetSendHotkey(next);
});
watch(petStopHotkey, (next) => {
  savePetStopHotkey(next);
  void updateDesktopPetSettingsPatch({ stopHotkey: next });
  void registerDesktopPetStopHotkey(next);
});
watch(alertRecords, saveAlertRecords, { deep: true });
watch(
  [unreadByConversation, conversations],
  () => {
    void syncTrayAttention();
  },
  { deep: true },
);
function readSavedTheme(): UiThemeKey {
  if (typeof window === "undefined") return "theme-dingtalk";
  const saved = window.localStorage.getItem("lanchat-ui-theme") as UiThemeKey | null;
  return themeOptions.some((item) => item.key === saved) ? saved! : "theme-dingtalk";
}
function readSavedUpdateInfo(): UpdateCheckResult | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem("lanchat-last-update-info");
  if (!raw) return null;
  try {
    return JSON.parse(raw) as UpdateCheckResult;
  } catch {
    return null;
  }
}
function saveUpdateInfo(value: UpdateCheckResult) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-last-update-info", JSON.stringify(value));
    window.localStorage.setItem("lanchat-last-update-check-at", String(Date.now()));
  }
}
function updateReminderKey(info: UpdateCheckResult) {
  return `${info.latestVersion}:${info.latestBuild ?? ""}`;
}
function readDismissedUpdateReminderKey() {
  if (typeof window === "undefined") return "";
  return window.localStorage.getItem("lanchat-dismissed-update-reminder") ?? "";
}
function dismissUpdateReminder() {
  if (forceUpdateRequired.value) {
    updateReminderOpen.value = true;
    return;
  }
  const info = updateInfo.value;
  updateReminderOpen.value = false;
  if (pendingAdminRemoteUpdate.value) {
    pendingAdminRemoteUpdate.value = null;
    return;
  }
  if (info && typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-dismissed-update-reminder", updateReminderKey(info));
  }
}
function handleUpdateReminderShowChange(show: boolean) {
  if (show) {
    updateReminderOpen.value = true;
  } else if (forceUpdateRequired.value) {
    updateReminderOpen.value = true;
  } else {
    dismissUpdateReminder();
  }
}
function maybeOpenUpdateReminder(info: UpdateCheckResult, manual = false) {
  if (!info.updateAvailable) return;
  if (info.forceRequired) {
    updateReminderOpen.value = true;
    return;
  }
  if (!manual && readDismissedUpdateReminderKey() === updateReminderKey(info)) return;
  updateReminderOpen.value = true;
}

function readDismissedAdminNotificationIds(): string[] {
  try {
    const value = JSON.parse(window.localStorage.getItem("lanchat-dismissed-admin-notifications") ?? "[]");
    return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string").slice(-200) : [];
  } catch {
    return [];
  }
}
function readSavedNavExpanded() {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem("lanchat-nav-expanded") === "true";
}
function clampPaneWidth(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Math.round(value)));
}
function readSavedPaneWidth(key: string, fallback: number, min: number, max: number) {
  if (typeof window === "undefined") return fallback;
  const saved = window.localStorage.getItem(key);
  if (!saved) return fallback;
  const parsed = Number(saved);
  return Number.isFinite(parsed) ? clampPaneWidth(parsed, min, max) : fallback;
}
function savePaneWidth(key: string, value: number) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(key, String(value));
  }
}
function startPaneResize(kind: ResizePaneKind, event: MouseEvent) {
  if (typeof window === "undefined") return;
  event.preventDefault();
  event.stopPropagation();
  stopPaneResize();
  paneResizeState.value = {
    kind,
    startX: event.clientX,
    startWidth: kind === "list" ? listPaneWidth.value : groupInspectorWidth.value,
  };
  window.addEventListener("mousemove", handlePaneResize);
  window.addEventListener("mouseup", stopPaneResize, { once: true });
  if (typeof document !== "undefined") {
    document.body.classList.add("pane-resizing");
  }
}
function handlePaneResize(event: MouseEvent) {
  const state = paneResizeState.value;
  if (!state) return;
  if (state.kind === "list") {
    listPaneWidth.value = clampPaneWidth(state.startWidth + event.clientX - state.startX, 240, 380);
  } else {
    groupInspectorWidth.value = clampPaneWidth(state.startWidth + state.startX - event.clientX, 210, 340);
  }
}
function stopPaneResize() {
  if (typeof window !== "undefined") {
    window.removeEventListener("mousemove", handlePaneResize);
    window.removeEventListener("mouseup", stopPaneResize);
  }
  paneResizeState.value = null;
  if (typeof document !== "undefined") {
    document.body.classList.remove("pane-resizing");
  }
}
function readSavedPrivateChannelInviteStates(): Record<string, "accepted" | "rejected"> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem("lanchat-private-channel-invite-states-v1");
    return raw ? JSON.parse(raw) as Record<string, "accepted" | "rejected"> : {};
  } catch {
    return {};
  }
}
function savePrivateChannelInviteStates() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-private-channel-invite-states-v1", JSON.stringify(handledPrivateChannelInvites.value));
  }
}
function readSavedChannelNotices(): Record<string, string> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem("lanchat-channel-notices-v1");
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>).filter(([, value]) => typeof value === "string"),
    ) as Record<string, string>;
  } catch {
    return {};
  }
}
function saveChannelNotices() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-channel-notices-v1", JSON.stringify(channelNotices.value));
  }
}
function readSavedPublicChannelMutedIds(): Record<string, boolean> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem("lanchat-public-channel-muted-v1");
    return raw ? JSON.parse(raw) as Record<string, boolean> : {};
  } catch {
    return {};
  }
}
function savePublicChannelMutedIds() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-public-channel-muted-v1", JSON.stringify(publicChannelMutedIds.value));
  }
}
function readMigratedPetSetting(key: string, legacyKey: string) {
  if (typeof window === "undefined") return null;
  const current = window.localStorage.getItem(key);
  if (current !== null) return current;
  const legacy = window.localStorage.getItem(legacyKey);
  if (legacy !== null) {
    window.localStorage.setItem(key, legacy);
    window.localStorage.removeItem(legacyKey);
  }
  return legacy;
}
function readSavedPetAlertEnabled() {
  if (typeof window === "undefined") return true;
  return readMigratedPetSetting("lanchat-pet-alert-enabled", "lanchat-frog-alert-enabled") !== "false";
}
function savePetAlertEnabled(value: boolean) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-pet-alert-enabled", String(value));
  }
}
function readSavedQuickAlertText() {
  if (typeof window === "undefined") return "呱呱~呱~~";
  return readMigratedPetSetting("lanchat-pet-alert-text", "lanchat-frog-alert-text") || "呱呱~呱~~";
}
function saveQuickAlertText(value: string) {
  if (typeof window !== "undefined") {
    const text = value.trim() || "呱呱~呱~~";
    window.localStorage.setItem("lanchat-pet-alert-text", text);
  }
}
function readSavedPetAlertMode(): PetAlertMode {
  if (typeof window === "undefined") return "normal";
  return readMigratedPetSetting("lanchat-pet-alert-mode", "lanchat-frog-alert-mode") === "disco" ? "disco" : "normal";
}
function savePetAlertMode(value: PetAlertMode) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-pet-alert-mode", value === "disco" ? "disco" : "normal");
  }
}
function normalizePetAlertMode(value: unknown): PetAlertMode {
  return value === "disco" ? "disco" : "normal";
}
function readSavedPetSendHotkey() {
  if (typeof window === "undefined") return "Ctrl+Alt+G";
  const current = window.localStorage.getItem("lanchat-pet-send-hotkey");
  if (current !== null) return current || "Ctrl+Alt+G";
  const legacy = readMigratedPetSetting("lanchat-pet-stop-hotkey", "lanchat-frog-stop-hotkey");
  const migrated = legacy || "Ctrl+Alt+G";
  window.localStorage.setItem("lanchat-pet-send-hotkey", migrated);
  return migrated;
}
function savePetSendHotkey(value: string) {
  if (typeof window !== "undefined") {
    const text = value.trim();
    if (text) {
      window.localStorage.setItem("lanchat-pet-send-hotkey", text);
    } else {
      window.localStorage.removeItem("lanchat-pet-send-hotkey");
    }
  }
}
function readSavedPetStopHotkey() {
  if (typeof window === "undefined") return "Ctrl+Alt+S";
  const current = window.localStorage.getItem("lanchat-pet-stop-hotkey");
  const send = window.localStorage.getItem("lanchat-pet-send-hotkey");
  if (current !== null) return current && current !== send ? current : "Ctrl+Alt+S";
  return "Ctrl+Alt+S";
}
function savePetStopHotkey(value: string) {
  if (typeof window !== "undefined") {
    const text = value.trim();
    if (text) {
      window.localStorage.setItem("lanchat-pet-stop-hotkey", text);
    } else {
      window.localStorage.removeItem("lanchat-pet-stop-hotkey");
    }
  }
}
function hotkeyFromEvent(event: KeyboardEvent) {
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return "";
  return [
    event.ctrlKey ? "Ctrl" : "",
    event.altKey ? "Alt" : "",
    event.shiftKey ? "Shift" : "",
    event.metaKey ? "Meta" : "",
    key,
  ].filter(Boolean).join("+");
}
function captureDesktopPetStopHotkey(event: KeyboardEvent) {
  const hotkey = hotkeyFromEvent(event);
  if (!hotkey) return;
  event.preventDefault();
  petStopHotkey.value = hotkey;
}
function captureDesktopPetSendHotkey(event: KeyboardEvent) {
  const hotkey = hotkeyFromEvent(event);
  if (!hotkey) return;
  event.preventDefault();
  petSendHotkey.value = hotkey;
}
function clearDesktopPetSendHotkey() {
  petSendHotkey.value = "";
}
function clearDesktopPetStopHotkey() {
  petStopHotkey.value = "";
}
async function registerDesktopPetSendHotkey(value = petSendHotkey.value) {
  await api.registerDesktopPetSendHotkey(value).catch(() => undefined);
}
async function registerDesktopPetStopHotkey(value = petStopHotkey.value) {
  await api.registerDesktopPetStopHotkey(value).catch(() => undefined);
}
function handleDesktopPetStopHotkey(event: KeyboardEvent) {
  if (!petStopHotkey.value) return;
  if (hotkeyFromEvent(event) !== petStopHotkey.value) return;
  event.preventDefault();
  stopPetAlertVisuals();
}
function handleDesktopPetSendHotkey(event: KeyboardEvent) {
  if (!petSendHotkey.value) return;
  if (hotkeyFromEvent(event) !== petSendHotkey.value) return;
  event.preventDefault();
  void sendPetQuickAlert("disco");
}
function normalizeAlertRecords(records: AlertRecord[]) {
  return records
    .filter((item) => item.alertId && item.senderDeviceId)
    .map((item) => ({
      ...item,
      senderAddress: item.senderAddress ?? null,
      content: item.content || "呱呱~呱~~",
      mode: normalizePetAlertMode(item.mode),
      feedbacks: Array.isArray(item.feedbacks) ? item.feedbacks : [],
      handled: Boolean(item.handled),
      incoming: Boolean(item.incoming),
    }))
    .sort((a, b) => b.createdAt - a.createdAt)
    .slice(0, 200);
}
function readSavedAlertRecords(): AlertRecord[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = readMigratedPetSetting("lanchat-pet-alert-records-v1", "lanchat-frog-alert-records-v1");
    return raw ? normalizeAlertRecords(JSON.parse(raw) as AlertRecord[]) : [];
  } catch {
    return [];
  }
}
function saveAlertRecords() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-pet-alert-records-v1", JSON.stringify(normalizeAlertRecords(alertRecords.value)));
  }
}
function alertDisplayTemperature(alert?: AlertRecord | null) {
  if (!alert) return 0;
  return alertTemperature(senderCredibility(alertRecords.value, alert.senderDeviceId, nowTick.value));
}
function selectTheme(key: string | number) {
  if (themeOptions.some((item) => item.key === key)) {
    selectedTheme.value = key as UiThemeKey;
  }
}
function selectLanguage(key: string | number) {
  setLanguagePreference(key);
}

function encodePrivateChannelInvite(invite: PrivateChannelInvitePayload) {
  return `${PRIVATE_CHANNEL_INVITE_PREFIX}${JSON.stringify(invite)}`;
}
function parsePrivateChannelInvite(content: string): PrivateChannelInvitePayload | null {
  if (!content.startsWith(PRIVATE_CHANNEL_INVITE_PREFIX)) return null;
  try {
    const payload = JSON.parse(content.slice(PRIVATE_CHANNEL_INVITE_PREFIX.length)) as PrivateChannelInvitePayload;
    if (!payload.channel_id || !payload.title || !payload.owner_device_id || !payload.channel_key) return null;
    return { ...payload, members: payload.members ?? [] };
  } catch {
    return null;
  }
}
function privateChannelInvitePayload(message: Message) {
  return message.message_type === "text" ? parsePrivateChannelInvite(message.content) : null;
}
function privateChannelInviteKey(invite: PrivateChannelInvitePayload) {
  return `${invite.channel_id}:${invite.created_at || 0}`;
}
function latestPrivateChannelInviteTime(channelId: string) {
  return Math.max(
    0,
    ...Object.values(messagesByConversation.value)
      .flat()
      .map((message) => privateChannelInvitePayload(message))
      .filter((invite): invite is PrivateChannelInvitePayload => invite?.channel_id === channelId)
      .map((invite) => invite.created_at || 0),
  );
}
function privateChannelInviteState(invite: PrivateChannelInvitePayload | null) {
  if (!invite) return "";
  if ((invite.created_at || 0) < latestPrivateChannelInviteTime(invite.channel_id)) return "expired";
  if (conversations.value.some((conversation) => conversation.id === invite.channel_id)) return "accepted";
  return handledPrivateChannelInvites.value[privateChannelInviteKey(invite)] ?? "";
}
async function sendPrivateChannelInviteCards(conversationId: string, targetIds: string[]) {
  const existingMemberIds = new Set((channelMembersByConversation.value[conversationId] ?? []).map((member) => member.device_id));
  const uniqueTargetIds = [...new Set(targetIds.filter(Boolean))].filter((targetId) => !existingMemberIds.has(targetId));
  if (uniqueTargetIds.length === 0) return;
  const invite = await store.buildPrivateChannelInvite(conversationId, superAdminEnabled.value);
  const content = encodePrivateChannelInvite(invite);
  for (const targetId of uniqueTargetIds) {
    await store.sendMessageToConversation(targetId, content);
  }
}
async function acceptPrivateChannelInviteCard(invite: PrivateChannelInvitePayload | null) {
  if (!invite) return;
  await store.acceptPrivateChannelInvite(invite);
  await store.addSystemNotice(invite.channel_id, `${profile.value?.nickname ?? "我"} 加入了群聊`);
  const key = privateChannelInviteKey(invite);
  const { [key]: _ignored, ...rest } = handledPrivateChannelInvites.value;
  handledPrivateChannelInvites.value = rest;
  activeSection.value = "chat";
}
function rejectPrivateChannelInviteCard(invite: PrivateChannelInvitePayload | null) {
  if (!invite) return;
  handledPrivateChannelInvites.value = {
    ...handledPrivateChannelInvites.value,
    [privateChannelInviteKey(invite)]: "rejected",
  };
}
function openRecipientPicker(mode: RecipientPickerMode) {
  recipientPickerMode.value = mode;
  selectedRecipientPeerIds.value = [];
  if (mode === "privateChannelCreate") {
    privateChannelTitleDraft.value = "私有频道";
  }
  recipientPickerOpen.value = true;
}
function toggleRecipientPeer(deviceId: string) {
  selectedRecipientPeerIds.value = selectedRecipientPeerIds.value.includes(deviceId)
    ? selectedRecipientPeerIds.value.filter((id) => id !== deviceId)
    : [...selectedRecipientPeerIds.value, deviceId];
}
async function confirmRecipientPicker() {
  if (recipientConfirmDisabled.value) return;
  if (recipientPickerMode.value === "privateChannelCreate") {
    const selectedTargets = [...selectedRecipientPeerIds.value];
    const conversation = await store.createPrivateChannel(privateChannelTitleDraft.value, []);
    activeSection.value = "chat";
    await sendPrivateChannelInviteCards(conversation.id, selectedTargets);
  } else if (activeConversation.value?.is_private) {
    const selectedTargets = [...selectedRecipientPeerIds.value];
    await sendPrivateChannelInviteCards(activeConversation.value.id, selectedTargets);
  }
  recipientPickerOpen.value = false;
}
async function notifyIncomingActivity() {
  await syncTrayAttention();
  try {
    await getCurrentWindow().requestUserAttention(UserAttentionType.Critical);
  } catch {
    // 浏览器预览时没有 Tauri 窗口对象。
  }
}
function alertRecordFromFrame(alert: QuickAlert): AlertRecord {
  const sender = resolveAlertSender(alert);
  return {
    alertId: alert.alert_id,
    senderDeviceId: alert.sender_device_id,
    senderNickname: sender.nickname,
    senderAddress: sender.address,
    content: alert.content || "呱呱~呱~~",
    mode: normalizePetAlertMode(alert.mode),
    simulation: alert.simulation ?? null,
    createdAt: alert.created_at,
    incoming: alert.sender_device_id !== profile.value?.device_id,
    handled: alert.sender_device_id === profile.value?.device_id,
    feedbacks: [],
  };
}
function startCallPanelDrag(event: MouseEvent) {
  if (event.button !== 0 || typeof window === "undefined") return;
  if ((event.target as HTMLElement | null)?.closest("button")) return;
  const panel = (event.currentTarget as HTMLElement).closest<HTMLElement>(".private-call-float");
  if (!panel) return;
  const bounds = panel.getBoundingClientRect();
  event.preventDefault();
  callPanelDrag = {
    offsetX: event.clientX - bounds.left,
    offsetY: event.clientY - bounds.top,
    width: bounds.width,
    height: bounds.height,
  };
  window.addEventListener("mousemove", moveCallPanel);
  window.addEventListener("mouseup", stopCallPanelDrag, { once: true });
}
function moveCallPanel(event: MouseEvent) {
  if (!callPanelDrag || typeof window === "undefined") return;
  const margin = 8;
  const left = Math.min(
    Math.max(margin, event.clientX - callPanelDrag.offsetX),
    Math.max(margin, window.innerWidth - callPanelDrag.width - margin),
  );
  const top = Math.min(
    Math.max(margin, event.clientY - callPanelDrag.offsetY),
    Math.max(margin, window.innerHeight - callPanelDrag.height - margin),
  );
  callPanelPosition.value = { left, top };
}
function stopCallPanelDrag() {
  if (typeof window !== "undefined") {
    window.removeEventListener("mousemove", moveCallPanel);
    window.removeEventListener("mouseup", stopCallPanelDrag);
  }
  callPanelDrag = null;
}
function resolveAlertSender(alert: Pick<QuickAlert, "sender_device_id" | "sender_nickname" | "sender_address">) {
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, alert.sender_device_id));
  const isSelf = sameDeviceId(alert.sender_device_id, profile.value?.device_id);
  return {
    nickname: alert.sender_nickname?.trim() || peer?.nickname || (isSelf ? profile.value?.nickname : "") || "未知设备",
    address: alert.sender_address?.trim() || peer?.address || null,
  };
}
function applyQuickAlert(alert: QuickAlert) {
  const nextStopped = new Set(visuallyStoppedAlertIds.value);
  nextStopped.delete(alert.alert_id);
  visuallyStoppedAlertIds.value = nextStopped;
  const current = alertRecords.value.find((item) => item.alertId === alert.alert_id);
  if (current) {
    const sender = resolveAlertSender(alert);
    alertRecords.value = alertRecords.value.map((item) =>
      item.alertId === alert.alert_id
        ? {
            ...item,
            senderNickname: sender.nickname || item.senderNickname,
            senderAddress: sender.address ?? item.senderAddress ?? null,
            content: alert.content || item.content,
            mode: normalizePetAlertMode(alert.mode || item.mode),
            simulation: alert.simulation ?? item.simulation ?? null,
            createdAt: alert.created_at || item.createdAt,
          }
        : item,
    );
    return;
  }
  alertRecords.value = normalizeAlertRecords([alertRecordFromFrame(alert), ...alertRecords.value]);
  if (normalizePetAlertMode(alert.mode) === "disco") {
    discoModeUntil.value = Math.max(discoModeUntil.value, Date.now() + petDiscoDurationMs.value);
  }
}
function applyQuickAlertFeedback(feedback: QuickAlertFeedback) {
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
}
async function chooseAdminRemoteUpdatePackage() {
  const selected = await openFileDialog({
    multiple: false,
    directory: false,
    title: "选择 LanChat 更新包",
    filters: [{ name: "LanChat Tauri 更新包", extensions: ["exe", "msi"] }],
  });
  adminRemoteUpdatePackagePath.value = typeof selected === "string" ? selected : "";
}

async function chooseAdminRemoteUpdateSignature() {
  const selected = await openFileDialog({
    multiple: false,
    directory: false,
    title: "选择 LanChat 更新签名文件",
    filters: [{ name: "Tauri 签名文件", extensions: ["sig"] }],
  });
  adminRemoteUpdateSignaturePath.value = typeof selected === "string" ? selected : "";
}

async function issueAdminRemoteUpdate() {
  const targetVersion = adminRemoteUpdateVersion.value.trim();
  const allOnlineWindows = adminRemoteUpdateScope.value === "all_online_windows";
  const targetDeviceIds = allOnlineWindows
    ? []
    : (adminRemoteUpdateTargetId.value ? [adminRemoteUpdateTargetId.value] : []);
  if ((!targetDeviceIds.length && !allOnlineWindows) || !targetVersion || adminRemoteUpdateSending.value) return;
  adminRemoteUpdateSending.value = true;
  try {
    const dispatch = await api.sendAdminRemoteUpdate(
      targetDeviceIds,
      allOnlineWindows,
      targetVersion,
      adminRemoteUpdatePackagePath.value.trim() || null,
      adminRemoteUpdateSignaturePath.value.trim() || null,
      adminRemoteUpdateForce.value,
    );
    adminRemoteUpdateResults.value = dispatch.targets;
    showOperationSuccess(`已创建 ${dispatch.targets.length} 台设备的 ${targetVersion} 更新任务`);
  } catch (err) {
    store.error = stringifyError(err);
  } finally {
    adminRemoteUpdateSending.value = false;
  }
}

async function applyQuickAlertTrustReset(reset: QuickAlertTrustReset) {
  if (reset.target_device_id === QUICK_ALERT_TRUST_RESET_ALL_TARGET) {
    alertRecords.value = [];
    visuallyStoppedAlertIds.value = new Set();
    ownAlertFlashUntil.value = 0;
    nowTick.value = Date.now();
    return;
  }
  alertRecords.value = normalizeAlertRecords(alertRecords.value.map((alert) =>
    alert.senderDeviceId !== reset.target_device_id
      ? alert
      : { ...alert, feedbacks: [], localFeedback: undefined },
  ));
}
function applyAdminDiscoMode(mode: AdminDiscoMode) {
  if (mode.target_device_id !== profile.value?.device_id) return;
  discoModeUntil.value = Math.max(discoModeUntil.value, Date.now() + petDiscoDurationMs.value);
  nowTick.value = Date.now();
}
function applyAdminAlertMode(mode: AdminAlertMode) {
  if (mode.target_device_id !== profile.value?.device_id) return;
  petAlertMode.value = normalizePetAlertMode(mode.mode);
}
async function sendPetQuickAlert(mode: PetAlertMode = petAlertMode.value) {
  if (!petAlertEnabled.value) return;
  const now = Date.now();
  if (now - lastOwnAlertSentAt.value < ALERT_SEND_COOLDOWN_MS) return;
  const credibility = profile.value
    ? senderCredibility(alertRecords.value, profile.value.device_id, Date.now()) ?? 100
    : 100;
  const alert = await store.sendQuickAlert(quickAlertDraft.value || "呱呱~呱~~", mode, Math.round(credibility));
  if (alert) {
    applyQuickAlert(alert);
    lastOwnAlertSentAt.value = now;
    ownAlertFlashUntil.value = Date.now();
    nowTick.value = Date.now();
  }
}
async function resetAlertCredibilityForPeer() {
  if (!superAdminEnabled.value || !alertTrustResetTargetId.value) return;
  const reset = await store.resetQuickAlertCredibility(alertTrustResetTargetId.value);
  if (reset) await applyQuickAlertTrustReset(reset);
}
async function resetAllAlertCredibilityRecords() {
  if (!superAdminEnabled.value) return;
  const reset = await store.resetQuickAlertCredibility(QUICK_ALERT_TRUST_RESET_ALL_TARGET);
  if (reset) await applyQuickAlertTrustReset(reset);
}
async function sendAdminAlertModeToPeer() {
  if (!superAdminEnabled.value || !adminAlertModeTargetId.value) return;
  const mode = await store.sendAdminAlertMode(adminAlertModeTargetId.value, adminAlertModeDraft.value);
  if (mode && mode.target_device_id === profile.value?.device_id) {
    applyAdminAlertMode(mode);
  }
}
async function sendAdminAlertPushPolicyToPeer() {
  if (!superAdminEnabled.value || !adminAlertPushPolicyTargetId.value) return;
  const policy = await store.sendAdminAlertPushPolicy(
    adminAlertPushPolicyTargetId.value,
    Math.max(0, Math.min(100, Math.round(adminAlertPushPolicyDraft.value))),
    adminAlertPushPolicyLockAfterIssue.value,
  );
  if (policy && (policy.target_device_id === "*" || sameDeviceId(policy.target_device_id, profile.value?.device_id))) {
    await desktopPetStore.refreshSettings();
  }
}

function callSignalFrame(callId: string, kind: CallSignal["kind"], media: CallMedia, payload: unknown = {}) {
  return {
    call_id: callId,
    sender_device_id: profile.value?.device_id ?? "",
    sender_nickname: profile.value?.nickname ?? "LanChat",
    kind,
    media,
    payload,
    created_at: Date.now(),
  } satisfies CallSignal;
}
async function ensureCallMediaPlaying(element: HTMLMediaElement | null, role: "远端音频" | "远端视频") {
  if (!element) return;
  try {
    await element.play();
  } catch (error) {
    // 某些 WebView 会在流切换时暂时拒绝自动播放。下一次用户操作或流更新会再次尝试。
    console.debug(`${role}等待用户播放许可`, error);
  }
}
async function attachCallStreams() {
  await nextTick();
  if (localCallVideo.value && callLocalStream) {
    localCallVideo.value.srcObject = callLocalStream;
  }
  if (remoteCallVideo.value && callRemoteStream) {
    remoteCallVideo.value.srcObject = callRemoteStream;
    void ensureCallMediaPlaying(remoteCallVideo.value, "远端视频");
  }
  if (remoteCallAudio.value && callRemoteStream) {
    remoteCallAudio.value.srcObject = callRemoteStream;
    void ensureCallMediaPlaying(remoteCallAudio.value, "远端音频");
  }
  syncDetachedCallWindow();
}
function callStatusLabel(session: CallSession) {
  return session.status === "incoming"
    ? "等待接听"
    : session.status === "outgoing"
      ? "正在呼叫"
      : session.status === "failed"
        ? (session.error ?? "通话未建立")
        : "通话中";
}
function closeDetachedCallWindow() {
  const current = detachedCallWindow;
  detachedCallWindow = null;
  if (current && !current.window.closed) current.window.close();
}
function syncDetachedCallWindow() {
  const current = detachedCallWindow;
  const session = callSession.value;
  if (!current) return;
  if (current.window.closed || !session) {
    detachedCallWindow = null;
    return;
  }
  current.title.textContent = `${session.media === "video" ? "视频" : "语音"}通话 · ${session.peerNickname}`;
  current.status.textContent = callStatusLabel(session);
  if (current.localVideo && callLocalStream) current.localVideo.srcObject = callLocalStream;
  if (current.remoteVideo && callRemoteStream) {
    current.remoteVideo.srcObject = callRemoteStream;
    void ensureCallMediaPlaying(current.remoteVideo, "远端视频");
  }
  if (current.remoteAudio && callRemoteStream) {
    current.remoteAudio.srcObject = callRemoteStream;
    void ensureCallMediaPlaying(current.remoteAudio, "远端音频");
  }
}
function fallbackToInlineCallPanel(error?: unknown) {
  // WebView2 may expose Document Picture-in-Picture but still reject window creation.
  // Calling is independent from its presentation, so keep the active call usable here.
  detachedCallWindowUnavailable = true;
  callPanelExpanded.value = true;
  if (error) console.warn("独立通话窗口不可用，已回退到主界面通话面板", error);
}
async function openDetachedCallWindow() {
  const session = callSession.value;
  if (!session || typeof window === "undefined") return;
  if (detachedCallWindow && !detachedCallWindow.window.closed) {
    detachedCallWindow.window.focus();
    syncDetachedCallWindow();
    return;
  }
  type PictureInPictureApi = { requestWindow: (options: { width: number; height: number }) => Promise<Window> };
  const pictureInPicture = (window as Window & { documentPictureInPicture?: PictureInPictureApi }).documentPictureInPicture;
  if (detachedCallWindowUnavailable || !pictureInPicture?.requestWindow) {
    fallbackToInlineCallPanel();
    return;
  }
  try {
    const popup = await pictureInPicture.requestWindow({
      width: session.media === "video" ? 420 : 320,
      height: session.media === "video" ? 330 : 220,
    });
    const doc = popup.document;
    doc.title = "LanChat 通话";
    doc.documentElement.style.cssText = "width:100%;height:100%;background:#ffffff;";
    doc.body.replaceChildren();
    doc.body.style.cssText = "margin:0;width:100%;height:100%;min-width:100%;min-height:100%;overflow:hidden;background:#ffffff;color:#1f2937;font-family:system-ui,-apple-system,BlinkMacSystemFont,'Microsoft YaHei',sans-serif;";
    const shell = doc.createElement("main");
    shell.style.cssText = "box-sizing:border-box;display:flex;flex-direction:column;width:100%;height:100%;padding:12px;background:#ffffff;";
    const header = doc.createElement("header");
    header.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:10px;";
    const title = doc.createElement("strong");
    title.style.cssText = "min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:14px;";
    const status = doc.createElement("span");
    status.style.cssText = "color:#7c8796;font-size:12px;white-space:nowrap;";
    header.append(title, status);
    shell.append(header);
    let remoteVideo: HTMLVideoElement | null = null;
    let localVideo: HTMLVideoElement | null = null;
    let remoteAudio: HTMLAudioElement | null = null;
    remoteAudio = doc.createElement("audio");
    remoteAudio.autoplay = true;
    remoteAudio.controls = false;
    remoteAudio.style.cssText = "position:absolute;width:1px;height:1px;opacity:0;pointer-events:none;";
    shell.append(remoteAudio);
    if (session.media === "video") {
      const videoStage = doc.createElement("section");
      videoStage.style.cssText = "position:relative;flex:1;min-height:0;border-radius:8px;overflow:hidden;background:#1d2735;";
      remoteVideo = doc.createElement("video");
      remoteVideo.autoplay = true;
      remoteVideo.muted = true;
      remoteVideo.playsInline = true;
      remoteVideo.style.cssText = "display:block;width:100%;height:100%;background:#1d2735;object-fit:cover;";
      localVideo = doc.createElement("video");
      localVideo.autoplay = true;
      localVideo.muted = true;
      localVideo.playsInline = true;
      localVideo.style.cssText = "position:absolute;right:10px;bottom:10px;width:96px;height:72px;border:2px solid #ffffff;border-radius:6px;background:#263241;object-fit:cover;box-shadow:0 4px 14px rgba(0,0,0,.28);";
      videoStage.append(remoteVideo, localVideo);
      shell.append(videoStage);
    } else {
      const audio = doc.createElement("div");
      audio.style.cssText = "display:grid;place-items:center;align-content:center;gap:10px;flex:1;border-radius:8px;background:#f4f7fb;";
      const avatar = doc.createElement("div");
      avatar.textContent = firstLetter(session.peerNickname);
      avatar.style.cssText = "display:grid;place-items:center;width:72px;height:72px;border-radius:50%;background:#1677ff;color:#fff;font-size:28px;font-weight:700;";
      const name = doc.createElement("strong");
      name.textContent = session.peerNickname;
      audio.append(avatar, name);
      shell.append(audio);
    }
    const controls = doc.createElement("footer");
    controls.style.cssText = "display:flex;justify-content:center;gap:8px;margin-top:10px;";
    const hangup = doc.createElement("button");
    hangup.type = "button";
    hangup.textContent = "挂断";
    hangup.style.cssText = "border:0;border-radius:6px;padding:7px 18px;background:#e5484d;color:#fff;cursor:pointer;font:inherit;";
    hangup.addEventListener("click", () => { void endPrivateCall(); });
    controls.append(hangup);
    shell.append(controls);
    doc.body.append(shell);
    detachedCallWindow = { window: popup, title, status, remoteVideo, localVideo, remoteAudio };
    popup.addEventListener("pagehide", () => {
      if (detachedCallWindow?.window === popup) detachedCallWindow = null;
    }, { once: true });
    syncDetachedCallWindow();
  } catch (error) {
    fallbackToInlineCallPanel(error);
  }
}
function formatCallMediaPermissionError(error: unknown, media: CallMedia) {
  const name = error instanceof DOMException ? error.name : "";
  const deviceLabel = media === "video" ? "麦克风和摄像头" : "麦克风";
  if (name === "NotAllowedError" || name === "SecurityError") {
    return `未获得${deviceLabel}权限，请在系统或浏览器权限设置中允许后重试`;
  }
  if (name === "NotFoundError") {
    return media === "video" ? "未检测到可用的麦克风或摄像头设备" : "未检测到可用的麦克风设备";
  }
  if (name === "NotReadableError") {
    return `${deviceLabel}正被其他应用占用，请关闭占用后重试`;
  }
  return `无法启用${deviceLabel}：${stringifyError(error)}`;
}
function callFailureMessage(error: unknown, media: CallMedia) {
  return error instanceof Error && error.message ? error.message : formatCallMediaPermissionError(error, media);
}
function releaseCallMedia() {
  if (callDisconnectTimer) window.clearTimeout(callDisconnectTimer);
  callDisconnectTimer = undefined;
  callPeerConnection?.close();
  callPeerConnection = null;
  if (callSession.value) callMediaCoordinator.releaseCall(callSession.value.media);
  callLocalStream = null;
  callRemoteStream?.getTracks().forEach((track) => track.stop());
  callRemoteStream = null;
  queuedCallCandidates = [];
  if (localCallVideo.value) localCallVideo.value.srcObject = null;
  if (remoteCallVideo.value) remoteCallVideo.value.srcObject = null;
  if (remoteCallAudio.value) remoteCallAudio.value.srcObject = null;
  closeDetachedCallWindow();
}
function createCallPeerConnection(session: CallSession) {
  const peerConnection = new RTCPeerConnection({ iceServers: [] });
  peerConnection.onicecandidate = (event) => {
    if (!event.candidate) return;
    void store.sendCallSignal(session.peerDeviceId, callSignalFrame(session.callId, "ice_candidate", session.media, event.candidate.toJSON())).catch(() => undefined);
  };
  peerConnection.ontrack = (event) => {
    callRemoteStream = event.streams[0] ?? new MediaStream([event.track]);
    void attachCallStreams();
  };
  peerConnection.onconnectionstatechange = () => {
    const current = callSession.value;
    if (!current || current.callId !== session.callId) return;
    if (peerConnection.connectionState === "connected") {
      if (callDisconnectTimer) window.clearTimeout(callDisconnectTimer);
      callDisconnectTimer = undefined;
      callSession.value = { ...current, status: "connected", error: undefined };
      return;
    }
    if (peerConnection.connectionState === "disconnected") {
      if (callDisconnectTimer) window.clearTimeout(callDisconnectTimer);
      callDisconnectTimer = window.setTimeout(() => {
        if (peerConnection.connectionState === "disconnected" && callSession.value?.callId === session.callId) {
          callSession.value = { ...callSession.value, status: "failed", error: "通话网络连接已断开，请重试或挂断" };
        }
      }, 4_000);
      return;
    }
    if (peerConnection.connectionState === "failed") {
      callSession.value = { ...current, status: "failed", error: "通话网络连接失败，请重试或挂断" };
    }
  };
  callPeerConnection = peerConnection;
  return peerConnection;
}
async function prepareLocalCallMedia(media: CallMedia) {
  try {
    callLocalStream = await callMediaCoordinator.acquireForCall(media);
  } catch (error) {
    throw new Error(formatCallMediaPermissionError(error, media));
  }
  await attachCallStreams();
  return callLocalStream;
}
async function queueOrAddCallCandidate(candidate: RTCIceCandidateInit) {
  if (!callPeerConnection || !callPeerConnection.remoteDescription) {
    queuedCallCandidates.push(candidate);
    return;
  }
  await callPeerConnection.addIceCandidate(candidate);
}
async function flushQueuedCallCandidates() {
  if (!callPeerConnection?.remoteDescription) return;
  const candidates = queuedCallCandidates.splice(0);
  for (const candidate of candidates) {
    await callPeerConnection.addIceCandidate(candidate);
  }
}
async function startPrivateCall(media: CallMedia) {
  const peer = activePeer.value;
  if (!peer || !canStartPrivateCall.value || callSession.value) return;
  const session: CallSession = {
    callId: crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    peerDeviceId: peer.device_id,
    peerNickname: peerDisplayName(peer),
    media,
    status: "outgoing",
  };
  try {
    callMuted.value = false;
    callCameraOn.value = media === "video";
    callSession.value = session;
    callPanelExpanded.value = true;
    const stream = await prepareLocalCallMedia(media);
    const peerConnection = createCallPeerConnection(session);
    stream.getTracks().forEach((track) => peerConnection.addTrack(track, stream));
    const offer = await peerConnection.createOffer();
    await peerConnection.setLocalDescription(offer);
    await store.sendCallSignal(peer.device_id, callSignalFrame(session.callId, "offer", media, offer));
  } catch (err) {
    releaseCallMedia();
    const message = callFailureMessage(err, media);
    callSession.value = { ...session, status: "failed", error: message };
    store.error = message;
  }
}
async function sendPrivateNudge() {
  const peer = activePeer.value;
  if (!peer || !canStartPrivateCall.value) return;
  const nudge = await store.sendNudge(peer.device_id);
  if (nudge) await store.addSystemNotice(peer.device_id, `你抖了抖 ${peerDisplayName(peer)}`);
}
async function handleIncomingNudge(nudge: Nudge) {
  activeSection.value = "chat";
  let peer = peers.value.find((item) => sameDeviceId(item.device_id, nudge.sender_device_id));
  if (!peer) {
    await store.refreshPeers();
    peer = peers.value.find((item) => sameDeviceId(item.device_id, nudge.sender_device_id));
  }
  if (peer) await store.openDirect(peer);
  else await store.selectConversation(nudge.sender_device_id);
  await store.addSystemNotice(nudge.sender_device_id, `${nudge.sender_nickname} 抖了一下你`);
  await api.revealAndShakeMainWindow().catch(() => undefined);
}
async function acceptIncomingCall() {
  const signal = incomingCallSignal.value;
  const session = callSession.value;
  if (!signal || !session || session.status !== "incoming") return;
  try {
    callPanelExpanded.value = true;
    const stream = await prepareLocalCallMedia(session.media);
    callMuted.value = false;
    callCameraOn.value = session.media === "video";
    const peerConnection = createCallPeerConnection(session);
    stream.getTracks().forEach((track) => peerConnection.addTrack(track, stream));
    await peerConnection.setRemoteDescription(signal.payload as RTCSessionDescriptionInit);
    await flushQueuedCallCandidates();
    const answer = await peerConnection.createAnswer();
    await peerConnection.setLocalDescription(answer);
    await store.sendCallSignal(session.peerDeviceId, callSignalFrame(session.callId, "answer", session.media, answer));
    callSession.value = { ...session, status: "connected" };
    incomingCallSignal.value = null;
  } catch (err) {
    releaseCallMedia();
    const message = callFailureMessage(err, session.media);
    callSession.value = { ...session, status: "failed", error: message };
    store.error = message;
  }
}
async function rejectIncomingCall() {
  await endPrivateCall("reject");
}
async function retryPrivateCall() {
  const session = callSession.value;
  if (!session || session.status !== "failed") return;
  if (incomingCallSignal.value?.call_id === session.callId) {
    callSession.value = { ...session, status: "incoming", error: undefined };
    await acceptIncomingCall();
    return;
  }
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, session.peerDeviceId));
  clearCallSession();
  if (!peer || !peer.online) {
    store.error = "对方已离线，无法重新发起通话";
    return;
  }
  await store.openDirect(peer);
  await startPrivateCall(session.media);
}
async function openCallConversation(session: CallSession) {
  activeSection.value = "chat";
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, session.peerDeviceId));
  if (peer) await store.openDirect(peer);
  else await store.selectConversation(session.peerDeviceId);
  await api.showFromTray().catch(() => undefined);
}
async function handleDesktopPetCallAction(action: "accept_call" | "reject_call", callId?: string | null) {
  if (callActionInProgress.value) return;
  const session = callSession.value;
  const signal = incomingCallSignal.value;
  if (!session || session.status !== "incoming" || !signal || signal.call_id !== session.callId || (callId && callId !== session.callId)) {
    store.error = "通话邀请已失效，请从聊天界面重新发起通话";
    await syncDesktopPetRuntime();
    return;
  }
  callActionInProgress.value = true;
  try {
    if (action === "accept_call") {
      await acceptIncomingCall();
      if (callSession.value?.status === "connected") {
        await store.addSystemNotice(session.peerDeviceId, `已接听 ${session.peerNickname} 的${session.media === "video" ? "视频" : "语音"}通话`);
      }
    } else {
      await rejectIncomingCall();
      await store.addSystemNotice(session.peerDeviceId, `已拒绝 ${session.peerNickname} 的${session.media === "video" ? "视频" : "语音"}通话`);
    }
    // Do not make the pet action wait for the main window animation. The
    // answer/reject signal must leave first, then the chat can be revealed.
    void openCallConversation(session);
  } finally {
    callActionInProgress.value = false;
    await syncDesktopPetRuntime();
  }
}
function clearCallSession() {
  if (callSession.value) pendingCallCandidatesById.delete(callSession.value.callId);
  releaseCallMedia();
  callSession.value = null;
  incomingCallSignal.value = null;
  callActionInProgress.value = false;
  callMuted.value = false;
  callCameraOn.value = true;
}
function toggleCallMuted() {
  callLocalStream?.getAudioTracks().forEach((track) => {
    track.enabled = !track.enabled;
  });
  callMuted.value = !callMuted.value;
}
function toggleCallCamera() {
  callLocalStream?.getVideoTracks().forEach((track) => {
    track.enabled = !track.enabled;
  });
  callCameraOn.value = !callCameraOn.value;
}
async function endPrivateCall(kind: "hangup" | "reject" = "hangup") {
  const session = callSession.value;
  if (session) {
    await store.sendCallSignal(session.peerDeviceId, callSignalFrame(session.callId, kind, session.media)).catch(() => undefined);
  }
  clearCallSession();
}
async function handleCallSignal(signal: CallSignal) {
  if (sameDeviceId(signal.sender_device_id, profile.value?.device_id)) return;
  if (signal.kind === "offer") {
    if (callSession.value) {
      await store.sendCallSignal(signal.sender_device_id, callSignalFrame(signal.call_id, "reject", signal.media === "video" ? "video" : "audio")).catch(() => undefined);
      return;
    }
    const media: CallMedia = signal.media === "video" ? "video" : "audio";
    queuedCallCandidates = pendingCallCandidatesById.get(signal.call_id) ?? [];
    pendingCallCandidatesById.delete(signal.call_id);
    callSession.value = { callId: signal.call_id, peerDeviceId: signal.sender_device_id, peerNickname: signal.sender_nickname, media, status: "incoming" };
    incomingCallSignal.value = signal;
    return;
  }
  const session = callSession.value;
  if (!session || session.callId !== signal.call_id || session.peerDeviceId !== signal.sender_device_id) {
    if (signal.kind === "ice_candidate") {
      const candidates = pendingCallCandidatesById.get(signal.call_id) ?? [];
      candidates.push(signal.payload as RTCIceCandidateInit);
      pendingCallCandidatesById.set(signal.call_id, candidates.slice(-32));
    }
    return;
  }
  if (signal.kind === "answer" && callPeerConnection) {
    await callPeerConnection.setRemoteDescription(signal.payload as RTCSessionDescriptionInit);
    await flushQueuedCallCandidates();
    callSession.value = { ...session, status: "connected" };
    await store.addSystemNotice(session.peerDeviceId, `${signal.sender_nickname} 已接听${session.media === "video" ? "视频" : "语音"}通话`);
  } else if (signal.kind === "ice_candidate") {
    await queueOrAddCallCandidate(signal.payload as RTCIceCandidateInit);
  } else if (signal.kind === "hangup" || signal.kind === "reject") {
    await store.addSystemNotice(session.peerDeviceId, `${signal.sender_nickname}${signal.kind === "reject" ? " 拒绝了" : " 结束了"}${session.media === "video" ? "视频" : "语音"}通话`);
    clearCallSession();
  }
}
function stopPetAlertVisuals() {
  const localDeviceId = profile.value?.device_id;
  const pendingQuickAlertIds = alertRecords.value
    .filter((item) => item.incoming && !item.handled && item.senderDeviceId !== localDeviceId)
    .map((item) => item.alertId);
  if (activePetAlert.value) pendingQuickAlertIds.push(activePetAlert.value.alertId);
  visuallyStoppedAlertIds.value = new Set([
    ...visuallyStoppedAlertIds.value,
    ...pendingQuickAlertIds,
  ]);

  ownAlertFlashUntil.value = 0;
  lastOwnAlertSentAt.value = 0;
  discoModeUntil.value = 0;
  nowTick.value = Date.now();
  void syncDesktopPetRuntime();
}
async function syncDesktopPetRuntime() {
  const alert = activePetAlert.value;
  const runtimeState: DesktopPetRuntimeState = {
    revision: ++desktopPetRuntimeRevision,
    enabled: petAlertEnabled.value,
    pending_count: pendingAlertCount.value,
    temperature: Number(petAlertProbability.value),
    latest_alert_id: alert?.alertId ?? null,
    latest_alert_kind: alert ? "quick_alert" : null,
    latest_sender: alert?.senderNickname ?? null,
    latest_sender_address: alert?.senderAddress ?? null,
    latest_content: alert ? `${alert.content}${simulationLabel(alert.simulation) ? ` · ${simulationLabel(alert.simulation)}` : ""}` : null,
    latest_created_at: alert?.createdAt ?? null,
    incoming_call_id: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.callId : null,
    incoming_call_sender: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.peerNickname : null,
    incoming_call_media: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.media : null,
    feedbackable: !!latestPendingAlert.value,
    // 停止快捷键只关闭动画，不改变 pending_count，也不替告警提交反馈。
    flashing: !!alert && !visuallyStoppedAlertIds.value.has(alert.alertId),
    disco: discoModeActive.value && (!alert || !visuallyStoppedAlertIds.value.has(alert.alertId)),
    theme_accent: currentTheme.value.accent,
    random_move_enabled: desktopPetSettings.value?.randomMoveEnabled ?? true,
    random_life_enabled: desktopPetSettings.value?.randomLifeEnabled ?? true,
    disco_movement_mode: desktopPetSettings.value?.discoMovementMode ?? "jump",
  };
  await api.updateDesktopPetState(runtimeState).catch(() => undefined);
}
async function feedbackPetAlert(alert: AlertRecord | null, result: AlertFeedbackResult) {
  if (!alert || alert.localFeedback) return;
  alertRecords.value = alertRecords.value.map((item) =>
    item.alertId === alert.alertId ? { ...item, handled: true, localFeedback: result } : item,
  );
  // Keep the detail panel open while another pending alert exists. The runtime
  // switches to the next alert and closes the panel only when pending_count is zero.
  await syncDesktopPetRuntime();
  const feedback = await store.sendQuickAlertFeedback(alert.alertId, alert.senderDeviceId, result);
  if (feedback) {
    applyQuickAlertFeedback(feedback);
  }
}

async function handleDesktopPetFeedbackAction(payload: {
  action: string;
  alert_id?: string | null;
  alert_kind?: string | null;
}) {
  const result: AlertFeedbackResult = payload.action === "feedback_real" ? "real" : "false";
  const alertId = payload.alert_id?.trim() ?? "";
  const quickAlert = alertId
    ? alertRecords.value.find((item) => item.alertId === alertId)
    : latestPendingAlert.value ?? undefined;
  const targetId = quickAlert?.alertId;
  if (!targetId || desktopPetFeedbackInFlight.has(targetId)) return;
  if (quickAlert?.localFeedback) return;

  desktopPetFeedbackInFlight.add(targetId);
  try {
    await feedbackPetAlert(quickAlert, result);
  } finally {
    desktopPetFeedbackInFlight.delete(targetId);
  }
}
function alertProbabilityLabel(alert?: AlertRecord | null) {
  if (!alert) return "0°C";
  const score = alertTruthScore(alert, nowTick.value);
  return score.feedbackCount === 0 ? `${alertDisplayTemperature(alert)}°C` : `${score.probability}%`;
}
function openSection(section: MainSection) {
  if (section === "games" && !gamesFeatureAvailable.value) {
    activeSection.value = "chat";
    return;
  }
  if (section === "alerts" && !petAlertEnabled.value) {
    activeSection.value = "settings";
    return;
  }
  activeSection.value = section;
  if (section === "chat") {
    unreadByConversation.value = { ...unreadByConversation.value, [activeConversationId.value]: 0 };
    void scrollActiveChatToBottom();
  }
  if (section !== "games") {
    listPaneCollapsed.value = false;
  }
}
function toggleListPane() {
  listPaneCollapsed.value = !listPaneCollapsed.value;
}
function toggleNav() {
  navExpanded.value = !navExpanded.value;
}
function appendEmojiToDraft(emoji: string) {
  draft.value += emoji;
  chatEmojiOpen.value = false;
}
function insertMentionToDraft(member?: ChannelMember | Peer) {
  if (!canMentionInActiveConversation.value) return;
  const name = member?.nickname?.trim() || "所有人";
  const prefix = draft.value && !/\s$/.test(draft.value) ? " " : "";
  draft.value = `${draft.value}${prefix}@${name} `;
  mentionPickerOpen.value = false;
  mentionSearch.value = "";
}
function openDevice(peer: Peer) {
  selectedPeerId.value = peer.device_id;
  selectedDeviceChannelId.value = "";
  adminNicknameDraft.value = peer.nickname;
  peerNoteDraft.value = peer.note ?? "";
  adminNicknameLockAfterIssue.value = !!peer.nickname_locked;
}
function openSimulationModal() {
  if (!superAdminEnabled.value || !selectedPeerDetail.value) return;
  simulationKind.value = "channel";
  simulationTargetId.value = DEFAULT_GROUP_ID;
  simulationContent.value = "";
  simulationDisplayLabel.value = true;
  simulationModalOpen.value = true;
}
function openAdminNotificationModal() {
  if (!superAdminEnabled.value) return;
  adminNotificationScope.value = "device";
  adminNotificationTargetId.value = selectedPeerDetail.value?.online
    ? selectedPeerDetail.value.device_id
    : onlinePeers.value[0]?.device_id ?? null;
  adminNotificationTitle.value = "通知";
  adminNotificationContent.value = "";
  adminNotificationTemplate.value = "announcement";
  adminNotificationSupportUrl.value = "";
  adminNotificationDisplayMode.value = "dismissible";
  adminNotificationDeadline.value = "";
  adminNotificationTimeoutPolicy.value = "manual_review";
  adminNotificationForceOpenMainWindow.value = false;
  adminNotificationModalOpen.value = true;
}
async function openAdminNotificationReview() {
  await store.refreshAdminNotifications();
  adminNotificationReviewPage.value = 1;
  adminNotificationReviewOpen.value = true;
}
function triggerAdminNotificationImageSelect() {
  adminNotificationImageInput.value?.click();
}
function clearAdminNotificationImage() {
  adminNotificationSupportUrl.value = "";
  if (adminNotificationImageInput.value) adminNotificationImageInput.value.value = "";
}
function handleAdminNotificationImageSelected(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    store.error = "请选择图片作为公告配图";
    input.value = "";
    return;
  }
  if (file.size > AVATAR_MAX_BYTES) {
    store.error = "公告图片不能超过 5M";
    input.value = "";
    return;
  }
  const reader = new FileReader();
  reader.onload = () => { adminNotificationSupportUrl.value = typeof reader.result === "string" ? reader.result : ""; };
  reader.onerror = () => { store.error = "读取公告图片失败"; };
  reader.readAsDataURL(file);
}
function adminNotificationDeadlineAt() {
  if (!adminNotificationDeadline.value) return null;
  const value = new Date(adminNotificationDeadline.value).getTime();
  return Number.isFinite(value) ? value : -1;
}
async function submitAdminNotification() {
  if (!superAdminEnabled.value || (adminNotificationScope.value === "device" && !adminNotificationTargetId.value)) return;
  if (!adminNotificationTitle.value.trim() || !adminNotificationContent.value.trim()) {
    store.error = "请填写通知标题和内容";
    return;
  }
  if (adminNotificationDeadlineAt() === -1) {
    store.error = "截至时间格式无效，请使用 2026-08-06 18:00";
    return;
  }
  adminNotificationSending.value = true;
  try {
    await store.sendAdminNotification(adminNotificationScope.value === "device" ? adminNotificationTargetId.value : null, adminNotificationScope.value, adminNotificationTitle.value, adminNotificationContent.value, adminNotificationTemplate.value, adminNotificationSupportUrl.value.trim() || null, adminNotificationDisplayMode.value, adminNotificationDeadlineAt(), adminNotificationTimeoutPolicy.value, adminNotificationForceOpenMainWindow.value);
    adminNotificationModalOpen.value = false;
  } finally { adminNotificationSending.value = false; }
}
async function submitBlockingAdminNotification(notification: AdminNotification) {
  try { await store.submitAdminNotification(notification.notification_id); } catch (err) { store.error = String(err); }
}
function dismissAdminAnnouncement(notification: AdminNotification) {
  if (dismissedAdminNotificationIds.value.includes(notification.notification_id)) return;
  dismissedAdminNotificationIds.value = [...dismissedAdminNotificationIds.value, notification.notification_id].slice(-200);
  window.localStorage.setItem("lanchat-dismissed-admin-notifications", JSON.stringify(dismissedAdminNotificationIds.value));
}
async function openAdminNotificationHistory() {
  await store.refreshAdminNotifications();
  adminNotificationHistoryOpen.value = true;
}
async function decideAdminNotification(notification: AdminNotification, decision: "approved" | "rejected" | "revoked") {
  try { await store.decideAdminNotification(notification.notification_id, decision); } catch (err) { store.error = String(err); }
}
function adminNotificationTargetDetail(notification: AdminNotification) {
  return peers.value.find((peer) => sameDeviceId(peer.device_id, notification.target_device_id));
}
function openAdminNotificationDetail(notification: AdminNotification) {
  adminNotificationDetail.value = notification;
  adminNotificationDetailOpen.value = true;
}
function isAdminNotificationIssuer(notification: AdminNotification) {
  return notification.issued_by_device_id === profile.value?.device_id;
}
function isAdminNotificationRecipient(notification: AdminNotification) {
  return notification.target_device_id === profile.value?.device_id;
}
async function submitAdminNotificationFromDetail() {
  if (!adminNotificationDetail.value) return;
  await submitBlockingAdminNotification(adminNotificationDetail.value);
  adminNotificationDetailOpen.value = false;
}
function dismissAdminNotificationFromDetail() {
  if (!adminNotificationDetail.value) return;
  dismissAdminAnnouncement(adminNotificationDetail.value);
  adminNotificationDetailOpen.value = false;
}
async function decideAdminNotificationFromDetail(decision: "approved" | "rejected" | "revoked") {
  if (!adminNotificationDetail.value) return;
  await decideAdminNotification(adminNotificationDetail.value, decision);
  adminNotificationDetailOpen.value = false;
}
async function decideAllSubmittedAdminNotifications(decision: "approved" | "rejected") {
  const pending = adminNotifications.value.filter((item) => item.issued_by_device_id === profile.value?.device_id && item.status === "submitted");
  if (pending.length === 0 || adminNotificationBulkProcessing.value) return;
  adminNotificationBulkProcessing.value = true;
  try {
    for (const notification of pending) await store.decideAdminNotification(notification.notification_id, decision);
  } catch (err) {
    store.error = String(err);
  } finally {
    adminNotificationBulkProcessing.value = false;
  }
}
async function submitSimulation() {
  const simulated = selectedPeerDetail.value;
  if (!simulated || !superAdminEnabled.value) return;
  const content = simulationContent.value.trim() || "呱呱~呱~~";
  if ((simulationKind.value === "direct" || simulationKind.value === "channel") && !simulationContent.value.trim()) {
    store.error = "消息内容不能为空";
    return;
  }
  if ((simulationKind.value === "direct" || simulationKind.value === "channel") && !simulationTargetId.value) {
    store.error = simulationKind.value === "direct" ? "请选择在线接收设备" : "请选择频道";
    return;
  }
  simulationSending.value = true;
  try {
    if (simulationKind.value === "alert" || simulationKind.value === "disco") {
      const alert = await store.simulateQuickAlert(simulated.device_id, content, simulationKind.value === "disco" ? "disco" : "normal", simulationDisplayLabel.value);
      if (alert) applyQuickAlert(alert);
    } else {
      await store.simulateMessage(simulated.device_id, simulationTargetId.value, simulationContent.value, simulationDisplayLabel.value);
    }
    if (!store.error) simulationModalOpen.value = false;
  } finally {
    simulationSending.value = false;
  }
}
async function openDeviceChannel(conversation: Conversation) {
  selectedDeviceChannelId.value = conversation.id;
  selectedPeerId.value = "";
  if (conversation.is_private) {
    await store.loadChannelMembers(conversation.id);
  }
}
async function enterSelectedDeviceChannel() {
  const conversation = selectedDeviceChannelDetail.value;
  if (!conversation) return;
  activeSection.value = "chat";
  await store.selectConversation(conversation.id);
}
async function inviteSelectedDeviceChannelMembers() {
  const conversation = selectedDeviceChannelDetail.value;
  if (!conversation?.is_private) return;
  await store.selectConversation(conversation.id);
  openRecipientPicker("privateChannelInvite");
}
function startEditChannelNotice() {
  try {
    channelNoticeDraft.value = activeChannelNotice.value || DEFAULT_CHANNEL_NOTICE;
    channelNoticeEditing.value = true;
  } catch (err) {
    store.error = stringifyError(err);
  }
}
function cancelEditChannelNotice() {
  try {
    channelNoticeDraft.value = activeChannelNotice.value || DEFAULT_CHANNEL_NOTICE;
    channelNoticeEditing.value = false;
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function saveActiveChannelNotice() {
  const conversationId = activeConversation.value?.id;
  if (!conversationId) return;
  store.error = "";
  const previousNotice = activeChannelNotice.value || DEFAULT_CHANNEL_NOTICE;
  try {
    const text = channelNoticeDraft.value.trim();
    const notice = text || DEFAULT_CHANNEL_NOTICE;
    const updater = profile.value?.nickname ?? "管理员";
    await api.broadcastChannelNotice(conversationId, notice);
    channelNotices.value = {
      ...channelNotices.value,
      [conversationId]: notice,
    };
    channelNoticeEditing.value = false;
    await store.addSystemNotice(conversationId, `${updater} 更新了群公告`);
  } catch (err) {
    channelNoticeDraft.value = previousNotice;
    channelNoticeEditing.value = true;
    store.error = stringifyError(err);
  }
}
function isChannelOwnerMember(member: ChannelMember | Peer) {
  return "is_owner" in member && member.is_owner;
}
function channelMemberMuted(member: ChannelMember | Peer) {
  if (member.device_id === profile.value?.device_id && activeConversation.value?.kind === "group") {
    return channelMutedByConversation.value[activeConversation.value.id] === true;
  }
  return "muted" in member ? member.muted : publicChannelMutedIds.value[member.device_id] === true;
}
function channelMemberPresenceLabel(member: ChannelMember | Peer) {
  return member.device_id === profile.value?.device_id || member.online ? "在线" : "离线";
}
function canManageChannelMember(member: ChannelMember | Peer) {
  if (member.device_id === profile.value?.device_id) return false;
  if (activeConversation.value?.is_private) return canManageActivePrivateChannel.value && !isChannelOwnerMember(member);
  return canManageActivePublicChannel.value;
}
async function toggleActiveChannelMemberMute(member: ChannelMember | Peer) {
  const conversation = activeConversation.value;
  if (!conversation || !canManageChannelMember(member)) return;
  const muted = !channelMemberMuted(member);
  if (conversation.is_private) {
    await store.setPrivateChannelMemberMuted(conversation.id, member.device_id, muted, superAdminEnabled.value);
    await store.addSystemNotice(conversation.id, `${member.nickname} ${muted ? "已被禁言" : "已解除禁言"}`);
    return;
  }
  await store.adminMuteChannelMember(conversation.id, member.device_id, muted);
  await store.addSystemNotice(conversation.id, `${member.nickname} ${muted ? "已被禁言" : "已解除禁言"}`);
  publicChannelMutedIds.value = {
    ...publicChannelMutedIds.value,
    [member.device_id]: muted,
  };
}
async function removeActivePrivateChannelMember(member: ChannelMember | Peer) {
  const conversation = activeConversation.value;
  if (!conversation?.is_private || !canManageActivePrivateChannel.value || isChannelOwnerMember(member)) return;
  if (typeof window !== "undefined" && !window.confirm(`确定将 ${member.nickname} 移出频道吗？`)) return;
  await store.removePrivateChannelMember(conversation.id, member.device_id, superAdminEnabled.value);
  await store.addSystemNotice(conversation.id, `${member.nickname} 已被移出群聊`);
}
async function leaveActivePrivateChannel() {
  const conversation = activeConversation.value;
  if (!conversation?.is_private) return;
  if (sameDeviceId(conversation.owner_device_id, profile.value?.device_id)) {
    store.error = "群主不能退出频道，请使用解散频道";
    return;
  }
  if (typeof window !== "undefined" && !window.confirm(`确定退出「${conversation.title}」吗？`)) return;
  try {
    await store.leavePrivateChannel(conversation.id);
    await store.selectConversation(DEFAULT_GROUP_ID);
    activeSection.value = "chat";
    showOperationSuccess(`已退出「${conversation.title}」`);
  } catch (error) {
    store.error = stringifyError(error);
  }
}
async function dissolveActivePrivateChannel() {
  const conversation = activeConversation.value;
  if (!conversation?.is_private || !canManageActivePrivateChannel.value) return;
  if (typeof window !== "undefined" && !window.confirm(`确定解散「${conversation.title}」吗？解散后成员将无法继续在此频道聊天。`)) return;
  try {
    await store.dissolvePrivateChannel(conversation.id, superAdminEnabled.value);
    await store.selectConversation(DEFAULT_GROUP_ID);
    activeSection.value = "chat";
    showOperationSuccess(`已解散「${conversation.title}」`);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function dissolveSelectedDeviceChannel() {
  const conversation = selectedDeviceChannelDetail.value;
  if (!conversation?.is_private || !canManageSelectedDeviceChannel.value) return;
  if (typeof window !== "undefined" && !window.confirm(`确定解散「${conversation.title}」吗？`)) return;
  try {
    await store.dissolvePrivateChannel(conversation.id, superAdminEnabled.value);
    selectedDeviceChannelId.value = "";
    await store.selectConversation(DEFAULT_GROUP_ID);
    activeSection.value = "chat";
    showOperationSuccess(`已解散「${conversation.title}」`);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
function showOperationSuccess(message: string) {
  operationNotice.value = message;
  if (operationNoticeTimer) clearTimeout(operationNoticeTimer);
  operationNoticeTimer = setTimeout(() => {
    operationNotice.value = "";
  }, 3600);
}

function closeOperationError() {
  store.error = "";
  desktopPetError.value = "";
  autostartError.value = "";
  updateError.value = "";
}
async function startDirectChat(peer = selectedPeerDetail.value) {
  if (!peer) return;
  activeSection.value = "chat";
  await store.openDirect(peer);
}
function requestDeleteDirectConversation(conversation: Conversation) {
  if (conversation.kind !== "direct") return;
  pendingDeleteDirectConversation.value = conversation;
  deleteDirectConversationOpen.value = true;
}
async function confirmDeleteDirectConversation() {
  const conversation = pendingDeleteDirectConversation.value;
  if (!conversation) return;
  try {
    await store.deleteDirectConversation(conversation.id);
    deleteDirectConversationOpen.value = false;
    pendingDeleteDirectConversation.value = null;
    showOperationSuccess(`已删除与「${conversationDisplayName(conversation)}」的本机对话记录`);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function deleteSelectedPeer() {
  const peer = selectedPeerDetail.value;
  if (!peer) return;
  await store.deletePeer(peer.device_id);
  selectedPeerId.value = "";
}
async function saveSelectedPeerNote() {
  const peer = selectedPeerDetail.value;
  if (!peer) return;
  const updated = await store.updatePeerNote(peer.device_id, peerNoteDraft.value);
  selectedPeerId.value = updated.device_id;
  peerNoteDraft.value = updated.note ?? "";
}
async function adminRenameSelectedPeer() {
  const peer = selectedPeerDetail.value;
  const nickname = adminNicknameDraft.value.trim();
  if (!peer || !nickname) return;
  const updated = await store.adminRenamePeer(peer.device_id, nickname, adminNicknameLockAfterIssue.value ? true : null);
  selectedPeerId.value = updated.device_id;
  adminNicknameDraft.value = updated.nickname;
  adminNicknameLockAfterIssue.value = !!updated.nickname_locked;
}
async function adminUnlockSelectedPeerNickname() {
  const peer = selectedPeerDetail.value;
  const nickname = adminNicknameDraft.value.trim() || peer?.nickname || "";
  if (!peer || !nickname) return;
  const updated = await store.adminRenamePeer(peer.device_id, nickname, false);
  selectedPeerId.value = updated.device_id;
  adminNicknameDraft.value = updated.nickname;
  adminNicknameLockAfterIssue.value = false;
}
async function adminUseSystemUsernameForSelectedPeer() {
  const peer = selectedPeerDetail.value;
  if (!peer) return;
  const updated = await store.adminRenamePeer(peer.device_id, "", adminNicknameLockAfterIssue.value ? true : null, true);
  selectedPeerId.value = updated.device_id;
}
function peerLastSeenLabel(peer?: Peer | null) {
  if (!peer?.last_seen_at) return "未知";
  const diff = Date.now() - peer.last_seen_at;
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(new Date(peer.last_seen_at));
}
function peerSupportsFullFeatures(peer?: Peer | null) {
  return !!peer && peer.supports_chat !== false;
}
function peerClientKindLabel(peer?: Peer | null) {
  return peer?.supports_chat === false ? "受限设备" : "完整版";
}
function peerBuildVersionLabel(peer?: Peer | null) {
  return peer?.build_version?.trim() || "未知";
}
function peerBuildTimeLabel(peer?: Peer | null) {
  const value = peer?.build_timestamp ?? 0;
  if (!value) return "未知";
  if (value >= 20_000_000_000_000) {
    const text = String(value);
    const date = new Date(
      Number(text.slice(0, 4)),
      Number(text.slice(4, 6)) - 1,
      Number(text.slice(6, 8)),
      Number(text.slice(8, 10)),
      Number(text.slice(10, 12)),
      Number(text.slice(12, 14)),
    );
    return `${value} · ${new Intl.DateTimeFormat("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }).format(date)}`;
  }
  const millis = value > 10_000_000_000 ? value : value * 1000;
  return `${value} · ${new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(millis))}`;
}
function setSavedSuperAdminEnabled(enabled: boolean) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem("lanchat-super-admin-enabled", enabled ? "true" : "false");
}
async function restoreSavedSuperAdminSession() {
  if (typeof window === "undefined" || window.localStorage.getItem("lanchat-super-admin-enabled") !== "true") return;
  const authenticated = await api.isSuperAdminAuthenticated().catch(() => false);
  superAdminEnabled.value = authenticated;
  if (!authenticated) setSavedSuperAdminEnabled(false);
}
function disableSuperAdmin() {
  superAdminEnabled.value = false;
  superAdminTapCount.value = 0;
  superAdminAuthOpen.value = false;
  superAdminPasswordDraft.value = "";
  superAdminPasswordError.value = "";
  setSavedSuperAdminEnabled(false);
  void api.clearSuperAdminSession();
}
function handleSuperAdminTap() {
  if (superAdminEnabled.value) {
    disableSuperAdmin();
    return;
  }
  superAdminTapCount.value += 1;
  if (superAdminTapCount.value >= 8) {
    superAdminAuthOpen.value = true;
    superAdminTapCount.value = 0;
    superAdminPasswordDraft.value = "";
    superAdminPasswordError.value = "";
  }
}
async function confirmSuperAdminPassword() {
  const actual = CryptoJS.MD5(superAdminPasswordDraft.value).toString().toUpperCase();
  if (actual !== SUPER_ADMIN_PASSWORD_MD5.toUpperCase()) {
    superAdminPasswordError.value = "验证失败";
    return;
  }
  try {
    await api.authenticateSuperAdmin(superAdminPasswordDraft.value);
  } catch {
    superAdminPasswordError.value = "验证失败";
    return;
  }
  superAdminEnabled.value = true;
  superAdminAuthOpen.value = false;
  superAdminPasswordDraft.value = "";
  superAdminPasswordError.value = "";
  setSavedSuperAdminEnabled(true);
}
async function openManualDevice() {
  activeSection.value = "chat";
  await store.connectManualPeer();
}
function formatTime(value: number) {
  if (!value) return "";
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
function formatDateTime(value?: number | null) {
  if (!value) return "未知";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
function updateNotesPreview(value?: string | null) {
  const text = value?.trim() ?? "";
  if (!text) return "暂无更新说明。";
  return text.length > 260 ? `${text.slice(0, 260)}...` : text;
}
function formatDebugTime(value: number) {
  if (!value) return "";
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}
function conversationPeer(conversation: Conversation) {
  if (conversation.kind !== "direct") return null;
  const peerId = conversation.peer_device_id ?? conversation.id;
  return peers.value.find((peer) => peer.device_id === peerId) ?? null;
}
function conversationBadge(conversation: Conversation) {
  if (conversation.kind === "group") return conversation.is_private ? "私有" : "频道";
  return conversationPeer(conversation)?.online ? "在线" : "离线";
}
function conversationTagType(conversation: Conversation) {
  if (conversation.kind === "group") return conversation.is_private ? "warning" : "success";
  return conversationPeer(conversation)?.online ? "success" : "default";
}
function conversationSubtitle(conversation: Conversation) {
  if (conversation.kind === "group") return conversation.is_private ? "私有加密频道" : `${onlinePeers.value.length} 台设备在线`;
  const peer = conversationPeer(conversation);
  if (peer && !peerSupportsFullFeatures(peer)) return "受限设备";
  return peer ? `${peer.address}:${peer.port}` : "设备未在列表中";
}
function messageClass(message: Message) {
  if (message.message_type === "system") return "system";
  return message.sender_device_id === profile.value?.device_id ? "mine" : "theirs";
}
function senderName(message: Message) {
  if (message.sender_device_id === profile.value?.device_id) {
    return profile.value?.nickname || "我";
  }
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, message.sender_device_id));
  return peer ? peerDisplayName(peer) : "局域网用户";
}
function conversationDisplayName(conversation: Conversation) {
  if (conversation.kind === "group") return conversation.title;
  const peer = conversationPeer(conversation);
  return peer ? peerDisplayName(peer) : conversation.title;
}
function messageSenderTitle(message: Message) {
  if (message.message_type === "system") return "";
  return messageClass(message) === "mine" ? `我 · ${senderName(message)}` : senderName(message);
}
function messageTextSegments(content: string) {
  return content
    .split(/(@[^\s@]{1,32})/g)
    .filter(Boolean)
    .map((text) => ({ text, mention: text.startsWith("@") }));
}
function canRecallMessage(message?: Message | null) {
  return !!message && message.sender_device_id === profile.value?.device_id && message.message_type !== "system" && message.status !== "failed";
}
async function recallMessage(message: Message) {
  if (!canRecallMessage(message)) return;
  await store.recallMessage(message.id);
}
function openMessageContextMenu(message: Message, event: MouseEvent) {
  if (!canRecallMessage(message)) return;
  event.preventDefault();
  messageContextMessage.value = message;
  messageContextMenuX.value = event.clientX;
  messageContextMenuY.value = event.clientY;
  messageContextMenuOpen.value = true;
}
async function selectMessageContextAction(key: string | number) {
  const message = messageContextMessage.value;
  messageContextMenuOpen.value = false;
  if (key === "recall" && message) {
    await recallMessage(message);
  }
}
function peerSubtitle(peer: Peer) {
  const kind = peerSupportsFullFeatures(peer) ? "完整版" : "受限设备";
  const originalName = peerOriginalName(peer);
  return [originalName ? `原昵称：${originalName}` : "", kind, `${peer.address}:${peer.port}`].filter(Boolean).join(" · ");
}
function simulationLabel(meta?: SimulationMeta | null) {
  return meta?.display_label ? `超管模拟发送 · ${meta.operator_nickname}` : "";
}
function memberDisplayName(member: ChannelMember | Peer) {
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, member.device_id));
  return peer ? peerDisplayName(peer) : member.nickname;
}
function sortChannelMembers(members: readonly (ChannelMember | Peer)[]) {
  return [...members].sort((left, right) => {
    const leftSelf = sameDeviceId(left.device_id, profile.value?.device_id);
    const rightSelf = sameDeviceId(right.device_id, profile.value?.device_id);
    if (leftSelf !== rightSelf) return leftSelf ? -1 : 1;
    if (left.online !== right.online) return left.online ? -1 : 1;
    return memberDisplayName(left).localeCompare(memberDisplayName(right), "zh-CN");
  });
}
function memberSubtitle(member: ChannelMember | Peer) {
  if ("address" in member) return peerSubtitle(member);
  return [channelMemberPresenceLabel(member), isChannelOwnerMember(member) ? "群主" : "成员", channelMemberMuted(member) ? "已禁言" : ""].filter(Boolean).join(" · ");
}
function openMemberDevice(member: ChannelMember | Peer) {
  const peer = peers.value.find((item) => sameDeviceId(item.device_id, member.device_id));
  if (!peer) return;
  openDevice(peer);
  activeSection.value = "devices";
}
function statusText(status: Message["status"]) {
  const map = {
    sending: "发送中",
    sent: "已发送",
    delivered: "已送达",
    failed: "失败",
  } satisfies Record<Message["status"], string>;
  return map[status];
}
function statusIcon(status: Message["status"]) {
  const map = {
    sending: "◷",
    sent: "✓",
    delivered: "✓",
    failed: "!",
  } satisfies Record<Message["status"], string>;
  return map[status];
}
function firstLetter(value: string | undefined) {
  return value?.trim().slice(0, 1).toUpperCase() || "L";
}
function stringifyError(err: unknown) {
  return err instanceof Error ? err.message : String(err);
}
function avatarLabel(value: string | undefined | null, fallback?: string) {
  const text = value?.trim() || fallback?.trim() || "L";
  return text.slice(0, 1).toUpperCase();
}
function avatarImage(value: string | undefined | null) {
  const trimmed = value?.trim() ?? "";
  if (!trimmed) return undefined;
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://") || trimmed.startsWith("blob:")) {
    return trimmed;
  }
  if (trimmed.startsWith("data:image/")) {
    void cacheAvatarBlobUrl(trimmed);
    return avatarBlobUrls.value[trimmed] ?? undefined;
  }
  const payload = trimmed.includes(",") ? trimmed.split(",").pop()?.trim() ?? "" : trimmed;
  if (!payload || !/^[A-Za-z0-9+/=_-]+$/.test(payload)) return undefined;
  const mime = payload.startsWith("/9j/")
    ? "image/jpeg"
    : payload.startsWith("R0lG")
      ? "image/gif"
      : payload.startsWith("UklGR")
        ? "image/webp"
        : "image/png";
  const dataUrl = `data:${mime};base64,${payload}`;
  void cacheAvatarBlobUrl(dataUrl);
  return avatarBlobUrls.value[dataUrl] ?? undefined;
}

async function cacheAvatarBlobUrl(dataUrl: string) {
  if (avatarBlobUrls.value[dataUrl] || avatarBlobUrlPending.has(dataUrl)) return;
  avatarBlobUrlPending.add(dataUrl);
  try {
    const response = await fetch(dataUrl);
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const next = { ...avatarBlobUrls.value, [dataUrl]: url };
    avatarBlobUrlOrder.set(dataUrl, Date.now());
    while (Object.keys(next).length > 80) {
      const oldest = [...avatarBlobUrlOrder.entries()].sort((a, b) => a[1] - b[1])[0]?.[0];
      if (!oldest) break;
      const oldUrl = next[oldest];
      if (oldUrl) URL.revokeObjectURL(oldUrl);
      delete next[oldest];
      avatarBlobUrlOrder.delete(oldest);
    }
    avatarBlobUrls.value = next;
  } catch {
    // 无法转成 Blob URL 时回退为首字母头像，避免重复解码大 Base64。
  } finally {
    avatarBlobUrlPending.delete(dataUrl);
  }
}

async function compressAvatarImage(file: File) {
  const sourceUrl = URL.createObjectURL(file);
  try {
    const source = await new Promise<HTMLImageElement>((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = () => reject(new Error("无法解码头像图片"));
      image.src = sourceUrl;
    });
    const canvas = document.createElement("canvas");
    canvas.width = 256;
    canvas.height = 256;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("当前环境不支持头像压缩");
    const scale = Math.max(256 / source.naturalWidth, 256 / source.naturalHeight);
    const width = source.naturalWidth * scale;
    const height = source.naturalHeight * scale;
    context.drawImage(source, (256 - width) / 2, (256 - height) / 2, width, height);
    const blob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((result) => result ? resolve(result) : reject(new Error("头像压缩失败")), "image/webp", 0.82);
    });
    if (blob.size > 160 * 1024) throw new Error("头像压缩后仍然过大，请换一张更简单的图片");
    return await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => typeof reader.result === "string" ? resolve(reader.result) : reject(new Error("读取压缩头像失败"));
      reader.onerror = () => reject(new Error("读取压缩头像失败"));
      reader.readAsDataURL(blob);
    });
  } finally {
    URL.revokeObjectURL(sourceUrl);
  }
}
function peerAvatar(deviceId: string | undefined | null) {
  if (!deviceId) return undefined;
  if (deviceId === profile.value?.device_id) return profile.value.avatar;
  return peers.value.find((peer) => peer.device_id === deviceId)?.avatar;
}
function senderAvatar(message: Message) {
  return peerAvatar(message.sender_device_id);
}
function conversationAvatar(conversation: Conversation) {
  const peer = conversationPeer(conversation);
  return peer?.avatar;
}
function triggerProfileAvatarSelect() {
  profileAvatarInput.value?.click();
}
function clearProfileAvatar() {
  avatarDraft.value = "";
  if (profileAvatarInput.value) profileAvatarInput.value.value = "";
}
async function handleProfileAvatarSelected(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    store.error = "请选择图片作为头像";
    input.value = "";
    return;
  }
  if (file.size > AVATAR_MAX_BYTES) {
    store.error = "头像图片不能超过 5M";
    input.value = "";
    return;
  }
  try {
    avatarDraft.value = await compressAvatarImage(file);
  } catch (err) {
    store.error = stringifyError(err);
  } finally {
    input.value = "";
  }
}
function formatFileSize(size?: number | null) {
  if (!size) return "0 B";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
function base64ByteLength(value: string | null | undefined) {
  const payload = value?.split(",").pop()?.replace(/\s/g, "") ?? "";
  if (!payload) return 0;
  return Math.max(0, Math.floor((payload.length * 3) / 4) - (payload.endsWith("==") ? 2 : payload.endsWith("=") ? 1 : 0));
}
async function refreshMemoryDiagnostic() {
  previewMediaCacheInfo.value = await api.getPreviewMediaCacheInfo().catch(() => previewMediaCacheInfo.value);
  const performanceWithMemory = performance as Performance & { memory?: { usedJSHeapSize: number; jsHeapSizeLimit: number } };
  const messages = Object.values(messagesByConversation.value);
  memoryDiagnostic.value = {
    jsHeapBytes: performanceWithMemory.memory?.usedJSHeapSize ?? null,
    jsHeapLimitBytes: performanceWithMemory.memory?.jsHeapSizeLimit ?? null,
    cachedConversations: messages.length,
    cachedMessages: messages.reduce((sum, items) => sum + items.length, 0),
    visibleMessages: visibleMessages.value.length,
    avatarBytes: [profile.value?.avatar, ...peers.value.map((peer) => peer.avatar)].reduce((sum, avatar) => sum + base64ByteLength(avatar), 0),
    previewCacheBytes: previewMediaCacheInfo.value?.totalBytes ?? 0,
  };
}
function fileExtension(name?: string) {
  return name?.split(".").pop()?.toLowerCase() ?? "";
}
function isImageFile(message: Message) {
  const meta = message.file_meta;
  const ext = fileExtension(meta?.name);
  return Boolean(meta?.mime_type?.startsWith("image/")) || ["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(ext);
}
function imagePreviewSource(message: Message) {
  const cachedPath = previewMediaPaths.value[message.id];
  if (!cachedPath) return message.file_meta?.url ?? "";
  const cachedUrl = imagePreviewBlobUrls.get(message.id);
  if (cachedUrl) return cachedUrl;
  const url = convertFileSrc(cachedPath);
  imagePreviewBlobUrls.set(message.id, url);
  return url;
}
function imageThumbnailSource(message: Message) {
  const url = message.file_meta?.url ?? "";
  return url ? `${url}${url.includes("?") ? "&" : "?"}thumbnail=1` : "";
}
async function openImagePreview(message: Message) {
  imagePreviewScale.value = 1;
  imagePreviewMessage.value = message;
  await cacheImagePreview(message);
}
function closeImagePreview() {
  imagePreviewMessage.value = null;
  imagePreviewScale.value = 1;
}
function changeImagePreviewScale(step: number) {
  imagePreviewScale.value = Math.min(4, Math.max(0.25, Number((imagePreviewScale.value + step).toFixed(2))));
}
function handleImagePreviewWheel(event: WheelEvent) {
  changeImagePreviewScale(event.deltaY < 0 ? 0.2 : -0.2);
}
async function cacheImagePreview(message: Message) {
  if (!isImageFile(message) || !message.file_meta?.url || previewMediaPaths.value[message.id]) return;
  try {
    const path = await api.cachePreviewMedia(message.id, message.file_meta.url, message.file_meta.name);
    previewMediaPaths.value = { ...previewMediaPaths.value, [message.id]: path };
    previewMediaCacheInfo.value = await api.getPreviewMediaCacheInfo().catch(() => previewMediaCacheInfo.value);
  } catch {
    // 预览缓存失败时继续使用发送方的临时文件服务地址。
  }
}
async function clearImagePreviewCache() {
  previewMediaCacheClearing.value = true;
  try {
    previewMediaCacheInfo.value = await api.clearPreviewMediaCache();
    imagePreviewBlobUrls.forEach((url) => URL.revokeObjectURL(url));
    imagePreviewBlobUrls.clear();
    previewMediaPaths.value = {};
  } catch (err) {
    store.error = stringifyError(err);
  } finally {
    previewMediaCacheClearing.value = false;
  }
}
function isAudioFile(message: Message) {
  const meta = message.file_meta;
  const ext = fileExtension(meta?.name);
  return message.message_type === "voice" || Boolean(meta?.mime_type?.startsWith("audio/")) || ["mp3", "wav", "ogg", "m4a", "webm"].includes(ext);
}
async function saveProfile() {
  await store.saveProfile(nicknameDraft.value, portDraft.value, avatarDraft.value);
}
async function requestCallDevicePermission(media: CallMedia) {
  if (!navigator.mediaDevices?.getUserMedia) {
    store.error = "当前环境不支持麦克风或摄像头权限申请";
    return;
  }
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: media === "video" });
    stream.getTracks().forEach((track) => track.stop());
    showOperationSuccess(media === "video" ? "麦克风和摄像头权限已授权" : "麦克风权限已授权");
  } catch (error) {
    store.error = formatCallMediaPermissionError(error, media);
  }
}

async function chooseAndSendFile() {
  if (!canSendActive.value) return;
  const selected = await openFileDialog({ multiple: false, directory: false });
  const path = Array.isArray(selected) ? selected[0] : selected;
  if (typeof path === "string" && path) {
    await store.sendFile(path);
  }
}
async function sendPastedImageFile(file: File) {
  if (!file.type.startsWith("image/")) return;
  const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
  await store.sendPastedImage(file.name || `paste-image-${Date.now()}.png`, bytes, file.type || "image/png");
}
async function handleComposerPaste(event: ClipboardEvent) {
  if (!canSendActive.value) return;
  const files = Array.from(event.clipboardData?.files ?? []).filter((file) => file.type.startsWith("image/"));
  if (files.length === 0) return;
  event.preventDefault();
  for (const file of files) {
    await sendPastedImageFile(file);
  }
}
async function toggleVoiceRecording() {
  if (!canSendActive.value) return;
  if (isRecording.value) {
    mediaRecorder?.stop();
    return;
  }
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    recordingChunks = [];
    mediaRecorder = new MediaRecorder(stream, { mimeType: "audio/webm" });
    recordingStartedAt.value = Date.now();
    mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) recordingChunks.push(event.data);
    };
    mediaRecorder.onstop = async () => {
      const durationMs = Date.now() - recordingStartedAt.value;
      stream.getTracks().forEach((track) => track.stop());
      isRecording.value = false;
      if (recordingTimer !== null) {
        window.clearTimeout(recordingTimer);
        recordingTimer = null;
      }
      const blob = new Blob(recordingChunks, { type: "audio/webm" });
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      await store.sendVoice(`voice-${Date.now()}.webm`, bytes, durationMs);
    };
    mediaRecorder.start();
    isRecording.value = true;
    recordingTimer = window.setTimeout(() => mediaRecorder?.stop(), 60_000);
  } catch (err) {
    store.error = err instanceof Error ? err.message : String(err);
  }
}
async function startWindowDrag(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("button, input, textarea, .titlebar-actions")) return;
  try {
    await api.startMainWindowDrag();
  } catch {
    try {
      await getCurrentWindow().startDragging();
    } catch {
      // 浏览器预览时没有 Tauri 窗口对象。
    }
  }
}
async function minimizeWindow() {
  try {
    await api.minimizeMainWindow();
  } catch {
    try {
      await getCurrentWindow().minimize();
    } catch {
      // 浏览器预览时没有 Tauri 窗口对象。
    }
  }
}
async function toggleMaximizeWindow() {
  try {
    await api.toggleMainWindowMaximized();
  } catch {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch {
      // 浏览器预览时没有 Tauri 窗口对象。
    }
  }
}
async function closeWindow() {
  await syncTrayAttention();
  try {
    await api.hideToTray();
  } catch {
    try {
      await getCurrentWindow().close();
    } catch {
      // 浏览器预览时没有 Tauri 窗口对象。
    }
  }
}
</script>
<template>
  <NConfigProvider :theme-overrides="themeOverrides" :locale="naiveLocale" :date-locale="dateLocale" :class="['provider-root', selectedTheme]">
    <NMessageProvider>
      <NModal
        :show="updateReminderOpen"
        preset="card"
        class="update-reminder-modal"
        :mask-closable="!forceUpdateRequired"
        :close-on-esc="!forceUpdateRequired"
        :closable="!forceUpdateRequired"
        @update:show="handleUpdateReminderShowChange"
      >
        <div class="force-update-panel">
          <NTag :type="forceUpdateRequired ? 'error' : 'warning'" :bordered="false">{{ forceUpdateRequired ? '必须更新' : '发现新版本' }}</NTag>
          <h2>{{ updateInfo?.title || `LanChat ${updateInfo?.latestVersion ?? ''}` }}</h2>
          <p>{{ forceUpdateRequired ? '当前版本已停止支持，请下载并安装新版本后继续使用。' : '有新版本可以安装，建议更新后继续使用。绿色版可以下载 ZIP 后解压覆盖当前目录。' }}</p>
          <div class="update-version-grid">
            <span>当前版本</span><strong>{{ localVersionLabel }}</strong>
            <span>最新版本</span><strong>{{ updateInfo?.latestVersion ?? "未知" }}</strong>
            <span>检查时间</span><strong>{{ formatDateTime(updateInfo?.checkedAt) }}</strong>
          </div>
          <pre class="update-notes">{{ updateNotesPreview(updateInfo?.notes) }}</pre>
          <div v-if="nativeUpdateInstalling" class="update-progress-panel">
            <NProgress type="line" :percentage="nativeUpdateProgressPercent" :height="10" processing />
            <span>{{ nativeUpdateProgressLabel }}</span>
          </div>
          <div class="force-update-actions">
            <NButton type="primary" size="large" :loading="nativeUpdateInstalling" @click="installNativeUpdate(false)">自动更新</NButton>
            <NButton secondary size="large" @click="openPreferredUpdateUrl">手动下载</NButton>
            <NButton secondary size="large" @click="openReleasePage">Release 页面</NButton>
            <NButton v-if="!forceUpdateRequired" quaternary size="large" @click="dismissUpdateReminder">稍后提醒</NButton>
            <NButton v-else quaternary size="large" @click="api.quitApp">退出软件</NButton>
          </div>
        </div>
      </NModal>
      <NModal
        v-model:show="deleteDirectConversationOpen"
        preset="card"
        title="删除对话"
        class="delete-conversation-modal"
        @after-leave="pendingDeleteDirectConversation = null"
      >
        <p>将删除本机与「{{ pendingDeleteDirectConversation ? conversationDisplayName(pendingDeleteDirectConversation) : '' }}」的聊天记录。对方设备和对方保存的消息不会被删除。</p>
        <NSpace justify="end">
          <NButton @click="deleteDirectConversationOpen = false">取消</NButton>
          <NButton type="error" @click="confirmDeleteDirectConversation">删除</NButton>
        </NSpace>
      </NModal>
      <Teleport to="body">
        <aside
          v-if="callSession"
          class="private-call-float"
          :class="{ video: callSession.media === 'video', expanded: callPanelExpanded }"
          :style="callPanelStyle"
          role="dialog"
          aria-label="语音或视频通话"
        >
          <div v-if="callSession" class="private-call-panel">
            <audio ref="remoteCallAudio" class="remote-call-audio" autoplay></audio>
            <div class="private-call-title" @mousedown="startCallPanelDrag">
              <div class="private-call-summary">
                <strong>{{ callSession.status === 'incoming' ? `${callSession.peerNickname} 邀请${callSession.media === 'video' ? '视频' : '语音'}通话` : `${callSession.media === 'video' ? '视频' : '语音'}通话 · ${callSession.peerNickname}` }}</strong>
                <span>{{ callSession.status === 'incoming' ? '等待接听' : callSession.status === 'outgoing' ? '正在呼叫' : callSession.status === 'failed' ? (callSession.error ?? '通话未建立') : '已连接' }}</span>
              </div>
              <button class="private-call-toggle" type="button" title="弹出独立通话窗口" @click="openDetachedCallWindow">▣</button>
              <button class="private-call-toggle" type="button" :title="callPanelExpanded ? '收起画面' : '展开画面'" @click="callPanelExpanded = !callPanelExpanded">{{ callPanelExpanded ? '⌃' : '⌄' }}</button>
            </div>
          <div v-if="callPanelExpanded" class="private-call-videos" :class="{ audio: callSession.media === 'audio' }">
            <video v-if="callSession.media === 'video'" ref="remoteCallVideo" autoplay muted playsinline></video>
            <video v-if="callSession.media === 'video'" ref="localCallVideo" autoplay muted playsinline></video>
            <div v-else class="private-call-audio-profile">
              <img v-if="avatarImage(peerAvatar(callSession.peerDeviceId))" :src="avatarImage(peerAvatar(callSession.peerDeviceId))" class="private-call-audio-avatar-image" alt="对方头像" />
              <div v-else class="private-call-audio-avatar">{{ firstLetter(callSession.peerNickname) }}</div>
              <strong>{{ callSession.peerNickname }}</strong>
              <span>{{ callSession.status === 'connected' ? '语音通话中' : callSession.status === 'incoming' ? '邀请你进行语音通话' : callSession.status === 'failed' ? (callSession.error ?? '通话未建立') : '正在等待对方接听' }}</span>
            </div>
          </div>
          <NSpace justify="center">
            <template v-if="callSession.status === 'incoming'">
              <NButton type="primary" @click="() => acceptIncomingCall()">接听</NButton>
              <NButton type="error" @click="rejectIncomingCall">拒绝</NButton>
            </template>
            <template v-else-if="callSession.status === 'failed'">
              <NButton type="primary" :loading="callActionInProgress" @click="retryPrivateCall">重新尝试</NButton>
              <NButton type="error" @click="() => endPrivateCall()">关闭</NButton>
            </template>
            <template v-else>
              <NButton secondary @click="toggleCallMuted">{{ callMuted ? '取消静音' : '静音' }}</NButton>
              <NButton v-if="callSession.media === 'video'" secondary @click="toggleCallCamera">{{ callCameraOn ? '关闭摄像头' : '打开摄像头' }}</NButton>
              <NButton type="error" @click="() => endPrivateCall()">挂断</NButton>
            </template>
          </NSpace>
        </div>
        </aside>
      </Teleport>
      <NModal
        :show="imagePreviewMessage !== null"
        class="image-preview-modal"
        :mask-closable="true"
        @update:show="(visible) => { if (!visible) closeImagePreview(); }"
      >
        <div v-if="imagePreviewMessage" class="image-preview-dialog" @click.stop>
          <button class="image-preview-close" type="button" title="关闭预览" @click="closeImagePreview">×</button>
          <div class="image-preview-toolbar">
            <button type="button" title="缩小" :disabled="imagePreviewScale <= 0.25" @click="changeImagePreviewScale(-0.2)">−</button>
            <span>{{ Math.round(imagePreviewScale * 100) }}%</span>
            <button type="button" title="放大" :disabled="imagePreviewScale >= 4" @click="changeImagePreviewScale(0.2)">+</button>
            <button type="button" title="还原" :disabled="imagePreviewScale === 1" @click="imagePreviewScale = 1">↺</button>
          </div>
          <div class="image-preview-viewport" @wheel.prevent="handleImagePreviewWheel">
            <img
              :src="imagePreviewSource(imagePreviewMessage)"
              :alt="imagePreviewMessage.file_meta?.name ?? '图片预览'"
              :style="{ transform: `scale(${imagePreviewScale})` }"
            />
          </div>
        </div>
      </NModal>
      <NModal v-model:show="simulationModalOpen" preset="card" title="超管模拟发送" class="simulation-modal" :mask-closable="!simulationSending">
        <div v-if="selectedPeerDetail" class="simulation-form">
          <div class="simulation-identity">
            <img v-if="avatarImage(selectedPeerDetail.avatar)" class="avatar-image peer-avatar" :src="avatarImage(selectedPeerDetail.avatar)" alt="模拟设备头像" />
            <NAvatar v-else class="peer-avatar">{{ firstLetter(peerDisplayName(selectedPeerDetail)) }}</NAvatar>
            <div><strong>{{ peerDisplayName(selectedPeerDetail) }}</strong><small>{{ selectedPeerDetail.device_id }}</small></div>
          </div>
          <NRadioGroup v-model:value="simulationKind" name="simulation-kind">
            <NSpace>
              <NRadioButton value="direct">模拟私聊</NRadioButton>
              <NRadioButton value="channel">模拟频道消息</NRadioButton>
              <NRadioButton value="alert">模拟普通告警</NRadioButton>
              <NRadioButton value="disco">模拟蹦迪告警</NRadioButton>
            </NSpace>
          </NRadioGroup>
          <NFormItem v-if="simulationKind === 'direct'" label="接收设备" :show-feedback="false">
            <NSelect v-model:value="simulationTargetId" :options="simulationDirectTargetOptions" filterable placeholder="仅显示在线且支持聊天的设备" />
          </NFormItem>
          <NFormItem v-else-if="simulationKind === 'channel'" label="发送频道" :show-feedback="false">
            <NSelect v-model:value="simulationTargetId" :options="simulationChannelOptions" placeholder="选择频道" />
          </NFormItem>
          <NFormItem label="内容" :show-feedback="false">
            <NInput v-model:value="simulationContent" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" maxlength="120" :placeholder="simulationKind === 'alert' || simulationKind === 'disco' ? '留空使用默认告警文案' : '输入要模拟发送的文本'" />
          </NFormItem>
          <NCheckbox v-model:checked="simulationDisplayLabel">显示超管模拟发送</NCheckbox>
          <NText depth="3">仅支持文本消息和告警；文件、图片、语音与游戏操作不支持模拟。</NText>
          <div class="simulation-actions"><NButton @click="simulationModalOpen = false">取消</NButton><NButton type="warning" :loading="simulationSending" @click="submitSimulation">发送</NButton></div>
        </div>
      </NModal>
      <NModal v-model:show="adminNotificationModalOpen" preset="card" title="下发超管通知" class="simulation-modal" :mask-closable="!adminNotificationSending">
        <div class="simulation-form">
          <NRadioGroup v-model:value="adminNotificationScope" name="admin-notification-scope"><NSpace><NRadioButton value="device">指定设备</NRadioButton><NRadioButton value="all_online">所有在线成员</NRadioButton></NSpace></NRadioGroup>
          <NFormItem v-if="adminNotificationScope === 'device'" label="接收设备" :show-feedback="false"><NSelect v-model:value="adminNotificationTargetId" :options="adminNotificationTargetOptions" filterable placeholder="选择在线设备" /></NFormItem>
          <NText v-else depth="3">将向 {{ onlinePeers.length }} 台在线设备分别下发。</NText>
          <NFormItem label="通知标题" :show-feedback="false"><NInput v-model:value="adminNotificationTitle" maxlength="60" /></NFormItem>
          <NFormItem label="通知内容" :show-feedback="false"><NInput v-model:value="adminNotificationContent" type="textarea" :autosize="{ minRows: 3, maxRows: 6 }" maxlength="1000" /></NFormItem>
          <NFormItem label="通知类型" :show-feedback="false"><NSelect v-model:value="adminNotificationTemplate" :options="[{ label: '普通公告', value: 'announcement' }, { label: '赞赏提醒', value: 'support' }]" /></NFormItem>
          <NFormItem label="公告配图" :show-feedback="false"><div class="admin-notification-image-picker"><input ref="adminNotificationImageInput" class="hidden-file-input" type="file" accept="image/*" @change="handleAdminNotificationImageSelected" /><img v-if="adminNotificationSupportUrl.startsWith('data:image/')" :src="adminNotificationSupportUrl" alt="公告图片预览" /><NText v-else depth="3">可选择本地图片，最大 5MB。</NText><NSpace><NButton size="small" secondary @click="triggerAdminNotificationImageSelect">选择图片</NButton><NButton v-if="adminNotificationSupportUrl" size="small" quaternary @click="clearAdminNotificationImage">清除</NButton></NSpace></div></NFormItem>
          <NFormItem v-if="adminNotificationTemplate === 'support' && !adminNotificationSupportUrl.startsWith('data:image/')" label="赞赏页面地址" :show-feedback="false"><NInput v-model:value="adminNotificationSupportUrl" placeholder="可直接粘贴 https:// 图片或页面地址" /></NFormItem>
          <NFormItem label="处理方式" :show-feedback="false"><NRadioGroup v-model:value="adminNotificationDisplayMode"><NSpace><NRadioButton value="dismissible">可关闭</NRadioButton><NRadioButton value="requires_confirmation">必须确认</NRadioButton></NSpace></NRadioGroup></NFormItem>
          <NCheckbox v-model:checked="adminNotificationForceOpenMainWindow">强制打开目标主窗口</NCheckbox>
          <template v-if="adminNotificationDisplayMode === 'requires_confirmation'"><NFormItem label="截至时间" :show-feedback="false"><NInput v-model:value="adminNotificationDeadline" placeholder="例如 2026-08-06 18:00，留空则不超时" /></NFormItem><NFormItem label="超时策略" :show-feedback="false"><NSelect v-model:value="adminNotificationTimeoutPolicy" :options="[{ label: '等待超管手动决定', value: 'manual_review' }, { label: '自动撤销并放行', value: 'auto_release' }, { label: '继续锁定', value: 'keep_locked' }]" /></NFormItem></template>
          <div class="simulation-actions"><NButton @click="adminNotificationModalOpen = false">取消</NButton><NButton type="warning" :loading="adminNotificationSending" @click="submitAdminNotification">下发</NButton></div>
        </div>
      </NModal>
      <NModal v-model:show="adminNotificationDetailOpen" preset="card" :title="adminNotificationDetail && isAdminNotificationIssuer(adminNotificationDetail) ? '通知审核详情' : '公告通知详情'" class="admin-notification-announcement">
        <div v-if="adminNotificationDetail" class="admin-notification-lock-content">
          <div class="admin-notification-detail-device"><img v-if="avatarImage(adminNotificationTargetDetail(adminNotificationDetail)?.avatar)" class="avatar-image large-avatar" :src="avatarImage(adminNotificationTargetDetail(adminNotificationDetail)?.avatar)" alt="设备头像" /><NAvatar v-else :size="48" class="peer-avatar">{{ firstLetter(adminNotificationTargetDetail(adminNotificationDetail)?.nickname ?? '?') }}</NAvatar><div><strong>{{ adminNotificationTargetDetail(adminNotificationDetail)?.nickname ?? '未知设备' }}</strong><small>IP：{{ adminNotificationTargetDetail(adminNotificationDetail)?.address ?? '未知' }}</small><small>MAC：{{ adminNotificationDetail.target_device_id }}</small></div></div>
          <NTag :type="adminNotificationDetail.status === 'submitted' ? 'warning' : 'default'">{{ adminNotificationDetail.status }}</NTag><h2>{{ adminNotificationDetail.title }}</h2><p>{{ adminNotificationDetail.content }}</p><img v-if="adminNotificationDetail.support_url && /^(https?:|asset:|data:image)/.test(adminNotificationDetail.support_url)" :src="adminNotificationDetail.support_url" alt="通知配图" /><NText depth="3">下发时间：{{ formatTime(adminNotificationDetail.created_at) }}</NText>
          <NSpace v-if="isAdminNotificationIssuer(adminNotificationDetail) && adminNotificationDetail.display_mode === 'requires_confirmation' && adminNotificationDetail.status === 'submitted'"><NButton type="success" @click="decideAdminNotificationFromDetail('approved')">通过</NButton><NButton type="error" @click="decideAdminNotificationFromDetail('rejected')">拒绝</NButton></NSpace>
          <NButton v-else-if="isAdminNotificationIssuer(adminNotificationDetail) && adminNotificationDetail.display_mode === 'requires_confirmation' && ['pending','rejected','expired_locked'].includes(adminNotificationDetail.status)" tertiary type="warning" @click="decideAdminNotificationFromDetail('revoked')">撤销并放行</NButton>
          <NButton v-else-if="isAdminNotificationRecipient(adminNotificationDetail) && adminNotificationDetail.display_mode === 'requires_confirmation' && ['pending','rejected'].includes(adminNotificationDetail.status)" type="primary" @click="submitAdminNotificationFromDetail">重新提交确认</NButton>
          <NButton v-else-if="isAdminNotificationRecipient(adminNotificationDetail) && adminNotificationDetail.display_mode === 'dismissible' && !dismissedAdminNotificationIds.includes(adminNotificationDetail.notification_id)" type="primary" @click="dismissAdminNotificationFromDetail">我知道了</NButton>
        </div>
      </NModal>
      <NModal :show="!!blockingAdminNotification" preset="card" class="admin-notification-lock" :mask-closable="false" :closable="false">
        <div v-if="blockingAdminNotification" class="admin-notification-lock-content"><NTag type="warning">需要完成确认</NTag><h2>{{ blockingAdminNotification.title }}</h2><p>{{ blockingAdminNotification.content }}</p><img v-if="blockingAdminNotification.support_url && /^(https?:|asset:|data:image)/.test(blockingAdminNotification.support_url)" :src="blockingAdminNotification.support_url" alt="通知图片" /><NText v-if="blockingAdminNotification.deadline_at" depth="3">截至：{{ formatTime(blockingAdminNotification.deadline_at) }}</NText><NAlert v-if="blockingAdminNotification.status === 'rejected'" type="error" :show-icon="false">超管未确认，请完成后重新提交。</NAlert><NAlert v-else-if="blockingAdminNotification.status === 'expired_locked'" type="warning" :show-icon="false">已超时，等待超管决定。</NAlert><NButton v-if="blockingAdminNotification.status !== 'expired_locked'" type="primary" @click="submitBlockingAdminNotification(blockingAdminNotification)">提交已完成</NButton><NButton quaternary @click="api.quitApp">退出软件</NButton></div>
      </NModal>
      <NModal :show="!!visibleAdminAnnouncement" preset="card" class="admin-notification-announcement" :mask-closable="true" @update:show="(visible) => { if (!visible && visibleAdminAnnouncement) dismissAdminAnnouncement(visibleAdminAnnouncement); }">
        <div v-if="visibleAdminAnnouncement" class="admin-notification-lock-content"><NTag type="info">超管公告</NTag><h2>{{ visibleAdminAnnouncement.title }}</h2><p>{{ visibleAdminAnnouncement.content }}</p><img v-if="visibleAdminAnnouncement.support_url && /^(https?:|asset:|data:image)/.test(visibleAdminAnnouncement.support_url)" :src="visibleAdminAnnouncement.support_url" alt="公告图片" /><NText depth="3">{{ visibleAdminAnnouncement.issued_by_nickname }} · {{ formatTime(visibleAdminAnnouncement.created_at) }}</NText><NButton type="primary" @click="dismissAdminAnnouncement(visibleAdminAnnouncement)">我知道了</NButton></div>
      </NModal>
      <NModal v-model:show="adminNotificationHistoryOpen" preset="card" title="历史公告" class="admin-notification-history-modal">
        <div class="admin-notification-history-list">
          <NEmpty v-if="recipientAdminNotifications.length === 0" description="暂无收到的公告通知" />
          <article v-for="notification in recipientAdminNotifications" :key="notification.notification_id" class="admin-notification-history-item" role="button" tabindex="0" @click="openAdminNotificationDetail(notification)" @keydown.enter="openAdminNotificationDetail(notification)">
            <header>
              <strong>{{ notification.title }}</strong>
              <NTag size="small" :bordered="false" :type="notification.display_mode === 'requires_confirmation' ? 'warning' : 'info'">
                {{ notification.display_mode === 'requires_confirmation' ? '需确认' : '公告' }}
              </NTag>
            </header>
            <p>{{ notification.content }}</p>
            <footer>{{ notification.issued_by_nickname }} · {{ formatTime(notification.created_at) }} · {{ notification.status }}</footer>
          </article>
        </div>
      </NModal>
      <div class="desktop-frame">
        <header class="app-titlebar" @mousedown="startWindowDrag">
          <div class="titlebar-brand">
            <span class="app-mark">L</span>
            <strong>LanChat</strong>
            <span>局域网聊天</span>
          </div>
          <div class="titlebar-actions" @mousedown.stop.prevent>
            <button class="window-btn" title="最小化" @mousedown.stop.prevent @click.stop="minimizeWindow">─</button>
            <button class="window-btn" title="最大化" @mousedown.stop.prevent @click.stop="toggleMaximizeWindow">□</button>
            <button class="window-btn close" title="关闭" @mousedown.stop.prevent @click.stop="closeWindow">×</button>
          </div>
        </header>
        <NLayout class="app-shell" has-sider>
          <AppNavigationRail
            :expanded="navExpanded"
            :active-section="activeSection"
            :profile-nickname="profile?.nickname ?? '个人资料'"
            :profile-avatar-src="avatarImage(profile?.avatar) ?? null"
            :profile-avatar-label="avatarLabel(profile?.avatar, profile?.nickname)"
            :total-unread="totalUnread"
            :games-available="gamesFeatureAvailable"
            :game-attention-count="0"
            :show-game-attention="false"
            :pet-alert-enabled="petAlertEnabled"
            :pending-notification-count="pendingAdminNotificationCount"
            :update-available="visibleUpdateAvailable"
            :update-badge-label="updateBadgeLabel"
            @select="openSection"
            @toggle="toggleNav"
            @open-notification-history="openAdminNotificationHistory"
          />
          <button
            v-if="listPaneAvailable"
            class="list-pane-toggle"
            :class="{ collapsed: listPaneCollapsed }"
            :title="listPaneToggleTitle"
            @click="toggleListPane"
          >
            {{ listPaneCollapsed ? "›" : "‹" }}
          </button>
          <NLayoutSider v-if="activeSection === 'chat' && !listPaneCollapsed" class="list-pane" :width="listPaneWidth" bordered>
            <div class="pane-header">
              <div class="pane-title-row">
                <strong>聊天</strong>
                <div class="pane-actions">
                  <NButton quaternary circle size="small" title="新建私有频道" @click="openRecipientPicker('privateChannelCreate')">＋</NButton>
                  <NButton quaternary circle size="small" title="刷新发现" @click="store.refreshPeers">↻</NButton>
                </div>
              </div>
              <NInput v-model:value="conversationSearch" size="small" clearable placeholder="搜索聊天" />
            </div>
            <NScrollbar class="list-scroll">
              <NList hoverable clickable class="conversation-list">
                <NListItem
                  v-for="conversation in sortedConversations"
                  :key="conversation.id"
                  class="conversation-item"
                  :class="{ active: conversation.id === activeConversationId }"
                  @click="store.selectConversation(conversation.id)"
                >
                  <NThing :title="conversationDisplayName(conversation)">
                    <template #avatar>
                      <NAvatar v-if="conversation.kind === 'group'" class="conversation-avatar">
                        {{ conversation.is_private ? "私" : "局" }}
                      </NAvatar>
                      <img v-else-if="avatarImage(conversationAvatar(conversation))" class="avatar-image conversation-avatar" :src="avatarImage(conversationAvatar(conversation))" alt="会话头像" />
                      <NAvatar v-else class="conversation-avatar">{{ firstLetter(conversationDisplayName(conversation)) }}</NAvatar>
                    </template>
                    <template #description>
                      <div class="conversation-desc">
                        <span v-if="conversationMentionLabel(conversation)" class="conversation-mention-alert">[{{ conversationMentionLabel(conversation) }}]</span>
                        <template v-else>
                          <NTag v-if="conversation.kind === 'group'" size="small" :bordered="false" :type="conversationTagType(conversation)">
                            {{ conversationBadge(conversation) }}
                          </NTag>
                          <span v-else class="conversation-status-dot" :class="{ online: conversationPeer(conversation)?.online }"></span>
                          <span>{{ conversationSubtitle(conversation) }}</span>
                        </template>
                      </div>
                    </template>
                    <template #header-extra>
                      <span class="conversation-time">{{ formatTime(conversation.updated_at) }}</span>
                      <NBadge v-if="(unreadByConversation[conversation.id] ?? 0) > 0" :value="unreadByConversation[conversation.id]" :max="99" type="error" />
                      <button
                        v-if="conversation.kind === 'direct'"
                        class="conversation-delete-button"
                        type="button"
                        title="删除本机对话"
                        aria-label="删除本机对话"
                        @click.stop="requestDeleteDirectConversation(conversation)"
                      >×</button>
                    </template>
                  </NThing>
                </NListItem>
              </NList>
            </NScrollbar>
            <button class="pane-resize-handle left-list" type="button" aria-label="拖动调整列表宽度" title="拖动调整宽度" @mousedown="startPaneResize('list', $event)"></button>
          </NLayoutSider>
          <PluginGamesSidebar
            v-else-if="activeSection === 'games' && !listPaneCollapsed"
            :width="listPaneWidth"
            :games="enabledGamePlugins"
            :active-game-id="activePluginGame?.gameId"
            @select="openPluginGame"
            @begin-resize="startPaneResize('list', $event)"
          />
          <NLayoutSider v-else-if="activeSection === 'devices' && !listPaneCollapsed" class="list-pane" :width="listPaneWidth" bordered>
            <div class="pane-header">
              <div class="pane-title-row">
                <strong>设备列表</strong>
                <NButton quaternary circle size="small" title="刷新发现" @click="store.refreshPeers">↻</NButton>
              </div>
              <NInput v-model:value="deviceSearch" size="small" clearable placeholder="搜索设备、IP" />
            </div>
            <NScrollbar class="list-scroll">
              <div class="add-device-box">
                <div>
                  <strong>添加设备</strong>
                  <span>输入 IP 和端口建立单聊</span>
                </div>
                <NSpace vertical :size="8">
                  <NInput v-model:value="manualAddress" placeholder="192.168.1.23" clearable />
                  <NInputNumber v-model:value="manualPort" :min="1" :max="65535" style="width: 100%" />
                  <NButton block type="primary" @click="openManualDevice">连接</NButton>
                </NSpace>
              </div>
              <div class="section-label">频道</div>
              <NEmpty v-if="deviceChannelConversations.length === 0" description="暂无频道" class="list-empty compact" />
              <NList v-else hoverable clickable class="device-list channel-category-list">
                <NListItem
                  v-for="conversation in deviceChannelConversations"
                  :key="conversation.id"
                  class="device-item"
                  :class="{ active: conversation.id === selectedDeviceChannelId }"
                  @click="openDeviceChannel(conversation)"
                >
                  <NThing :title="conversation.title" :description="conversation.is_private ? '私有加密频道' : '局域网公开频道'">
                    <template #avatar>
                      <NAvatar class="conversation-avatar">{{ conversation.is_private ? "私" : "局" }}</NAvatar>
                    </template>
                    <template #header-extra>
                      <NTag size="small" :bordered="false" :type="conversation.is_private ? 'warning' : 'success'">{{ conversation.is_private ? "私有" : "公有" }}</NTag>
                    </template>
                  </NThing>
                </NListItem>
              </NList>
              <div class="section-label">已发现设备</div>
              <NList hoverable clickable class="device-list local-device-list">
                <NListItem class="device-item local-device-item" @click="openSection('settings')">
                  <NThing :title="profile?.nickname ?? '本机设备'">
                    <template #avatar>
                      <img v-if="avatarImage(profile?.avatar)" class="avatar-image peer-avatar" :src="avatarImage(profile?.avatar)" alt="本机头像" />
                      <NAvatar v-else class="peer-avatar">{{ firstLetter(profile?.nickname ?? '本机') }}</NAvatar>
                    </template>
                    <template #description>
                      <div class="conversation-desc">
                        <span class="conversation-status-dot online"></span>
                        <span>本机 · {{ profile?.device_id ?? '读取中' }}</span>
                      </div>
                    </template>
                    <template #header-extra><NTag size="small" :bordered="false" type="success">本机</NTag></template>
                  </NThing>
                </NListItem>
              </NList>
              <NEmpty v-if="filteredPeers.length === 0" description="暂未发现设备" class="list-empty">
                <template #extra>
                  <NText depth="3">可点击上方添加设备。</NText>
                </template>
              </NEmpty>
              <NList v-else hoverable clickable class="device-list">
                <NListItem v-for="peer in filteredPeers" :key="peer.device_id" class="device-item" :class="{ active: peer.device_id === selectedPeerId }" @click="openDevice(peer)">
                  <NThing :title="peerDisplayName(peer)">
                    <template #avatar>
                      <img v-if="avatarImage(peer.avatar)" class="avatar-image peer-avatar" :src="avatarImage(peer.avatar)" alt="设备头像" />
                      <NAvatar v-else class="peer-avatar">{{ firstLetter(peerDisplayName(peer)) }}</NAvatar>
                    </template>
                    <template #description>
                      <div class="conversation-desc">
                        <span class="conversation-status-dot" :class="{ online: peer.online }"></span>
                        <span>{{ peerSubtitle(peer) }}</span>
                      </div>
                    </template>
                  </NThing>
                </NListItem>
              </NList>
            </NScrollbar>
            <button class="pane-resize-handle left-list" type="button" aria-label="拖动调整列表宽度" title="拖动调整宽度" @mousedown="startPaneResize('list', $event)"></button>
          </NLayoutSider>
          <NLayout class="content-panel">
            <section v-if="activeSection === 'chat'" class="chat-view">
              <header class="chat-header" data-tauri-drag-region>
                <div class="chat-title" :class="{ 'direct-chat-title': activeConversation?.kind === 'direct' }">
                  <h2>{{ activeConversation ? conversationDisplayName(activeConversation) : "局域网频道" }}</h2>
                  <p v-if="activeConversation?.kind === 'group'">{{ activeConversation?.is_private ? `${activePrivateChannelMembers.length} 名成员 · 私有加密频道` : `${onlinePeers.length} 台设备在线 · 频道广播` }}</p>
                  <p v-else class="peer-status-line">
                    <span>{{ activePeer ? `${activePeer.address}:${activePeer.port}` : "点对点单聊" }}</span>
                    <NTag size="small" :bordered="false" :type="activePeerStatusType">{{ activePeerStatusLabel }}</NTag>
                  </p>
                </div>
              </header>
              <div ref="messagePane" class="messages-pane" @scroll.passive="handleMessagePaneScroll">
                <NSpin :show="loading">
                  <button v-if="hasMoreEarlierMessages" class="message-history-loader" type="button" @click="loadEarlierMessages">加载更早消息</button>
                  <button v-if="hasLaterMessages" class="message-history-loader latest" type="button" @click="jumpToLatestMessages">回到最新消息</button>
                  <NEmpty v-if="!loading && activeMessages.length === 0" description="还没有消息" class="empty-state">
                    <template #extra>
                      <span>选择在线设备单聊，或在局域网频道里发第一句。</span>
                    </template>
                  </NEmpty>
                  <article
                    v-for="message in visibleMessages"
                    :key="message.id"
                    :id="`message-${message.id}`"
                    class="message-row"
                    :class="[messageClass(message), { 'mention-target-highlight': highlightedMentionMessageId === message.id }]"
                    @contextmenu="openMessageContextMenu(message, $event)"
                  >
                    <div v-if="message.message_type === 'system'" class="system-message">
                      <span>{{ message.content }}</span>
                    </div>
                    <template v-else>
                    <img v-if="avatarImage(senderAvatar(message))" class="avatar-image message-avatar" :src="avatarImage(senderAvatar(message))" alt="消息头像" />
                    <NAvatar v-else class="message-avatar">{{ firstLetter(senderName(message)) }}</NAvatar>
                    <div class="message-stack">
                      <div class="message-meta">
                        <span class="message-meta-name">{{ messageSenderTitle(message) }}</span>
                        <NTag v-if="simulationLabel(message.simulation)" size="tiny" :bordered="false" type="warning">{{ simulationLabel(message.simulation) }}</NTag>
                        <span class="message-meta-time">{{ formatTime(message.created_at) }}</span>
                      </div>
                      <div class="message-content-line">
                        <div class="message-bubble" :class="{ 'message-card-bubble': privateChannelInvitePayload(message) }">
                          <template v-if="privateChannelInvitePayload(message)">
                            <div class="channel-invite-card invite-message-card">
                              <span class="channel-invite-icon">私</span>
                              <span class="channel-invite-copy">
                                <strong>{{ privateChannelInvitePayload(message)?.title }}</strong>
                                <small>{{ privateChannelInvitePayload(message)?.owner_nickname }} 邀请你加入私有频道</small>
                              </span>
                              <span class="channel-invite-actions">
                                <NTag v-if="privateChannelInviteState(privateChannelInvitePayload(message)) === 'accepted'" size="small" :bordered="false" type="success">已加入</NTag>
                                <NTag v-else-if="privateChannelInviteState(privateChannelInvitePayload(message)) === 'rejected'" size="small" :bordered="false" type="default">已拒绝</NTag>
                                <NTag v-else-if="privateChannelInviteState(privateChannelInvitePayload(message)) === 'expired'" size="small" :bordered="false" type="default">已过期</NTag>
                                <template v-else-if="messageClass(message) !== 'mine'">
                                  <NButton size="tiny" type="primary" @click="acceptPrivateChannelInviteCard(privateChannelInvitePayload(message))">加入</NButton>
                                  <NButton size="tiny" secondary @click="rejectPrivateChannelInviteCard(privateChannelInvitePayload(message))">拒绝</NButton>
                                </template>
                                <NTag v-else size="small" :bordered="false" type="success">已发送</NTag>
                              </span>
                            </div>
                          </template>
                          <template v-else-if="message.message_type === 'text'">
                            <p>
                              <span
                                v-for="(segment, index) in messageTextSegments(message.content)"
                                :key="`${message.id}-segment-${index}`"
                                :class="{ 'message-mention': segment.mention }"
                              >{{ segment.text }}</span>
                            </p>
                          </template>
                          <div v-else-if="message.file_meta" class="file-message">
                            <img v-if="isImageFile(message)" class="file-preview-image" :src="imageThumbnailSource(message)" :alt="message.file_meta.name" title="点击放大查看" loading="lazy" decoding="async" @click="openImagePreview(message)" />
                            <audio v-else-if="isAudioFile(message)" class="voice-player" controls :src="message.file_meta.url"></audio>
                            <a v-else class="file-info file-link" :href="message.file_meta.url">
                              <strong>{{ message.file_meta.name }}</strong>
                              <span>{{ formatFileSize(message.file_meta.size) }}</span>
                            </a>
                          </div>
                          <p v-else>{{ message.content }}</p>
                        </div>
                        <span
                          v-if="messageClass(message) === 'mine'"
                          class="message-status-outside"
                          :class="`status-${message.status}`"
                          :title="statusText(message.status)"
                          aria-label="消息状态"
                        >{{ statusIcon(message.status) }}</span>
                      </div>
                    </div>
                    </template>
                  </article>
                </NSpin>
                <NDropdown
                  placement="bottom-start"
                  trigger="manual"
                  :x="messageContextMenuX"
                  :y="messageContextMenuY"
                  :show="messageContextMenuOpen"
                  :options="messageContextOptions"
                  @clickoutside="messageContextMenuOpen = false"
                  @select="selectMessageContextAction"
                />
              </div>
              <button
                v-if="activeConversation?.kind === 'group' && activeMentionNotices.length > 0"
                class="mention-jump-button"
                type="button"
                title="定位到提及我的消息"
                @click="jumpToActiveMention"
              >
                <span>@</span>
                <strong>{{ activeMentionLabel }}</strong>
                <small>{{ activeMentionNotices.length }}</small>
              </button>
              <footer class="composer work-composer">
                <div class="composer-tools">
                  <button class="composer-tool" title="发送文件" :disabled="!canSendActive" @click="chooseAndSendFile">📎</button>
                  <button class="composer-tool" :class="{ recording: isRecording }" :disabled="!canSendActive" :title="isRecording ? '停止录音' : '发送语音'" @click="toggleVoiceRecording">🎙</button>
                  <template v-if="activeConversation?.kind === 'direct'">
                    <span class="composer-tool-divider" aria-hidden="true"></span>
                    <NTooltip>
                      <template #trigger><button class="composer-tool call-composer-tool" type="button" title="语音通话" :disabled="!canStartPrivateCall || !!callSession" @click="startPrivateCall('audio')">☎</button></template>
                      语音通话
                    </NTooltip>
                    <NTooltip>
                      <template #trigger><button class="composer-tool call-composer-tool" type="button" title="视频通话" :disabled="!canStartPrivateCall || !!callSession" @click="startPrivateCall('video')">▣</button></template>
                      视频通话
                    </NTooltip>
                    <NTooltip>
                      <template #trigger><button class="composer-tool nudge-composer-tool" type="button" title="抖一抖" :disabled="!canStartPrivateCall" @click="sendPrivateNudge">〰</button></template>
                      抖一抖
                    </NTooltip>
                  </template>
                  <div class="emoji-wrap">
                    <button class="composer-tool" title="表情" :disabled="!canSendActive" @click="chatEmojiOpen = !chatEmojiOpen">☺</button>
                    <div v-if="chatEmojiOpen" class="emoji-panel">
                      <button v-for="emoji in emojiOptions" :key="emoji" @click="appendEmojiToDraft(emoji)">{{ emoji }}</button>
                    </div>
                  </div>
                  <div v-if="activeConversation?.kind === 'group'" class="mention-wrap">
                    <button class="composer-tool mention-trigger" title="@成员" :disabled="!canMentionInActiveConversation" @click="mentionPickerOpen = !mentionPickerOpen">@</button>
                    <div v-if="mentionPickerOpen" class="mention-panel">
                      <NInput v-model:value="mentionSearch" size="small" clearable placeholder="搜索成员" />
                      <div class="mention-list">
                        <button class="mention-row mention-all" type="button" @click="insertMentionToDraft()">
                          <span class="mention-avatar">@</span>
                          <span>
                            <strong>所有人</strong>
                            <small>提醒频道内所有成员</small>
                          </span>
                        </button>
                        <button
                          v-for="member in mentionPickerMembers"
                          :key="member.device_id"
                          class="mention-row"
                          type="button"
                          @click="insertMentionToDraft(member)"
                        >
                          <img v-if="avatarImage(member.avatar)" class="avatar-image mention-avatar" :src="avatarImage(member.avatar)" alt="成员头像" />
                          <span v-else class="mention-avatar">{{ firstLetter(member.nickname) }}</span>
                          <span>
                            <strong>{{ member.device_id === profile?.device_id ? `我 · ${member.nickname}` : member.nickname }}</strong>
                            <small>{{ memberSubtitle(member) }}</small>
                          </span>
                        </button>
                      </div>
                    </div>
                  </div>
                  <button class="composer-tool" title="清空输入" @click="draft = ''">⌫</button>
                </div>
                <div class="composer-input-frame" @paste="handleComposerPaste">
                  <ChatComposerInput
                    v-model="draft"
                    :disabled="!canSendActive"
                    :placeholder="composerPlaceholder"
                    @submit="store.sendActiveMessage"
                  />
                  <div class="composer-footer">
                    <span>Enter 发送 · Shift+Enter 换行</span>
                    <NButton type="primary" size="medium" :disabled="!canSendActive" @click="store.sendActiveMessage">发送</NButton>
                  </div>
                </div>
              </footer>
            </section>
            <GamesPage
              v-else-if="activeSection === 'games' && activePluginGame"
                :key="activePluginGame.pluginId"
                :runtime="pluginRuntime"
                :plugin-id="activePluginGame.pluginId"
                :game-id="activePluginGame.gameId"
                @error="handlePluginViewportError"
            />
            <DevicesPage
              v-else-if="activeSection === 'devices'"
              v-model:peer-note-draft="peerNoteDraft"
              v-model:admin-nickname-draft="adminNicknameDraft"
              v-model:admin-nickname-lock-after-issue="adminNicknameLockAfterIssue"
              :selected-peer="selectedPeerDetail"
              :selected-channel="selectedDeviceChannelDetail"
              :channel-owner-name="selectedDeviceChannelOwnerName"
              :channel-members="selectedDeviceChannelMembers"
              :profile-device-id="profile?.device_id"
              :super-admin-enabled="superAdminEnabled"
              :can-manage-channel="canManageSelectedDeviceChannel"
              :avatar-image="avatarImage"
              :first-letter="firstLetter"
              :peer-client-kind-label="peerClientKindLabel"
              :peer-build-version-label="peerBuildVersionLabel"
              :peer-build-time-label="peerBuildTimeLabel"
              :peer-supports-full-features="peerSupportsFullFeatures"
              :peer-last-seen-label="peerLastSeenLabel"
              :member-display-name="memberDisplayName"
              :member-subtitle="memberSubtitle"
              :format-time="formatTime"
              @save-peer-note="saveSelectedPeerNote"
              @admin-rename="adminRenameSelectedPeer"
              @admin-use-system-username="adminUseSystemUsernameForSelectedPeer"
              @admin-unlock-nickname="adminUnlockSelectedPeerNickname"
              @open-simulation="openSimulationModal"
              @start-direct-chat="startDirectChat"
              @delete-peer="deleteSelectedPeer"
              @open-member="openMemberDevice"
              @enter-channel="enterSelectedDeviceChannel"
              @invite-members="inviteSelectedDeviceChannelMembers"
              @dissolve-channel="dissolveSelectedDeviceChannel"
            />
            <AlertsPage
              v-else-if="activeSection === 'alerts'"
              v-model:quick-alert-draft="quickAlertDraft"
              :profile-device-id="profile?.device_id"
              :alert-mode="petAlertMode"
              :ranking-rows="alertRankingRows"
              :alert-records="alertRecords"
              :now="nowTick"
              :simulation-label="simulationLabel"
              :probability-label="alertProbabilityLabel"
              :format-time="formatTime"
              @send="sendPetQuickAlert"
            />
            <section v-show="activeSection === 'settings'" class="workspace-view settings-view">
              <div class="settings-layout">
                <nav class="settings-subnav" aria-label="设置分类">
                  <button
                    type="button"
                    :class="{ active: settingsCategory === 'basic' }"
                    :aria-current="settingsCategory === 'basic' ? 'page' : undefined"
                    @click="settingsCategory = 'basic'"
                  >
                    {{ t("settings.basic") }}
                  </button>
                  <button
                    type="button"
                    :class="{ active: settingsCategory === 'pet' }"
                    :aria-current="settingsCategory === 'pet' ? 'page' : undefined"
                    @click="settingsCategory = 'pet'"
                  >
                    {{ t("settings.pet") }}
                  </button>
                  <button
                    type="button"
                    :class="{ active: settingsCategory === 'plugins' }"
                    :aria-current="settingsCategory === 'plugins' ? 'page' : undefined"
                    @click="settingsCategory = 'plugins'"
                  >
                    插件中心
                  </button>
                  <button
                    v-if="superAdminEnabled"
                    type="button"
                    :class="{ active: settingsCategory === 'admin' }"
                    :aria-current="settingsCategory === 'admin' ? 'page' : undefined"
                    @click="settingsCategory = 'admin'"
                  >
                    {{ t("settings.admin") }}
                  </button>
                </nav>
                <div class="settings-content">
                  <div class="workspace-header settings-header">
                    <div class="settings-heading-row">
                      <h2 class="settings-title">设置<button class="settings-secret-trigger" type="button" aria-label="设置" @click="handleSuperAdminTap">✦</button></h2>
                    </div>
                    <p>{{ settingsCategory === 'basic' ? '管理本机资料、网络、主题和语言。' : settingsCategory === 'pet' ? '管理桌宠资源、行为与告警能力。' : settingsCategory === 'plugins' ? '安装、授权、启停和卸载独立插件。' : '集中管理通知、桌宠告警、识别策略和设备更新。' }}</p>
                  </div>
                  <div class="settings-grid" :class="{ 'basic-settings-grid': settingsCategory === 'basic', 'admin-settings-grid': settingsCategory === 'admin' }">
                <PluginCenterPage v-if="settingsCategory === 'plugins'" @changed="initializePluginFeatures" />
                <NCard v-if="settingsCategory === 'basic' && profile" title="本机资料" size="small">
                  <NSpace vertical>
                    <NFormItem label="昵称" :show-feedback="false">
                      <NInput v-model:value="nicknameDraft" maxlength="24" clearable :disabled="profile.nickname_locked" />
                    </NFormItem>
                    <NAlert v-if="profile.nickname_locked" type="warning" :show-icon="false">
                      管理员已禁止本机修改昵称，如需修改请联系管理员解除限制。
                    </NAlert>
                    <NFormItem label="监听端口" :show-feedback="false">
                      <NInputNumber v-model:value="portDraft" :min="1" :max="65535" style="width: 100%" />
                    </NFormItem>
                    <NFormItem label="头像" :show-feedback="false">
                      <div class="profile-avatar-picker">
                        <img v-if="avatarImage(avatarDraft)" class="avatar-image self-avatar large-avatar" :src="avatarImage(avatarDraft)" alt="头像预览" />
                        <NAvatar v-else :size="56" class="self-avatar">{{ avatarLabel(avatarDraft, nicknameDraft) }}</NAvatar>
                        <div class="profile-avatar-actions">
                          <input ref="profileAvatarInput" class="hidden-file-input" type="file" accept="image/*" @change="handleProfileAvatarSelected" />
                          <NSpace :size="8">
                            <NButton size="small" secondary @click="triggerProfileAvatarSelect">选择图片</NButton>
                            <NButton size="small" quaternary @click="clearProfileAvatar">清除</NButton>
                          </NSpace>
                          <NText depth="3">仅支持 5M 以内图片，保存后会转成 base64 通知在线设备。</NText>
                        </div>
                      </div>
                    </NFormItem>
                    <NText depth="3">设备标识：{{ shortDeviceId }}</NText>
                    <NButton block secondary type="primary" @click="saveProfile">保存资料</NButton>
                  </NSpace>
                </NCard>
                <BasicSystemSettings
                  v-if="settingsCategory === 'basic'"
                  v-model:update-github-token-draft="updateGithubTokenDraft"
                  :autostart-enabled="autostartEnabled"
                  :autostart-loading="autostartLoading"
                  :network-repair-description="networkRepairDescription"
                  :can-repair-windows-network="canRepairWindowsNetwork"
                  :network-repairing="networkRepairing"
                  :network-repair-status="networkRepairStatus"
                  :preview-media-cache-info="previewMediaCacheInfo"
                  :preview-media-cache-clearing="previewMediaCacheClearing"
                  :local-version-label="localVersionLabel"
                  :update-status-type="updateStatusType"
                  :update-status-label="updateStatusLabel"
                  :update-info="updateInfo"
                  :native-update-installing="nativeUpdateInstalling"
                  :native-update-progress-percent="nativeUpdateProgressPercent"
                  :native-update-progress-label="nativeUpdateProgressLabel"
                  :update-checking="updateChecking"
                  :preferred-update-url="preferredUpdateUrl"
                  :update-github-token-info="updateGithubTokenInfo"
                  :update-github-token-saving="updateGithubTokenSaving"
                  :format-file-size="formatFileSize"
                  :format-date-time="formatDateTime"
                  :update-notes-preview="updateNotesPreview"
                  @update-autostart="updateAutostart"
                  @repair-network="store.repairNetwork"
                  @request-call-permission="requestCallDevicePermission"
                  @clear-image-cache="clearImagePreviewCache"
                  @check-updates="checkUpdates(true)"
                  @download-update="openPreferredUpdateUrl"
                  @open-release-page="openReleasePage"
                  @save-update-token="saveUpdateGithubToken"
                  @clear-update-token="clearUpdateGithubToken"
                />
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="超管通知" size="small">
                  <NSpace vertical>
                    <NText depth="3">指定设备会直连送达；全员模式会为每台在线设备独立创建一条通知，便于逐人审核。</NText>
                    <NSpace>
                      <NButton type="warning" @click="openAdminNotificationModal">下发通知</NButton>
                      <NButton secondary @click="openAdminNotificationReview">查看通知审核记录</NButton>
                    </NSpace>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="远程更新下发" size="small">
                  <NSpace vertical>
                    <NText depth="3">仅向当前在线的 Windows 客户端下发。携带本地安装包时，目标设备会从本机局域网文件服务下载；留空则从 GitHub Release 下载。局域网包无需 .sig 签名文件，但会校验 SHA-256 完整性。</NText>
                    <NRadioGroup v-model:value="adminRemoteUpdateScope" name="admin-remote-update-scope">
                      <NSpace>
                        <NRadioButton value="device">指定设备</NRadioButton>
                        <NRadioButton value="all_online_windows">全部在线 Windows（{{ adminRemoteUpdateWindowsCount }}）</NRadioButton>
                      </NSpace>
                    </NRadioGroup>
                    <NFormItem v-if="adminRemoteUpdateScope === 'device'" label="目标设备" :show-feedback="false">
                      <NSelect v-model:value="adminRemoteUpdateTargetId" :options="adminRemoteUpdateTargetOptions" filterable placeholder="选择在线设备" />
                    </NFormItem>
                    <NFormItem label="目标版本" :show-feedback="false">
                      <NInput v-model:value="adminRemoteUpdateVersion" placeholder="例如 0.5.1" maxlength="32" />
                    </NFormItem>
                    <NFormItem label="本地安装包（可选）" :show-feedback="false">
                      <NSpace vertical style="width: 100%">
                        <NInput v-model:value="adminRemoteUpdatePackagePath" readonly clearable placeholder="不选择时从 GitHub 下载" />
                        <NButton secondary @click="chooseAdminRemoteUpdatePackage">选择安装包 EXE / MSI</NButton>
                        <NInput v-model:value="adminRemoteUpdateSignaturePath" readonly clearable placeholder="选择安装包后必须同时选择同名 .sig 文件" />
                        <NButton secondary :disabled="!adminRemoteUpdatePackagePath" @click="chooseAdminRemoteUpdateSignature">选择 .sig 签名文件</NButton>
                        <NText depth="3">局域网下发必须选择 Tauri 构建产物及同名 .sig 文件，例如 lanchat_0.6.5_x64-setup.exe 和 lanchat_0.6.5_x64-setup.exe.sig。</NText>
                      </NSpace>
                    </NFormItem>
                    <NCheckbox v-model:checked="adminRemoteUpdateForce">强制更新：即使目标版本不低于指定版本也重新安装</NCheckbox>
                    <NButton type="warning" :loading="adminRemoteUpdateSending" :disabled="!canIssueAdminRemoteUpdate" @click="issueAdminRemoteUpdate">下发更新</NButton>
                    <div v-if="adminRemoteUpdateResults.length" class="admin-remote-update-results">
                      <div v-for="row in adminRemoteUpdateResults" :key="row.command_id || `${row.target_device_id}-${row.phase}`" class="admin-remote-update-result-row">
                        <div class="admin-remote-update-result-meta">
                          <strong>{{ row.nickname }}</strong>
                          <span>{{ row.address }}</span>
                        </div>
                        <div class="admin-remote-update-result-status">
                          <span>{{ adminRemoteUpdatePhaseLabel(row.phase) }}</span>
                          <NProgress v-if="row.phase === 'downloading'" type="line" :percentage="adminRemoteUpdateProgressPercent(row)" :height="5" processing />
                          <small v-if="row.error">{{ row.error }}</small>
                        </div>
                      </div>
                    </div>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'pet'" title="桌宠与告警器" size="small" class="desktop-pet-settings-card">
                  <NSpace vertical>
                    <div class="desktop-pet-toolbar">
                      <NButton size="small" type="primary" :loading="desktopPetLoading" @click="importDesktopPetPackage">导入桌宠</NButton>
                      <NTooltip placement="bottom-start" trigger="hover">
                        <template #trigger>
                          <button type="button" class="desktop-pet-import-info" aria-label="桌宠导入规则">i</button>
                        </template>
                        <div class="desktop-pet-import-help">
                          <strong>桌宠导入目录规则</strong>
                          <span>下载 ZIP 后请先解压，导入时选择解压后的桌宠根目录，不要选择 ZIP 文件。</span>
                          <span>目录名必须与 manifest.json 的 id 一致，例如 violet-tail-girl/manifest.json。</span>
                          <span>根目录必须放置 manifest.json、icon.png，可选 preview.png；icon.png 会显示在桌宠列表中。</span>
                          <span>动作资源放在 Idle、Alert、Move、Interact、Life 目录中，每个动作使用独立子目录。</span>
                          <span>PNG 帧建议使用透明背景，并按动作播放顺序连续命名；每个目录图片数量按实际文件计算。</span>
                          <span>导入后右键桌宠图标可编辑 manifest.json 中的动作数量、持续时间和停顿。</span>
                        </div>
                      </NTooltip>
                      <NButton size="small" secondary :loading="desktopPetLoading" @click="desktopPetStore.refresh">刷新</NButton>
                      <NButton size="small" quaternary @click="api.openDesktopPetFolder">打开资源目录</NButton>
                    </div>
                    <div v-if="desktopPetPackages.length > 0" class="desktop-pet-package-section">
                      <button type="button" class="desktop-pet-package-toggle" @click="desktopPetPackagesExpanded = !desktopPetPackagesExpanded">
                        <span>桌宠资源（{{ desktopPetPackages.length }}）</span>
                        <span aria-hidden="true">{{ desktopPetPackagesExpanded ? '⌃' : '⌄' }}</span>
                      </button>
                      <div v-if="desktopPetPackagesExpanded" class="desktop-pet-package-list">
                        <NTooltip
                          v-for="pet in desktopPetPackages"
                          :key="pet.source + '-' + pet.manifest.id"
                          placement="bottom"
                        >
                          <template #trigger>
                            <button
                              type="button"
                              class="desktop-pet-logo-button"
                              :class="{ active: selectedDesktopPetPackage?.manifest.id === pet.manifest.id }"
                              @click="selectDesktopPetPackage(pet)"
                              @contextmenu.prevent="openDesktopPetManifestEditor(pet)"
                            >
                              <img class="desktop-pet-logo" :src="desktopPetPreview(pet)" :alt="pet.manifest.name" />
                              <span v-if="selectedDesktopPetPackage?.manifest.id === pet.manifest.id" class="desktop-pet-selected-mark">✓</span>
                            </button>
                          </template>
                          {{ pet.manifest.name }} · {{ desktopPetSourceLabel(pet.source) }} · {{ desktopPetFrameCount(pet, 'Idle') }} 帧；右键编辑配置
                        </NTooltip>
                      </div>
                    </div>
                    <NEmpty v-else size="small" description="尚未发现可用的桌宠资源包。" />
                    <NModal v-model:show="desktopPetManifestEditorOpen">
                      <NCard
                        class="desktop-pet-manifest-editor"
                        :title="`动作配置 · ${desktopPetManifestEditorTarget?.manifest.name ?? ''}`"
                        size="small"
                        closable
                        @close="desktopPetManifestEditorOpen = false"
                      >
                        <NTabs type="line" animated>
                          <NTabPane v-for="state in DESKTOP_PET_STATE_ORDER" :key="state" :name="state" :tab="DESKTOP_PET_STATE_LABELS[state]">
                            <div v-if="desktopPetPlaybackDraft[state]" class="desktop-pet-playback-grid">
                              <NFormItem label="单动作最短持续（毫秒）" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].minDurationMs" :min="0" :max="300000" />
                              </NFormItem>
                              <NFormItem label="单动作最长持续（毫秒）" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].maxDurationMs" :min="0" :max="300000" />
                              </NFormItem>
                              <NFormItem label="最少随机动作数" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].minActionCount" :min="1" :max="20" />
                              </NFormItem>
                              <NFormItem label="最多随机动作数" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].maxActionCount" :min="1" :max="20" />
                              </NFormItem>
                              <NFormItem label="动作间最短停顿（毫秒）" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].minIntervalMs" :min="0" :max="60000" />
                              </NFormItem>
                              <NFormItem label="动作间最长停顿（毫秒）" :show-feedback="false">
                                <NInputNumber v-model:value="desktopPetPlaybackDraft[state].maxIntervalMs" :min="0" :max="60000" />
                              </NFormItem>
                            </div>
                          </NTabPane>
                        </NTabs>
                        <template #footer>
                          <div class="desktop-pet-manifest-actions">
                            <NButton
                              v-if="desktopPetManifestEditorTarget?.source === 'user'"
                              size="small"
                              type="error"
                              quaternary
                              @click="removeDesktopPetFromEditor"
                            >删除桌宠</NButton>
                            <span></span>
                            <NButton size="small" @click="desktopPetManifestEditorOpen = false">取消</NButton>
                            <NButton size="small" type="primary" :loading="desktopPetLoading" @click="saveDesktopPetManifestConfig">保存并应用</NButton>
                          </div>
                        </template>
                      </NCard>
                    </NModal>
                    <NAlert v-if="desktopPetIssues.length > 0" type="warning" title="发现无法使用的资源包">
                      <div v-for="issue in desktopPetIssues.slice(0, 3)" :key="issue.root + issue.error">
                        {{ issue.root }}：{{ issue.error }}
                      </div>
                    </NAlert>
                    <div class="setting-switch-row">
                      <div>
                        <strong>启用桌面桌宠告警器</strong>
                        <p>开启后左侧显示告警入口，并允许桌宠接收、反馈和展示告警真实度。</p>
                      </div>
                      <NSwitch v-model:value="petAlertEnabled" />
                    </div>
                    <div class="setting-switch-row">
                      <div>
                        <strong>随机巡逻</strong>
                        <p>空闲时轮换播放 Move 动作，后续资源包可提供左右方向动画。</p>
                      </div>
                      <NSwitch
                        :value="desktopPetSettings?.randomMoveEnabled ?? true"
                        @update:value="updateDesktopPetBehavior('randomMoveEnabled', $event)"
                      />
                    </div>
                    <div class="setting-switch-row">
                      <div>
                        <strong>随机生活动作</strong>
                        <p>空闲时低频播放 Life 动作，完成后自动回到 Idle。</p>
                      </div>
                      <NSwitch
                        :value="desktopPetSettings?.randomLifeEnabled ?? true"
                        @update:value="updateDesktopPetBehavior('randomLifeEnabled', $event)"
                      />
                    </div>
                    <NText depth="3">收到告警时桌宠会播放告警动作并显示未处理数量；反馈真实/误报后会更新排行榜。</NText>
                    <NFormItem label="默认告警文案" :show-feedback="false">
                      <NInput v-model:value="quickAlertDraft" maxlength="60" clearable placeholder="呱呱~呱~~" />
                    </NFormItem>
                    <NFormItem label="本机报警模式" :show-feedback="false">
                      <NRadioGroup v-model:value="petAlertMode" name="pet-alert-mode">
                        <NSpace>
                          <NRadioButton value="normal">普通报警</NRadioButton>
                          <NRadioButton value="disco">蹦迪报警</NRadioButton>
                        </NSpace>
                      </NRadioGroup>
                    </NFormItem>
                    <NFormItem label="蹦迪移动方式" :show-feedback="false">
                      <NRadioGroup
                        :value="desktopPetSettings?.discoMovementMode ?? 'jump'"
                        name="desktop-pet-disco-movement"
                        @update:value="updateDesktopPetBehavior('discoMovementMode', $event)"
                      >
                        <NSpace>
                          <NRadioButton value="linear">线性移动</NRadioButton>
                          <NRadioButton value="jump">跳跃移动</NRadioButton>
                        </NSpace>
                      </NRadioGroup>
                    </NFormItem>
                    <NFormItem label="蹦迪持续时长" :show-feedback="false">
                      <NInputNumber
                        :value="desktopPetSettings?.discoDurationSeconds ?? 60"
                        :min="10"
                        :max="3600"
                        :step="10"
                        style="width: 180px"
                        @update:value="updateDesktopPetBehavior('discoDurationSeconds', Number($event ?? 60))"
                      >
                        <template #suffix>秒</template>
                      </NInputNumber>
                    </NFormItem>
                    <NFormItem label="发送快捷键" :show-feedback="false">
                      <NSpace vertical :size="6" style="width: 100%">
                        <NInput v-model:value="petSendHotkey" readonly clearable placeholder="点击后按下快捷键，例如 Ctrl+Alt+G" @keydown="captureDesktopPetSendHotkey" @clear="clearDesktopPetSendHotkey" />
                        <NText depth="3">正常状态下按此快捷键会快速发起一次蹦迪报警。</NText>
                      </NSpace>
                    </NFormItem>
                    <NFormItem label="停止快捷键" :show-feedback="false">
                      <NSpace vertical :size="6" style="width: 100%">
                        <NInput v-model:value="petStopHotkey" readonly clearable placeholder="点击后按下快捷键，例如 Ctrl+Alt+S" @keydown="captureDesktopPetStopHotkey" @clear="clearDesktopPetStopHotkey" />
                        <NText depth="3">报警或蹦迪状态下按此快捷键会停止提醒，不再触发发送。</NText>
                      </NSpace>
                    </NFormItem>
                    <NButton v-if="petAlertEnabled" block type="error" @click="sendPetQuickAlert(petAlertMode)">发送一次测试告警</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'pet'" title="外部推送" size="small">
                  <NSpace vertical>
                    <div class="setting-switch-row">
                      <div>
                        <strong>开启外部推送</strong>
                        <p>桌宠发起告警后，同时推送到已启用的群机器人。第一期支持企业微信和钉钉。</p>
                      </div>
                      <NSwitch
                        :value="desktopPetSettings?.externalPushEnabled ?? false"
                        @update:value="updateDesktopPetBehavior('externalPushEnabled', $event)"
                      />
                    </div>
                    <NSpace>
                      <NButton size="small" secondary type="primary" @click="addExternalPushConfig('wechat_work')">添加企业微信群</NButton>
                      <NButton size="small" secondary type="primary" @click="addExternalPushConfig('dingtalk')">添加钉钉群</NButton>
                    </NSpace>
                    <NText depth="3">这里只配置推送内容正文，来源固定追加在最后一行，格式为 昵称（WLAN IP）。正文为空时只推送来源。</NText>
                    <NFormItem label="最低可信度" :show-feedback="false">
                      <NInputNumber
                        :value="desktopPetSettings?.externalPushMinCredibility ?? 50"
                        :min="0"
                        :max="100"
                        style="width: 180px"
                        :disabled="desktopPetSettings?.externalPushMinCredibilityLocked"
                        @update:value="(value) => updateDesktopPetSettingsPatch({ externalPushMinCredibility: Number(value ?? 50) })"
                      />
                    </NFormItem>
                    <NText depth="3">告警发送人的可信度低于该值时，不触发企业微信或钉钉群机器人推送；没有反馈历史的人员默认按 100 处理。</NText>
                    <NAlert v-if="desktopPetSettings?.externalPushMinCredibilityLocked" type="warning" :show-icon="false">管理员已禁止本机修改告警可信度阈值。</NAlert>
                    <div v-if="desktopPetSettings?.externalPushConfigs?.length" class="external-push-list">
                      <div
                        v-for="config in desktopPetSettings.externalPushConfigs"
                        :key="config.id"
                        class="external-push-item"
                      >
                        <div class="external-push-head">
                          <strong>{{ config.name || externalPushKindLabel(config.kind) }}</strong>
                          <NSpace align="center" :size="8">
                            <NTag size="small">{{ externalPushKindLabel(config.kind) }}</NTag>
                            <NSwitch
                              size="small"
                              :value="config.enabled"
                              @update:value="updateExternalPushConfig(config.id, { enabled: $event })"
                            />
                          </NSpace>
                        </div>
                        <div class="external-push-grid">
                          <NFormItem label="类型" :show-feedback="false">
                            <NSelect
                              :value="config.kind"
                              :options="externalPushKindOptions"
                              @update:value="updateExternalPushConfig(config.id, { kind: $event as ExternalPushKind })"
                            />
                          </NFormItem>
                          <NFormItem label="名称" :show-feedback="false">
                            <NInput
                              :value="config.name"
                              maxlength="30"
                              placeholder="午休告警群"
                              @update:value="updateExternalPushConfig(config.id, { name: $event })"
                            />
                          </NFormItem>
                        </div>
                        <NFormItem label="Webhook" :show-feedback="false">
                          <NInput
                            :value="config.webhook"
                            type="password"
                            show-password-on="click"
                            placeholder="https://..."
                            @update:value="updateExternalPushConfig(config.id, { webhook: $event })"
                          />
                        </NFormItem>
                        <NFormItem label="推送内容" :show-feedback="false">
                          <NInput
                            :value="config.template"
                            type="textarea"
                            :autosize="{ minRows: 4, maxRows: 7 }"
                            @update:value="updateExternalPushConfig(config.id, { template: $event })"
                          />
                        </NFormItem>
                        <div class="external-push-actions">
                          <NCheckbox
                            :checked="config.mentionAll"
                            @update:checked="updateExternalPushConfig(config.id, { mentionAll: $event })"
                          >@所有人</NCheckbox>
                          <NButton size="small" quaternary type="error" @click="removeExternalPushConfig(config.id)">删除</NButton>
                        </div>
                      </div>
                    </div>
                    <NEmpty v-else size="small" description="还没有外部推送配置。" />
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="告警真实度" size="small">
                  <NSpace vertical>
                    <NText depth="3">选择某个告警发送人后，会清空所有在线设备里该人员的可信度反馈记录。</NText>
                    <NSelect v-model:value="alertTrustResetTargetId" :options="adminDeviceOptions" filterable clearable placeholder="选择要清空可信度的人员" />
                    <NButton secondary type="error" :disabled="!alertTrustResetTargetId" @click="resetAlertCredibilityForPeer">清空该用户可信度</NButton>
                    <NButton type="error" @click="resetAllAlertCredibilityRecords">一键清空狼来了排行榜</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="报警模式下发" size="small">
                  <NSpace vertical>
                    <NText depth="3">给指定设备下发本机报警模式。普通报警只闪烁提醒；蹦迪报警会在收到告警时满屏跳动。</NText>
                    <NSelect v-model:value="adminAlertModeTargetId" :options="adminDeviceOptions" filterable clearable placeholder="选择要下发报警模式的设备" />
                    <NRadioGroup v-model:value="adminAlertModeDraft" name="admin-alert-mode">
                      <NSpace>
                        <NRadioButton value="normal">普通报警</NRadioButton>
                        <NRadioButton value="disco">蹦迪报警</NRadioButton>
                      </NSpace>
                    </NRadioGroup>
                    <NButton secondary type="warning" :disabled="!adminAlertModeTargetId" @click="sendAdminAlertModeToPeer">下发报警模式</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="狼来了推送阈值下发" size="small">
                  <NSpace vertical>
                    <NText depth="3">下发后，目标设备仅在告警发送者可信度达到阈值时才触发它自己配置的外部群机器人。</NText>
                    <NSelect
                      v-model:value="adminAlertPushPolicyTargetId"
                      :options="[{ label: '所有在线设备', value: '*' }, ...adminDeviceOptions]"
                      filterable
                      placeholder="选择设备或所有在线设备"
                    />
                    <NInputNumber v-model:value="adminAlertPushPolicyDraft" :min="0" :max="100" style="width: 180px" />
                    <NCheckbox v-model:checked="adminAlertPushPolicyLockAfterIssue">下发后禁止对方本地修改阈值</NCheckbox>
                    <NButton secondary type="warning" :disabled="!adminAlertPushPolicyTargetId" @click="sendAdminAlertPushPolicyToPeer">下发推送阈值</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="LanChat Hub 演进" size="small">
                  <NSpace vertical>
                    <NText depth="3">当前仍使用局域网点对点 TCP 广播；桌宠告警、反馈和排行榜同步已经设计成独立事件帧，后续 Hub 只需要转发这些事件。</NText>
                    <div class="hub-evolution-list">
                      <span>1. 客户端发现 Hub 后优先连接 Hub</span>
                      <span>2. 告警和反馈由 Hub 转发给在线设备</span>
                      <span>3. 无 Hub 时自动回退当前 P2P 模式</span>
                    </div>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="Debug 日志" size="small" class="debug-card">
                  <NSpace vertical>
                    <NText depth="3">打开后会记录设备发现、UDP 广播、mDNS、TCP 连接、在线/离线判定和前端事件。</NText>
                    <NSpace>
                      <NButton :type="debugEnabled ? 'primary' : 'default'" @click="store.setDebugEnabled(!debugEnabled)">
                        {{ debugEnabled ? "关闭 Debug" : "开启 Debug" }}
                      </NButton>
                      <NButton secondary :disabled="debugLogs.length === 0" @click="store.clearDebugLogs">清空日志</NButton>
                    </NSpace>
                    <div v-if="debugEnabled" class="debug-log-panel">
                      <div v-if="debugLogs.length === 0" class="debug-empty">暂无日志，等待设备发现或点击刷新发现。</div>
                      <div v-for="log in debugLogs" :key="`${log.ts}-${log.scope}-${log.message}`" class="debug-line" :class="`level-${log.level}`">
                        <span>{{ formatDebugTime(log.ts) }}</span>
                        <strong>{{ log.level }}</strong>
                        <em>{{ log.scope }}</em>
                        <p>{{ log.message }}</p>
                        <small v-if="log.detail">{{ log.detail }}</small>
                      </div>
                    </div>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="内存诊断" size="small" class="basic-memory-diagnostic-card">
                  <NSpace vertical>
                    <NText depth="3">用于定位消息、头像或图片缓存导致的内存上升。JS 堆数据仅在 WebView2 支持时显示。</NText>
                    <div class="update-info-grid">
                      <span>JS 堆</span><strong>{{ memoryDiagnostic.jsHeapBytes === null ? '当前环境未提供' : `${formatFileSize(memoryDiagnostic.jsHeapBytes)} / ${formatFileSize(memoryDiagnostic.jsHeapLimitBytes)}` }}</strong>
                      <span>消息缓存</span><strong>{{ memoryDiagnostic.cachedConversations }} 个会话 · {{ memoryDiagnostic.cachedMessages }} 条</strong>
                      <span>当前消息节点</span><strong>{{ memoryDiagnostic.visibleMessages }} 条</strong>
                      <span>头像 Base64</span><strong>{{ formatFileSize(memoryDiagnostic.avatarBytes) }}</strong>
                      <span>图片缓存</span><strong>{{ formatFileSize(memoryDiagnostic.previewCacheBytes) }}</strong>
                    </div>
                    <div v-if="memoryDiagnosticConversationRows.length" class="memory-diagnostic-rows">
                      <span>消息最多</span>
                      <strong v-for="row in memoryDiagnosticConversationRows" :key="row.conversationId">{{ row.title }} · {{ row.count }} 条</strong>
                    </div>
                    <div v-if="memoryDiagnosticAvatarRows.length" class="memory-diagnostic-rows">
                      <span>头像最大</span>
                      <strong v-for="row in memoryDiagnosticAvatarRows" :key="row.name">{{ row.name }} · {{ formatFileSize(row.bytes) }}</strong>
                    </div>
                    <NButton secondary type="primary" @click="refreshMemoryDiagnostic">刷新诊断</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="外观" size="small">
                  <NSpace vertical>
                    <NFormItem label="主题" :show-feedback="false">
                      <NDropdown trigger="click" :options="themeMenuOptions" @select="selectTheme">
                        <NButton block>{{ selectedThemeLabel }}</NButton>
                      </NDropdown>
                    </NFormItem>
                    <NFormItem label="语言" :show-feedback="false">
                      <NDropdown trigger="click" :options="languageOptions" @select="selectLanguage">
                        <NButton block>{{ selectedLanguageLabel }}</NButton>
                      </NDropdown>
                    </NFormItem>
                  </NSpace>
                </NCard>
                <NAlert v-if="operationNotice" type="success" closable @close="operationNotice = ''">
                  {{ operationNotice }}
                </NAlert>
                  </div>
                </div>
              </div>
            </section>
          </NLayout>
          <NLayoutSider v-if="groupInspectorAvailable" class="group-inspector" :width="groupInspectorWidth" bordered>
            <button class="pane-resize-handle right-group" type="button" aria-label="拖动调整群成员栏宽度" title="拖动调整宽度" @mousedown="startPaneResize('group', $event)"></button>
            <NScrollbar>
              <div class="group-inspector-inner">
                <section class="group-inspector-section">
                  <div class="group-inspector-headline">
                    <strong>群公告</strong>
                    <NButton v-if="canEditActiveChannelNotice" size="tiny" text @click="channelNoticeEditing ? cancelEditChannelNotice() : startEditChannelNotice()">
                      {{ channelNoticeEditing ? '取消' : '编辑' }}
                    </NButton>
                  </div>
                  <div v-if="channelNoticeEditing" class="group-notice-editor">
                    <NInput v-model:value="channelNoticeDraft" type="textarea" maxlength="240" show-count :rows="4" />
                    <NButton size="small" type="primary" @click="saveActiveChannelNotice">保存公告</NButton>
                  </div>
                  <p v-else class="group-notice-text">{{ activeChannelNotice }}</p>
                </section>
                <section class="group-inspector-section">
                  <div class="group-inspector-headline">
                    <strong>群成员 · {{ normalizedChannelMembers.length }}</strong>
                    <span>{{ channelMembersOnlineCount }} 在线</span>
                  </div>
                  <NEmpty v-if="normalizedChannelMembers.length === 0" description="暂无成员" class="list-empty compact" />
                  <div v-else class="group-member-list">
                    <div v-for="member in normalizedChannelMembers" :key="member.device_id" class="group-member-row" :class="{ 'is-offline': !sameDeviceId(member.device_id, profile?.device_id) && !member.online }">
                      <button class="group-member-main" type="button" @click="openMemberDevice(member)">
                        <img v-if="avatarImage(member.avatar)" class="avatar-image peer-avatar compact-avatar" :src="avatarImage(member.avatar)" alt="成员头像" />
                        <NAvatar v-else class="peer-avatar compact-avatar">{{ firstLetter(memberDisplayName(member)) }}</NAvatar>
                        <span class="group-member-copy">
                          <strong>{{ sameDeviceId(member.device_id, profile?.device_id) ? `我 · ${memberDisplayName(member)}` : memberDisplayName(member) }}</strong>
                          <small>
                            <i class="presence-dot" :class="{ online: sameDeviceId(member.device_id, profile?.device_id) || member.online }"></i>
                            {{ channelMemberPresenceLabel(member) }}
                            <template v-if="isChannelOwnerMember(member)"> · 群主</template>
                            <template v-if="channelMemberMuted(member)"> · 已禁言</template>
                          </small>
                        </span>
                      </button>
                      <div v-if="canManageChannelMember(member)" class="group-member-actions">
                        <NButton size="tiny" text :type="channelMemberMuted(member) ? 'success' : 'warning'" @click.stop="toggleActiveChannelMemberMute(member)">
                          {{ channelMemberMuted(member) ? '解禁' : '禁言' }}
                        </NButton>
                        <NButton v-if="activeConversation?.is_private" size="tiny" text type="error" @click.stop="removeActivePrivateChannelMember(member)">移除</NButton>
                      </div>
                    </div>
                  </div>
                </section>
                <div v-if="canInviteActivePrivateChannel" class="group-inspector-actions">
                  <NButton size="small" secondary type="primary" @click="openRecipientPicker('privateChannelInvite')">邀请成员</NButton>
                  <NButton v-if="activeConversation?.is_private && !sameDeviceId(activeConversation.owner_device_id, profile?.device_id)" size="small" secondary @click="leaveActivePrivateChannel">退出群聊</NButton>
                  <NButton v-if="canManageActivePrivateChannel" size="small" secondary type="error" @click="dissolveActivePrivateChannel">解散频道</NButton>
                </div>
              </div>
            </NScrollbar>
          </NLayoutSider>
        </NLayout>
      </div>
      <NModal
        :show="!!operationErrorMessage"
        preset="card"
        title="操作失败"
        class="operation-error-modal"
        :mask-closable="true"
        @update:show="(visible) => { if (!visible) closeOperationError(); }"
      >
        <div class="operation-error-content">
          <span class="operation-error-icon" aria-hidden="true">×</span>
          <p>{{ operationErrorMessage }}</p>
        </div>
        <template #footer>
          <div class="operation-error-actions">
            <NButton type="primary" size="small" @click="closeOperationError">我知道了</NButton>
          </div>
        </template>
      </NModal>
      <NModal v-model:show="adminNotificationReviewOpen" preset="card" title="通知审核记录" class="admin-notification-review-modal">
        <div class="admin-notification-review-list">
          <div v-if="issuedAdminNotifications.some((item) => item.display_mode === 'requires_confirmation' && item.status === 'submitted')" class="admin-notification-review-actions">
            <NButton size="small" type="success" :loading="adminNotificationBulkProcessing" @click="decideAllSubmittedAdminNotifications('approved')">一键通过待审核</NButton>
            <NButton size="small" type="error" secondary :loading="adminNotificationBulkProcessing" @click="decideAllSubmittedAdminNotifications('rejected')">一键拒绝待审核</NButton>
          </div>
          <NEmpty v-if="issuedAdminNotifications.length === 0" description="暂无本机下发的通知" />
          <div v-for="notification in pagedIssuedAdminNotifications" :key="notification.notification_id" class="admin-notification-review-row">
            <div class="admin-notification-review-device"><img v-if="avatarImage(adminNotificationTargetDetail(notification)?.avatar)" class="avatar-image compact-avatar" :src="avatarImage(adminNotificationTargetDetail(notification)?.avatar)" alt="设备头像" /><NAvatar v-else :size="28" class="peer-avatar">{{ firstLetter(adminNotificationTargetDetail(notification)?.nickname ?? '?') }}</NAvatar><div><strong>{{ notification.title }}</strong><small>昵称：{{ adminNotificationTargetDetail(notification)?.nickname ?? '未知设备' }} · IP：{{ adminNotificationTargetDetail(notification)?.address ?? '未知' }} · MAC：{{ notification.target_device_id }} · {{ notification.status }}</small></div></div>
            <NSpace :size="6"><NButton size="small" quaternary @click="openAdminNotificationDetail(notification)">详情</NButton><template v-if="notification.display_mode === 'requires_confirmation' && notification.status === 'submitted'"><NButton size="small" type="success" @click="decideAdminNotification(notification, 'approved')">通过</NButton><NButton size="small" type="error" secondary @click="decideAdminNotification(notification, 'rejected')">拒绝</NButton></template><NButton v-else-if="notification.display_mode === 'requires_confirmation' && ['pending','expired_locked','rejected'].includes(notification.status)" size="small" tertiary type="warning" @click="decideAdminNotification(notification, 'revoked')">撤销放行</NButton></NSpace>
          </div>
          <NPagination v-if="issuedAdminNotifications.length > ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE" v-model:page="adminNotificationReviewPage" :page-count="adminNotificationReviewPageCount" :page-size="ADMIN_NOTIFICATION_REVIEW_PAGE_SIZE" />
        </div>
      </NModal>
      <NModal v-model:show="superAdminAuthOpen" preset="card" title="超级管理员验证" class="super-admin-auth-modal">
          <NSpace vertical>
            <NFormItem label="密码" :show-feedback="false">
              <NInput
                v-model:value="superAdminPasswordDraft"
                type="password"
                show-password-on="click"
                clearable
                placeholder="请输入超级管理员密码"
                @keydown.enter="confirmSuperAdminPassword"
              />
            </NFormItem>
            <NAlert v-if="superAdminPasswordError" type="error" :show-icon="false">{{ superAdminPasswordError }}</NAlert>
            <NButton block type="primary" @click="confirmSuperAdminPassword">验证并开启</NButton>
          </NSpace>
      </NModal>
        <NModal v-model:show="recipientPickerOpen" preset="card" :title="recipientPickerTitle" class="recipient-picker-modal">
          <div class="recipient-picker">
            <NFormItem v-if="recipientPickerMode === 'privateChannelCreate'" label="频道名称" :show-feedback="false">
              <NInput v-model:value="privateChannelTitleDraft" maxlength="24" clearable placeholder="例如 项目私有频道" />
            </NFormItem>
            <div class="recipient-scroll">
              <section class="recipient-picker-section">
                <div class="recipient-section-head">
                  <strong>选择频道成员</strong>
                  <span>{{ selectedRecipientPeerIds.length }} 已选</span>
                </div>
                <div v-if="pickerPeerOptions.length > 0" class="recipient-list">
                  <button
                    v-for="peer in pickerPeerOptions"
                    :key="peer.device_id"
                    class="recipient-list-row"
                    :class="{ active: selectedRecipientPeerIds.includes(peer.device_id) }"
                    type="button"
                    @click="toggleRecipientPeer(peer.device_id)"
                  ><img v-if="avatarImage(peer.avatar)" class="avatar-image peer-avatar" :src="avatarImage(peer.avatar)" alt="设备头像" /><NAvatar v-else class="peer-avatar">{{ firstLetter(peer.nickname) }}</NAvatar>
                    <span class="recipient-list-main">
                      <strong>{{ peer.nickname }}</strong>
                      <small>{{ peer.address }}:{{ peer.port }}</small>
                    </span>
                    <span class="recipient-list-check">{{ selectedRecipientPeerIds.includes(peer.device_id) ? '✓' : '' }}</span>
                  </button>
                </div>
                <div v-else class="recipient-empty">暂无可选择的在线设备</div>
              </section>
            </div>
            <div class="recipient-picker-footer">
              <NText depth="3">
                私有频道消息会广播投递，但只有持有频道密钥的成员能解密。
              </NText>
              <NSpace justify="end">
                <NButton secondary @click="recipientPickerOpen = false">取消</NButton>
                <NButton type="primary" :disabled="recipientConfirmDisabled" @click="confirmRecipientPicker">
                  {{ recipientPickerMode === 'privateChannelCreate' ? '创建频道' : '邀请加入' }}
                </NButton>
              </NSpace>
            </div>
          </div>
        </NModal>
      </NMessageProvider>
  </NConfigProvider>
</template>

<style scoped src="../styles/app.css"></style>
