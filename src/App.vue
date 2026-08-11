<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
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
import { api } from "./services/tauri-api";
import ChatComposerInput from "./components/ChatComposerInput.vue";
import { DEFAULT_GROUP_ID, useLanChatStore } from "./stores/lanchat";
import { useDesktopPetStore } from "./stores/desktopPet";
import type { DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings, ExternalPushConfig, ExternalPushKind, PetPackageSource, PetStateKind, PetStatePlaybackConfig } from "./types/desktop-pet";
import type { AdminAlertMode, AdminAlertPushPolicy, AdminDiscoMode, AdminNotification, AppVersionInfo, CallSignal, ChannelMember, Conversation, DesktopPetRuntimeState, GameFrame, Message, Nudge, Peer, PetAlertMode, PlatformInfo, PreviewMediaCacheInfo, PrivateChannelInvitePayload, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset, SimulationMeta, TrayAttentionItem, UpdateCheckResult } from "./types/lanchat";
import { DDZ_TURN_TIMEOUT_MS, canBeat, dealHands, evaluatePlay, isTurnTimedOut, playLabel, sortCards, turnRemainingSeconds, type DdzCard, type DdzPhase, type DdzPlay } from "./games/doudizhu";
import { GOMOKU_TURN_TIMEOUT_MS, chooseAutoGomokuPoint, cloneGomokuBoard, createGomokuBoard, gomokuStoneLabel, gomokuTurnRemainingSeconds, isGomokuTurnTimedOut, placeGomokuStone, type GomokuBoard, type GomokuPhase, type GomokuPoint, type GomokuStone } from "./games/gomoku";
import { cloneXiangqiBoard, createXiangqiBoard, createXiangqiDisplayGrid, isLegalXiangqiMove, moveXiangqiPiece, otherXiangqiSide, resignXiangqiSide, undoXiangqiMove, xiangqiPieceLabel, xiangqiSideLabel, type XiangqiBoard, type XiangqiPhase, type XiangqiPiece, type XiangqiPoint, type XiangqiSide } from "./games/xiangqi";
import { MINESWEEPER_DEFAULT_HEIGHT, MINESWEEPER_DEFAULT_MINES, MINESWEEPER_DEFAULT_WIDTH, chordRevealMinesweeperCell, cloneMinesweeperBoard, createMinesweeperBoard, getMinesweeperProgress, revealMinesweeperCell, toggleMinesweeperFlag, type MinesweeperBoard, type MinesweeperCell, type MinesweeperPhase, type MinesweeperPoint } from "./games/minesweeper";
import { MINESWEEPER_DIFFICULTIES, createMinesweeperLeaderboardRecord, difficultyByKey, formatMinesweeperElapsed, minesweeperDifficultyLabel, recordsForDifficulty, upsertMinesweeperLeaderboardRecords, type MinesweeperLeaderboardRecord } from "./games/minesweeperLeaderboard";
import { formatWinRate, incrementGameStats, recordsForGame, upsertGameStatsRecords, type GameStatsRecord, type RankedGameType } from "./games/gameLeaderboard";
import { createGameRoomShell, gameDefinitionOf, gameRegistry, type GameRoomShell, type GameType } from "./games/registry";
import { alertTemperature, alertTruthScore, senderCredibility } from "./utils/alertCredibility";
import { detectMentionKind, trayConversationTitle, type MentionKind } from "./utils/messageMentions";
import { peerDisplayName, peerOriginalName, sameDeviceId, sortPeersForDisplay } from "./utils/peerPresentation";
type UiThemeKey = "theme-dingtalk" | "theme-work" | "theme-lan" | "theme-light";
type MainSection = "chat" | "devices" | "games" | "alerts" | "settings";
type RecipientPickerMode = "gameInvite" | "privateChannelCreate" | "privateChannelInvite";
type SimulationKind = "direct" | "channel" | "alert" | "disco";
type UndoRequest = {
  requesterId: string;
  requesterName: string;
  createdAt: number;
};
type RoomChatItem = {
  id: string;
  senderDeviceId: string;
  sender: string;
  content: string;
  mine: boolean;
  createdAt: number;
};
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
  simulation?: SimulationMeta | null;
  createdAt: number;
  incoming: boolean;
  handled: boolean;
  localFeedback?: AlertFeedbackResult;
  feedbacks: AlertFeedbackRecord[];
};
type DdzSeat = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  ready: boolean;
  role?: "landlord" | "farmer";
  handCount: number;
};
type DdzTableState = {
  roomId: string;
  phase: DdzPhase;
  players: DdzSeat[];
  landlordCards: DdzCard[];
  hands: Record<string, DdzCard[]>;
  turnDeviceId?: string;
  turnStartedAt?: number;
  landlordDeviceId?: string;
  bidOrder: string[];
  bidIndex: number;
  bids: Record<string, boolean>;
  lastPlay: DdzPlay | null;
  passCount: number;
  winnerDeviceId?: string;
  winnerName?: string;
  chatMessages: RoomChatItem[];
  logs: string[];
  updatedAt: number;
};
type DdzActionPayload =
  | { action: "join"; player: DdzSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "bid"; playerId: string; call: boolean }
  | { action: "play"; playerId: string; cardIds: string[] }
  | { action: "pass"; playerId: string }
  | { action: "leave"; playerId: string }
  | { action: "chat"; message: RoomChatItem };
type GomokuSeat = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  ready: boolean;
  stone?: GomokuStone;
};
type GomokuMove = GomokuPoint & {
  playerId: string;
  playerName: string;
  stone: GomokuStone;
  createdAt: number;
};
type GomokuTableState = {
  roomId: string;
  phase: GomokuPhase;
  players: GomokuSeat[];
  board: GomokuBoard;
  moves: GomokuMove[];
  turnDeviceId?: string;
  turnStartedAt?: number;
  winnerDeviceId?: string;
  winnerName?: string;
  winnerStone?: GomokuStone;
  winLine: GomokuPoint[];
  pendingUndo?: UndoRequest;
  chatMessages: RoomChatItem[];
  logs: string[];
  updatedAt: number;
};
type GomokuActionPayload =
  | { action: "join"; player: GomokuSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "move"; playerId: string; x: number; y: number }
  | { action: "undo_request"; playerId: string }
  | { action: "undo_response"; playerId: string; accepted: boolean }
  | { action: "resign"; playerId: string }
  | { action: "leave"; playerId: string }
  | { action: "chat"; message: RoomChatItem };
type MinesweeperSeat = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  ready: boolean;
};
type MinesweeperPlayerState = {
  board: MinesweeperBoard;
  status: "playing" | "won" | "lost";
  moves: number;
  startedAt: number;
  finishedAt?: number;
  revealedSafe: number;
  totalSafe: number;
  flagged: number;
};
type MinesweeperTableState = {
  roomId: string;
  phase: MinesweeperPhase;
  players: MinesweeperSeat[];
  width: number;
  height: number;
  mines: number;
  seed: number;
  boards: Record<string, MinesweeperPlayerState>;
  winnerDeviceId?: string;
  winnerName?: string;
  chatMessages: RoomChatItem[];
  logs: string[];
  updatedAt: number;
};
type MinesweeperActionPayload =
  | { action: "join"; player: MinesweeperSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "difficulty"; playerId: string; width: number; height: number; mines: number }
  | { action: "reveal"; playerId: string; x: number; y: number }
  | { action: "flag"; playerId: string; x: number; y: number }
  | { action: "chord"; playerId: string; x: number; y: number }
  | { action: "leave"; playerId: string }
  | { action: "chat"; message: RoomChatItem };type XiangqiSeat = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  ready: boolean;
  side?: XiangqiSide;
};
type XiangqiMove = {
  from: XiangqiPoint;
  to: XiangqiPoint;
  playerId: string;
  playerName: string;
  side: XiangqiSide;
  piece?: XiangqiPiece;
  captured?: XiangqiPiece | null;
  previousCheckSide?: XiangqiSide;
  pieceLabel: string;
  capturedLabel?: string;
  createdAt: number;
};
type XiangqiTableState = {
  roomId: string;
  phase: XiangqiPhase;
  players: XiangqiSeat[];
  board: XiangqiBoard;
  moves: XiangqiMove[];
  turnDeviceId?: string;
  turnStartedAt?: number;
  winnerDeviceId?: string;
  winnerName?: string;
  winnerSide?: XiangqiSide;
  checkSide?: XiangqiSide;
  pendingUndo?: UndoRequest;
  chatMessages: RoomChatItem[];
  logs: string[];
  updatedAt: number;
};
type XiangqiActionPayload =
  | { action: "join"; player: XiangqiSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "move"; playerId: string; from: XiangqiPoint; to: XiangqiPoint }
  | { action: "undo_request"; playerId: string }
  | { action: "undo_response"; playerId: string; accepted: boolean }
  | { action: "resign"; playerId: string }
  | { action: "leave"; playerId: string }
  | { action: "chat"; message: RoomChatItem };
type GameActionPayload = DdzActionPayload | GomokuActionPayload | XiangqiActionPayload | MinesweeperActionPayload;
type GameInvitePayload = {
  roomId: string;
  roomName: string;
  gameType: GameType;
  gameName: string;
  hostName: string;
  hostDeviceId?: string;
  createdAt: number;
};
type LeaderboardSyncPayload = {
  gameStatsRecords?: GameStatsRecord[];
  minesweeperLeaderboardRecords?: MinesweeperLeaderboardRecord[];
};
const GAME_INVITE_PREFIX = "LANCHAT_GAME_INVITE:";
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
  manualAddress,
  manualPort,
  draft,
} = storeToRefs(store);
const messagePane = ref<HTMLElement | null>(null);
const roomChatPane = ref<HTMLElement | null>(null);
const mentionPickerOpen = ref(false);
const mentionSearch = ref("");
type MentionNotice = { messageId: string; kind: MentionKind; createdAt: number };
const mentionNoticesByConversation = ref<Record<string, MentionNotice[]>>({});
const highlightedMentionMessageId = ref("");
let mentionHighlightTimer: number | null = null;
const platformInfo = ref<PlatformInfo | null>(null);
const nicknameDraft = ref("");
const portDraft = ref(18145);
const avatarDraft = ref("");
const profileAvatarInput = ref<HTMLInputElement | null>(null);
const adminNotificationImageInput = ref<HTMLInputElement | null>(null);
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
let autoTurnRunning = false;
let unlistenTrayOpenTarget: (() => void) | null = null;
let unlistenDesktopPetAction: (() => void) | null = null;
let unlistenDesktopPetStopHotkey: (() => void) | null = null;
let unlistenDesktopPetSendHotkey: (() => void) | null = null;
let unlistenDesktopPetRegistry: (() => void) | null = null;
const conversationSearch = ref("");
const deviceSearch = ref("");
const selectedPeerId = ref("");
const peerNoteDraft = ref("");
const previewMediaPaths = ref<Record<string, string>>({});
const previewMediaCacheInfo = ref<PreviewMediaCacheInfo | null>(null);
const previewMediaCacheClearing = ref(false);
const imagePreviewMessage = ref<Message | null>(null);
const imagePreviewScale = ref(1);
const operationNotice = ref("");
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
const callMuted = ref(false);
const callCameraOn = ref(true);
const callActionInProgress = ref(false);
let callPeerConnection: RTCPeerConnection | null = null;
let callLocalStream: MediaStream | null = null;
let callRemoteStream: MediaStream | null = null;
let detachedCallWindow: DetachedCallWindow | null = null;
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
const updateReminderOpen = ref(false);
const nativeUpdateInstalling = ref(false);
const nativeUpdateProgress = ref({ downloaded: 0, total: 0, phase: "idle" as "idle" | "downloading" | "installing" });
const forceUpdateRequired = computed(() => updateInfo.value?.forceRequired === true);
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
const adminNotificationTargetOptions = computed(() => onlinePeers.value.map((peer) => ({
  label: `${peerDisplayName(peer)} · ${peer.address} · ${peer.device_id}`,
  value: peer.device_id,
})));
const UPDATE_CHECK_INTERVAL_MS = 12 * 60 * 60 * 1000;
const activeSection = ref<MainSection>("chat");
const settingsCategory = ref<"basic" | "pet" | "admin">("basic");
const listPaneCollapsed = ref(false);
type ResizePaneKind = "list" | "group";
type PaneResizeState = { kind: ResizePaneKind; startX: number; startWidth: number };
const listPaneWidth = ref(readSavedPaneWidth("lanchat-list-pane-width", 292, 240, 380));
const groupInspectorWidth = ref(readSavedPaneWidth("lanchat-group-inspector-width", 252, 210, 340));
const paneResizeState = ref<PaneResizeState | null>(null);
const chatEmojiOpen = ref(false);
const roomEmojiOpen = ref(false);
const roomChatDraft = ref("");
const nowTick = ref(Date.now());
const createRoomOpen = ref(false);
const createRoomGameMenuOpen = ref(false);
const channelNoticeEditing = ref(false);
const channelNoticeDraft = ref("");
const channelNotices = ref<Record<string, string>>(readSavedChannelNotices());
const publicChannelMutedIds = ref<Record<string, boolean>>(readSavedPublicChannelMutedIds());
const recipientPickerOpen = ref(false);
const recipientPickerMode = ref<RecipientPickerMode>("gameInvite");
const selectedRecipientPeerIds = ref<string[]>([]);
const selectedRecipientConversationIds = ref<string[]>([]);
const privateChannelTitleDraft = ref("私有频道");
const handledPrivateChannelInvites = ref<Record<string, "accepted" | "rejected">>(readSavedPrivateChannelInviteStates());
const leaderboardOpen = ref(false);
const gameStatsRecords = ref<GameStatsRecord[]>(readSavedGameStatsRecords());
const minesweeperLeaderboardRecords = ref<MinesweeperLeaderboardRecord[]>(readSavedMinesweeperLeaderboardRecords());
const selectedMinesweeperLeaderboardKey = ref(MINESWEEPER_DIFFICULTIES[0]?.key ?? "");
const recordedGameResultIds = new Set<string>();
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
const PET_DISCO_ALERT_DURATION_MS = 60_000;
const selectedGameType = ref<GameType>("doudizhu");
const roomNameDraft = ref("午休娱乐局");
const gameRoomsState = ref<GameRoomShell[]>([]);
const activeGameRoomId = ref("");
const selectedCardIds = ref<string[]>([]);
const selectedXiangqiPoint = ref<XiangqiPoint | null>(null);
const doudizhuRooms = ref<Record<string, DdzTableState>>({});
const gomokuRooms = ref<Record<string, GomokuTableState>>({});
const xiangqiRooms = ref<Record<string, XiangqiTableState>>({});
const minesweeperRooms = ref<Record<string, MinesweeperTableState>>({});
const emojiOptions = ["😀", "😄", "😂", "😉", "👍", "👏", "🎉", "🔥", "❤️", "👌", "😎", "🤝", "🍵", "🃏", "💣", "🚀"];
const navExpanded = ref(readSavedNavExpanded());
const themeOptions: Array<{ label: string; key: UiThemeKey; accent: string; hover: string; pressed: string }> = [
  { label: "钉钉商务蓝", key: "theme-dingtalk", accent: "#1677ff", hover: "#4096ff", pressed: "#0958d9" },
  { label: "企业灰白", key: "theme-work", accent: "#2f6fed", hover: "#5287f2", pressed: "#1f55bf" },
  { label: "局域网设备感", key: "theme-lan", accent: "#0f8f83", hover: "#14a99a", pressed: "#0b746c" },
  { label: "轻量清新", key: "theme-light", accent: "#2a9df4", hover: "#55b5fb", pressed: "#177ec7" },
];
const languageOptions = [
  { label: "简体中文", key: "zh-CN" },
  { label: "English", key: "en-US" },
];
const selectedLanguage = ref(readSavedLanguage());
const themeMenuOptions = themeOptions.map((item) => ({ label: item.label, key: item.key }));
const messageContextOptions = computed(() => {
  const message = messageContextMessage.value;
  return canRecallMessage(message) ? [{ label: "撤回", key: "recall" }] : [];
});
const selectedTheme = ref<UiThemeKey>(readSavedTheme());
const currentTheme = computed(() => themeOptions.find((item) => item.key === selectedTheme.value) ?? themeOptions[0]);
const selectedThemeLabel = computed(() => currentTheme.value.label);
const selectedLanguageLabel = computed(
  () => languageOptions.find((item) => item.key === selectedLanguage.value)?.label ?? "简体中文",
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
const pickerConversationOptions = computed(() => sortedConversations.value.filter((conversation) => {
  if (recipientPickerMode.value !== "gameInvite") return false;
  return conversation.kind === "group";
}));
const recipientPickerTitle = computed(() => {
  if (recipientPickerMode.value === "privateChannelCreate") return "创建私有频道";
  if (recipientPickerMode.value === "privateChannelInvite") return "邀请频道成员";
  return "发送游戏邀请";
});
const recipientConfirmDisabled = computed(() => {
  if (recipientPickerMode.value === "privateChannelCreate") return !privateChannelTitleDraft.value.trim();
  if (recipientPickerMode.value === "privateChannelInvite") return selectedRecipientPeerIds.value.length === 0;
  return selectedRecipientPeerIds.value.length + selectedRecipientConversationIds.value.length === 0;
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
const selectedDeviceChannelMembers = computed<Array<ChannelMember | Peer>>(() => {
  const channel = selectedDeviceChannelDetail.value;
  if (!channel) return [];
  return sortChannelMembers(channel.is_private ? channelMembersByConversation.value[channel.id] ?? [] : chatCapablePeers.value);
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
const channelMembers = computed<Array<ChannelMember | Peer>>(() => activeConversation.value?.is_private ? activePrivateChannelMembers.value : chatCapablePeers.value);
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
const activeGameRoom = computed(() => gameRoomsState.value.find((room) => room.roomId === activeGameRoomId.value) ?? null);
const activeGameDefinition = computed(() => gameDefinitionOf(activeGameRoom.value?.gameType ?? selectedGameType.value));
const activeDdzState = computed(() => doudizhuRooms.value[activeGameRoomId.value] ?? null);
const activeGomokuState = computed(() => gomokuRooms.value[activeGameRoomId.value] ?? null);
const activeXiangqiState = computed(() => xiangqiRooms.value[activeGameRoomId.value] ?? null);
const activeMinesweeperState = computed(() => minesweeperRooms.value[activeGameRoomId.value] ?? null);
const myDeviceId = computed(() => profile.value?.device_id ?? "");
const myDdzSeat = computed(() => activeDdzState.value?.players.find((player) => player.deviceId === myDeviceId.value) ?? null);
const myGomokuSeat = computed(() => activeGomokuState.value?.players.find((player) => player.deviceId === myDeviceId.value) ?? null);
const myXiangqiSeat = computed(() => activeXiangqiState.value?.players.find((player) => player.deviceId === myDeviceId.value) ?? null);
const myMinesweeperSeat = computed(() => activeMinesweeperState.value?.players.find((player) => player.deviceId === myDeviceId.value) ?? null);
const myGameSeat = computed(() => {
  if (activeGameRoom.value?.gameType === "gomoku") return myGomokuSeat.value;
  if (activeGameRoom.value?.gameType === "xiangqi") return myXiangqiSeat.value;
  if (activeGameRoom.value?.gameType === "minesweeper") return myMinesweeperSeat.value;
  return myDdzSeat.value;
});
const myDdzHand = computed(() => sortCards(activeDdzState.value?.hands[myDeviceId.value] ?? []));
const selectedCards = computed(() => myDdzHand.value.filter((card) => selectedCardIds.value.includes(card.id)));
const selectedPlay = computed(() => evaluatePlay(selectedCards.value));
const isMyDdzTurn = computed(() => activeDdzState.value?.turnDeviceId === myDeviceId.value);
const isMyGomokuTurn = computed(() => activeGomokuState.value?.turnDeviceId === myDeviceId.value);
const isMyXiangqiTurn = computed(() => activeXiangqiState.value?.turnDeviceId === myDeviceId.value);
const isDdzLeading = computed(() => !activeDdzState.value?.lastPlay || activeDdzState.value.lastPlay.playerId === myDeviceId.value);
const canPassDdz = computed(() => activeDdzState.value?.phase === "playing" && isMyDdzTurn.value && !isDdzLeading.value);
const canPlaySelectedCards = computed(() => {
  if (activeDdzState.value?.phase !== "playing" || !isMyDdzTurn.value) return false;
  return canBeat(selectedCards.value, isDdzLeading.value ? null : activeDdzState.value.lastPlay);
});
const playHint = computed(() => {
  if (!activeDdzState.value) return "先创建或加入斗地主房间";
  if (activeDdzState.value.phase === "lobby") return "凑齐 3 人并全部准备后自动发牌";
  if (activeDdzState.value.phase === "bidding") return isMyDdzTurn.value ? "轮到你叫地主" : "等待其他玩家叫地主";
  if (activeDdzState.value.phase === "ended") return activeDdzState.value.winnerName ? `${activeDdzState.value.winnerName} 获胜` : "牌局结束";
  if (selectedCards.value.length === 0) return isMyDdzTurn.value ? "请选择要出的牌" : "等待对方出牌";
  const label = playLabel(selectedPlay.value);
  return canPlaySelectedCards.value ? `${label}，可以出牌` : `${label}，压不过上家，只能不要`;
});
const minesweeperDifficultyOptions = MINESWEEPER_DIFFICULTIES.map((difficulty) => ({ label: `${difficulty.label} · ${difficulty.mines} 雷`, key: difficulty.key }));
const selectedCreateRoomGame = computed(() => gameDefinitionOf(selectedGameType.value));
const leaderboardTitle = computed(() => `${activeGameDefinition.value.name}排行榜`);
const rankedActiveGame = computed<RankedGameType | null>(() => {
  const game = activeGameRoom.value?.gameType ?? selectedGameType.value;
  return game === "doudizhu" || game === "gomoku" || game === "xiangqi" ? game : null;
});
const activeGameStatsRows = computed(() => {
  const game = rankedActiveGame.value;
  return game ? recordsForGame(gameStatsRecords.value, game, 30) : [];
});
const minesweeperLeaderboardRows = computed(() => recordsForDifficulty(
  minesweeperLeaderboardRecords.value,
  selectedMinesweeperLeaderboardKey.value,
  Number.MAX_SAFE_INTEGER,
));
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
  (latestPendingAlert.value && !visuallyStoppedAlertIds.value.has(latestPendingAlert.value.alertId) ? latestPendingAlert.value : null)
  ?? (latestOwnAlert.value && ownAlertFlashUntil.value > 0 && !visuallyStoppedAlertIds.value.has(latestOwnAlert.value.alertId) ? latestOwnAlert.value : null),
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
const petAlertProbability = computed(() => alertDisplayTemperature(activePetAlert.value));
const discoModeActive = computed(() => discoModeUntil.value > nowTick.value);
const activeRoomChatMessages = computed(() => {
  if (activeGameRoom.value?.gameType === "gomoku") return activeGomokuState.value?.chatMessages ?? [];
  if (activeGameRoom.value?.gameType === "xiangqi") return activeXiangqiState.value?.chatMessages ?? [];
  if (activeGameRoom.value?.gameType === "minesweeper") return activeMinesweeperState.value?.chatMessages ?? [];
  return activeDdzState.value?.chatMessages ?? [];
});
const activeTurnRemainingSeconds = computed(() => {
  const state = activeDdzState.value;
  if (!state || !state.turnDeviceId || (state.phase !== "bidding" && state.phase !== "playing")) return 0;
  return turnRemainingSeconds(state.turnStartedAt, nowTick.value, DDZ_TURN_TIMEOUT_MS);
});
const activeGomokuTurnRemainingSeconds = computed(() => {
  const state = activeGomokuState.value;
  if (!state || !state.turnDeviceId || state.phase !== "playing" || state.pendingUndo) return 0;
  return gomokuTurnRemainingSeconds(state.turnStartedAt, nowTick.value, GOMOKU_TURN_TIMEOUT_MS);
});
const visibleLandlordCards = computed(() => {
  const state = activeDdzState.value;
  if (!state || state.landlordCards.length === 0 || state.phase === "bidding") return [null, null, null];
  return state.landlordCards;
});
const tableLastCards = computed(() => activeDdzState.value?.lastPlay?.cards ?? []);
const settlementRows = computed(() => {
  const state = activeDdzState.value;
  if (!state) return [];
  return state.players.map((player) => ({
    ...player,
    remaining: state.hands[player.deviceId]?.length ?? player.handCount,
  }));
});
const settlementWinnerLabel = computed(() => activeDdzState.value?.winnerName ?? "本局结束");
const gomokuSeats = computed(() => activeGomokuState.value?.players ?? []);
const blackGomokuSeat = computed(() => gomokuSeats.value.find((player) => player.stone === "black") ?? null);
const whiteGomokuSeat = computed(() => gomokuSeats.value.find((player) => player.stone === "white") ?? null);
const gomokuWinPointKeys = computed(() => new Set((activeGomokuState.value?.winLine ?? []).map((point) => `${point.x}:${point.y}`)));
const gomokuBoardPoints = computed(() => (activeGomokuState.value?.board ?? []).flatMap((row, y) => row.map((cell, x) => ({ x, y, cell }))));
const lastOpponentGomokuMove = computed(() => {
  const moves = activeGomokuState.value?.moves ?? [];
  for (let index = moves.length - 1; index >= 0; index -= 1) {
    if (moves[index]?.playerId !== myDeviceId.value) return moves[index] ?? null;
  }
  return null;
});
const lastGomokuMove = computed(() => activeGomokuState.value?.moves.slice(-1)[0] ?? null);
const canRequestUndoGomoku = computed(() => activeGomokuState.value?.phase === "playing" && !!myGomokuSeat.value && !!lastGomokuMove.value && !activeGomokuState.value?.pendingUndo);
const canRespondGomokuUndo = computed(() => {
  const state = activeGomokuState.value;
  const pending = state?.pendingUndo;
  return state?.phase === "playing" && !!pending && pending.requesterId !== myDeviceId.value;
});
const canResignGomoku = computed(() => activeGomokuState.value?.phase === "playing" && !!myGomokuSeat.value);
const gomokuSettlementRows = computed(() => gomokuSeats.value.map((player) => ({
  ...player,
  result: activeGomokuState.value?.winnerDeviceId === player.deviceId ? "胜利" : activeGomokuState.value?.winnerDeviceId ? "失败" : "平局",
})));
const xiangqiSeats = computed(() => activeXiangqiState.value?.players ?? []);

const xiangqiPerspectiveSide = computed<XiangqiSide>(() => myXiangqiSeat.value?.side === "black" ? "black" : "red");
const leftXiangqiSide = computed<XiangqiSide>(() => xiangqiPerspectiveSide.value === "black" ? "red" : "black");
const rightXiangqiSide = computed<XiangqiSide>(() => xiangqiPerspectiveSide.value === "black" ? "black" : "red");
const leftXiangqiSeat = computed(() => xiangqiSeats.value.find((player) => player.side === leftXiangqiSide.value) ?? null);
const rightXiangqiSeat = computed(() => xiangqiSeats.value.find((player) => player.side === rightXiangqiSide.value) ?? null);
const xiangqiDisplayRows = computed(() => {
  const board = activeXiangqiState.value?.board;
  if (!board) return [];
  return createXiangqiDisplayGrid(xiangqiPerspectiveSide.value).map((row) =>
    row.map((point) => ({ ...point, cell: board[point.y]?.[point.x] ?? null })),
  );
});
const lastXiangqiMove = computed(() => activeXiangqiState.value?.moves.slice(-1)[0] ?? null);
const lastOpponentXiangqiMove = computed(() => {
  const moves = activeXiangqiState.value?.moves ?? [];
  for (let index = moves.length - 1; index >= 0; index -= 1) {
    if (moves[index]?.playerId !== myDeviceId.value) return moves[index] ?? null;
  }
  return null;
});
const canRequestUndoXiangqi = computed(() => activeXiangqiState.value?.phase === "playing" && !!myXiangqiSeat.value && !!lastXiangqiMove.value?.piece && !activeXiangqiState.value?.pendingUndo);
const canRespondXiangqiUndo = computed(() => {
  const state = activeXiangqiState.value;
  const pending = state?.pendingUndo;
  return state?.phase === "playing" && !!pending && pending.requesterId !== myDeviceId.value;
});
const canResignXiangqi = computed(() => activeXiangqiState.value?.phase === "playing" && !!myXiangqiSeat.value);
const isMyXiangqiChecked = computed(() => !!activeXiangqiState.value?.checkSide && activeXiangqiState.value.checkSide === myXiangqiSeat.value?.side);
const xiangqiSettlementRows = computed(() => xiangqiSeats.value.map((player) => ({
  ...player,
  result: activeXiangqiState.value?.winnerDeviceId === player.deviceId ? "胜利" : activeXiangqiState.value?.winnerDeviceId ? "失败" : "结束",
})));
const minesweeperPlayers = computed(() => activeMinesweeperState.value?.players ?? []);
const myMinesweeperBoardState = computed(() => activeMinesweeperState.value?.boards[myDeviceId.value] ?? null);

const activeMinesweeperDifficultyLabel = computed(() => {
  const state = activeMinesweeperState.value;
  return state
    ? minesweeperDifficultyLabel(state.width, state.height, state.mines)
    : minesweeperDifficultyLabel(MINESWEEPER_DEFAULT_WIDTH, MINESWEEPER_DEFAULT_HEIGHT, MINESWEEPER_DEFAULT_MINES);
});
const minesweeperBoardStyle = computed<Record<string, string>>(() => {
  const width = activeMinesweeperState.value?.width ?? MINESWEEPER_DEFAULT_WIDTH;
  const height = activeMinesweeperState.value?.height ?? MINESWEEPER_DEFAULT_HEIGHT;
  return {
    gridTemplateColumns: `repeat(${width}, minmax(0, 1fr))`,
    aspectRatio: `${width} / ${height}`,
    "--minesweeper-ratio": String(width / height),
  };
});
const minesweeperSettlementRows = computed(() => minesweeperPlayers.value.map((player) => {
  const boardState = activeMinesweeperState.value?.boards[player.deviceId];
  return {
    ...player,
    boardState,
    result: activeMinesweeperState.value?.winnerDeviceId === player.deviceId ? "胜利" : boardState?.status === "lost" ? "失败" : boardState?.status === "won" ? "完成" : "进行中",
  };
}));
const leftDdzSeat = computed(() => otherDdzSeats().slice(0, 1)[0] ?? null);
const rightDdzSeat = computed(() => otherDdzSeats().slice(1, 2)[0] ?? null);
const roomPrimaryLabel = computed(() => {
  const room = activeGameRoom.value;
  if (!room) return "先创建房间";
  if (room.gameType === "gomoku") {
    const state = activeGomokuState.value;
    if (!state) return "先创建房间";
    if (!myGomokuSeat.value) return "加入房间";
    if (state.phase === "lobby") return myGomokuSeat.value.ready ? "取消准备" : "准备";
    if (state.phase === "ended") return isRoomHost() ? "再来一局" : "等待房主开局";
    return isMyGomokuTurn.value ? "轮到你" : "等待中";
  }
  if (room.gameType === "minesweeper") {
    const state = activeMinesweeperState.value;
    if (!state) return "先创建房间";
    if (!myMinesweeperSeat.value) return "加入房间";
    if (state.phase === "lobby") return myMinesweeperSeat.value.ready ? "取消准备" : "准备";
    if (state.phase === "ended") return isRoomHost() ? "再来一局" : "等待房主开局";
    return myMinesweeperBoardState.value?.status === "playing" ? "扫雷中" : "等待结算";
  }  if (room.gameType === "xiangqi") {
    const state = activeXiangqiState.value;
    if (!state) return "先创建房间";
    if (!myXiangqiSeat.value) return "加入房间";
    if (state.phase === "lobby") return myXiangqiSeat.value.ready ? "取消准备" : "准备";
    if (state.phase === "ended") return isRoomHost() ? "再来一局" : "等待房主开局";
    return isMyXiangqiTurn.value ? "轮到你" : "等待中";
  }
  if (!activeDdzState.value) return "先创建房间";
  if (!myDdzSeat.value) return "加入房间";
  if (activeDdzState.value.phase === "lobby") return myDdzSeat.value.ready ? "取消准备" : "准备";
  if (activeDdzState.value.phase === "ended") return isRoomHost() ? "再来一局" : "等待房主开局";
  return isMyDdzTurn.value ? "轮到你" : "等待中";
});
const listPaneAvailable = computed(() => ["chat", "devices", "games"].includes(activeSection.value));
const listPaneToggleTitle = computed(() => listPaneCollapsed.value ? "展开列表栏" : "收起列表栏");
const isGameStarted = computed(() => {
  if (activeGameRoom.value?.gameType === "gomoku") return activeGomokuState.value?.phase === "playing";
  if (activeGameRoom.value?.gameType === "xiangqi") return activeXiangqiState.value?.phase === "playing";
  if (activeGameRoom.value?.gameType === "minesweeper") return activeMinesweeperState.value?.phase === "playing";
  return activeDdzState.value?.phase === "bidding" || activeDdzState.value?.phase === "playing";
});
const gameAttentionCount = computed(() => {
  const deviceId = myDeviceId.value;
  if (!deviceId) return 0;
  let count = 0;
  for (const state of Object.values(doudizhuRooms.value)) {
    if ((state.phase === "bidding" || state.phase === "playing") && state.turnDeviceId === deviceId) count += 1;
  }
  for (const state of Object.values(gomokuRooms.value)) {
    if (state.phase !== "playing") continue;
    const shouldRemind = state.turnDeviceId === deviceId || (!!state.pendingUndo && state.pendingUndo.requesterId !== deviceId);
    if (shouldRemind) count += 1;
  }
  for (const state of Object.values(xiangqiRooms.value)) {
    if (state.phase !== "playing") continue;
    const shouldRemind = state.turnDeviceId === deviceId || (!!state.pendingUndo && state.pendingUndo.requesterId !== deviceId);
    if (shouldRemind) count += 1;
  }
  return count;
});
function gameRoomTrayTitle(roomId: string) {
  const room = gameRoomsState.value.find((item) => item.roomId === roomId);
  if (!room) return "游戏房间";
  return `${room.roomName} · ${gameDefinitionOf(room.gameType).name}`;
}
function buildTrayAttentionItems(): TrayAttentionItem[] {
  const chatItems = sortedConversations.value
    .map((conversation) => ({
      id: conversation.id,
      kind: "chat",
      title: trayConversationTitle(conversation.title, conversation.kind),
      count: unreadByConversation.value[conversation.id] ?? 0,
    }))
    .filter((item) => item.count > 0);
  const deviceId = myDeviceId.value;
  const gameItems: TrayAttentionItem[] = [];
  if (deviceId) {
    for (const state of Object.values(doudizhuRooms.value)) {
      if ((state.phase === "bidding" || state.phase === "playing") && state.turnDeviceId === deviceId) {
        gameItems.push({ id: state.roomId, kind: "game", title: gameRoomTrayTitle(state.roomId), count: 1 });
      }
    }
    for (const state of Object.values(gomokuRooms.value)) {
      if (state.phase !== "playing") continue;
      if (state.turnDeviceId === deviceId || (!!state.pendingUndo && state.pendingUndo.requesterId !== deviceId)) {
        gameItems.push({ id: state.roomId, kind: "game", title: gameRoomTrayTitle(state.roomId), count: 1 });
      }
    }
    for (const state of Object.values(xiangqiRooms.value)) {
      if (state.phase !== "playing") continue;
      if (state.turnDeviceId === deviceId || (!!state.pendingUndo && state.pendingUndo.requesterId !== deviceId)) {
        gameItems.push({ id: state.roomId, kind: "game", title: gameRoomTrayTitle(state.roomId), count: 1 });
      }
    }
  }
  return [...chatItems, ...gameItems].slice(0, 12);
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
  if (target.kind === "game") {
    openGameRoom(target.id);
  } else {
    activeSection.value = "chat";
    await store.selectConversation(target.id);
  }
  await syncTrayAttention();
}
const showGameAttention = computed(() => activeSection.value !== "games" && gameAttentionCount.value > 0);
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
    mentionAll: false,
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
async function installNativeUpdate(force = false) {
  if (nativeUpdateInstalling.value) return;
  nativeUpdateInstalling.value = true;
  nativeUpdateProgress.value = { downloaded: 0, total: 0, phase: "downloading" };
  try {
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
    if (!forceUpdateRequired.value) updateReminderOpen.value = false;
    await api.openUpdateUrl(url);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
async function openReleasePage() {
  const url = updateInfo.value?.downloads.releasePage || updateInfo.value?.releaseUrl;
  if (!url) return;
  try {
    if (!forceUpdateRequired.value) updateReminderOpen.value = false;
    await api.openUpdateUrl(url);
  } catch (err) {
    store.error = stringifyError(err);
  }
}
onMounted(async () => {
  platformInfo.value = await api.getPlatformInfo().catch(() => null);
  appVersionInfo.value = await api.getAppVersionInfo().catch(() => null);
  await store.initialize();
  await restoreSavedSuperAdminSession();
  previewMediaCacheInfo.value = await api.getPreviewMediaCacheInfo().catch(() => null);
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
    unlistenDesktopPetAction = await listen<{ action: string; alert_id?: string | null }>("desktop_pet_action", (event) => {
      if (event.payload.action === "quick_alert") {
        void sendPetQuickAlert(petAlertMode.value);
      } else if (event.payload.action === "open_main_window") {
        void api.showFromTray();
      } else if (event.payload.action === "broadcast_disco_alert") {
        void sendPetQuickAlert("disco");
      } else if (event.payload.action === "stop_visuals") {
        stopPetAlertVisuals();
      } else if (event.payload.action === "feedback_real" || event.payload.action === "feedback_false") {
        const target = alertRecords.value.find((item) => item.alertId === event.payload.alert_id) ?? latestPendingAlert.value;
        if (target) {
          void feedbackPetAlert(target, event.payload.action === "feedback_real" ? "real" : "false");
        }
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
  clearCallSession();
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
  void scrollActiveChatToBottom();
  for (const message of messages) {
    void cacheImagePreview(message);
  }
});
watch(() => activeConversationId.value, () => {
  void scrollActiveChatToBottom();
});
watch(activeSection, (section) => {
  if (section === "chat") {
    void scrollActiveChatToBottom();
  }
});
watch(activeRoomChatMessages, async () => {
  await nextTick();
  if (roomChatPane.value) {
    roomChatPane.value.scrollTop = roomChatPane.value.scrollHeight;
  }
});
watch(selectedTheme, (next) => {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-ui-theme", next);
  }
  void syncDesktopPetRuntime();
});
watch(selectedLanguage, (next) => {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-language", next);
  }
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
watch(activeTurnRemainingSeconds, async (remaining) => {
  const state = activeDdzState.value;
  if (!state || remaining > 0 || autoTurnRunning) return;
  await handleTurnTimeout(state);
});
watch(activeGomokuTurnRemainingSeconds, async (remaining) => {
  const state = activeGomokuState.value;
  if (!state || remaining > 0 || autoTurnRunning) return;
  await handleGomokuTurnTimeout(state);
});
watch(latestGameFrame, (frame) => {
  if (!frame) return;
  processGameFrame(frame);
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
  applyQuickAlertTrustReset(reset);
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
watch(isGameStarted, (started) => {
  if (activeSection.value === "games" && started) {
    listPaneCollapsed.value = true;
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
  [unreadByConversation, conversations, gameRoomsState, doudizhuRooms, gomokuRooms, xiangqiRooms],
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
function readSavedLanguage() {
  if (typeof window === "undefined") return "zh-CN";
  const saved = window.localStorage.getItem("lanchat-language");
  return languageOptions.some((item) => item.key === saved) ? saved! : "zh-CN";
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
function readSavedGameStatsRecords(): GameStatsRecord[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem("lanchat-game-stats-v1");
    return raw ? upsertGameStatsRecords([], JSON.parse(raw) as GameStatsRecord[]) : [];
  } catch {
    return [];
  }
}
function saveGameStatsRecords() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-game-stats-v1", JSON.stringify(gameStatsRecords.value));
  }
}
function readSavedMinesweeperLeaderboardRecords(): MinesweeperLeaderboardRecord[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem("lanchat-minesweeper-leaderboard-v1");
    return raw ? upsertMinesweeperLeaderboardRecords([], JSON.parse(raw) as MinesweeperLeaderboardRecord[]) : [];
  } catch {
    return [];
  }
}
function saveMinesweeperLeaderboardRecords() {
  if (typeof window !== "undefined") {
    window.localStorage.setItem("lanchat-minesweeper-leaderboard-v1", JSON.stringify(minesweeperLeaderboardRecords.value));
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
  if (languageOptions.some((item) => item.key === key)) {
    selectedLanguage.value = String(key);
  }
}

function selectCreateRoomGame(type: GameType) {
  selectedGameType.value = type;
  createRoomGameMenuOpen.value = false;
}
function openBuiltinGame(type: GameType) {
  selectedGameType.value = type;
  activeGameRoomId.value = "";
  selectedCardIds.value = [];
  selectedXiangqiPoint.value = null;
  activeSection.value = "games";
  void broadcastLeaderboardSync();
}
async function selectMinesweeperDifficulty(key: string | number) {
  if (!profile.value || activeGameRoom.value?.gameType !== "minesweeper" || activeMinesweeperState.value?.phase !== "lobby" || !isRoomHost()) return;
  const difficulty = difficultyByKey(String(key));
  await sendRoomAction({
    action: "difficulty",
    playerId: profile.value.device_id,
    width: difficulty.width,
    height: difficulty.height,
    mines: difficulty.mines,
  });
}
async function createGameRoom() {
  if (!profile.value) return;
  const room = createGameRoomShell(
    selectedGameType.value,
    roomNameDraft.value,
    profile.value.device_id,
    profile.value.nickname,
    profile.value.avatar,
  );
  const state = createInitialGameState(room);
  upsertGameRoom(room);
  if (room.gameType === "gomoku") {
    gomokuRooms.value = { ...gomokuRooms.value, [room.roomId]: state as GomokuTableState };
  } else if (room.gameType === "minesweeper") {
    minesweeperRooms.value = { ...minesweeperRooms.value, [room.roomId]: state as MinesweeperTableState };
  } else if (room.gameType === "xiangqi") {
    xiangqiRooms.value = { ...xiangqiRooms.value, [room.roomId]: state as XiangqiTableState };
  } else {
    doudizhuRooms.value = { ...doudizhuRooms.value, [room.roomId]: state as DdzTableState };
  }
  activeGameRoomId.value = room.roomId;
  selectedCardIds.value = [];
  selectedXiangqiPoint.value = null;
  createRoomOpen.value = false;
  activeSection.value = "games";
  await broadcastGameFrame("room_created", { room, state }, room.roomId);
}
function openGameRoom(roomId: string) {
  activeGameRoomId.value = roomId;
  selectedCardIds.value = [];
  selectedXiangqiPoint.value = null;
  activeSection.value = "games";
}
function createInitialGameState(room: GameRoomShell): DdzTableState | GomokuTableState | XiangqiTableState | MinesweeperTableState {
  if (room.gameType === "gomoku") return createInitialGomokuState(room);
  if (room.gameType === "minesweeper") return createInitialMinesweeperState(room);
  if (room.gameType === "xiangqi") return createInitialXiangqiState(room);
  return createInitialDdzState(room);
}
function createInitialDdzState(room: GameRoomShell): DdzTableState {
  return {
    roomId: room.roomId,
    phase: "lobby",
    players: room.players.map((player) => ({ ...player, handCount: 0 })),
    landlordCards: [],
    hands: {},
    bidOrder: [],
    bidIndex: 0,
    bids: {},
    lastPlay: null,
    passCount: 0,
    chatMessages: [],
    logs: [`${room.hostName} 创建了 ${gameDefinitionOf(room.gameType).name} 房间`],
    updatedAt: Date.now(),
  };
}
function createInitialGomokuState(room: GameRoomShell): GomokuTableState {
  return {
    roomId: room.roomId,
    phase: "lobby",
    players: room.players.map((player) => ({ ...player, stone: undefined })),
    board: createGomokuBoard(),
    moves: [],
    winLine: [],
    chatMessages: [],
    logs: [`${room.hostName} 创建了 ${gameDefinitionOf(room.gameType).name} 房间`],
    updatedAt: Date.now(),
  };
}
function createInitialMinesweeperState(room: GameRoomShell): MinesweeperTableState {
  const difficulty = MINESWEEPER_DIFFICULTIES[0];
  return {
    roomId: room.roomId,
    phase: "lobby",
    players: room.players.map((player) => ({ ...player })),
    width: difficulty.width,
    height: difficulty.height,
    mines: difficulty.mines,
    seed: Date.now(),
    boards: {},
    chatMessages: [],
    logs: [`${room.hostName} 创建了 ${gameDefinitionOf(room.gameType).name} 房间`],
    updatedAt: Date.now(),
  };
}
function createInitialXiangqiState(room: GameRoomShell): XiangqiTableState {
  return {
    roomId: room.roomId,
    phase: "lobby",
    players: room.players.map((player) => ({ ...player, side: undefined })),
    board: createXiangqiBoard(),
    moves: [],
    chatMessages: [],
    logs: [`${room.hostName} 创建了 ${gameDefinitionOf(room.gameType).name} 房间`],
    updatedAt: Date.now(),
  };
}
function currentGomokuPlayer(): GomokuSeat | null {
  if (!profile.value) return null;
  return {
    deviceId: profile.value.device_id,
    nickname: profile.value.nickname,
    avatar: profile.value.avatar,
    online: true,
    ready: false,
  };
}
function currentMinesweeperPlayer(): MinesweeperSeat | null {
  if (!profile.value) return null;
  return {
    deviceId: profile.value.device_id,
    nickname: profile.value.nickname,
    avatar: profile.value.avatar,
    online: true,
    ready: false,
  };
}
function currentXiangqiPlayer(): XiangqiSeat | null {
  if (!profile.value) return null;
  return {
    deviceId: profile.value.device_id,
    nickname: profile.value.nickname,
    avatar: profile.value.avatar,
    online: true,
    ready: false,
  };
}
function currentDdzPlayer(): DdzSeat | null {
  if (!profile.value) return null;
  return {
    deviceId: profile.value.device_id,
    nickname: profile.value.nickname,
    avatar: profile.value.avatar,
    online: true,
    ready: false,
    handCount: 0,
  };
}
function otherDdzSeats() {
  const state = activeDdzState.value;
  if (!state) return [];
  return state.players.filter((player) => player.deviceId !== myDeviceId.value);
}
function isRoomHost(room = activeGameRoom.value) {
  return !!room && room.hostDeviceId === myDeviceId.value;
}
function upsertGameRoom(room: GameRoomShell) {
  const next = gameRoomsState.value.filter((item) => item.roomId !== room.roomId);
  next.unshift(room);
  gameRoomsState.value = next.sort((a, b) => b.updatedAt - a.updatedAt);
}
function removeGameRoom(roomId: string) {
  gameRoomsState.value = gameRoomsState.value.filter((room) => room.roomId !== roomId);
  const { [roomId]: _removed, ...rest } = doudizhuRooms.value;
  doudizhuRooms.value = rest;
  const { [roomId]: _removedGomoku, ...gomokuRest } = gomokuRooms.value;
  gomokuRooms.value = gomokuRest;
  const { [roomId]: _removedXiangqi, ...xiangqiRest } = xiangqiRooms.value;
  xiangqiRooms.value = xiangqiRest;
  const { [roomId]: _removedMinesweeper, ...minesweeperRest } = minesweeperRooms.value;
  minesweeperRooms.value = minesweeperRest;
  if (activeGameRoomId.value === roomId) {
    activeGameRoomId.value = gameRoomsState.value[0]?.roomId ?? "";
    selectedCardIds.value = [];
    selectedXiangqiPoint.value = null;
    roomChatDraft.value = "";
  }
}
function updateRoomFromState(roomId: string, state: { players: Array<{ deviceId: string; nickname: string; avatar?: string | null; online: boolean; ready: boolean }>; updatedAt: number }) {
  const room = gameRoomsState.value.find((item) => item.roomId === roomId);
  if (!room) return;
  const updated: GameRoomShell = {
    ...room,
    players: state.players.map((player) => ({
      deviceId: player.deviceId,
      nickname: player.nickname,
      avatar: player.avatar,
      online: player.online,
      ready: player.ready,
    })),
    updatedAt: state.updatedAt,
  };
  upsertGameRoom(updated);
}
function makeGameFrame(kind: string, payload: unknown, roomId = activeGameRoomId.value, game: GameType = activeGameRoom.value?.gameType ?? selectedGameType.value): GameFrame {
  return {
    frame_id: `game-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    game,
    room_id: roomId,
    sender_device_id: profile.value?.device_id ?? "",
    sender_nickname: profile.value?.nickname ?? "局域网用户",
    kind,
    payload,
    created_at: Date.now(),
  };
}
async function broadcastGameFrame(kind: string, payload: unknown, roomId = activeGameRoomId.value) {
  await store.sendGameFrame(null, makeGameFrame(kind, payload, roomId));
}
function ownLeaderboardSyncPayload(): LeaderboardSyncPayload {
  const deviceId = myDeviceId.value;
  if (!deviceId) return {};
  return {
    gameStatsRecords: gameStatsRecords.value.filter((record) => record.deviceId === deviceId),
    minesweeperLeaderboardRecords: minesweeperLeaderboardRecords.value.filter((record) => record.deviceId === deviceId),
  };
}
async function broadcastLeaderboardSync() {
  const payload = ownLeaderboardSyncPayload();
  if ((payload.gameStatsRecords?.length ?? 0) + (payload.minesweeperLeaderboardRecords?.length ?? 0) === 0) return;
  await store.sendGameFrame(null, makeGameFrame("leaderboard_sync", payload, "leaderboard", "doudizhu"));
}
function applyLeaderboardSync(payload: LeaderboardSyncPayload) {
  let changed = false;
  if (payload.gameStatsRecords?.length) {
    const next = upsertGameStatsRecords(gameStatsRecords.value, payload.gameStatsRecords);
    changed = changed || JSON.stringify(next) !== JSON.stringify(gameStatsRecords.value);
    gameStatsRecords.value = next;
    saveGameStatsRecords();
  }
  if (payload.minesweeperLeaderboardRecords?.length) {
    const next = upsertMinesweeperLeaderboardRecords(minesweeperLeaderboardRecords.value, payload.minesweeperLeaderboardRecords);
    changed = changed || JSON.stringify(next) !== JSON.stringify(minesweeperLeaderboardRecords.value);
    minesweeperLeaderboardRecords.value = next;
    saveMinesweeperLeaderboardRecords();
  }
  if (changed) {
    void store.addSystemNotice(DEFAULT_GROUP_ID, `${payload.gameStatsRecords?.[0]?.nickname ?? payload.minesweeperLeaderboardRecords?.[0]?.nickname ?? "局域网玩家"} 同步了游戏排行榜`);
  }
}
async function sendRoomAction(action: GameActionPayload) {
  const room = activeGameRoom.value;
  if (!room) return;
  if (isRoomHost(room)) {
    const changed = applyRoomAction(room.roomId, action);
    if (changed) await broadcastSnapshot(room.roomId);
    return;
  }
  await store.sendGameFrame(room.hostDeviceId, makeGameFrame("room_action", { roomId: room.roomId, action }, room.roomId, room.gameType));
}
async function broadcastSnapshot(roomId: string) {
  const room = gameRoomsState.value.find((item) => item.roomId === roomId);
  const state = roomStateForSnapshot(roomId);
  if (!room || !state) return;
  maybeRecordGameResult(room, state);
  await broadcastGameFrame("room_snapshot", { room, state }, roomId);
}
function roomStateForSnapshot(roomId: string) {
  const room = gameRoomsState.value.find((item) => item.roomId === roomId);
  if (!room) return null;
  if (room.gameType === "gomoku") return gomokuRooms.value[roomId] ?? null;
  if (room.gameType === "xiangqi") return xiangqiRooms.value[roomId] ?? null;
  if (room.gameType === "minesweeper") return minesweeperRooms.value[roomId] ?? null;
  return doudizhuRooms.value[roomId] ?? null;
}
function applyRoomAction(roomId: string, action: GameActionPayload) {
  const room = gameRoomsState.value.find((item) => item.roomId === roomId);
  if (!room) return false;
  if (room.gameType === "gomoku") return applyGomokuAction(roomId, action as GomokuActionPayload);
  if (room.gameType === "xiangqi") return applyXiangqiAction(roomId, action as XiangqiActionPayload);
  if (room.gameType === "minesweeper") return applyMinesweeperAction(roomId, action as MinesweeperActionPayload);
  return applyDdzAction(roomId, action as DdzActionPayload);
}
function applyDdzAction(roomId: string, action: DdzActionPayload) {
  const current = doudizhuRooms.value[roomId];
  if (!current) return false;
  const state: DdzTableState = cloneDdzState(current);
  if (action.action === "join") {
    if (state.players.length >= 3 || state.players.some((player) => player.deviceId === action.player.deviceId)) return false;
    state.players.push(action.player);
    state.logs.push(`${action.player.nickname} 加入房间`);
  }
  if (action.action === "ready") {
    state.players = state.players.map((player) => player.deviceId === action.playerId ? { ...player, ready: action.ready } : player);
    const player = state.players.find((item) => item.deviceId === action.playerId);
    state.logs.push(`${player?.nickname ?? "玩家"}${action.ready ? "已准备" : "取消准备"}`);
  }
  if (action.action === "bid") {
    applyBidAction(state, action.playerId, action.call);
  }
  if (action.action === "play") {
    const ok = applyPlayAction(state, action.playerId, action.cardIds);
    if (!ok) return false;
  }
  if (action.action === "pass") {
    const ok = applyPassAction(state, action.playerId);
    if (!ok) return false;
  }
  if (action.action === "leave") {
    const leaving = state.players.find((player) => player.deviceId === action.playerId);
    state.players = state.players.filter((player) => player.deviceId !== action.playerId);
    delete state.hands[action.playerId];
    state.logs.push(`${leaving?.nickname ?? "玩家"} 退出房间`);
    if (state.phase === "bidding" || state.phase === "playing") {
      state.phase = "ended";
      state.turnDeviceId = undefined;
      state.turnStartedAt = undefined;
      state.winnerName = "房间人数不足，本局结束";
    }
  }
  if (action.action === "chat") {
    state.chatMessages.push(action.message);
  }
  state.updatedAt = Date.now();
  maybeAutoStartDdz(state);
  doudizhuRooms.value = { ...doudizhuRooms.value, [roomId]: state };
  updateRoomFromState(roomId, state);
  return true;
}
function cloneDdzState(state: DdzTableState): DdzTableState {
  return {
    ...state,
    players: state.players.map((player) => ({ ...player })),
    landlordCards: [...state.landlordCards],
    hands: Object.fromEntries(Object.entries(state.hands).map(([id, cards]) => [id, [...cards]])),
    bids: { ...state.bids },
    lastPlay: state.lastPlay ? { ...state.lastPlay, cards: [...state.lastPlay.cards] } : null,
    chatMessages: state.chatMessages.map((item) => ({ ...item })),
    logs: [...state.logs],
  };
}
function applyMinesweeperAction(roomId: string, action: MinesweeperActionPayload) {
  const current = minesweeperRooms.value[roomId];
  if (!current) return false;
  const state = cloneMinesweeperState(current);
  if (action.action === "join") {
    if (state.phase !== "lobby" || state.players.length >= 6 || state.players.some((player) => player.deviceId === action.player.deviceId)) return false;
    state.players.push({ ...action.player });
    state.logs.push(`${action.player.nickname} 加入房间`);
  }
  if (action.action === "ready") {
    state.players = state.players.map((player) => player.deviceId === action.playerId ? { ...player, ready: action.ready } : player);
    const player = state.players.find((item) => item.deviceId === action.playerId);
    state.logs.push(`${player?.nickname ?? "玩家"}${action.ready ? "已准备" : "取消准备"}`);
  }
  if (action.action === "difficulty") {
    const room = gameRoomsState.value.find((item) => item.roomId === roomId);
    const allowed = state.phase === "lobby" && room?.hostDeviceId === action.playerId;
    if (!allowed) return false;
    state.width = action.width;
    state.height = action.height;
    state.mines = action.mines;
    state.seed = Date.now();
    state.boards = {};
    state.players = state.players.map((player) => ({ ...player, ready: false }));
    state.logs.push(`难度切换为 ${minesweeperDifficultyLabel(action.width, action.height, action.mines)}，${action.mines} 雷`);
  }
  if (action.action === "reveal") {
    const ok = applyMinesweeperBoardAction(state, action.playerId, "reveal", { x: action.x, y: action.y });
    if (!ok) return false;
  }
  if (action.action === "flag") {
    const ok = applyMinesweeperBoardAction(state, action.playerId, "flag", { x: action.x, y: action.y });
    if (!ok) return false;
  }
  if (action.action === "chord") {
    const ok = applyMinesweeperBoardAction(state, action.playerId, "chord", { x: action.x, y: action.y });
    if (!ok) return false;
  }
  if (action.action === "leave") {
    const leaving = state.players.find((player) => player.deviceId === action.playerId);
    state.players = state.players.filter((player) => player.deviceId !== action.playerId);
    delete state.boards[action.playerId];
    state.logs.push(`${leaving?.nickname ?? "玩家"} 退出房间`);
    if (state.phase === "playing" && !state.winnerDeviceId && state.players.filter((player) => state.boards[player.deviceId]?.status === "playing").length <= 1) {
      const survivor = state.players.find((player) => state.boards[player.deviceId]?.status === "playing");
      if (survivor) finishMinesweeperWithWinner(state, survivor.deviceId);
    }
  }
  if (action.action === "chat") {
    state.chatMessages.push(action.message);
  }
  state.updatedAt = Date.now();
  maybeAutoStartMinesweeper(state);
  minesweeperRooms.value = { ...minesweeperRooms.value, [roomId]: state };
  updateRoomFromState(roomId, state);
  return true;
}
function cloneMinesweeperState(state: MinesweeperTableState): MinesweeperTableState {
  return {
    ...state,
    players: state.players.map((player) => ({ ...player })),
    boards: Object.fromEntries(Object.entries(state.boards).map(([id, boardState]) => [id, {
      ...boardState,
      board: cloneMinesweeperBoard(boardState.board),
    }])),
    chatMessages: state.chatMessages.map((item) => ({ ...item })),
    logs: [...state.logs],
  };
}
function maybeAutoStartMinesweeper(state: MinesweeperTableState) {
  if (state.phase !== "lobby" || state.players.length < 1 || !state.players.every((player) => player.ready)) return;
  const seed = Date.now();
  state.seed = seed;
  state.phase = "playing";
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.boards = Object.fromEntries(state.players.map((player) => [player.deviceId, createMinesweeperPlayerState(state, seed)]));
  state.logs.push(`扫雷竞速开始：${state.width}x${state.height}，${state.mines} 颗雷`);
}
function createMinesweeperPlayerState(state: MinesweeperTableState, seed = state.seed): MinesweeperPlayerState {
  const board = createMinesweeperBoard({ width: state.width, height: state.height, mines: state.mines, seed });
  const progress = getMinesweeperProgress(board);
  return {
    board,
    status: "playing",
    moves: 0,
    startedAt: Date.now(),
    revealedSafe: progress.revealedSafe,
    totalSafe: progress.totalSafe,
    flagged: progress.flagged,
  };
}
function applyMinesweeperBoardAction(state: MinesweeperTableState, playerId: string, action: "reveal" | "flag" | "chord", point: MinesweeperPoint) {
  if (state.phase !== "playing") return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  const boardState = state.boards[playerId];
  if (!player || !boardState || boardState.status !== "playing") return false;
  const result = action === "reveal"
    ? revealMinesweeperCell(boardState.board, point)
    : action === "flag"
      ? toggleMinesweeperFlag(boardState.board, point)
      : chordRevealMinesweeperCell(boardState.board, point);
  if (!result.ok || !result.changed) return false;
  const progress = getMinesweeperProgress(result.board);
  state.boards[playerId] = {
    ...boardState,
    board: result.board,
    moves: boardState.moves + 1,
    status: result.lost ? "lost" : result.won ? "won" : "playing",
    finishedAt: result.lost || result.won ? Date.now() : boardState.finishedAt,
    revealedSafe: progress.revealedSafe,
    totalSafe: progress.totalSafe,
    flagged: progress.flagged,
  };
  if (result.lost) {
    state.logs.push(`${player.nickname} 踩雷出局`);
    maybeFinishMinesweeperBySurvivor(state);
    return true;
  }
  if (result.won) {
    finishMinesweeperWithWinner(state, playerId);
    return true;
  }
  return true;
}
function maybeFinishMinesweeperBySurvivor(state: MinesweeperTableState) {
  const playing = state.players.filter((player) => state.boards[player.deviceId]?.status === "playing");
  if (playing.length === 1) finishMinesweeperWithWinner(state, playing[0]!.deviceId);
  if (playing.length === 0 && !state.winnerDeviceId) {
    state.phase = "ended";
    state.winnerName = "全部失败，本局结束";
  }
}
function finishMinesweeperWithWinner(state: MinesweeperTableState, winnerId: string) {
  const winner = state.players.find((player) => player.deviceId === winnerId);
  if (!winner) return;
  const boardState = state.boards[winnerId];
  if (boardState) {
    state.boards[winnerId] = { ...boardState, status: "won", finishedAt: boardState.finishedAt ?? Date.now() };
  }
  state.phase = "ended";
  state.winnerDeviceId = winnerId;
  state.winnerName = winner.nickname;
  state.logs.push(`${winner.nickname} 完成扫雷，获得胜利`);
}
function applyGomokuAction(roomId: string, action: GomokuActionPayload) {
  const current = gomokuRooms.value[roomId];
  if (!current) return false;
  const state = cloneGomokuState(current);
  if (action.action === "join") {
    if (state.phase !== "lobby" || state.players.length >= 2 || state.players.some((player) => player.deviceId === action.player.deviceId)) return false;
    state.players.push({ ...action.player, stone: undefined });
    state.logs.push(`${action.player.nickname} 加入房间`);
  }
  if (action.action === "ready") {
    state.players = state.players.map((player) => player.deviceId === action.playerId ? { ...player, ready: action.ready } : player);
    const player = state.players.find((item) => item.deviceId === action.playerId);
    state.logs.push(`${player?.nickname ?? "玩家"}${action.ready ? "已准备" : "取消准备"}`);
  }
  if (action.action === "move") {
    const ok = applyGomokuMoveAction(state, action.playerId, action.x, action.y);
    if (!ok) return false;
  }
  if (action.action === "undo_request") {
    const ok = applyGomokuUndoRequestAction(state, action.playerId);
    if (!ok) return false;
  }
  if (action.action === "undo_response") {
    const ok = applyGomokuUndoResponseAction(state, action.playerId, action.accepted);
    if (!ok) return false;
  }
  if (action.action === "resign") {
    const ok = applyGomokuResignAction(state, action.playerId);
    if (!ok) return false;
  }
  if (action.action === "leave") {
    const leaving = state.players.find((player) => player.deviceId === action.playerId);
    state.players = state.players.filter((player) => player.deviceId !== action.playerId);
    state.logs.push(`${leaving?.nickname ?? "玩家"} 退出房间`);
    if (state.phase === "playing") {
      state.phase = "ended";
      state.turnDeviceId = undefined;
      state.turnStartedAt = undefined;
      state.winnerName = "对方退出，本局结束";
    }
  }
  if (action.action === "chat") {
    state.chatMessages.push(action.message);
  }
  state.updatedAt = Date.now();
  maybeAutoStartGomoku(state);
  gomokuRooms.value = { ...gomokuRooms.value, [roomId]: state };
  updateRoomFromState(roomId, state);
  return true;
}
function cloneGomokuState(state: GomokuTableState): GomokuTableState {
  return {
    ...state,
    players: state.players.map((player) => ({ ...player })),
    board: cloneGomokuBoard(state.board),
    moves: state.moves.map((move) => ({ ...move })),
    winLine: state.winLine.map((point) => ({ ...point })),
    pendingUndo: state.pendingUndo ? { ...state.pendingUndo } : undefined,
    chatMessages: state.chatMessages.map((item) => ({ ...item })),
    logs: [...state.logs],
  };
}
function maybeAutoStartGomoku(state: GomokuTableState) {
  if (state.phase !== "lobby" || state.players.length !== 2 || !state.players.every((player) => player.ready)) return;
  state.board = createGomokuBoard();
  state.moves = [];
  state.winLine = [];
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.winnerStone = undefined;
  state.pendingUndo = undefined;
  state.players = state.players.map((player, index) => ({ ...player, stone: index === 0 ? "black" : "white" }));
  state.phase = "playing";
  setGomokuTurn(state, state.players[0]?.deviceId);
  state.logs.push(`${state.players[0]?.nickname ?? "玩家"} 执黑先行`);
}
function applyGomokuMoveAction(state: GomokuTableState, playerId: string, x: number, y: number) {
  if (state.phase !== "playing" || state.turnDeviceId !== playerId || state.pendingUndo) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player?.stone) return false;
  const result = placeGomokuStone(state.board, { x, y }, player.stone);
  if (!result.ok) return false;
  state.board = result.board;
  state.moves.push({ x, y, playerId, playerName: player.nickname, stone: player.stone, createdAt: Date.now() });
  state.logs.push(`${player.nickname} 落 ${gomokuStoneLabel(player.stone)} (${x + 1}, ${y + 1})`);
  if (result.winner) {
    state.phase = "ended";
    state.turnDeviceId = undefined;
    state.turnStartedAt = undefined;
    state.winnerDeviceId = playerId;
    state.winnerName = player.nickname;
    state.winnerStone = result.winner;
    state.winLine = result.winLine ?? [];
    state.logs.push(`${player.nickname} 五连获胜`);
    return true;
  }
  if (result.draw) {
    state.phase = "ended";
    state.turnDeviceId = undefined;
    state.turnStartedAt = undefined;
    state.winnerName = undefined;
    state.winLine = [];
    state.logs.push("棋盘已满，本局平局");
    return true;
  }
  setGomokuTurn(state, nextGomokuPlayerId(state, playerId));
  return true;
}
function applyGomokuUndoRequestAction(state: GomokuTableState, playerId: string) {
  if (state.phase !== "playing" || state.pendingUndo || state.moves.length === 0) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player) return false;
  state.pendingUndo = { requesterId: playerId, requesterName: player.nickname, createdAt: Date.now() };
  state.logs.push(`${player.nickname} 请求悔棋`);
  return true;
}
function applyGomokuUndoResponseAction(state: GomokuTableState, playerId: string, accepted: boolean) {
  if (state.phase !== "playing" || !state.pendingUndo || state.pendingUndo.requesterId === playerId) return false;
  const responder = state.players.find((item) => item.deviceId === playerId);
  const requesterName = state.pendingUndo.requesterName;
  const lastMove = state.moves[state.moves.length - 1];
  state.pendingUndo = undefined;
  if (!accepted) {
    state.logs.push(`${responder?.nickname ?? "玩家"} 拒绝 ${requesterName} 的悔棋请求`);
    return true;
  }
  if (!lastMove) return false;
  state.moves = state.moves.slice(0, -1);
  let board = createGomokuBoard();
  let winLine: GomokuPoint[] = [];
  let winner: GomokuStone | null = null;
  for (const move of state.moves) {
    const placed = placeGomokuStone(board, { x: move.x, y: move.y }, move.stone);
    if (placed.ok) {
      board = placed.board;
      winner = placed.winner ?? null;
      winLine = placed.winLine ?? [];
    }
  }
  state.board = board;
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.winnerStone = winner ?? undefined;
  state.winLine = winLine;
  setGomokuTurn(state, lastMove.playerId);
  state.logs.push(`${responder?.nickname ?? "玩家"} 同意悔棋，撤回 ${lastMove.playerName} 的落子`);
  return true;
}
function applyGomokuResignAction(state: GomokuTableState, playerId: string) {
  if (state.phase !== "playing") return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  const winner = state.players.find((item) => item.deviceId !== playerId);
  if (!player || !winner) return false;
  state.phase = "ended";
  state.turnDeviceId = undefined;
  state.turnStartedAt = undefined;
  state.pendingUndo = undefined;
  state.winnerDeviceId = winner.deviceId;
  state.winnerName = winner.nickname;
  state.winnerStone = winner.stone;
  state.logs.push(`${player.nickname} 投降，${winner.nickname} 获胜`);
  return true;
}
function setGomokuTurn(state: GomokuTableState, playerId?: string) {
  state.turnDeviceId = playerId;
  state.turnStartedAt = playerId ? Date.now() : undefined;
}
function nextGomokuPlayerId(state: GomokuTableState, currentId: string) {
  const index = state.players.findIndex((player) => player.deviceId === currentId);
  return state.players[(index + 1) % state.players.length]?.deviceId;
}
function gomokuPointStyle(x: number, y: number) {
  const size = Math.max((activeGomokuState.value?.board.length ?? 15) - 1, 1);
  return {
    "--gx": String(x / size),
    "--gy": String(y / size),
  } as Record<string, string>;
}
function canPlaceGomokuCell(x: number, y: number) {
  const state = activeGomokuState.value;
  return !!state && state.phase === "playing" && !state.pendingUndo && isMyGomokuTurn.value && !state.board[y]?.[x];
}
function isGomokuWinPoint(x: number, y: number) {
  return gomokuWinPointKeys.value.has(`${x}:${y}`);
}
async function placeGomokuCell(x: number, y: number) {
  if (!profile.value || !canPlaceGomokuCell(x, y)) return;
  await sendRoomAction({ action: "move", playerId: profile.value.device_id, x, y });
}
function applyXiangqiAction(roomId: string, action: XiangqiActionPayload) {
  const current = xiangqiRooms.value[roomId];
  if (!current) return false;
  const state = cloneXiangqiState(current);
  if (action.action === "join") {
    if (state.phase !== "lobby" || state.players.length >= 2 || state.players.some((player) => player.deviceId === action.player.deviceId)) return false;
    state.players.push({ ...action.player, side: undefined });
    state.logs.push(`${action.player.nickname} 加入房间`);
  }
  if (action.action === "ready") {
    state.players = state.players.map((player) => player.deviceId === action.playerId ? { ...player, ready: action.ready } : player);
    const player = state.players.find((item) => item.deviceId === action.playerId);
    state.logs.push(`${player?.nickname ?? "玩家"}${action.ready ? "已准备" : "取消准备"}`);
  }
  if (action.action === "move") {
    const ok = applyXiangqiMoveAction(state, action.playerId, action.from, action.to);
    if (!ok) return false;
  }
  if (action.action === "undo_request") {
    const ok = applyXiangqiUndoRequestAction(state, action.playerId);
    if (!ok) return false;
  }
  if (action.action === "undo_response") {
    const ok = applyXiangqiUndoResponseAction(state, action.playerId, action.accepted);
    if (!ok) return false;
  }
  if (action.action === "resign") {
    const ok = applyXiangqiResignAction(state, action.playerId);
    if (!ok) return false;
  }
  if (action.action === "leave") {
    const leaving = state.players.find((player) => player.deviceId === action.playerId);
    state.players = state.players.filter((player) => player.deviceId !== action.playerId);
    state.logs.push(`${leaving?.nickname ?? "玩家"} 退出房间`);
    if (state.phase === "playing") {
      state.phase = "ended";
      state.turnDeviceId = undefined;
      state.turnStartedAt = undefined;
      state.winnerName = "对方退出，本局结束";
    }
  }
  if (action.action === "chat") {
    state.chatMessages.push(action.message);
  }
  state.updatedAt = Date.now();
  maybeAutoStartXiangqi(state);
  xiangqiRooms.value = { ...xiangqiRooms.value, [roomId]: state };
  updateRoomFromState(roomId, state);
  return true;
}
function cloneXiangqiState(state: XiangqiTableState): XiangqiTableState {
  return {
    ...state,
    players: state.players.map((player) => ({ ...player })),
    board: cloneXiangqiBoard(state.board),
    moves: state.moves.map((move) => ({ ...move, from: { ...move.from }, to: { ...move.to }, piece: move.piece ? { ...move.piece } : undefined, captured: move.captured ? { ...move.captured } : move.captured, previousCheckSide: move.previousCheckSide })),
    pendingUndo: state.pendingUndo ? { ...state.pendingUndo } : undefined,
    chatMessages: state.chatMessages.map((item) => ({ ...item })),
    logs: [...state.logs],
  };
}
function maybeAutoStartXiangqi(state: XiangqiTableState) {
  if (state.phase !== "lobby" || state.players.length !== 2 || !state.players.every((player) => player.ready)) return;
  state.board = createXiangqiBoard();
  state.moves = [];
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.winnerSide = undefined;
  state.checkSide = undefined;
  state.pendingUndo = undefined;
  state.players = state.players.map((player, index) => ({ ...player, side: index === 0 ? "red" : "black" }));
  state.phase = "playing";
  setXiangqiTurn(state, state.players.find((player) => player.side === "red")?.deviceId);
  state.logs.push(`${state.players.find((player) => player.side === "red")?.nickname ?? "玩家"} 执红先行`);
}
function applyXiangqiMoveAction(state: XiangqiTableState, playerId: string, from: XiangqiPoint, to: XiangqiPoint) {
  if (state.phase !== "playing" || state.turnDeviceId !== playerId || state.pendingUndo) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player?.side) return false;
  const movingPiece = state.board[from.y]?.[from.x] ?? null;
  if (!movingPiece || movingPiece.side !== player.side) return false;
  const capturedPiece = state.board[to.y]?.[to.x] ?? null;
  const result = moveXiangqiPiece(state.board, from, to, player.side);
  if (!result.ok) return false;
  state.board = result.board;
  const capturedLabel = result.captured ? xiangqiPieceLabel(result.captured) : undefined;
  state.moves.push({
    from: { ...from },
    to: { ...to },
    playerId,
    playerName: player.nickname,
    side: player.side,
    piece: { ...movingPiece },
    captured: capturedPiece ? { ...capturedPiece } : null,
    previousCheckSide: state.checkSide,
    pieceLabel: xiangqiPieceLabel(movingPiece),
    capturedLabel,
    createdAt: Date.now(),
  });
  state.logs.push(`${player.nickname} ${xiangqiPieceLabel(movingPiece)} ${from.x + 1},${from.y + 1} → ${to.x + 1},${to.y + 1}${capturedLabel ? `，吃 ${capturedLabel}` : ""}`);
  if (result.winner) {
    state.phase = "ended";
    state.turnDeviceId = undefined;
    state.turnStartedAt = undefined;
    state.winnerDeviceId = playerId;
    state.winnerName = player.nickname;
    state.winnerSide = result.winner;
    state.checkSide = undefined;
    state.logs.push(`${player.nickname} 获胜`);
    return true;
  }
  state.checkSide = result.check ? otherXiangqiSide(player.side) : undefined;
  setXiangqiTurn(state, nextXiangqiPlayerId(state, playerId));
  return true;
}
function applyXiangqiUndoAction(state: XiangqiTableState, playerId: string) {
  if (state.phase !== "playing" || !state.players.some((player) => player.deviceId === playerId)) return false;
  const lastMove = state.moves[state.moves.length - 1];
  if (!lastMove?.piece) return false;
  state.board = undoXiangqiMove(state.board, {
    from: lastMove.from,
    to: lastMove.to,
    piece: lastMove.piece,
    captured: lastMove.captured ?? null,
  });
  state.moves = state.moves.slice(0, -1);
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.winnerSide = undefined;
  state.checkSide = lastMove.previousCheckSide;
  setXiangqiTurn(state, lastMove.playerId);
  const operator = state.players.find((player) => player.deviceId === playerId)?.nickname ?? "玩家";
  state.pendingUndo = undefined;
  state.logs.push(`${operator} 悔棋，撤回 ${lastMove.playerName} 的 ${lastMove.pieceLabel}`);
  return true;
}
function applyXiangqiUndoRequestAction(state: XiangqiTableState, playerId: string) {
  if (state.phase !== "playing" || state.pendingUndo || state.moves.length === 0) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player) return false;
  state.pendingUndo = { requesterId: playerId, requesterName: player.nickname, createdAt: Date.now() };
  state.logs.push(`${player.nickname} 请求悔棋`);
  return true;
}
function applyXiangqiUndoResponseAction(state: XiangqiTableState, playerId: string, accepted: boolean) {
  if (state.phase !== "playing" || !state.pendingUndo || state.pendingUndo.requesterId === playerId) return false;
  const responder = state.players.find((item) => item.deviceId === playerId);
  const requesterId = state.pendingUndo.requesterId;
  const requesterName = state.pendingUndo.requesterName;
  if (!accepted) {
    state.pendingUndo = undefined;
    state.logs.push(`${responder?.nickname ?? "玩家"} 拒绝 ${requesterName} 的悔棋请求`);
    return true;
  }
  const ok = applyXiangqiUndoAction(state, requesterId);
  if (ok) state.logs.push(`${responder?.nickname ?? "玩家"} 同意 ${requesterName} 的悔棋请求`);
  return ok;
}
function applyXiangqiResignAction(state: XiangqiTableState, playerId: string) {
  if (state.phase !== "playing") return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player?.side) return false;
  const winnerSide = resignXiangqiSide(player.side);
  const winner = state.players.find((item) => item.side === winnerSide);
  state.phase = "ended";
  state.turnDeviceId = undefined;
  state.turnStartedAt = undefined;
  state.winnerDeviceId = winner?.deviceId;
  state.winnerName = winner?.nickname ?? xiangqiSideLabel(winnerSide);
  state.winnerSide = winnerSide;
  state.checkSide = undefined;
  state.pendingUndo = undefined;
  state.logs.push(`${player.nickname} 投降，${state.winnerName} 获胜`);
  return true;
}
function setXiangqiTurn(state: XiangqiTableState, playerId?: string) {
  state.turnDeviceId = playerId;
  state.turnStartedAt = playerId ? Date.now() : undefined;
}
function nextXiangqiPlayerId(state: XiangqiTableState, currentId: string) {
  const index = state.players.findIndex((player) => player.deviceId === currentId);
  return state.players[(index + 1) % state.players.length]?.deviceId;
}
function xiangqiSideShortLabel(side: XiangqiSide) {
  return side === "black" ? "黑" : "红";
}

function xiangqiSeatName(seat: XiangqiSeat | null, side: XiangqiSide) {
  if (!seat) return `等待${xiangqiSideShortLabel(side)}方`;
  return seat.deviceId === myDeviceId.value ? `我 · ${seat.nickname}` : seat.nickname;
}

function xiangqiSeatStatus(seat: XiangqiSeat | null, side: XiangqiSide) {
  if (seat?.ready) return "已准备";
  if (activeXiangqiState.value?.phase === "playing") return `执${xiangqiSideShortLabel(side)}`;
  return "未准备";
}

function isSelectedXiangqiCell(x: number, y: number) {
  return selectedXiangqiPoint.value?.x === x && selectedXiangqiPoint.value.y === y;
}
function canSelectXiangqiCell(x: number, y: number) {
  const state = activeXiangqiState.value;
  const piece = state?.board[y]?.[x];
  return !!state && state.phase === "playing" && !state.pendingUndo && isMyXiangqiTurn.value && !!myXiangqiSeat.value?.side && piece?.side === myXiangqiSeat.value.side;
}
function canMoveSelectedXiangqiTo(x: number, y: number) {
  const state = activeXiangqiState.value;
  const from = selectedXiangqiPoint.value;
  const side = myXiangqiSeat.value?.side;
  if (!state || !from || !side || state.phase !== "playing" || state.pendingUndo || !isMyXiangqiTurn.value) return false;
  return isLegalXiangqiMove(state.board, from, { x, y }, side);
}
function isXiangqiCellPlayable(x: number, y: number) {
  return canSelectXiangqiCell(x, y) || canMoveSelectedXiangqiTo(x, y);
}
async function clickXiangqiCell(x: number, y: number) {
  const state = activeXiangqiState.value;
  if (!profile.value || state?.phase !== "playing" || state.pendingUndo || !isMyXiangqiTurn.value) return;
  if (canSelectXiangqiCell(x, y)) {
    selectedXiangqiPoint.value = { x, y };
    return;
  }
  const from = selectedXiangqiPoint.value;
  if (from && canMoveSelectedXiangqiTo(x, y)) {
    selectedXiangqiPoint.value = null;
    await sendRoomAction({ action: "move", playerId: profile.value.device_id, from, to: { x, y } });
    return;
  }
  selectedXiangqiPoint.value = null;
}
function canUseMinesweeperBoard() {
  return activeMinesweeperState.value?.phase === "playing" && myMinesweeperBoardState.value?.status === "playing";
}
function minesweeperCellText(cell: MinesweeperCell) {
  if (cell.flagged && !cell.revealed) return "⚑";
  if (!cell.revealed) return "";
  if (cell.mine) return "✹";
  return cell.adjacent > 0 ? String(cell.adjacent) : "";
}
function minesweeperCellTone(cell: MinesweeperCell) {
  if (!cell.revealed || cell.mine || cell.adjacent === 0) return "";
  return `n${cell.adjacent}`;
}
async function revealMinesweeperAt(x: number, y: number) {
  if (!profile.value || !canUseMinesweeperBoard()) return;
  await sendRoomAction({ action: "reveal", playerId: profile.value.device_id, x, y });
}
async function flagMinesweeperAt(x: number, y: number) {
  if (!profile.value || !canUseMinesweeperBoard()) return;
  await sendRoomAction({ action: "flag", playerId: profile.value.device_id, x, y });
}
async function chordMinesweeperAt(x: number, y: number) {
  if (!profile.value || !canUseMinesweeperBoard()) return;
  await sendRoomAction({ action: "chord", playerId: profile.value.device_id, x, y });
}
function minesweeperElapsedLabel(startedAt?: number, finishedAt?: number) {
  if (!startedAt) return "--";
  const end = finishedAt ?? nowTick.value;
  return `${Math.max(0, Math.floor((end - startedAt) / 1000))}s`;
}
function minesweeperProgressPercent(boardState?: MinesweeperPlayerState | null) {
  if (!boardState?.totalSafe) return 0;
  return Math.round((boardState.revealedSafe / boardState.totalSafe) * 100);
}
function isOpponentLastGomokuCell(x: number, y: number) {
  const move = lastOpponentGomokuMove.value;
  return !!move && move.x === x && move.y === y;
}
function isOpponentLastXiangqiCell(x: number, y: number) {
  const move = lastOpponentXiangqiMove.value;
  return !!move && move.to.x === x && move.to.y === y;
}
async function requestGomokuUndo() {
  if (!profile.value || !canRequestUndoGomoku.value) return;
  await sendRoomAction({ action: "undo_request", playerId: profile.value.device_id });
}
async function respondGomokuUndo(accepted: boolean) {
  if (!profile.value || !canRespondGomokuUndo.value) return;
  await sendRoomAction({ action: "undo_response", playerId: profile.value.device_id, accepted });
}
async function resignGomoku() {
  if (!profile.value || !canResignGomoku.value) return;
  await sendRoomAction({ action: "resign", playerId: profile.value.device_id });
}
async function requestXiangqiUndo() {
  if (!profile.value || !canRequestUndoXiangqi.value) return;
  selectedXiangqiPoint.value = null;
  await sendRoomAction({ action: "undo_request", playerId: profile.value.device_id });
}
async function respondXiangqiUndo(accepted: boolean) {
  if (!profile.value || !canRespondXiangqiUndo.value) return;
  selectedXiangqiPoint.value = null;
  await sendRoomAction({ action: "undo_response", playerId: profile.value.device_id, accepted });
}
async function resignXiangqi() {
  if (!profile.value || !canResignXiangqi.value) return;
  selectedXiangqiPoint.value = null;
  await sendRoomAction({ action: "resign", playerId: profile.value.device_id });
}
function maybeAutoStartDdz(state: DdzTableState) {
  if (state.phase !== "lobby" || state.players.length !== 3 || !state.players.every((player) => player.ready)) return;
  const { hands, landlordCards } = dealHands(state.players);
  state.hands = hands;
  state.landlordCards = landlordCards;
  state.players = state.players.map((player) => ({ ...player, handCount: hands[player.deviceId]?.length ?? 0, role: undefined }));
  state.phase = "bidding";
  state.bidOrder = state.players.map((player) => player.deviceId);
  state.bidIndex = 0;
  state.bids = {};
  setDdzTurn(state, state.bidOrder[0]);
  state.lastPlay = null;
  state.passCount = 0;
  state.winnerDeviceId = undefined;
  state.winnerName = undefined;
  state.logs.push("三人已准备，开始叫地主");
}
function applyBidAction(state: DdzTableState, playerId: string, call: boolean) {
  if (state.phase !== "bidding" || state.turnDeviceId !== playerId) return;
  state.bids[playerId] = call;
  const player = state.players.find((item) => item.deviceId === playerId);
  state.logs.push(`${player?.nickname ?? "玩家"}${call ? "叫地主" : "不叫"}`);
  if (call || state.bidIndex >= state.bidOrder.length - 1) {
    const landlordId = call ? playerId : state.bidOrder[0];
    state.landlordDeviceId = landlordId;
    state.players = state.players.map((item) => ({ ...item, role: item.deviceId === landlordId ? "landlord" : "farmer" }));
    state.hands[landlordId] = sortCards([...(state.hands[landlordId] ?? []), ...state.landlordCards]);
    state.players = state.players.map((item) => ({ ...item, handCount: state.hands[item.deviceId]?.length ?? 0 }));
    state.phase = "playing";
    setDdzTurn(state, landlordId);
    state.logs.push(`${state.players.find((item) => item.deviceId === landlordId)?.nickname ?? "玩家"} 成为地主`);
    return;
  }
  state.bidIndex += 1;
  setDdzTurn(state, state.bidOrder[state.bidIndex]);
}
function applyPlayAction(state: DdzTableState, playerId: string, cardIds: string[]) {
  if (state.phase !== "playing" || state.turnDeviceId !== playerId) return false;
  const hand = state.hands[playerId] ?? [];
  const cards = sortCards(hand.filter((card) => cardIds.includes(card.id)));
  if (cards.length !== cardIds.length) return false;
  const leading = !state.lastPlay || state.lastPlay.playerId === playerId;
  const evaluated = evaluatePlay(cards);
  if (!evaluated || !canBeat(cards, leading ? null : state.lastPlay)) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  state.hands[playerId] = hand.filter((card) => !cardIds.includes(card.id));
  state.lastPlay = { ...evaluated, playerId, playerName: player?.nickname ?? "玩家", cards };
  state.passCount = 0;
  state.players = state.players.map((item) => item.deviceId === playerId ? { ...item, handCount: state.hands[playerId].length } : item);
  state.logs.push(`${player?.nickname ?? "玩家"} 打出 ${playLabel(evaluated)}`);
  if (state.hands[playerId].length === 0) {
    state.phase = "ended";
    state.turnDeviceId = undefined;
    state.turnStartedAt = undefined;
    state.winnerDeviceId = playerId;
    state.winnerName = player?.nickname ?? "玩家";
    state.logs.push(`${state.winnerName} 获胜`);
    return true;
  }
  setDdzTurn(state, nextDdzPlayerId(state, playerId));
  return true;
}
function applyPassAction(state: DdzTableState, playerId: string) {
  if (state.phase !== "playing" || state.turnDeviceId !== playerId || !state.lastPlay || state.lastPlay.playerId === playerId) return false;
  const player = state.players.find((item) => item.deviceId === playerId);
  state.logs.push(`${player?.nickname ?? "玩家"} 不要`);
  state.passCount += 1;
  if (state.passCount >= state.players.length - 1) {
    setDdzTurn(state, state.lastPlay.playerId);
    state.lastPlay = null;
    state.passCount = 0;
  } else {
    setDdzTurn(state, nextDdzPlayerId(state, playerId));
  }
  return true;
}
function setDdzTurn(state: DdzTableState, playerId?: string) {
  state.turnDeviceId = playerId;
  state.turnStartedAt = playerId ? Date.now() : undefined;
}
async function handleTurnTimeout(state: DdzTableState) {
  if (!profile.value || state.turnDeviceId !== profile.value.device_id || !isTurnTimedOut(state.turnStartedAt, nowTick.value, DDZ_TURN_TIMEOUT_MS)) return;
  autoTurnRunning = true;
  try {
    if (state.phase === "bidding") {
      await sendRoomAction({ action: "bid", playerId: profile.value.device_id, call: false });
      return;
    }
    if (state.phase === "playing") {
      if (canPassDdz.value) {
        selectedCardIds.value = [];
        await sendRoomAction({ action: "pass", playerId: profile.value.device_id });
        return;
      }
      const fallbackCard = myDdzHand.value[0];
      if (fallbackCard) {
        selectedCardIds.value = [];
        await sendRoomAction({ action: "play", playerId: profile.value.device_id, cardIds: [fallbackCard.id] });
      }
    }
  } finally {
    autoTurnRunning = false;
  }
}
async function handleGomokuTurnTimeout(state: GomokuTableState) {
  if (!profile.value || state.pendingUndo || state.turnDeviceId !== profile.value.device_id || !isGomokuTurnTimedOut(state.turnStartedAt, nowTick.value, GOMOKU_TURN_TIMEOUT_MS)) return;
  const point = chooseAutoGomokuPoint(state.board);
  if (!point) return;
  autoTurnRunning = true;
  try {
    await sendRoomAction({ action: "move", playerId: profile.value.device_id, x: point.x, y: point.y });
  } finally {
    autoTurnRunning = false;
  }
}
function seatTurnLabel(seat: DdzSeat) {
  const state = activeDdzState.value;
  if (!state || state.turnDeviceId !== seat.deviceId || (state.phase !== "bidding" && state.phase !== "playing")) return "";
  return `${activeTurnRemainingSeconds.value}s`;
}
function gomokuSeatTurnLabel(seat: GomokuSeat | null) {
  const state = activeGomokuState.value;
  if (!seat || !state || state.turnDeviceId !== seat.deviceId || state.phase !== "playing") return "";
  return `${activeGomokuTurnRemainingSeconds.value}s`;
}
function nextDdzPlayerId(state: DdzTableState, currentId: string) {
  const index = state.players.findIndex((player) => player.deviceId === currentId);
  return state.players[(index + 1) % state.players.length]?.deviceId;
}
async function dissolveRoom() {
  const room = activeGameRoom.value;
  if (!room || !isRoomHost(room)) return;
  const frame = makeGameFrame("room_dissolved", { roomId: room.roomId }, room.roomId, room.gameType);
  removeGameRoom(room.roomId);
  await store.sendGameFrame(null, frame);
}
async function leaveRoom() {
  const room = activeGameRoom.value;
  if (!room || !profile.value) return;
  if (isRoomHost(room)) {
    await dissolveRoom();
    return;
  }
  const action: GameActionPayload = { action: "leave", playerId: profile.value.device_id };
  const frame = makeGameFrame("room_action", { roomId: room.roomId, action }, room.roomId, room.gameType);
  removeGameRoom(room.roomId);
  await store.sendGameFrame(room.hostDeviceId, frame);
}
async function roomPrimaryAction() {
  const room = activeGameRoom.value;
  if (!room) return;
  if (room.gameType === "gomoku") {
    const player = currentGomokuPlayer();
    if (!activeGomokuState.value || !player) return;
    if (!myGomokuSeat.value) {
      await sendRoomAction({ action: "join", player });
      return;
    }
    if (activeGomokuState.value.phase === "lobby") {
      await sendRoomAction({ action: "ready", playerId: player.deviceId, ready: !myGomokuSeat.value.ready });
      return;
    }
    if (activeGomokuState.value.phase === "ended" && isRoomHost(room)) {
      const reset = createInitialGomokuState({ ...room, players: room.players.map((item) => ({ ...item, ready: false })) });
      gomokuRooms.value = { ...gomokuRooms.value, [room.roomId]: reset };
      updateRoomFromState(room.roomId, reset);
      await broadcastSnapshot(room.roomId);
    }
    return;
  }
  if (room.gameType === "minesweeper") {
    const player = currentMinesweeperPlayer();
    if (!activeMinesweeperState.value || !player) return;
    if (!myMinesweeperSeat.value) {
      await sendRoomAction({ action: "join", player });
      return;
    }
    if (activeMinesweeperState.value.phase === "lobby") {
      await sendRoomAction({ action: "ready", playerId: player.deviceId, ready: !myMinesweeperSeat.value.ready });
      return;
    }
    if (activeMinesweeperState.value.phase === "ended" && isRoomHost(room)) {
      const reset = createInitialMinesweeperState({ ...room, players: room.players.map((item) => ({ ...item, ready: false })) });
      minesweeperRooms.value = { ...minesweeperRooms.value, [room.roomId]: reset };
      updateRoomFromState(room.roomId, reset);
      await broadcastSnapshot(room.roomId);
    }
    return;
  }  if (room.gameType === "xiangqi") {
    const player = currentXiangqiPlayer();
    if (!activeXiangqiState.value || !player) return;
    if (!myXiangqiSeat.value) {
      await sendRoomAction({ action: "join", player });
      return;
    }
    if (activeXiangqiState.value.phase === "lobby") {
      await sendRoomAction({ action: "ready", playerId: player.deviceId, ready: !myXiangqiSeat.value.ready });
      return;
    }
    if (activeXiangqiState.value.phase === "ended" && isRoomHost(room)) {
      const reset = createInitialXiangqiState({ ...room, players: room.players.map((item) => ({ ...item, ready: false })) });
      xiangqiRooms.value = { ...xiangqiRooms.value, [room.roomId]: reset };
      selectedXiangqiPoint.value = null;
      updateRoomFromState(room.roomId, reset);
      await broadcastSnapshot(room.roomId);
    }
    return;
  }
  const player = currentDdzPlayer();
  if (!activeDdzState.value || !player) return;
  if (!myDdzSeat.value) {
    await sendRoomAction({ action: "join", player });
    return;
  }
  if (activeDdzState.value.phase === "lobby") {
    await sendRoomAction({ action: "ready", playerId: player.deviceId, ready: !myDdzSeat.value.ready });
    return;
  }
  if (activeDdzState.value.phase === "ended" && isRoomHost(room)) {
    const reset = createInitialDdzState({ ...room, players: room.players.map((item) => ({ ...item, ready: false })) });
    doudizhuRooms.value = { ...doudizhuRooms.value, [room.roomId]: reset };
    updateRoomFromState(room.roomId, reset);
    await broadcastSnapshot(room.roomId);
  }
}
async function bidLandlord(call: boolean) {
  if (!profile.value || activeDdzState.value?.phase !== "bidding" || !isMyDdzTurn.value) return;
  await sendRoomAction({ action: "bid", playerId: profile.value.device_id, call });
}
function toggleCard(cardId: string) {
  if (activeDdzState.value?.phase !== "playing" || !isMyDdzTurn.value) return;
  selectedCardIds.value = selectedCardIds.value.includes(cardId)
    ? selectedCardIds.value.filter((id) => id !== cardId)
    : [...selectedCardIds.value, cardId];
}
async function playSelectedCards() {
  if (!profile.value || !canPlaySelectedCards.value || !selectedPlay.value) return;
  const cardIds = selectedCards.value.map((card) => card.id);
  selectedCardIds.value = [];
  await sendRoomAction({ action: "play", playerId: profile.value.device_id, cardIds });
}
async function passTurn() {
  if (!profile.value || !canPassDdz.value) return;
  selectedCardIds.value = [];
  await sendRoomAction({ action: "pass", playerId: profile.value.device_id });
}
async function sendRoomChat() {
  const content = roomChatDraft.value.trim();
  if (!content || !profile.value || !activeGameRoom.value) return;
  const message: RoomChatItem = {
    id: `room-chat-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    senderDeviceId: profile.value.device_id,
    sender: profile.value.nickname,
    content,
    mine: true,
    createdAt: Date.now(),
  };
  roomChatDraft.value = "";
  await sendRoomAction({ action: "chat", message });
}
function processGameFrame(frame: GameFrame) {
  if (frame.sender_device_id === profile.value?.device_id) return;
  if (!gameRegistry.some((game) => game.type === frame.game)) return;
  if (frame.kind === "leaderboard_sync") {
    applyLeaderboardSync(frame.payload as LeaderboardSyncPayload);
    return;
  }
  const payload = frame.payload as { room?: GameRoomShell; state?: DdzTableState | GomokuTableState | XiangqiTableState | MinesweeperTableState; roomId?: string; action?: GameActionPayload };
  if (frame.kind === "room_created" && payload.room && payload.state) {
    upsertGameRoom(payload.room);
    upsertIncomingGameState(payload.room, payload.state);
  }
  if (frame.kind === "room_dissolved" && payload.roomId) {
    removeGameRoom(payload.roomId);
    return;
  }
  if (frame.kind === "room_snapshot" && payload.room && payload.state) {
    upsertGameRoom(payload.room);
    upsertIncomingGameState(payload.room, payload.state);
  }
  if (frame.kind === "room_action" && payload.roomId && payload.action && isRoomHost(gameRoomsState.value.find((room) => room.roomId === payload.roomId) ?? null)) {
    const changed = applyRoomAction(payload.roomId, payload.action);
    if (changed) broadcastSnapshot(payload.roomId);
  }
}
function upsertIncomingGameState(room: GameRoomShell, state: DdzTableState | GomokuTableState | XiangqiTableState | MinesweeperTableState) {
  if (room.gameType === "gomoku") {
    const normalized = normalizeIncomingState(state as GomokuTableState);
    gomokuRooms.value = { ...gomokuRooms.value, [room.roomId]: normalized };
    maybeRecordGameResult(room, normalized);
    return;
  }
  if (room.gameType === "minesweeper") {
    const normalized = normalizeIncomingState(state as MinesweeperTableState);
    minesweeperRooms.value = { ...minesweeperRooms.value, [room.roomId]: normalized };
    maybeRecordGameResult(room, normalized);
    return;
  }
  if (room.gameType === "xiangqi") {
    const normalized = normalizeIncomingState(state as XiangqiTableState);
    xiangqiRooms.value = { ...xiangqiRooms.value, [room.roomId]: normalized };
    selectedXiangqiPoint.value = null;
    maybeRecordGameResult(room, normalized);
    return;
  }
  const normalized = normalizeIncomingState(state as DdzTableState);
  doudizhuRooms.value = { ...doudizhuRooms.value, [room.roomId]: normalized };
  maybeRecordGameResult(room, normalized);
}
function normalizeIncomingState<T extends { chatMessages: RoomChatItem[] }>(state: T): T {
  return {
    ...state,
    chatMessages: state.chatMessages.map((item) => ({ ...item, mine: item.senderDeviceId === profile.value?.device_id })),
  };
}
function openLeaderboard() {
  leaderboardOpen.value = true;
}
function rankedGameTypeOf(game: GameType): RankedGameType | null {
  return game === "doudizhu" || game === "gomoku" || game === "xiangqi" ? game : null;
}
function encodeGameInvite(room: GameRoomShell) {
  const payload: GameInvitePayload = {
    roomId: room.roomId,
    roomName: room.roomName,
    gameType: room.gameType,
    gameName: gameDefinitionOf(room.gameType).name,
    hostName: room.hostName,
    hostDeviceId: room.hostDeviceId,
    createdAt: Date.now(),
  };
  return `${GAME_INVITE_PREFIX}${JSON.stringify(payload)}`;
}
function parseGameInvite(content: string): GameInvitePayload | null {
  if (!content.startsWith(GAME_INVITE_PREFIX)) return null;
  try {
    const payload = JSON.parse(content.slice(GAME_INVITE_PREFIX.length)) as GameInvitePayload;
    if (!payload.roomId || !payload.gameType || !payload.roomName) return null;
    return payload;
  } catch {
    return null;
  }
}
function gameInvitePayload(message: Message) {
  return message.message_type === "text" ? parseGameInvite(message.content) : null;
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
  selectedRecipientConversationIds.value = [];
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
function toggleRecipientConversation(conversationId: string) {
  selectedRecipientConversationIds.value = selectedRecipientConversationIds.value.includes(conversationId)
    ? selectedRecipientConversationIds.value.filter((id) => id !== conversationId)
    : [...selectedRecipientConversationIds.value, conversationId];
}
async function confirmRecipientPicker() {
  if (recipientConfirmDisabled.value) return;
  if (recipientPickerMode.value === "gameInvite") {
    const room = activeGameRoom.value;
    if (!room) return;
    const payload = encodeGameInvite(room);
    const targets = [...selectedRecipientPeerIds.value, ...selectedRecipientConversationIds.value];
    for (const conversationId of targets) {
      await store.sendMessageToConversation(conversationId, payload);
    }
  } else if (recipientPickerMode.value === "privateChannelCreate") {
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
function seedInvitedRoomState(room: GameRoomShell) {
  if (room.gameType === "gomoku" && !gomokuRooms.value[room.roomId]) {
    gomokuRooms.value = { ...gomokuRooms.value, [room.roomId]: createInitialGameState(room) as GomokuTableState };
    return;
  }
  if (room.gameType === "minesweeper" && !minesweeperRooms.value[room.roomId]) {
    minesweeperRooms.value = { ...minesweeperRooms.value, [room.roomId]: createInitialGameState(room) as MinesweeperTableState };
    return;
  }
  if (room.gameType === "xiangqi" && !xiangqiRooms.value[room.roomId]) {
    xiangqiRooms.value = { ...xiangqiRooms.value, [room.roomId]: createInitialGameState(room) as XiangqiTableState };
    return;
  }
  if (room.gameType === "doudizhu" && !doudizhuRooms.value[room.roomId]) {
    doudizhuRooms.value = { ...doudizhuRooms.value, [room.roomId]: createInitialGameState(room) as DdzTableState };
  }
}
function ensureRoomFromInvite(invite: GameInvitePayload) {
  const existing = gameRoomsState.value.find((item) => item.roomId === invite.roomId);
  if (existing) return existing;
  const hostPeer = invite.hostDeviceId ? peers.value.find((peer) => peer.device_id === invite.hostDeviceId) : null;
  const now = Date.now();
  const room: GameRoomShell = {
    roomId: invite.roomId,
    roomName: invite.roomName,
    gameType: invite.gameType,
    hostDeviceId: invite.hostDeviceId ?? "",
    hostName: invite.hostName || hostPeer?.nickname || "房主",
    players: invite.hostDeviceId ? [{
      deviceId: invite.hostDeviceId,
      nickname: invite.hostName || hostPeer?.nickname || "房主",
      avatar: hostPeer?.avatar,
      online: hostPeer?.online ?? false,
      ready: false,
    }] : [],
    createdAt: invite.createdAt || now,
    updatedAt: now,
  };
  upsertGameRoom(room);
  seedInvitedRoomState(room);
  return room;
}
function openGameInvite(invite: GameInvitePayload | null) {
  if (!invite) return;
  selectedGameType.value = invite.gameType;
  const room = ensureRoomFromInvite(invite);
  openGameRoom(room.roomId);
}
function maybeRecordGameResult(room: GameRoomShell, state: DdzTableState | GomokuTableState | XiangqiTableState | MinesweeperTableState) {
  if (state.phase !== "ended") return;
  if (room.gameType === "minesweeper") {
    const table = state as MinesweeperTableState;
    const winnerId = table.winnerDeviceId;
    const boardState = winnerId ? table.boards[winnerId] : null;
    if (!winnerId || !boardState?.startedAt || !boardState.finishedAt) return;
    const key = `minesweeper:${room.roomId}:${winnerId}:${boardState.finishedAt}`;
    if (recordedGameResultIds.has(key)) return;
    recordedGameResultIds.add(key);
    const winner = table.players.find((player) => player.deviceId === winnerId);
    const record = createMinesweeperLeaderboardRecord({
      deviceId: winnerId,
      nickname: winner?.nickname ?? table.winnerName ?? "局域网玩家",
      width: table.width,
      height: table.height,
      mines: table.mines,
      elapsedMs: boardState.finishedAt - boardState.startedAt,
      moves: boardState.moves,
      finishedAt: boardState.finishedAt,
    });
    minesweeperLeaderboardRecords.value = upsertMinesweeperLeaderboardRecords(minesweeperLeaderboardRecords.value, [record]);
    saveMinesweeperLeaderboardRecords();
    return;
  }
  const game = rankedGameTypeOf(room.gameType);
  if (!game) return;
  const rankedState = state as DdzTableState | GomokuTableState | XiangqiTableState;
  const players = rankedState.players.map((player) => ({ deviceId: player.deviceId, nickname: player.nickname }));
  const winnerId = rankedState.winnerDeviceId;
  const key = `${game}:${room.roomId}:${rankedState.updatedAt}:${winnerId ?? "draw"}`;
  if (recordedGameResultIds.has(key)) return;
  recordedGameResultIds.add(key);
  let nextRecords = gameStatsRecords.value;
  for (const player of players) {
    nextRecords = incrementGameStats(nextRecords, {
      game,
      deviceId: player.deviceId,
      nickname: player.nickname,
      won: !!winnerId && player.deviceId === winnerId,
      updatedAt: rankedState.updatedAt,
    });
  }
  gameStatsRecords.value = upsertGameStatsRecords([], nextRecords);
  saveGameStatsRecords();
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
    discoModeUntil.value = Math.max(discoModeUntil.value, Date.now() + PET_DISCO_ALERT_DURATION_MS);
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
function applyQuickAlertTrustReset(reset: QuickAlertTrustReset) {
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
  discoModeUntil.value = Math.max(discoModeUntil.value, Date.now() + mode.duration_ms);
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
  if (reset) applyQuickAlertTrustReset(reset);
}
async function resetAllAlertCredibilityRecords() {
  if (!superAdminEnabled.value) return;
  const reset = await store.resetQuickAlertCredibility(QUICK_ALERT_TRUST_RESET_ALL_TARGET);
  if (reset) applyQuickAlertTrustReset(reset);
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
async function attachCallStreams() {
  await nextTick();
  if (localCallVideo.value && callLocalStream) {
    localCallVideo.value.srcObject = callLocalStream;
  }
  if (remoteCallVideo.value && callRemoteStream) {
    remoteCallVideo.value.srcObject = callRemoteStream;
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
  if (current.remoteVideo && callRemoteStream) current.remoteVideo.srcObject = callRemoteStream;
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
  if (!pictureInPicture) {
    store.error = "当前系统运行环境不支持独立通话窗口，可使用通话条上的展开按钮查看画面";
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
    if (session.media === "video") {
      const videoStage = doc.createElement("section");
      videoStage.style.cssText = "position:relative;flex:1;min-height:0;border-radius:8px;overflow:hidden;background:#1d2735;";
      remoteVideo = doc.createElement("video");
      remoteVideo.autoplay = true;
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
    detachedCallWindow = { window: popup, title, status, remoteVideo, localVideo };
    popup.addEventListener("pagehide", () => {
      if (detachedCallWindow?.window === popup) detachedCallWindow = null;
    }, { once: true });
    syncDetachedCallWindow();
  } catch (error) {
    store.error = `打开独立通话窗口失败：${stringifyError(error)}`;
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
  callLocalStream?.getTracks().forEach((track) => track.stop());
  callLocalStream = null;
  callRemoteStream = null;
  queuedCallCandidates = [];
  if (localCallVideo.value) localCallVideo.value.srcObject = null;
  if (remoteCallVideo.value) remoteCallVideo.value.srcObject = null;
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
  if (!navigator.mediaDevices?.getUserMedia) throw new Error("当前环境不支持语音或视频通话");
  try {
    callLocalStream = await navigator.mediaDevices.getUserMedia({ audio: true, video: media === "video" });
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
    void openDetachedCallWindow();
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
async function acceptIncomingCall(openIndependentWindow = true) {
  const signal = incomingCallSignal.value;
  const session = callSession.value;
  if (!signal || !session || session.status !== "incoming") return;
  try {
    if (openIndependentWindow) void openDetachedCallWindow();
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
      await acceptIncomingCall(false);
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
  if (activePetAlert.value) {
    visuallyStoppedAlertIds.value = new Set([...visuallyStoppedAlertIds.value, activePetAlert.value.alertId]);
  }
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
    latest_sender: alert?.senderNickname ?? null,
    latest_sender_address: alert?.senderAddress ?? null,
    latest_content: alert ? `${alert.content}${simulationLabel(alert.simulation) ? ` · ${simulationLabel(alert.simulation)}` : ""}` : null,
    latest_created_at: alert?.createdAt ?? null,
    incoming_call_id: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.callId : null,
    incoming_call_sender: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.peerNickname : null,
    incoming_call_media: petAlertEnabled.value && callSession.value?.status === "incoming" ? callSession.value.media : null,
    feedbackable: !!latestPendingAlert.value,
    flashing: !!alert,
    disco: discoModeActive.value && !!alert,
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
  // Close the badge and detail panel immediately; the monotonic revision keeps
  // an older queued runtime snapshot from bringing this alert back afterward.
  await syncDesktopPetRuntime();
  const feedback = await store.sendQuickAlertFeedback(alert.alertId, alert.senderDeviceId, result);
  if (feedback) {
    applyQuickAlertFeedback(feedback);
  }
}
function alertProbabilityLabel(alert?: AlertRecord | null) {
  if (!alert) return "0°C";
  const score = alertTruthScore(alert, nowTick.value);
  return score.feedbackCount === 0 ? `${alertDisplayTemperature(alert)}°C` : `${score.probability}%`;
}
function openSection(section: MainSection) {
  if (section === "alerts" && !petAlertEnabled.value) {
    activeSection.value = "settings";
    return;
  }
  activeSection.value = section;
  if (section === "chat") {
    unreadByConversation.value = { ...unreadByConversation.value, [activeConversationId.value]: 0 };
    void scrollActiveChatToBottom();
  }
  if (section === "games") {
    void broadcastLeaderboardSync();
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
function appendEmojiToRoomDraft(emoji: string) {
  roomChatDraft.value += emoji;
  roomEmojiOpen.value = false;
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
async function startDirectChat(peer = selectedPeerDetail.value) {
  if (!peer) return;
  activeSection.value = "chat";
  await store.openDirect(peer);
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
  if (trimmed.startsWith("data:image/") || trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed;
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
  return `data:${mime};base64,${payload}`;
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
function handleProfileAvatarSelected(event: Event) {
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
  const reader = new FileReader();
  reader.onload = () => {
    avatarDraft.value = typeof reader.result === "string" ? reader.result : "";
  };
  reader.onerror = () => {
    store.error = "读取头像图片失败";
  };
  reader.readAsDataURL(file);
}
function formatFileSize(size?: number | null) {
  if (!size) return "0 B";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
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
  return cachedPath ? convertFileSrc(cachedPath) : message.file_meta?.url ?? "";
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
  } catch {
    // 预览缓存失败时继续使用发送方的临时文件服务地址。
  }
}
async function clearImagePreviewCache() {
  previewMediaCacheClearing.value = true;
  try {
    previewMediaCacheInfo.value = await api.clearPreviewMediaCache();
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
function handleRoomChatEnter(event: KeyboardEvent) {
  if (event.shiftKey) return;
  event.preventDefault();
  sendRoomChat();
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
  <NConfigProvider :theme-overrides="themeOverrides" :class="['provider-root', selectedTheme]">
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
            <div class="private-call-title" @mousedown="startCallPanelDrag">
              <div class="private-call-summary">
                <strong>{{ callSession.status === 'incoming' ? `${callSession.peerNickname} 邀请${callSession.media === 'video' ? '视频' : '语音'}通话` : `${callSession.media === 'video' ? '视频' : '语音'}通话 · ${callSession.peerNickname}` }}</strong>
                <span>{{ callSession.status === 'incoming' ? '等待接听' : callSession.status === 'outgoing' ? '正在呼叫' : callSession.status === 'failed' ? (callSession.error ?? '通话未建立') : '已连接' }}</span>
              </div>
              <button class="private-call-toggle" type="button" title="弹出独立通话窗口" @click="openDetachedCallWindow">▣</button>
              <button class="private-call-toggle" type="button" :title="callPanelExpanded ? '收起画面' : '展开画面'" @click="callPanelExpanded = !callPanelExpanded">{{ callPanelExpanded ? '⌃' : '⌄' }}</button>
            </div>
          <div v-if="callPanelExpanded" class="private-call-videos" :class="{ audio: callSession.media === 'audio' }">
            <video v-if="callSession.media === 'video'" ref="remoteCallVideo" autoplay playsinline></video>
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
      <NModal v-model:show="adminNotificationDetailOpen" preset="card" title="通知审核详情" class="admin-notification-announcement">
        <div v-if="adminNotificationDetail" class="admin-notification-lock-content">
          <div class="admin-notification-detail-device"><img v-if="avatarImage(adminNotificationTargetDetail(adminNotificationDetail)?.avatar)" class="avatar-image large-avatar" :src="avatarImage(adminNotificationTargetDetail(adminNotificationDetail)?.avatar)" alt="设备头像" /><NAvatar v-else :size="48" class="peer-avatar">{{ firstLetter(adminNotificationTargetDetail(adminNotificationDetail)?.nickname ?? '?') }}</NAvatar><div><strong>{{ adminNotificationTargetDetail(adminNotificationDetail)?.nickname ?? '未知设备' }}</strong><small>IP：{{ adminNotificationTargetDetail(adminNotificationDetail)?.address ?? '未知' }}</small><small>MAC：{{ adminNotificationDetail.target_device_id }}</small></div></div>
          <NTag :type="adminNotificationDetail.status === 'submitted' ? 'warning' : 'default'">{{ adminNotificationDetail.status }}</NTag><h2>{{ adminNotificationDetail.title }}</h2><p>{{ adminNotificationDetail.content }}</p><img v-if="adminNotificationDetail.support_url && /^(https?:|asset:|data:image)/.test(adminNotificationDetail.support_url)" :src="adminNotificationDetail.support_url" alt="通知配图" /><NText depth="3">下发时间：{{ formatTime(adminNotificationDetail.created_at) }}</NText>
          <NSpace v-if="adminNotificationDetail.display_mode === 'requires_confirmation' && adminNotificationDetail.status === 'submitted'"><NButton type="success" @click="decideAdminNotificationFromDetail('approved')">通过</NButton><NButton type="error" @click="decideAdminNotificationFromDetail('rejected')">拒绝</NButton></NSpace>
          <NButton v-else-if="adminNotificationDetail.display_mode === 'requires_confirmation' && ['pending','rejected','expired_locked'].includes(adminNotificationDetail.status)" tertiary type="warning" @click="decideAdminNotificationFromDetail('revoked')">撤销并放行</NButton>
        </div>
      </NModal>
      <NModal :show="!!blockingAdminNotification" preset="card" class="admin-notification-lock" :mask-closable="false" :closable="false">
        <div v-if="blockingAdminNotification" class="admin-notification-lock-content"><NTag type="warning">需要完成确认</NTag><h2>{{ blockingAdminNotification.title }}</h2><p>{{ blockingAdminNotification.content }}</p><img v-if="blockingAdminNotification.support_url && /^(https?:|asset:|data:image)/.test(blockingAdminNotification.support_url)" :src="blockingAdminNotification.support_url" alt="通知图片" /><NText v-if="blockingAdminNotification.deadline_at" depth="3">截至：{{ formatTime(blockingAdminNotification.deadline_at) }}</NText><NAlert v-if="blockingAdminNotification.status === 'rejected'" type="error" :show-icon="false">超管未确认，请完成后重新提交。</NAlert><NAlert v-else-if="blockingAdminNotification.status === 'expired_locked'" type="warning" :show-icon="false">已超时，等待超管决定。</NAlert><NButton v-if="blockingAdminNotification.status !== 'expired_locked'" type="primary" @click="submitBlockingAdminNotification(blockingAdminNotification)">提交已完成</NButton><NButton quaternary @click="api.quitApp">退出软件</NButton></div>
      </NModal>
      <NModal :show="!!visibleAdminAnnouncement" preset="card" class="admin-notification-announcement" :mask-closable="true" @update:show="(visible) => { if (!visible && visibleAdminAnnouncement) dismissAdminAnnouncement(visibleAdminAnnouncement); }">
        <div v-if="visibleAdminAnnouncement" class="admin-notification-lock-content"><NTag type="info">超管公告</NTag><h2>{{ visibleAdminAnnouncement.title }}</h2><p>{{ visibleAdminAnnouncement.content }}</p><img v-if="visibleAdminAnnouncement.support_url && /^(https?:|asset:|data:image)/.test(visibleAdminAnnouncement.support_url)" :src="visibleAdminAnnouncement.support_url" alt="公告图片" /><NText depth="3">{{ visibleAdminAnnouncement.issued_by_nickname }} · {{ formatTime(visibleAdminAnnouncement.created_at) }}</NText><NButton type="primary" @click="dismissAdminAnnouncement(visibleAdminAnnouncement)">我知道了</NButton></div>
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
          <NLayoutSider class="rail" :class="{ expanded: navExpanded }" :width="navExpanded ? 176 : 64" bordered>
            <div class="rail-inner">
              <button class="rail-action profile-entry" title="个人资料" @click="openSection('settings')">
                <img v-if="avatarImage(profile?.avatar)" class="avatar-image self-avatar" :src="avatarImage(profile?.avatar)" alt="本机头像" />
                <NAvatar v-else class="self-avatar">{{ avatarLabel(profile?.avatar, profile?.nickname) }}</NAvatar>
                <span v-if="navExpanded" class="nav-label">{{ profile?.nickname ?? "个人资料" }}</span>
              </button>
              <button class="rail-collapse-toggle" :title="navExpanded ? '收起侧栏' : '展开侧栏'" @click="toggleNav">
                {{ navExpanded ? "‹" : "›" }}
              </button>
              <button
                class="rail-action"
                :class="{ active: activeSection === 'chat' }"
                title="聊天"
                @click="openSection('chat')"
              >
                <span class="nav-icon">💬</span>
                <span v-if="navExpanded" class="nav-label">聊天</span>
                <span v-if="totalUnread > 0" class="nav-unread">{{ totalUnread > 99 ? "99+" : totalUnread }}</span>
              </button>
              <button
                class="rail-action"
                :class="{ active: activeSection === 'devices' }"
                title="设备列表"
                @click="openSection('devices')"
              >
                <span class="nav-icon">🖥</span>
                <span v-if="navExpanded" class="nav-label">设备列表</span>
              </button>
              <button
                class="rail-action"
                :class="{ active: activeSection === 'games' }"
                title="游戏"
                @click="openSection('games')"
              >
                <span class="nav-icon">🎮</span>
                <span v-if="navExpanded" class="nav-label">游戏</span>
                <span v-if="showGameAttention" class="nav-unread">{{ gameAttentionCount > 9 ? "9+" : gameAttentionCount }}</span>
              </button>
              <button
                v-if="petAlertEnabled"
                class="rail-action"
                :class="{ active: activeSection === 'alerts' }"
                title="狼来了排行榜"
                @click="openSection('alerts')"
              >
                <span class="nav-icon">🐸</span>
                <span v-if="navExpanded" class="nav-label">狼来了</span>
              </button>
              <button class="rail-action add" title="添加设备" @click="openSection('devices')">
                <span class="nav-icon">＋</span>
                <span v-if="navExpanded" class="nav-label">添加设备</span>
              </button>
              <div class="rail-spacer"></div>
              <NTooltip trigger="hover" placement="right">
                <template #trigger>
                  <button
                    class="rail-action"
                    :class="{ active: activeSection === 'settings' }"
                    title="设置"
                    @click="openSection('settings')"
                  >
                    <span class="nav-icon">⚙</span>
                    <span v-if="navExpanded" class="nav-label">设置</span>
                    <span v-if="visibleUpdateAvailable" class="nav-upgrade-badge">{{ updateBadgeLabel }}</span>
                  </button>
                </template>
                设置
              </NTooltip>
            </div>
          </NLayoutSider>
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
                    </template>
                  </NThing>
                </NListItem>
              </NList>
            </NScrollbar>
            <button class="pane-resize-handle left-list" type="button" aria-label="拖动调整列表宽度" title="拖动调整宽度" @mousedown="startPaneResize('list', $event)"></button>
          </NLayoutSider>
          <NLayoutSider v-else-if="activeSection === 'games' && !listPaneCollapsed" class="list-pane" :width="listPaneWidth" bordered>
            <div class="pane-header">
              <div class="pane-title-row">
                <strong>游戏</strong>
                <NButton quaternary circle size="small" title="创建房间" @click="createRoomOpen = true">＋</NButton>
              </div>
              <NInput size="small" clearable placeholder="搜索游戏或房间" />
            </div>
            <NScrollbar class="list-scroll">
              <div class="section-label">内置游戏</div>
              <div
                v-for="game in gameRegistry"
                :key="game.type"
                class="game-list-card"
                :class="{ active: selectedGameType === game.type }"
                @click="openBuiltinGame(game.type)"
              >
                <div class="game-list-icon">{{ game.icon }}</div>
                <div>
                  <div class="game-list-title">{{ game.name }}</div>
                  <div class="game-list-sub">{{ game.minPlayers }}-{{ game.maxPlayers }} 人房间 · 支持房间聊天</div>
                </div>
                <NTag size="small" :bordered="false" type="success">可用</NTag>
              </div>
              <div class="section-label">房间</div>
              <div
                v-for="room in gameRoomsState"
                :key="room.roomId"
                class="game-list-card"
                :class="{ active: room.roomId === activeGameRoomId }"
                @click="openGameRoom(room.roomId)"
              >
                <div class="game-list-icon">{{ gameDefinitionOf(room.gameType).icon }}</div>
                <div>
                  <div class="game-list-title">{{ room.roomName }}</div>
                  <div class="game-list-sub">{{ gameDefinitionOf(room.gameType).name }} · {{ room.players.length }}/{{ gameDefinitionOf(room.gameType).maxPlayers }} 人</div>
                </div>
                <NTag size="small" :bordered="false">{{ room.hostDeviceId === profile?.device_id ? "我创建" : "可加入" }}</NTag>
              </div>
              <NEmpty v-if="gameRoomsState.length === 0" description="还没有游戏房间" class="list-empty">
                <template #extra>
                  <NText depth="3">点击创建后先选择游戏类型。</NText>
                </template>
              </NEmpty>
              <div class="create-game-box">
                <NButton block type="primary" @click="createRoomOpen = true">创建房间</NButton>
                <NText depth="3">先选择游戏，再创建对应房间；不同游戏会进入不同交互界面。</NText>
              </div>
            </NScrollbar>
            <button class="pane-resize-handle left-list" type="button" aria-label="拖动调整列表宽度" title="拖动调整宽度" @mousedown="startPaneResize('list', $event)"></button>
          </NLayoutSider>
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
              <div ref="messagePane" class="messages-pane">
                <NSpin :show="loading">
                  <NEmpty v-if="!loading && activeMessages.length === 0" description="还没有消息" class="empty-state">
                    <template #extra>
                      <span>选择在线设备单聊，或在局域网频道里发第一句。</span>
                    </template>
                  </NEmpty>
                  <article
                    v-for="message in activeMessages"
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
                        <div class="message-bubble" :class="{ 'message-card-bubble': privateChannelInvitePayload(message) || gameInvitePayload(message) }">
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
                          <template v-else-if="gameInvitePayload(message)">
                            <button class="game-invite-card invite-message-card" type="button" @click="openGameInvite(gameInvitePayload(message))">
                              <span class="game-invite-icon">{{ gameDefinitionOf(gameInvitePayload(message)?.gameType ?? 'doudizhu').icon }}</span>
                              <span class="game-invite-copy">
                                <strong>{{ gameInvitePayload(message)?.roomName }}</strong>
                                <small>{{ gameInvitePayload(message)?.gameName }} · {{ gameInvitePayload(message)?.hostName }} 邀请加入</small>
                              </span>
                            </button>
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
                            <img v-if="isImageFile(message)" class="file-preview-image" :src="imagePreviewSource(message)" :alt="message.file_meta.name" title="点击放大查看" @click="openImagePreview(message)" @load="cacheImagePreview(message)" />
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
            <section v-else-if="activeSection === 'games'" class="game-workspace" :class="{ 'gomoku-workspace': activeGameRoom?.gameType === 'gomoku', 'xiangqi-workspace': activeGameRoom?.gameType === 'xiangqi', 'minesweeper-workspace': activeGameRoom?.gameType === 'minesweeper' }">
              <header class="game-header" data-tauri-drag-region>
                <div>
                  <h2>{{ activeGameDefinition.name }} · {{ activeGameRoom?.roomName ?? "排行榜" }}</h2>
                  <p>{{ activeGameDefinition.description }} · 房间类型：{{ activeGameDefinition.name }}</p>
                </div>
                <div class="game-header-actions">
                  <NButton v-if="activeGameRoom" secondary @click="openRecipientPicker('gameInvite')">邀请</NButton>
                  <NButton v-if="activeGameRoom" secondary @click="openLeaderboard">排行榜</NButton>
                  <NButton v-if="!activeGameRoom" secondary @click="createRoomOpen = true">创建房间</NButton>
                  <NButton v-if="activeGameRoom && isRoomHost()" secondary type="error" @click="dissolveRoom">解散房间</NButton>
                  <NButton v-else-if="activeGameRoom && myGameSeat" secondary type="warning" @click="leaveRoom">退出房间</NButton>
                  <NButton v-if="activeGameRoom" type="primary" :disabled="activeGameRoom?.gameType === 'doudizhu' ? activeDdzState?.phase === 'playing' || activeDdzState?.phase === 'bidding' : activeGameRoom?.gameType === 'xiangqi' ? activeXiangqiState?.phase === 'playing' : activeGameRoom?.gameType === 'minesweeper' ? activeMinesweeperState?.phase === 'playing' : activeGomokuState?.phase === 'playing'" @click="roomPrimaryAction">{{ roomPrimaryLabel }}</NButton>
                </div>
              </header>
              <div v-if="activeGameRoom?.gameType === 'doudizhu'" class="doudizhu-layout">
                <main class="doudizhu-table">
                  <div class="landlord-cards">
                    <div v-for="(card, index) in visibleLandlordCards" :key="card ? card.id : `back-${index}`" class="poker-card" :class="card ? { red: card.red } : { back: true }">{{ card ? card.label : "牌" }}</div>
                  </div>
                  <div class="desk-surface">
                    <div class="desk-center">
                      <div class="played-cards">
                        <div v-for="card in tableLastCards" :key="card.id" class="poker-card" :class="{ red: card.red }">{{ card.label }}</div>
                      </div>
                      <div class="turn-note">上家出牌：{{ activeDdzState?.lastPlay ? `${activeDdzState.lastPlay.playerName} · ${playLabel(activeDdzState.lastPlay)}` : "无" }} · {{ playHint }}</div>
                    </div>
                  </div>
                  <div v-if="leftDdzSeat" class="table-player left">
                    <NAvatar class="table-avatar" :src="avatarImage(leftDdzSeat.avatar)">{{ firstLetter(leftDdzSeat.nickname) }}</NAvatar>
                    <div>
                      <div class="table-player-name">{{ leftDdzSeat.nickname }} <span v-if="seatTurnLabel(leftDdzSeat)" class="turn-countdown">{{ seatTurnLabel(leftDdzSeat) }}</span> <NTag v-if="leftDdzSeat.role === 'landlord'" size="small" :bordered="false" type="warning">地主</NTag></div>
                      <div class="table-player-meta">{{ leftDdzSeat.online ? "在线" : "离线" }} · 剩余 {{ leftDdzSeat.handCount }} 张</div>
                    </div>
                  </div>
                  <div v-if="rightDdzSeat" class="table-player right">
                    <NAvatar class="table-avatar" :src="avatarImage(rightDdzSeat.avatar)">{{ firstLetter(rightDdzSeat.nickname) }}</NAvatar>
                    <div>
                      <div class="table-player-name">{{ rightDdzSeat.nickname }} <span v-if="seatTurnLabel(rightDdzSeat)" class="turn-countdown">{{ seatTurnLabel(rightDdzSeat) }}</span> <NTag v-if="rightDdzSeat.role === 'landlord'" size="small" :bordered="false" type="warning">地主</NTag></div>
                      <div class="table-player-meta">{{ rightDdzSeat.online ? "在线" : "离线" }} · 剩余 {{ rightDdzSeat.handCount }} 张</div>
                    </div>
                  </div>
                  <div v-if="myDdzSeat" class="table-player me">
                    <NAvatar class="table-avatar" :src="avatarImage(myDdzSeat.avatar)">{{ firstLetter(myDdzSeat.nickname) }}</NAvatar>
                    <div>
                      <div class="table-player-name">我 · {{ myDdzSeat.nickname }} <span v-if="seatTurnLabel(myDdzSeat)" class="turn-countdown">{{ seatTurnLabel(myDdzSeat) }}</span> <NTag v-if="myDdzSeat.role === 'landlord'" size="small" :bordered="false" type="warning">地主</NTag></div>
                      <div class="table-player-meta">{{ myDdzSeat.role === "landlord" ? "地主" : myDdzSeat.role === "farmer" ? "农民" : myDdzSeat.ready ? "已准备" : "未准备" }} · {{ isMyDdzTurn ? "轮到你" : "等待" }}</div>
                    </div>
                  </div>
                  <div v-if="activeDdzState?.phase === 'ended'" class="settlement-overlay">
                    <div class="settlement-panel">
                      <div class="settlement-kicker">本局结算</div>
                      <h3>{{ settlementWinnerLabel }} 获胜</h3>
                      <div class="settlement-list">
                        <div v-for="player in settlementRows" :key="player.deviceId" class="settlement-row" :class="{ winner: player.deviceId === activeDdzState?.winnerDeviceId }">
                          <div class="settlement-player">
                            <NAvatar :size="28" class="table-avatar" :src="avatarImage(player.avatar)">{{ firstLetter(player.nickname) }}</NAvatar>
                            <span>{{ player.deviceId === myDeviceId ? `我 · ${player.nickname}` : player.nickname }}</span>
                          </div>
                          <NTag v-if="player.role" size="small" :bordered="false" :type="player.role === 'landlord' ? 'warning' : 'info'">
                            {{ player.role === "landlord" ? "地主" : "农民" }}
                          </NTag>
                          <strong>剩余 {{ player.remaining }} 张</strong>
                        </div>
                      </div>
                      <div class="settlement-actions">
                        <NButton v-if="activeGameRoom && isRoomHost()" type="primary" @click="roomPrimaryAction">再来一局</NButton>
                        <NButton secondary @click="leaveRoom">退出房间</NButton>
                      </div>
                    </div>
                  </div>                </main>
                <aside class="game-room-panel">
                  <div class="room-chat-panel">
                    <div class="room-chat-head">房间聊天</div>
                    <div ref="roomChatPane" class="room-chat-list">
                      <div v-for="item in activeRoomChatMessages" :key="item.id" class="room-chat-msg" :class="{ mine: item.mine }">
                        <div class="room-chat-name">{{ item.sender }}</div>
                        <div class="room-chat-bubble">{{ item.content }}</div>
                      </div>
                    </div>
                    <div class="room-chat-composer">
                      <div class="emoji-wrap">
                        <button class="emoji-trigger" title="表情" @click="roomEmojiOpen = !roomEmojiOpen">☺</button>
                        <div v-if="roomEmojiOpen" class="emoji-panel room-emoji-panel">
                          <button v-for="emoji in emojiOptions" :key="emoji" @click="appendEmojiToRoomDraft(emoji)">{{ emoji }}</button>
                        </div>
                      </div>
                      <NInput v-model:value="roomChatDraft" placeholder="房间聊天" @keydown.enter="handleRoomChatEnter" />
                      <NButton type="primary" @click="sendRoomChat">发</NButton>
                    </div>
                  </div>
                </aside>
              </div>
              <div v-else-if="activeGameRoom?.gameType === 'minesweeper'" class="minesweeper-layout">
                <main class="minesweeper-table">

                  <div class="minesweeper-race-area">
                    <aside class="minesweeper-player-list">
                      <div v-for="player in minesweeperSettlementRows" :key="player.deviceId" class="minesweeper-player" :class="{ me: player.deviceId === myDeviceId, winner: activeMinesweeperState?.winnerDeviceId === player.deviceId, lost: player.boardState?.status === 'lost' }">
                        <NAvatar class="table-avatar" :src="avatarImage(player.avatar)">{{ firstLetter(player.nickname) }}</NAvatar>
                        <div class="minesweeper-player-main">
                          <strong>{{ player.deviceId === myDeviceId ? `我 · ${player.nickname}` : player.nickname }}</strong>
                          <span>{{ player.result }} · {{ minesweeperProgressPercent(player.boardState) }}% · {{ minesweeperElapsedLabel(player.boardState?.startedAt, player.boardState?.finishedAt) }}</span>
                          <div class="minesweeper-progress"><i :style="{ width: `${minesweeperProgressPercent(player.boardState)}%` }"></i></div>
                        </div>
                      </div>
                    </aside>

                    <div class="minesweeper-board-wrap">
                      <div class="minesweeper-board-meta">
                        <NDropdown
                          v-if="activeMinesweeperState?.phase === 'lobby' && activeGameRoom && isRoomHost()"
                          trigger="click"
                          :options="minesweeperDifficultyOptions"
                          @select="selectMinesweeperDifficulty"
                        >
                          <button class="minesweeper-meta-chip difficulty" type="button">{{ activeMinesweeperDifficultyLabel }}</button>
                        </NDropdown>
                        <span v-else class="minesweeper-meta-chip">{{ activeMinesweeperDifficultyLabel }}</span>
                        <span class="minesweeper-meta-chip">{{ activeMinesweeperState?.mines ?? 40 }} 雷</span>
                        <span class="minesweeper-meta-chip">旗 {{ myMinesweeperBoardState?.flagged ?? 0 }}</span>
                      </div>
                      <div
                        class="minesweeper-board"
                        :style="minesweeperBoardStyle"
                        aria-label="扫雷棋盘"
                      >
                        <template v-for="(row, y) in myMinesweeperBoardState?.board ?? []" :key="`mine-row-${y}`">
                          <button
                            v-for="(cell, x) in row"
                            :key="`mine-cell-${x}-${y}`"
                            class="minesweeper-cell"
                            :class="[{ revealed: cell.revealed, flagged: cell.flagged, mine: cell.mine && cell.revealed, exploded: cell.exploded }, minesweeperCellTone(cell)]"
                            :disabled="!canUseMinesweeperBoard()"
                            @click="revealMinesweeperAt(x, y)"
                            @dblclick="chordMinesweeperAt(x, y)"
                            @contextmenu.prevent="flagMinesweeperAt(x, y)"
                          >
                            {{ minesweeperCellText(cell) }}
                          </button>
                        </template>
                      </div>
                    </div>
                  </div>

                  <div v-if="activeMinesweeperState?.phase === 'ended'" class="settlement-overlay minesweeper-settlement">
                    <div class="settlement-panel">
                      <div class="settlement-kicker">竞速结算</div>
                      <h3>{{ activeMinesweeperState?.winnerDeviceId ? `${activeMinesweeperState.winnerName} 获胜` : activeMinesweeperState?.winnerName ?? '本局结束' }}</h3>
                      <div class="settlement-list">
                        <div v-for="player in minesweeperSettlementRows" :key="player.deviceId" class="settlement-row" :class="{ winner: activeMinesweeperState?.winnerDeviceId === player.deviceId }">
                          <div class="settlement-player">
                            <NAvatar :size="28" class="table-avatar" :src="avatarImage(player.avatar)">{{ firstLetter(player.nickname) }}</NAvatar>
                            <span>{{ player.deviceId === myDeviceId ? `我 · ${player.nickname}` : player.nickname }}</span>
                          </div>
                          <NTag size="small" :bordered="false" :type="player.boardState?.status === 'lost' ? 'error' : player.boardState?.status === 'won' ? 'success' : 'info'">{{ player.result }}</NTag>
                          <strong>{{ minesweeperProgressPercent(player.boardState) }}%</strong>
                        </div>
                      </div>
                      <div class="settlement-actions">
                        <NButton v-if="activeGameRoom && isRoomHost()" type="primary" @click="roomPrimaryAction">再来一局</NButton>
                        <NButton secondary @click="leaveRoom">退出房间</NButton>
                      </div>
                    </div>
                  </div>
                </main>
                <aside class="game-room-panel minesweeper-room-panel">
                  <div class="room-chat-panel">
                    <div class="room-chat-head">房间聊天</div>
                    <div ref="roomChatPane" class="room-chat-list">
                      <div v-for="item in activeRoomChatMessages" :key="item.id" class="room-chat-msg" :class="{ mine: item.mine }">
                        <div class="room-chat-name">{{ item.sender }}</div>
                        <div class="room-chat-bubble">{{ item.content }}</div>
                      </div>
                    </div>
                    <div class="room-chat-composer">
                      <div class="emoji-wrap">
                        <button class="emoji-trigger" title="表情" @click="roomEmojiOpen = !roomEmojiOpen">☺</button>
                        <div v-if="roomEmojiOpen" class="emoji-panel room-emoji-panel">
                          <button v-for="emoji in emojiOptions" :key="emoji" @click="appendEmojiToRoomDraft(emoji)">{{ emoji }}</button>
                        </div>
                      </div>
                      <NInput v-model:value="roomChatDraft" placeholder="房间聊天" @keydown.enter="handleRoomChatEnter" />
                      <NButton type="primary" @click="sendRoomChat">发</NButton>
                    </div>
                  </div>
                </aside>
              </div>              <div v-else-if="activeGameRoom?.gameType === 'gomoku'" class="gomoku-layout">
                <main class="gomoku-table">
                  <div class="gomoku-arena">
                    <div class="gomoku-player-card gomoku-side-player" :class="{ active: activeGomokuState?.turnDeviceId === blackGomokuSeat?.deviceId, winner: activeGomokuState?.winnerDeviceId === blackGomokuSeat?.deviceId }">
                      <span class="gomoku-stone black"></span>
                      <div>
                        <strong class="gomoku-player-name">
                          <span class="gomoku-player-name-text">{{ blackGomokuSeat?.deviceId === myDeviceId ? `我 · ${blackGomokuSeat?.nickname}` : blackGomokuSeat?.nickname ?? '等待黑棋' }}</span>
                          <span v-if="gomokuSeatTurnLabel(blackGomokuSeat)" class="turn-countdown">{{ gomokuSeatTurnLabel(blackGomokuSeat) }}</span>
                        </strong>
                        <small>{{ blackGomokuSeat?.ready ? '已准备' : activeGomokuState?.phase === 'playing' ? '执黑' : '未准备' }}</small>
                      </div>
                    </div>
                    <div class="gomoku-board-shell">
                      <div class="gomoku-board" aria-label="五子棋棋盘">
                        <div class="gomoku-grid-lines"></div>
                        <button
                          v-for="point in gomokuBoardPoints"
                          :key="`gomoku-cell-${point.x}-${point.y}`"
                          class="gomoku-cell"
                          :class="{ occupied: !!point.cell, black: point.cell === 'black', white: point.cell === 'white', win: isGomokuWinPoint(point.x, point.y), opponentLast: isOpponentLastGomokuCell(point.x, point.y), playable: canPlaceGomokuCell(point.x, point.y) }"
                          :style="gomokuPointStyle(point.x, point.y)"
                          :disabled="!canPlaceGomokuCell(point.x, point.y)"
                          @click="placeGomokuCell(point.x, point.y)"
                        >
                          <span v-if="point.cell" class="gomoku-stone" :class="point.cell"></span>
                          <span v-if="isGomokuWinPoint(point.x, point.y)" class="gomoku-win-dot"></span>
                          <span v-if="isOpponentLastGomokuCell(point.x, point.y)" class="gomoku-last-move-ring"></span>
                        </button>
                      </div>
                    </div>
                    <div class="gomoku-player-card gomoku-side-player" :class="{ active: activeGomokuState?.turnDeviceId === whiteGomokuSeat?.deviceId, winner: activeGomokuState?.winnerDeviceId === whiteGomokuSeat?.deviceId }">
                      <span class="gomoku-stone white"></span>
                      <div>
                        <strong class="gomoku-player-name">
                          <span class="gomoku-player-name-text">{{ whiteGomokuSeat?.deviceId === myDeviceId ? `我 · ${whiteGomokuSeat?.nickname}` : whiteGomokuSeat?.nickname ?? '等待白棋' }}</span>
                          <span v-if="gomokuSeatTurnLabel(whiteGomokuSeat)" class="turn-countdown">{{ gomokuSeatTurnLabel(whiteGomokuSeat) }}</span>
                        </strong>
                        <small>{{ whiteGomokuSeat?.ready ? '已准备' : activeGomokuState?.phase === 'playing' ? '执白' : '未准备' }}</small>
                      </div>
                    </div>
                  </div>
                  <div class="gomoku-action-strip">
                    <template v-if="activeGomokuState?.pendingUndo">
                      <span class="undo-request-note">
                        {{ activeGomokuState.pendingUndo.requesterId === myDeviceId ? '已发起悔棋，等待对方同意' : `${activeGomokuState.pendingUndo.requesterName} 请求悔棋` }}
                      </span>
                      <NButton v-if="canRespondGomokuUndo" size="small" type="primary" @click="respondGomokuUndo(true)">同意</NButton>
                      <NButton v-if="canRespondGomokuUndo" size="small" secondary @click="respondGomokuUndo(false)">拒绝</NButton>
                    </template>
                    <template v-else>
                      <NButton size="small" secondary :disabled="!canRequestUndoGomoku" @click="requestGomokuUndo">悔棋</NButton>
                      <NButton size="small" secondary type="error" :disabled="!canResignGomoku" @click="resignGomoku">投降</NButton>
                    </template>
                  </div>
                  <div class="gomoku-log-strip">
                    <span v-for="move in activeGomokuState?.moves.slice(-6) ?? []" :key="`${move.playerId}-${move.createdAt}`">
                      {{ move.playerName }} {{ gomokuStoneLabel(move.stone) }} {{ move.x + 1 }},{{ move.y + 1 }}
                    </span>
                  </div>
                  <div v-if="activeGomokuState?.phase === 'ended'" class="settlement-overlay gomoku-settlement">
                    <div class="settlement-panel">
                      <div class="settlement-kicker">本局结算</div>
                      <h3>{{ activeGomokuState?.winnerName ? `${activeGomokuState.winnerName} 获胜` : '平局' }}</h3>
                      <div class="settlement-list">
                        <div v-for="player in gomokuSettlementRows" :key="player.deviceId" class="settlement-row" :class="{ winner: activeGomokuState?.winnerDeviceId === player.deviceId }">
                          <div class="settlement-player">
                            <span class="gomoku-stone" :class="player.stone"></span>
                            <span>{{ player.deviceId === myDeviceId ? `我 · ${player.nickname}` : player.nickname }}</span>
                          </div>
                          <NTag size="small" :bordered="false" :type="player.stone === 'black' ? 'default' : 'info'">{{ gomokuStoneLabel(player.stone) }}</NTag>
                          <strong>{{ player.result }}</strong>
                        </div>
                      </div>
                      <div class="settlement-actions">
                        <NButton v-if="activeGameRoom && isRoomHost()" type="primary" @click="roomPrimaryAction">再来一局</NButton>
                        <NButton secondary @click="leaveRoom">退出房间</NButton>
                      </div>
                    </div>
                  </div>
                </main>
                <aside class="game-room-panel gomoku-room-panel">
                  <div class="room-chat-panel">
                    <div class="room-chat-head">房间聊天</div>
                    <div ref="roomChatPane" class="room-chat-list">
                      <div v-for="item in activeRoomChatMessages" :key="item.id" class="room-chat-msg" :class="{ mine: item.mine }">
                        <div class="room-chat-name">{{ item.sender }}</div>
                        <div class="room-chat-bubble">{{ item.content }}</div>
                      </div>
                    </div>
                    <div class="room-chat-composer">
                      <div class="emoji-wrap">
                        <button class="emoji-trigger" title="表情" @click="roomEmojiOpen = !roomEmojiOpen">☺</button>
                        <div v-if="roomEmojiOpen" class="emoji-panel room-emoji-panel">
                          <button v-for="emoji in emojiOptions" :key="emoji" @click="appendEmojiToRoomDraft(emoji)">{{ emoji }}</button>
                        </div>
                      </div>
                      <NInput v-model:value="roomChatDraft" placeholder="房间聊天" @keydown.enter="handleRoomChatEnter" />
                      <NButton type="primary" @click="sendRoomChat">发</NButton>
                    </div>
                  </div>
                </aside>
              </div>
              <div v-else-if="activeGameRoom?.gameType === 'xiangqi'" class="xiangqi-layout">
                <main class="xiangqi-table">
                  <div class="xiangqi-arena">
                    <div class="xiangqi-player-card" :class="{ active: activeXiangqiState?.turnDeviceId === leftXiangqiSeat?.deviceId, winner: activeXiangqiState?.winnerDeviceId === leftXiangqiSeat?.deviceId }">
                      <div class="xiangqi-side-mark" :class="leftXiangqiSide">{{ xiangqiSideShortLabel(leftXiangqiSide) }}</div>
                      <div>
                        <strong>{{ xiangqiSeatName(leftXiangqiSeat, leftXiangqiSide) }}</strong>
                        <small>{{ xiangqiSeatStatus(leftXiangqiSeat, leftXiangqiSide) }}</small>
                      </div>
                    </div>

                    <div class="xiangqi-board-shell">
                      <div class="xiangqi-board" :class="{ 'black-perspective': xiangqiPerspectiveSide === 'black' }" aria-label="中国象棋棋盘">
                        <div class="xiangqi-river">楚河　　　　汉界</div>
                        <div v-if="activeXiangqiState?.checkSide" class="xiangqi-check-flash" :class="{ mine: isMyXiangqiChecked }">将</div>
                        <div v-for="(row, displayY) in xiangqiDisplayRows" :key="`xiangqi-row-${displayY}`" class="xiangqi-row">
                          <button
                            v-for="point in row"
                            :key="`xiangqi-cell-${point.x}-${point.y}`"
                            class="xiangqi-cell"
                            :class="{ selected: isSelectedXiangqiCell(point.x, point.y), playable: isXiangqiCellPlayable(point.x, point.y), target: canMoveSelectedXiangqiTo(point.x, point.y), opponentLast: isOpponentLastXiangqiCell(point.x, point.y), red: point.cell?.side === 'red', black: point.cell?.side === 'black' }"
                            :disabled="!isXiangqiCellPlayable(point.x, point.y)"
                            @click="clickXiangqiCell(point.x, point.y)"
                          >
                            <span v-if="point.cell" class="xiangqi-piece" :class="point.cell.side"><span>{{ xiangqiPieceLabel(point.cell) }}</span></span>
                            <span v-else-if="canMoveSelectedXiangqiTo(point.x, point.y)" class="xiangqi-move-dot"></span>
                            <span v-if="isOpponentLastXiangqiCell(point.x, point.y)" class="xiangqi-last-move-ring"></span>
                          </button>
                        </div>
                      </div>
                    </div>

                    <div class="xiangqi-player-card" :class="{ active: activeXiangqiState?.turnDeviceId === rightXiangqiSeat?.deviceId, winner: activeXiangqiState?.winnerDeviceId === rightXiangqiSeat?.deviceId }">
                      <div class="xiangqi-side-mark" :class="rightXiangqiSide">{{ xiangqiSideShortLabel(rightXiangqiSide) }}</div>
                      <div>
                        <strong>{{ xiangqiSeatName(rightXiangqiSeat, rightXiangqiSide) }}</strong>
                        <small>{{ xiangqiSeatStatus(rightXiangqiSeat, rightXiangqiSide) }}</small>
                      </div>
                    </div>
                  </div>

                  <div class="xiangqi-action-strip">
                    <template v-if="activeXiangqiState?.pendingUndo">
                      <span class="undo-request-note">
                        {{ activeXiangqiState.pendingUndo.requesterId === myDeviceId ? '已发起悔棋，等待对方同意' : `${activeXiangqiState.pendingUndo.requesterName} 请求悔棋` }}
                      </span>
                      <NButton v-if="canRespondXiangqiUndo" size="small" type="primary" @click="respondXiangqiUndo(true)">同意</NButton>
                      <NButton v-if="canRespondXiangqiUndo" size="small" secondary @click="respondXiangqiUndo(false)">拒绝</NButton>
                    </template>
                    <template v-else>
                      <NButton size="small" secondary :disabled="!canRequestUndoXiangqi" @click="requestXiangqiUndo">悔棋</NButton>
                      <NButton size="small" secondary type="error" :disabled="!canResignXiangqi" @click="resignXiangqi">投降</NButton>
                    </template>
                  </div>

                  <div class="xiangqi-log-strip">
                    <span v-for="move in activeXiangqiState?.moves.slice(-6) ?? []" :key="`${move.playerId}-${move.createdAt}`">
                      {{ move.playerName }} {{ move.pieceLabel }} {{ move.from.x + 1 }},{{ move.from.y + 1 }} → {{ move.to.x + 1 }},{{ move.to.y + 1 }}{{ move.capturedLabel ? ` 吃${move.capturedLabel}` : '' }}
                    </span>
                  </div>
                  <div v-if="activeXiangqiState?.phase === 'ended'" class="settlement-overlay xiangqi-settlement">
                    <div class="settlement-panel">
                      <div class="settlement-kicker">本局结算</div>
                      <h3>{{ activeXiangqiState?.winnerName ? `${activeXiangqiState.winnerName} 获胜` : '本局结束' }}</h3>
                      <div class="settlement-list">
                        <div v-for="player in xiangqiSettlementRows" :key="player.deviceId" class="settlement-row" :class="{ winner: activeXiangqiState?.winnerDeviceId === player.deviceId }">
                          <div class="settlement-player">
                            <span class="xiangqi-mini-piece" :class="player.side">{{ player.side === 'black' ? '黑' : '红' }}</span>
                            <span>{{ player.deviceId === myDeviceId ? `我 · ${player.nickname}` : player.nickname }}</span>
                          </div>
                          <NTag size="small" :bordered="false" :type="player.side === 'red' ? 'error' : 'default'">{{ xiangqiSideLabel(player.side) }}</NTag>
                          <strong>{{ player.result }}</strong>
                        </div>
                      </div>
                      <div class="settlement-actions">
                        <NButton v-if="activeGameRoom && isRoomHost()" type="primary" @click="roomPrimaryAction">再来一局</NButton>
                        <NButton secondary @click="leaveRoom">退出房间</NButton>
                      </div>
                    </div>
                  </div>
                </main>
                <aside class="game-room-panel xiangqi-room-panel">
                  <div class="room-chat-panel">
                    <div class="room-chat-head">房间聊天</div>
                    <div ref="roomChatPane" class="room-chat-list">
                      <div v-for="item in activeRoomChatMessages" :key="item.id" class="room-chat-msg" :class="{ mine: item.mine }">
                        <div class="room-chat-name">{{ item.sender }}</div>
                        <div class="room-chat-bubble">{{ item.content }}</div>
                      </div>
                    </div>
                    <div class="room-chat-composer">
                      <div class="emoji-wrap">
                        <button class="emoji-trigger" title="表情" @click="roomEmojiOpen = !roomEmojiOpen">☺</button>
                        <div v-if="roomEmojiOpen" class="emoji-panel room-emoji-panel">
                          <button v-for="emoji in emojiOptions" :key="emoji" @click="appendEmojiToRoomDraft(emoji)">{{ emoji }}</button>
                        </div>
                      </div>
                      <NInput v-model:value="roomChatDraft" placeholder="房间聊天" @keydown.enter="handleRoomChatEnter" />
                      <NButton type="primary" @click="sendRoomChat">发</NButton>
                    </div>
                  </div>
                </aside>
              </div>
              <div v-else class="game-catalog-board">
                <section class="game-catalog-hero">
                  <div class="game-catalog-icon">{{ activeGameDefinition.icon }}</div>
                  <div>
                    <h3>{{ activeGameDefinition.name }}排行榜</h3>
                    <p>{{ activeGameDefinition.description }}</p>
                  </div>
                  <NButton type="primary" @click="createRoomOpen = true">创建{{ activeGameDefinition.name }}房间</NButton>
                </section>
                <section class="game-catalog-leaderboard">
                  <NTabs
                    v-if="activeGameRoom?.gameType === 'minesweeper' || selectedGameType === 'minesweeper'"
                    v-model:value="selectedMinesweeperLeaderboardKey"
                    type="segment"
                    animated
                    class="minesweeper-leaderboard-tabs"
                  >
                    <NTabPane
                      v-for="difficulty in MINESWEEPER_DIFFICULTIES"
                      :key="difficulty.key"
                      :name="difficulty.key"
                      :tab="`${difficulty.label} · ${difficulty.mines} 雷`"
                    >
                      <div class="leaderboard-list minesweeper-rank-list">
                        <div class="leaderboard-table-head">
                          <span>名次</span>
                          <span>昵称</span>
                          <span>耗时</span>
                          <span>步数</span>
                        </div>
                        <div v-if="minesweeperLeaderboardRows.length === 0" class="leaderboard-empty">暂无记录</div>
                        <div v-for="(record, index) in minesweeperLeaderboardRows" :key="record.id" class="leaderboard-row">
                          <span class="leaderboard-rank">{{ index + 1 }}</span>
                          <strong>{{ record.nickname }}</strong>
                          <span>{{ formatMinesweeperElapsed(record.elapsedMs) }}</span>
                          <small>{{ record.moves }} 步</small>
                        </div>
                      </div>
                    </NTabPane>
                  </NTabs>
                  <div v-else class="leaderboard-list catalog-stats-list">
                    <div v-if="activeGameStatsRows.length === 0" class="leaderboard-empty">暂无战绩，完成一局后会出现在这里</div>
                    <div v-for="(record, index) in activeGameStatsRows" :key="record.id" class="leaderboard-row">
                      <span class="leaderboard-rank">{{ index + 1 }}</span>
                      <strong>{{ record.nickname }}</strong>
                      <span>{{ record.totalGames }} 局</span>
                      <small>{{ record.wins }} 胜 · 胜率 {{ formatWinRate(record) }}</small>
                    </div>
                  </div>
                </section>
              </div>
              <footer v-if="activeGameRoom?.gameType === 'doudizhu'" class="hand-zone">
                <div class="hand-actions">
                  <template v-if="activeDdzState?.phase === 'bidding'">
                    <NButton secondary :disabled="!isMyDdzTurn" @click="bidLandlord(false)">不叫</NButton>
                    <NButton type="primary" :disabled="!isMyDdzTurn" @click="bidLandlord(true)">叫地主</NButton>
                  </template>
                  <template v-else>
                    <NButton secondary :disabled="!canPassDdz" @click="passTurn">不要</NButton>
                    <NButton type="primary" :disabled="!canPlaySelectedCards" @click="playSelectedCards">出牌</NButton>
                  </template>
                </div>
                <div class="hand-cards">
                  <div
                    v-for="card in myDdzHand"
                    :key="card.id"
                    class="poker-card hand-card"
                    :class="{ red: card.red, selected: selectedCardIds.includes(card.id) }"
                    @click="toggleCard(card.id)"
                  >
                    {{ card.label }}
                  </div>
                </div>
              </footer>
            </section>
            <section v-else-if="activeSection === 'devices'" class="workspace-view device-address-book">
              <div class="workspace-header">
                <h2>设备通讯录</h2>
                <p>左侧选择设备或频道，在这里查看详细信息和操作。</p>
              </div>
              <div class="device-detail-shell">
                <div v-if="selectedPeerDetail" class="device-profile-panel">
                  <div class="device-detail-head large">
                    <img v-if="avatarImage(selectedPeerDetail.avatar)" class="avatar-image peer-avatar large-avatar" :src="avatarImage(selectedPeerDetail.avatar)" alt="设备头像" />
                    <NAvatar v-else :size="56" class="peer-avatar">{{ firstLetter(peerDisplayName(selectedPeerDetail)) }}</NAvatar>
                    <div>
                      <h3>{{ peerDisplayName(selectedPeerDetail) }}</h3>
                      <p><span class="presence-dot" :class="{ online: selectedPeerDetail.online }"></span>{{ selectedPeerDetail.online ? "在线" : "离线" }}</p>
                    </div>
                  </div>
                  <div class="device-detail-grid wide">
                    <span>IP 地址</span><strong>{{ selectedPeerDetail.address }}</strong>
                    <span>端口</span><strong>{{ selectedPeerDetail.port }}</strong>
                    <span>MAC 地址</span><strong>{{ selectedPeerDetail.device_id }}</strong>
                    <span>昵称</span><strong>{{ selectedPeerDetail.nickname }}</strong>
                    <span>昵称限制</span><strong>{{ selectedPeerDetail.nickname_locked ? "禁止本地修改" : "允许本地修改" }}</strong>
                    <span>客户端</span><strong>{{ peerClientKindLabel(selectedPeerDetail) }}</strong>
                    <span>软件版本</span><strong>{{ peerBuildVersionLabel(selectedPeerDetail) }}</strong>
                    <span>构建时间</span><strong>{{ peerBuildTimeLabel(selectedPeerDetail) }}</strong>
                    <span>支持能力</span><strong>{{ peerSupportsFullFeatures(selectedPeerDetail) ? "告警、聊天、频道、游戏、文件" : "桌宠告警" }}</strong>
                    <span>最近在线</span><strong>{{ peerLastSeenLabel(selectedPeerDetail) }}</strong>
                  </div>
                  <div class="device-note-editor">
                    <NFormItem label="设备备注" :show-feedback="false">
                      <NInput v-model:value="peerNoteDraft" maxlength="32" clearable placeholder="仅保存在本机，用于识别设备" @keyup.enter="saveSelectedPeerNote" />
                    </NFormItem>
                    <NButton secondary type="primary" @click="saveSelectedPeerNote">保存备注</NButton>
                  </div>
                  <div v-if="superAdminEnabled" class="admin-rename-box">
                    <NFormItem label="超管修改设备昵称">
                      <NInput v-model:value="adminNicknameDraft" maxlength="24" clearable />
                    </NFormItem>
                    <NCheckbox v-model:checked="adminNicknameLockAfterIssue">
                      下发后禁止对方本地修改昵称
                    </NCheckbox>
                    <NButton block type="warning" :disabled="!selectedPeerDetail.online || !adminNicknameDraft.trim()" @click="adminRenameSelectedPeer">
                      下发昵称修改
                    </NButton>
                    <NButton block secondary type="primary" :disabled="!selectedPeerDetail.online" @click="adminUseSystemUsernameForSelectedPeer">
                      改为电脑登录用户名
                    </NButton>
                    <NButton block secondary type="warning" :disabled="!selectedPeerDetail.online || !adminNicknameDraft.trim()" @click="adminUnlockSelectedPeerNickname">
                      解除昵称修改限制
                    </NButton>
                    <NButton block type="warning" @click="openSimulationModal">超管模拟发送</NButton>
                    <NText depth="3">目标设备在线时会立即更新本机昵称，并通过在线广播同步给局域网。</NText>
                  </div>
                  <div class="device-detail-actions">
                    <NButton type="primary" :disabled="!selectedPeerDetail.online && peerSupportsFullFeatures(selectedPeerDetail)" @click="startDirectChat(selectedPeerDetail)">
                      {{ peerSupportsFullFeatures(selectedPeerDetail) ? "发起单聊" : "查看历史" }}
                    </NButton>
                    <NButton secondary type="error" @click="deleteSelectedPeer">删除设备</NButton>
                  </div>
                </div>
                <div v-else-if="selectedDeviceChannelDetail" class="device-profile-panel">
                  <div class="device-detail-head large">
                    <NAvatar :size="56" class="conversation-avatar">{{ selectedDeviceChannelDetail.is_private ? "私" : "局" }}</NAvatar>
                    <div>
                      <h3>{{ selectedDeviceChannelDetail.title }}</h3>
                      <p>{{ selectedDeviceChannelDetail.is_private ? "私有加密频道" : "局域网公开频道" }}</p>
                    </div>
                  </div>
                  <div class="device-detail-grid wide">
                    <span>频道类型</span><strong>{{ selectedDeviceChannelDetail.is_private ? "私有频道" : "公开频道" }}</strong>
                    <span>创建人</span><strong>{{ selectedDeviceChannelOwnerName }}</strong>
                    <span>频道 ID</span><strong>{{ selectedDeviceChannelDetail.id }}</strong>
                    <span>成员数量</span><strong>{{ selectedDeviceChannelMembers.length }}</strong>
                    <span>更新时间</span><strong>{{ formatTime(selectedDeviceChannelDetail.updated_at) }}</strong>
                  </div>
                  <div class="channel-detail-members">
                    <div class="channel-detail-title">
                      <strong>频道成员</strong>
                      <span>{{ selectedDeviceChannelMembers.length }} 人</span>
                    </div>
                    <NEmpty v-if="selectedDeviceChannelMembers.length === 0" description="暂无成员" class="list-empty compact" />
                    <NList v-else hoverable clickable class="channel-member-list embedded">
                      <NListItem v-for="member in selectedDeviceChannelMembers" :key="member.device_id" class="device-item" :class="{ 'is-offline': !sameDeviceId(member.device_id, profile?.device_id) && !member.online }" @click="openMemberDevice(member)">
                        <NThing :title="memberDisplayName(member)" :description="memberSubtitle(member)">
                          <template #avatar>
                            <img v-if="avatarImage(member.avatar)" class="avatar-image peer-avatar" :src="avatarImage(member.avatar)" alt="成员头像" />
                            <NAvatar v-else class="peer-avatar">{{ firstLetter(memberDisplayName(member)) }}</NAvatar>
                          </template>
                          <template #header-extra>
                            <NTag v-if="'is_owner' in member && member.is_owner" size="small" :bordered="false" type="warning">群主</NTag>
                          </template>
                        </NThing>
                      </NListItem>
                    </NList>
                  </div>
                  <div class="device-detail-actions">
                    <NButton secondary @click="enterSelectedDeviceChannel">进入频道</NButton>
                    <NButton v-if="canManageSelectedDeviceChannel" type="primary" @click="inviteSelectedDeviceChannelMembers">邀请成员</NButton>
                    <NButton v-if="canManageSelectedDeviceChannel" secondary type="error" @click="dissolveSelectedDeviceChannel">解散频道</NButton>
                  </div>
                </div>
                <NEmpty v-else description="从左侧选择设备或频道" class="device-detail-empty">
                  <template #extra>
                    <NText depth="3">设备会自动发现；频道包含局域网公开频道和私有频道。</NText>
                  </template>
                </NEmpty>
              </div>
            </section>
            <section v-else-if="activeSection === 'alerts'" class="workspace-view alert-dashboard-view">
              <div class="workspace-header">
                <h2>狼来了排行榜</h2>
                <p>按别人反馈后的真实概率排行，真实概率也会作为桌宠温度展示。</p>
              </div>
              <div class="alert-dashboard-grid">
                <NCard title="呱呱告警" size="small" class="quick-alert-card">
                  <NSpace vertical>
                    <NText depth="3">双击桌面桌宠也可以发送呱呱告警。当前告警会广播给在线设备，后续接入 LanChat Hub 后由 Hub 转发。</NText>
                    <NInput v-model:value="quickAlertDraft" maxlength="60" clearable placeholder="例如：快来处理一下" />
                    <NButton type="error" block @click="sendPetQuickAlert(petAlertMode)">{{ quickAlertDraft || "呱呱~呱~~" }}</NButton>
                  </NSpace>
                </NCard>
                <NCard title="狼来了排行" size="small" class="alert-rank-card">
                  <div class="alert-rank-list">
                    <div class="alert-rank-head">
                      <span>名次</span>
                      <span>人员</span>
                      <span>真实度</span>
                      <span>反馈</span>
                    </div>
                    <div v-if="alertRankingRows.length === 0" class="leaderboard-empty">暂无告警反馈，收到或发送告警后会出现在这里</div>
                    <div v-for="(row, index) in alertRankingRows" :key="row.deviceId" class="alert-rank-row">
                      <span class="leaderboard-rank">{{ index + 1 }}</span>
                      <strong>{{ row.deviceId === profile?.device_id ? `我 · ${row.nickname}` : row.nickname }}</strong>
                      <span class="alert-temperature">{{ row.probability === null ? '待确认' : `${row.probability}%` }}</span>
                      <small>{{ row.real }} 真 / {{ row.falseCount }} 假 · {{ row.total }} 次告警</small>
                    </div>
                  </div>
                </NCard>
                <NCard title="最近告警" size="small" class="alert-history-card">
                  <div class="alert-history-list">
                    <div v-if="alertRecords.length === 0" class="leaderboard-empty">暂无告警记录</div>
                    <div v-for="alert in alertRecords.slice(0, 10)" :key="alert.alertId" class="alert-history-row" :class="{ pending: alert.incoming && !alert.handled }">
                      <div>
                        <strong>{{ alert.senderDeviceId === profile?.device_id ? `我 · ${alert.senderNickname}` : alert.senderNickname }}</strong>
                        <span>{{ alert.content }}</span>
                        <NTag v-if="simulationLabel(alert.simulation)" size="tiny" :bordered="false" type="warning">{{ simulationLabel(alert.simulation) }}</NTag>
                      </div>
                      <div class="alert-history-meta">
                        <NTag size="small" :bordered="false" :type="alertTruthScore(alert, nowTick).feedbackCount === 0 ? 'default' : alertTruthScore(alert, nowTick).probability >= 60 ? 'success' : 'error'">
                          {{ alertProbabilityLabel(alert) }}
                        </NTag>
                        <small>{{ formatTime(alert.createdAt) }}</small>
                      </div>
                    </div>
                  </div>
                </NCard>
              </div>
            </section>
            <section v-else class="workspace-view settings-view">
              <div class="settings-layout">
                <nav class="settings-subnav" aria-label="设置分类">
                  <button
                    type="button"
                    :class="{ active: settingsCategory === 'basic' }"
                    :aria-current="settingsCategory === 'basic' ? 'page' : undefined"
                    @click="settingsCategory = 'basic'"
                  >
                    基础设置
                  </button>
                  <button
                    type="button"
                    :class="{ active: settingsCategory === 'pet' }"
                    :aria-current="settingsCategory === 'pet' ? 'page' : undefined"
                    @click="settingsCategory = 'pet'"
                  >
                    桌宠设置
                  </button>
                  <button
                    v-if="superAdminEnabled"
                    type="button"
                    :class="{ active: settingsCategory === 'admin' }"
                    :aria-current="settingsCategory === 'admin' ? 'page' : undefined"
                    @click="settingsCategory = 'admin'"
                  >
                    超管通知
                  </button>
                </nav>
                <div class="settings-content">
                  <div class="workspace-header">
                    <h2 class="settings-title">设置<button class="settings-secret-trigger" type="button" aria-label="设置" @click="handleSuperAdminTap">✦</button></h2>
                    <p>{{ settingsCategory === 'basic' ? '管理本机资料、网络、主题和语言。' : settingsCategory === 'pet' ? '管理桌宠资源、行为与告警能力。' : '下发公告并审核设备提交的确认。' }}</p>
                  </div>
                  <div class="settings-grid">
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
                <NCard v-if="settingsCategory === 'basic'" title="网络修复" size="small">
                  <NSpace vertical>
                    <NText depth="3">{{ networkRepairDescription }}</NText>
                    <NText v-if="canRepairWindowsNetwork" depth="3">会请求管理员权限，并放行 LanChat.exe、TCP 18145、UDP 18146、UDP 5353。</NText>
                    <NButton v-if="canRepairWindowsNetwork" block type="primary" :loading="networkRepairing" @click="store.repairNetwork">网络修复</NButton>
                    <NAlert v-if="networkRepairStatus" type="success" title="已打开修复窗口">
                      {{ networkRepairStatus }}
                    </NAlert>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="通话权限" size="small">
                  <NSpace vertical>
                    <NText depth="3">首次发起或接听通话会自动申请权限；若此前拒绝，可在这里重新授权。</NText>
                    <NSpace>
                      <NButton secondary type="primary" @click="requestCallDevicePermission('audio')">重新授权麦克风</NButton>
                      <NButton secondary type="primary" @click="requestCallDevicePermission('video')">重新授权摄像头</NButton>
                    </NSpace>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="图片缓存" size="small">
                  <NSpace vertical>
                    <NText depth="3">带预览能力的图片会自动下载到本机缓存，聊天历史仍可在发送方离线后查看。</NText>
                    <div class="update-info-grid">
                      <span>缓存文件</span><strong>{{ previewMediaCacheInfo?.fileCount ?? 0 }} 个</strong>
                      <span>占用空间</span><strong>{{ formatFileSize(previewMediaCacheInfo?.totalBytes) }}</strong>
                    </div>
                    <NButton secondary type="warning" :loading="previewMediaCacheClearing" @click="clearImagePreviewCache">清理图片缓存</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'basic'" title="版本更新" size="small">
                  <NSpace vertical>
                    <div class="setting-switch-row">
                      <div>
                        <strong>自动检查更新</strong>
                        <p>每次启动都会检查，运行期间每 12 小时复检一次。强制更新版本必须安装后才能继续使用。</p>
                      </div>
                      <NTag type="success" :bordered="false">已启用</NTag>
                    </div>
                    <div class="update-info-grid">
                      <span>当前版本</span><strong>{{ localVersionLabel }}</strong>
                      <span>检查状态</span><NTag size="small" :type="updateStatusType" :bordered="false">{{ updateStatusLabel }}</NTag>
                      <span>最新版本</span><strong>{{ updateInfo?.latestVersion ?? "未知" }}</strong>
                      <span>上次检查</span><strong>{{ formatDateTime(updateInfo?.checkedAt) }}</strong>
                    </div>
                    <NAlert v-if="updateError" type="error" title="检查失败">{{ updateError }}</NAlert>
                    <NAlert v-else-if="updateInfo?.updateAvailable" type="warning" title="发现新版本">
                      {{ updateInfo.title }}
                    </NAlert>
                    <pre v-if="updateInfo" class="update-notes compact">{{ updateNotesPreview(updateInfo.notes) }}</pre>
                    <div v-if="nativeUpdateInstalling" class="update-progress-panel compact">
                      <NProgress type="line" :percentage="nativeUpdateProgressPercent" :height="8" processing />
                      <span>{{ nativeUpdateProgressLabel }}</span>
                    </div>
                    <NSpace>
                      <NButton type="primary" :loading="updateChecking" @click="checkUpdates(true)">检查更新</NButton>
                      <NButton secondary :disabled="!preferredUpdateUrl" @click="openPreferredUpdateUrl">下载更新</NButton>
                      <NButton quaternary :disabled="!updateInfo" @click="openReleasePage">Release 页面</NButton>
                    </NSpace>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="下发超管通知" size="small">
                  <NSpace vertical>
                    <NText depth="3">指定设备会直连送达；全员模式会为每台在线设备独立创建一条通知，便于逐人审核。</NText>
                    <NButton type="warning" @click="openAdminNotificationModal">下发通知</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'admin' && superAdminEnabled" title="超管通知审核" size="small">
                  <div class="admin-notification-review-list">
                    <NEmpty v-if="adminNotifications.filter((item) => item.issued_by_device_id === profile?.device_id).length === 0" description="暂无本机下发的通知" />
                    <div v-else-if="adminNotifications.some((item) => item.issued_by_device_id === profile?.device_id && item.display_mode === 'requires_confirmation' && item.status === 'submitted')" class="admin-notification-review-actions"><NButton size="small" type="success" :loading="adminNotificationBulkProcessing" @click="decideAllSubmittedAdminNotifications('approved')">一键通过待审核</NButton><NButton size="small" type="error" secondary :loading="adminNotificationBulkProcessing" @click="decideAllSubmittedAdminNotifications('rejected')">一键拒绝待审核</NButton></div>
                    <div v-for="notification in adminNotifications.filter((item) => item.issued_by_device_id === profile?.device_id).slice(0, 20)" :key="notification.notification_id" class="admin-notification-review-row">
                      <div class="admin-notification-review-device"><img v-if="avatarImage(adminNotificationTargetDetail(notification)?.avatar)" class="avatar-image compact-avatar" :src="avatarImage(adminNotificationTargetDetail(notification)?.avatar)" alt="设备头像" /><NAvatar v-else :size="28" class="peer-avatar">{{ firstLetter(adminNotificationTargetDetail(notification)?.nickname ?? '?') }}</NAvatar><div><strong>{{ notification.title }}</strong><small>昵称：{{ adminNotificationTargetDetail(notification)?.nickname ?? '未知设备' }} · IP：{{ adminNotificationTargetDetail(notification)?.address ?? '未知' }} · MAC：{{ notification.target_device_id }} · {{ notification.status }}</small></div></div>
                      <NSpace :size="6"><NButton size="small" quaternary @click="openAdminNotificationDetail(notification)">详情</NButton><template v-if="notification.display_mode === 'requires_confirmation' && notification.status === 'submitted'"><NButton size="small" type="success" @click="decideAdminNotification(notification, 'approved')">通过</NButton><NButton size="small" type="error" secondary @click="decideAdminNotification(notification, 'rejected')">拒绝</NButton></template><NButton v-else-if="notification.display_mode === 'requires_confirmation' && ['pending','expired_locked','rejected'].includes(notification.status)" size="small" tertiary type="warning" @click="decideAdminNotification(notification, 'revoked')">撤销放行</NButton></NSpace>
                    </div>
                  </div>
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
                    <NAlert v-if="desktopPetError" type="error" title="桌宠资源操作失败">{{ desktopPetError }}</NAlert>
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
                <NCard v-if="settingsCategory === 'pet' && superAdminEnabled" title="告警真实度" size="small">
                  <NSpace vertical>
                    <NText depth="3">选择某个告警发送人后，会清空所有在线设备里该人员的可信度反馈记录。</NText>
                    <NSelect v-model:value="alertTrustResetTargetId" :options="adminDeviceOptions" filterable clearable placeholder="选择要清空可信度的人员" />
                    <NButton secondary type="error" :disabled="!alertTrustResetTargetId" @click="resetAlertCredibilityForPeer">清空该用户可信度</NButton>
                    <NButton type="error" @click="resetAllAlertCredibilityRecords">一键清空狼来了排行榜</NButton>
                  </NSpace>
                </NCard>
                <NCard v-if="settingsCategory === 'pet' && superAdminEnabled" title="报警模式下发" size="small">
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
                <NCard v-if="settingsCategory === 'pet' && superAdminEnabled" title="狼来了推送阈值下发" size="small">
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
                <NAlert v-if="error" type="error" title="操作失败">
                  {{ error }}
                </NAlert>
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
        <NModal v-model:show="createRoomOpen" preset="card" title="创建房间" class="create-room-modal">
          <div class="create-room-form" @click="createRoomGameMenuOpen = false">
            <div class="create-room-game-dropdown" @click.stop>
              <button class="create-room-game-select" type="button" @click="createRoomGameMenuOpen = !createRoomGameMenuOpen">
                <span class="create-room-game-icon">{{ selectedCreateRoomGame.icon }}</span>
                <span class="create-room-game-copy">
                  <strong>{{ selectedCreateRoomGame.name }}</strong>
                  <small>{{ selectedCreateRoomGame.minPlayers }}-{{ selectedCreateRoomGame.maxPlayers }} 人 · {{ selectedCreateRoomGame.description }}</small>
                </span>
                <span class="create-room-caret" :class="{ open: createRoomGameMenuOpen }">⌄</span>
              </button>
              <div v-if="createRoomGameMenuOpen" class="create-room-game-menu">
                <button
                  v-for="game in gameRegistry"
                  :key="game.type"
                  class="create-room-game-option"
                  :class="{ active: selectedGameType === game.type }"
                  type="button"
                  @click="selectCreateRoomGame(game.type)"
                >
                  <span class="create-room-game-icon">{{ game.icon }}</span>
                  <span class="create-room-game-copy">
                    <strong>{{ game.name }}</strong>
                    <small>{{ game.minPlayers }}-{{ game.maxPlayers }} 人 · {{ game.description }}</small>
                  </span>
                </button>
              </div>
            </div>
            <NInput v-model:value="roomNameDraft" size="medium" maxlength="24" placeholder="房间名称" />
            <NButton block type="primary" @click="createGameRoom">创建房间</NButton>
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
                  <strong>{{ recipientPickerMode === 'gameInvite' ? '发送给设备' : '选择频道成员' }}</strong>
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
              <section v-if="recipientPickerMode === 'gameInvite'" class="recipient-picker-section">
                <div class="recipient-section-head">
                  <strong>发送到频道</strong>
                  <span>{{ selectedRecipientConversationIds.length }} 已选</span>
                </div>
                <div v-if="pickerConversationOptions.length > 0" class="recipient-list">
                  <button
                    v-for="conversation in pickerConversationOptions"
                    :key="conversation.id"
                    class="recipient-list-row"
                    :class="{ active: selectedRecipientConversationIds.includes(conversation.id) }"
                    type="button"
                    @click="toggleRecipientConversation(conversation.id)"
                  >
                    <span class="recipient-channel-icon">{{ conversation.is_private ? '私' : '局' }}</span>
                    <span class="recipient-list-main">
                      <strong>{{ conversation.title }}</strong>
                      <small>{{ conversation.is_private ? '私有加密频道' : '局域网公开频道' }}</small>
                    </span>
                    <span class="recipient-list-check">{{ selectedRecipientConversationIds.includes(conversation.id) ? '✓' : '' }}</span>
                  </button>
                </div>
                <div v-else class="recipient-empty">暂无可发送的频道</div>
              </section>
            </div>
            <div class="recipient-picker-footer">
              <NText depth="3">
                {{ recipientPickerMode === 'gameInvite' ? '游戏邀请会以卡片消息发送。' : '私有频道消息会广播投递，但只有持有频道密钥的成员能解密。' }}
              </NText>
              <NSpace justify="end">
                <NButton secondary @click="recipientPickerOpen = false">取消</NButton>
                <NButton type="primary" :disabled="recipientConfirmDisabled" @click="confirmRecipientPicker">
                  {{ recipientPickerMode === 'gameInvite' ? '发送邀请' : recipientPickerMode === 'privateChannelCreate' ? '创建频道' : '邀请加入' }}
                </NButton>
              </NSpace>
            </div>
          </div>
        </NModal>
        <NModal v-model:show="leaderboardOpen" preset="card" :title="leaderboardTitle" class="leaderboard-modal">
          <NTabs
            v-if="activeGameRoom?.gameType === 'minesweeper'"
            v-model:value="selectedMinesweeperLeaderboardKey"
            type="segment"
            animated
            class="minesweeper-leaderboard-tabs"
          >
            <NTabPane
              v-for="difficulty in MINESWEEPER_DIFFICULTIES"
              :key="difficulty.key"
              :name="difficulty.key"
              :tab="`${difficulty.label} · ${difficulty.mines} 雷`"
            >
              <div class="leaderboard-list minesweeper-rank-list">
                <div class="leaderboard-table-head">
                  <span>名次</span>
                  <span>昵称</span>
                  <span>耗时</span>
                  <span>步数</span>
                </div>
                <div v-if="minesweeperLeaderboardRows.length === 0" class="leaderboard-empty">暂无记录</div>
                <div v-for="(record, index) in minesweeperLeaderboardRows" :key="record.id" class="leaderboard-row">
                  <span class="leaderboard-rank">{{ index + 1 }}</span>
                  <strong>{{ record.nickname }}</strong>
                  <span>{{ formatMinesweeperElapsed(record.elapsedMs) }}</span>
                  <small>{{ record.moves }} 步</small>
                </div>
              </div>
            </NTabPane>
          </NTabs>
          <div v-else class="leaderboard-list">
            <div v-if="activeGameStatsRows.length === 0" class="leaderboard-empty">暂无战绩，完成一局后会出现在这里</div>
            <div v-for="(record, index) in activeGameStatsRows" :key="record.id" class="leaderboard-row">
              <span class="leaderboard-rank">{{ index + 1 }}</span>
              <strong>{{ record.nickname }}</strong>
              <span>{{ record.totalGames }} 局</span>
              <small>{{ record.wins }} 胜 · 胜率 {{ formatWinRate(record) }}</small>
            </div>
          </div>
        </NModal>
      </NMessageProvider>
  </NConfigProvider>
</template>

<style scoped>
.chat-call-actions { margin-left: auto; padding-right: 8px; }
.private-call-float { position: fixed; z-index: 2500; top: 64px; right: 16px; width: min(276px, calc(100vw - 32px)); padding: 9px 11px; border: 1px solid var(--panel-border); border-radius: 8px; background: var(--panel-bg, #ffffff); box-shadow: 0 10px 28px rgba(18, 36, 52, 0.16); }
.private-call-float.expanded { width: min(360px, calc(100vw - 32px)); padding: 14px; }
.private-call-float.video.expanded { width: min(440px, calc(100vw - 32px)); }
.private-call-panel { width: 100%; padding: 2px; }
.private-call-title { display: flex; align-items: center; justify-content: space-between; gap: 9px; margin-bottom: 8px; cursor: move; user-select: none; }
.private-call-float.expanded .private-call-title { margin-bottom: 14px; }
.private-call-summary { display: grid; min-width: 0; gap: 2px; }
.private-call-title strong { color: #1f2937; font-size: 16px; }
.private-call-title span { flex: 0 0 auto; color: #768397; font-size: 12px; }
.private-call-summary strong, .private-call-summary span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.private-call-toggle { flex: 0 0 auto; width: 26px; height: 26px; border: 0; border-radius: 5px; background: #edf2f5; color: #526274; cursor: pointer; font-size: 16px; line-height: 1; }
.private-call-videos { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) 104px; gap: 9px; aspect-ratio: 4 / 3; min-height: 0; margin-bottom: 16px; padding: 9px; border: 1px solid #e5eaf1; border-radius: 8px; background: #f5f7fa; }
.private-call-videos video { width: 100%; height: 100%; min-height: 0; border-radius: 7px; background: #202938; object-fit: cover; }
.private-call-videos video:last-child { height: 88px; align-self: end; box-shadow: 0 4px 14px rgba(29, 42, 61, 0.2); }
.private-call-videos.audio { display: flex; align-items: center; justify-content: center; min-height: 210px; aspect-ratio: auto; }
.private-call-audio-profile { display: grid; justify-items: center; gap: 9px; text-align: center; }
.private-call-audio-avatar { display: grid; place-items: center; width: 102px; height: 102px; border: 8px solid #e6f4ff; border-radius: 50%; background: #1677ff; color: #fff; font-size: 34px; box-shadow: 0 8px 20px rgba(22, 119, 255, 0.18); }
.private-call-audio-avatar-image { width: 102px; height: 102px; border: 4px solid #e6f4ff; border-radius: 50%; object-fit: cover; box-shadow: 0 8px 20px rgba(22, 119, 255, 0.18); }
.private-call-audio-profile strong { color: #1f2937; font-size: 17px; }
.private-call-audio-profile span { color: #768397; font-size: 12px; }
</style>











































































