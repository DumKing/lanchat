import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api } from "../services/tauri-api";
import { registerLanChatEvents } from "../services/event-bus";
import type { AdminAlertMode, AdminDiscoMode, ChannelMember, ChannelNoticePayload, Conversation, DebugLog, GameFrame, Message, Peer, PetAlertMode, PrivateChannelInvitePayload, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset } from "../types/lanchat";

export const DEFAULT_GROUP_ID = "lan-room";

export const useLanChatStore = defineStore("lanchat", () => {
  const profile = ref<Profile | null>(null);
  const peers = ref<Peer[]>([]);
  const conversations = ref<Conversation[]>([]);
  const messagesByConversation = ref<Record<string, Message[]>>({});
  const channelMembersByConversation = ref<Record<string, ChannelMember[]>>({});
  const channelMutedByConversation = ref<Record<string, boolean>>({});
  const activeConversationId = ref(DEFAULT_GROUP_ID);
  const loading = ref(false);
  const error = ref("");
  const manualAddress = ref("");
  const manualPort = ref(18145);
  const draft = ref("");
  const networkRepairing = ref(false);
  const networkRepairStatus = ref("");
  const debugEnabled = ref(readSavedDebugEnabled());
  const debugLogs = ref<DebugLog[]>([]);
  const unreadByConversation = ref<Record<string, number>>({});
  const latestIncomingMessage = ref<Message | null>(null);
  const latestGameFrame = ref<GameFrame | null>(null);
  const latestChannelNotice = ref<ChannelNoticePayload | null>(null);
  const latestQuickAlert = ref<QuickAlert | null>(null);
  const latestQuickAlertFeedback = ref<QuickAlertFeedback | null>(null);
  const latestQuickAlertTrustReset = ref<QuickAlertTrustReset | null>(null);
  const latestAdminDiscoMode = ref<AdminDiscoMode | null>(null);
  const latestAdminAlertMode = ref<AdminAlertMode | null>(null);
  let peerRefreshTimer: number | null = null;

  const activeConversation = computed(() =>
    conversations.value.find((item) => item.id === activeConversationId.value) ?? null,
  );

  const activeMessages = computed(
    () => messagesByConversation.value[activeConversationId.value] ?? [],
  );

  const onlinePeers = computed(() => peers.value.filter((peer) => peer.online));
  const totalUnread = computed(() => Object.values(unreadByConversation.value).reduce((sum, value) => sum + value, 0));

  const activePeer = computed(() => {
    const conversation = activeConversation.value;
    if (!conversation || conversation.kind !== "direct") return null;
    const peerId = conversation.peer_device_id ?? conversation.id;
    return peers.value.find((peer) => peer.device_id === peerId) ?? null;
  });

  const canSendActive = computed(() => {
    const conversation = activeConversation.value;
    if (!conversation) return false;
    if (conversation.kind === "direct") return activePeer.value?.online === true;
    if (channelMutedByConversation.value[conversation.id] === true) return false;
    const selfMember = channelMembersByConversation.value[conversation.id]?.find((member) => member.device_id === profile.value?.device_id);
    return selfMember?.muted !== true;
  });

  async function initialize() {
    loading.value = true;
    error.value = "";
    try {
      profile.value = await api.getProfile();
      await Promise.all([refreshPeers(), refreshConversations()]);
      await Promise.all([loadMessages(activeConversationId.value), refreshChannelMute(activeConversationId.value)]);
      startPeerRefreshTimer();
      await registerLanChatEvents({
        onPeerOnline(peer) {
          pushDebugLog({ ts: Date.now(), level: "info", scope: "frontend", message: "收到 peer_online 事件", detail: `${peer.nickname} ${peer.address}:${peer.port}` });
          const previous = peers.value.find((item) => item.device_id === peer.device_id);
          upsertPeer(peer);
          if (!previous || !previous.online) {
            void addSystemNotice(DEFAULT_GROUP_ID, `${peer.nickname} 上线了`);
          }
          refreshConversations();
        },
        onProfileUpdated(nextProfile) {
          profile.value = nextProfile;
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "admin", message: "本机昵称已被超管修改", detail: nextProfile.nickname });
        },
        onPeerOffline(deviceId) {
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "frontend", message: "收到 peer_offline 事件", detail: deviceId });
          const previous = peers.value.find((peer) => peer.device_id === deviceId);
          peers.value = peers.value.map((peer) =>
            peer.device_id === deviceId ? { ...peer, online: false } : peer,
          );
          if (previous?.online) {
            void addSystemNotice(DEFAULT_GROUP_ID, `${previous.nickname} 下线了`);
          }
        },
        onMessageReceived(message) {
          appendOrUpdateMessage(message);
          latestIncomingMessage.value = message;
          if (message.conversation_id !== activeConversationId.value) {
            unreadByConversation.value = {
              ...unreadByConversation.value,
              [message.conversation_id]: (unreadByConversation.value[message.conversation_id] ?? 0) + 1,
            };
          }
          refreshConversations();
        },
        onMessageStatusChanged(payload) {
          if (isMessage(payload)) {
            appendOrUpdateMessage(payload);
          } else if (isAck(payload)) {
            updateMessageStatus(payload.message_id, "delivered");
          }
        },
        onDebugLog(entry) {
          pushDebugLog(entry);
        },
        onGameFrame(frame) {
          latestGameFrame.value = frame;
          pushDebugLog({ ts: Date.now(), level: "info", scope: "game", message: "收到游戏消息", detail: `${frame.game} ${frame.kind}` });
        },
        async onPrivateChannelInvited(conversation) {
          await refreshConversations();
          await Promise.all([loadChannelMembers(conversation.id), refreshChannelMute(conversation.id)]);
          pushDebugLog({ ts: Date.now(), level: "info", scope: "channel", message: "收到私有频道邀请", detail: conversation.title });
        },
        async onPrivateChannelChanged(conversationId) {
          await refreshConversations();
          await Promise.all([loadChannelMembers(conversationId), refreshChannelMute(conversationId)]);
          if (!conversations.value.some((item) => item.id === activeConversationId.value)) {
            activeConversationId.value = DEFAULT_GROUP_ID;
            await Promise.all([loadMessages(activeConversationId.value), refreshChannelMute(activeConversationId.value)]);
          }
          pushDebugLog({ ts: Date.now(), level: "info", scope: "channel", message: "频道成员状态已更新", detail: conversationId });
        },
        async onChannelNoticeUpdated(payload) {
          applyChannelNotice(payload);
          await addSystemNotice(payload.conversation_id, `${payload.updated_by_nickname} 更新了群公告`);
        },
        onMessageRecalled(message) {
          appendOrUpdateMessage(message);
        },
        onQuickAlertReceived(alert) {
          latestQuickAlert.value = alert;
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "alert", message: "收到快捷告警", detail: `${alert.sender_nickname} ${alert.content}` });
        },
        onQuickAlertFeedbackReceived(feedback) {
          latestQuickAlertFeedback.value = feedback;
          pushDebugLog({ ts: Date.now(), level: "info", scope: "alert", message: "收到快捷告警反馈", detail: `${feedback.alert_id} ${feedback.responder_nickname} ${feedback.result}` });
        },
        onQuickAlertTrustResetReceived(reset) {
          latestQuickAlertTrustReset.value = reset;
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "alert", message: "收到告警可信度重置", detail: `${reset.target_device_id} ${reset.issued_by_nickname}` });
        },
        onAdminDiscoModeReceived(mode) {
          latestAdminDiscoMode.value = mode;
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "admin", message: "收到蹦迪模式", detail: `${mode.issued_by_nickname} ${mode.duration_ms}ms` });
        },
        onAdminAlertModeReceived(mode) {
          latestAdminAlertMode.value = mode;
          pushDebugLog({ ts: Date.now(), level: "warn", scope: "admin", message: "收到报警模式下发", detail: `${mode.issued_by_nickname} ${mode.mode}` });
        },
      });
    } catch (err) {
      error.value = stringifyError(err);
    } finally {
      loading.value = false;
    }
  }

  function startPeerRefreshTimer() {
    if (peerRefreshTimer !== null || typeof window === "undefined") return;
    peerRefreshTimer = window.setInterval(() => {
      refreshPeers().catch(() => undefined);
    }, 10_000);
  }
  async function refreshPeers() {
    const next = await api.listPeers();
    peers.value = dedupePeers(next);
    pushDebugLog({ ts: Date.now(), level: "info", scope: "frontend", message: "刷新设备列表", detail: `${peers.value.filter((peer) => peer.online).length}/${peers.value.length} 在线` });
  }

  async function refreshConversations() {
    conversations.value = await api.listConversations();
    if (!conversations.value.some((item) => item.id === activeConversationId.value)) {
      activeConversationId.value = conversations.value[0]?.id ?? DEFAULT_GROUP_ID;
    }
  }

  async function loadMessages(conversationId: string) {
    messagesByConversation.value = {
      ...messagesByConversation.value,
      [conversationId]: await api.listMessages(conversationId),
    };
  }

  async function loadChannelMembers(conversationId: string) {
    const conversation = conversations.value.find((item) => item.id === conversationId);
    if (!conversation?.is_private) return;
    channelMembersByConversation.value = {
      ...channelMembersByConversation.value,
      [conversationId]: await api.listChannelMembers(conversationId),
    };
  }

  async function selectConversation(conversationId: string) {
    activeConversationId.value = conversationId;
    unreadByConversation.value = { ...unreadByConversation.value, [conversationId]: 0 };
    await Promise.all([loadMessages(conversationId), loadChannelMembers(conversationId), refreshChannelMute(conversationId)]);
  }


  async function refreshChannelMute(conversationId: string) {
    const conversation = conversations.value.find((item) => item.id === conversationId);
    if (conversation?.kind !== "group") {
      channelMutedByConversation.value = {
        ...channelMutedByConversation.value,
        [conversationId]: false,
      };
      return false;
    }
    try {
      const muted = await api.isChannelMuted(conversationId);
      channelMutedByConversation.value = {
        ...channelMutedByConversation.value,
        [conversationId]: muted,
      };
      return muted;
    } catch {
      channelMutedByConversation.value = {
        ...channelMutedByConversation.value,
        [conversationId]: false,
      };
      return false;
    }
  }
  async function createPrivateChannel(title: string, memberDeviceIds: string[]) {
    error.value = "";
    try {
      const conversation = await api.createPrivateChannel(title, memberDeviceIds);
      await refreshConversations();
      await selectConversation(conversation.id);
      return conversation;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function invitePrivateChannelMembers(conversationId: string, memberDeviceIds: string[], superAdmin = false) {
    error.value = "";
    try {
      const members = await api.invitePrivateChannelMembers(conversationId, memberDeviceIds, superAdmin);
      channelMembersByConversation.value = {
        ...channelMembersByConversation.value,
        [conversationId]: members,
      };
      await refreshConversations();
      return members;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function removePrivateChannelMember(conversationId: string, memberDeviceId: string, superAdmin = false) {
    error.value = "";
    try {
      const members = await api.removePrivateChannelMember(conversationId, memberDeviceId, superAdmin);
      channelMembersByConversation.value = { ...channelMembersByConversation.value, [conversationId]: members };
      await refreshConversations();
      return members;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function setPrivateChannelMemberMuted(conversationId: string, memberDeviceId: string, muted: boolean, superAdmin = false) {
    error.value = "";
    try {
      const members = await api.setPrivateChannelMemberMuted(conversationId, memberDeviceId, muted, superAdmin);
      channelMembersByConversation.value = { ...channelMembersByConversation.value, [conversationId]: members };
      await refreshConversations();
      return members;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function dissolvePrivateChannel(conversationId: string, superAdmin = false) {
    error.value = "";
    try {
      await api.dissolvePrivateChannel(conversationId, superAdmin);
      await refreshConversations();
      activeConversationId.value = DEFAULT_GROUP_ID;
      await Promise.all([loadMessages(activeConversationId.value), refreshChannelMute(activeConversationId.value)]);
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function adminMuteChannelMember(conversationId: string, targetDeviceId: string, muted: boolean) {
    error.value = "";
    try {
      await api.adminMuteChannelMember(conversationId, targetDeviceId, muted);
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function buildPrivateChannelInvite(conversationId: string, superAdmin = false) {
    error.value = "";
    try {
      return await api.buildPrivateChannelInviteCard(conversationId, superAdmin);
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function acceptPrivateChannelInvite(invite: PrivateChannelInvitePayload) {
    error.value = "";
    try {
      const conversation = await api.acceptPrivateChannelInvite(invite);
      await refreshConversations();
      await selectConversation(conversation.id);
      return conversation;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }

  async function addSystemNotice(conversationId: string, content: string) {
    try {
      const message = await api.saveSystemNotice(conversationId, content);
      appendOrUpdateMessage(message);
      await refreshConversations();
      return message;
    } catch (err) {
      pushDebugLog({ ts: Date.now(), level: "warn", scope: "frontend", message: "保存系统通知失败", detail: stringifyError(err) });
      return null;
    }
  }

  async function recallMessage(messageId: string) {
    const message = await api.recallMessage(messageId);
    appendOrUpdateMessage(message);
    return message;
  }

  function applyChannelNotice(payload: ChannelNoticePayload) {
    latestChannelNotice.value = payload;
    pushDebugLog({ ts: Date.now(), level: "info", scope: "channel", message: "收到频道公告更新", detail: `${payload.conversation_id} ${payload.updated_by_nickname}` });
  }
  async function saveProfile(nickname: string, listenPort: number, avatar?: string | null) {
    error.value = "";
    try {
      profile.value = await api.updateProfile(nickname, listenPort, avatar);
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function deletePeer(deviceId: string) {
    error.value = "";
    try {
      await api.deletePeer(deviceId);
      peers.value = peers.value.filter((peer) => peer.device_id !== deviceId);
      conversations.value = conversations.value.filter((conversation) => conversation.id !== deviceId);
      if (activeConversationId.value === deviceId) {
        activeConversationId.value = DEFAULT_GROUP_ID;
      }
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function adminRenamePeer(deviceId: string, nickname: string) {
    error.value = "";
    try {
      const peer = await api.adminRenamePeer(deviceId, nickname);
      upsertPeer(peer);
      await refreshConversations();
      return peer;
    } catch (err) {
      error.value = stringifyError(err);
      throw err;
    }
  }
  async function connectManualPeer() {
    error.value = "";
    try {
      const peer = await api.connectPeer(manualAddress.value, manualPort.value);
      upsertPeer(peer);
      await refreshConversations();
      await selectConversation(peer.device_id);
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function openDirect(peer: Peer) {
    await refreshConversations();
    await selectConversation(peer.device_id);
  }

  async function sendActiveMessage() {
    if (!canSendActive.value) {
      error.value = "对方已离线，不能发送私聊消息";
      return;
    }
    const content = draft.value.trim();
    if (!content) return;
    draft.value = "";
    const message = await sendMessageToConversation(activeConversationId.value, content);
    if (!message) {
      draft.value = content;
    }
  }

  async function sendMessageToConversation(conversationId: string, content: string) {
    const targetConversation = conversations.value.find((item) => item.id === conversationId);
    if (targetConversation?.kind === "direct") {
      const peerId = targetConversation.peer_device_id ?? targetConversation.id;
      const peer = peers.value.find((item) => item.device_id === peerId);
      if (peer?.online !== true) {
        error.value = "对方已离线，不能发送私聊消息";
        return null;
      }
    }
    error.value = "";
    try {
      const message = await api.sendMessage(conversationId, content);
      if (conversationId === activeConversationId.value) {
        appendOrUpdateMessage(message);
      }
      await refreshConversations();
      return message;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }


  async function sendFile(path: string) {
    if (!path) return;
    if (!canSendActive.value) {
      error.value = "对方已离线，不能发送私聊文件";
      return;
    }
    error.value = "";
    try {
      const message = await api.sendFileMessage(activeConversationId.value, path);
      appendOrUpdateMessage(message);
      await refreshConversations();
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function sendVoice(fileName: string, bytes: number[], durationMs: number) {
    if (!canSendActive.value) {
      error.value = "对方已离线，不能发送语音消息";
      return;
    }
    error.value = "";
    try {
      const message = await api.sendVoiceMessage(activeConversationId.value, fileName, bytes, durationMs);
      appendOrUpdateMessage(message);
      await refreshConversations();
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function sendGameFrame(targetDeviceId: string | null, frame: GameFrame) {
    error.value = "";
    try {
      await api.sendGameFrame(targetDeviceId, frame);
    } catch (err) {
      error.value = stringifyError(err);
    }
  }

  async function sendQuickAlert(content = "呱呱~呱~~", mode: PetAlertMode = "normal") {
    error.value = "";
    try {
      const alert = await api.sendQuickAlert(content, mode);
      latestQuickAlert.value = alert;
      pushDebugLog({ ts: Date.now(), level: "warn", scope: "alert", message: "快捷告警已发出", detail: `${alert.alert_id} ${alert.mode}` });
      return alert;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }

  async function sendQuickAlertFeedback(alertId: string, alertSenderDeviceId: string, result: "real" | "false") {
    error.value = "";
    try {
      const feedback = await api.sendQuickAlertFeedback(alertId, alertSenderDeviceId, result);
      latestQuickAlertFeedback.value = feedback;
      pushDebugLog({ ts: Date.now(), level: "info", scope: "alert", message: "快捷告警反馈已发送", detail: `${alertId} ${result}` });
      return feedback;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }

  async function resetQuickAlertCredibility(targetDeviceId: string) {
    error.value = "";
    try {
      const reset = await api.resetQuickAlertCredibility(targetDeviceId);
      latestQuickAlertTrustReset.value = reset;
      pushDebugLog({ ts: Date.now(), level: "warn", scope: "alert", message: "告警可信度重置已下发", detail: reset.target_device_id });
      return reset;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }

  async function sendAdminDiscoMode(targetDeviceId: string, durationMs = 120_000) {
    error.value = "";
    try {
      const mode = await api.sendAdminDiscoMode(targetDeviceId, durationMs);
      latestAdminDiscoMode.value = mode;
      pushDebugLog({ ts: Date.now(), level: "warn", scope: "admin", message: "蹦迪模式已下发", detail: mode.target_device_id });
      return mode;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }

  async function sendAdminAlertMode(targetDeviceId: string, mode: PetAlertMode) {
    error.value = "";
    try {
      const frame = await api.sendAdminAlertMode(targetDeviceId, mode);
      latestAdminAlertMode.value = frame;
      pushDebugLog({ ts: Date.now(), level: "warn", scope: "admin", message: "报警模式已下发", detail: `${frame.target_device_id} ${frame.mode}` });
      return frame;
    } catch (err) {
      error.value = stringifyError(err);
      return null;
    }
  }

  function readSavedDebugEnabled() {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem("lanchat-debug-enabled") === "true";
  }

  function setDebugEnabled(value: boolean) {
    debugEnabled.value = value;
    if (typeof window !== "undefined") {
      window.localStorage.setItem("lanchat-debug-enabled", String(value));
    }
    pushDebugLog({ ts: Date.now(), level: "info", scope: "frontend", message: value ? "Debug 模式已开启" : "Debug 模式已关闭", detail: null });
  }

  function pushDebugLog(entry: DebugLog) {
    if (!debugEnabled.value) return;
    debugLogs.value = [...debugLogs.value.slice(-499), entry];
  }

  function clearDebugLogs() {
    debugLogs.value = [];
  }
  async function repairNetwork() {
    error.value = "";
    networkRepairStatus.value = "";
    networkRepairing.value = true;
    try {
      networkRepairStatus.value = await api.repairWindowsFirewall();
    } catch (err) {
      error.value = stringifyError(err);
    } finally {
      networkRepairing.value = false;
    }
  }
  function upsertPeer(peer: Peer) {
    const endpoint = `${peer.address}:${peer.port}`;
    const next = peers.value.filter((item) => item.device_id !== peer.device_id && `${item.address}:${item.port}` !== endpoint);
    next.push(peer);
    peers.value = dedupePeers(next).sort((a, b) => Number(b.online) - Number(a.online) || b.last_seen_at - a.last_seen_at || a.nickname.localeCompare(b.nickname));
  }

  function dedupePeers(items: Peer[]) {
    const seenIds = new Set<string>();
    const seenEndpoints = new Set<string>();
    return [...items]
      .sort((a, b) => Number(b.online) - Number(a.online) || b.last_seen_at - a.last_seen_at)
      .filter((peer) => {
        const idKey = peer.device_id.trim().toLowerCase();
        const endpointKey = `${peer.address}:${peer.port}`;
        if (seenIds.has(idKey) || seenEndpoints.has(endpointKey)) return false;
        seenIds.add(idKey);
        seenEndpoints.add(endpointKey);
        return true;
      });
  }
  function appendOrUpdateMessage(message: Message) {
    const current = messagesByConversation.value[message.conversation_id] ?? [];
    const next = current.filter((item) => item.id !== message.id);
    next.push(message);
    next.sort((a, b) => a.created_at - b.created_at);
    messagesByConversation.value = {
      ...messagesByConversation.value,
      [message.conversation_id]: next,
    };
  }

  function updateMessageStatus(messageId: string, status: Message["status"]) {
    const next: Record<string, Message[]> = {};
    for (const [conversationId, messages] of Object.entries(messagesByConversation.value)) {
      next[conversationId] = messages.map((message) =>
        message.id === messageId ? { ...message, status } : message,
      );
    }
    messagesByConversation.value = next;
  }

  function stringifyError(err: unknown) {
    return err instanceof Error ? err.message : String(err);
  }

  function isMessage(payload: unknown): payload is Message {
    return typeof payload === "object" && payload !== null && "conversation_id" in payload && "content" in payload;
  }

  function isAck(payload: unknown): payload is { message_id: string } {
    return typeof payload === "object" && payload !== null && "message_id" in payload;
  }

  return {
    profile,
    peers,
    conversations,
    messagesByConversation,
    channelMembersByConversation,
    channelMutedByConversation,
    activeConversationId,
    activeConversation,
    activeMessages,
    onlinePeers,
    activePeer,
    canSendActive,
    loading,
    error,
    manualAddress,
    manualPort,
    draft,
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
    initialize,
    refreshPeers,
    refreshConversations,
    loadMessages,
    loadChannelMembers,
    refreshChannelMute,
    selectConversation,
    createPrivateChannel,
    invitePrivateChannelMembers,
    removePrivateChannelMember,
    setPrivateChannelMemberMuted,
    dissolvePrivateChannel,
    adminMuteChannelMember,
    buildPrivateChannelInvite,
    acceptPrivateChannelInvite,
    addSystemNotice,
    recallMessage,
    saveProfile,
    deletePeer,
    adminRenamePeer,
    connectManualPeer,
    openDirect,
    sendActiveMessage,
    sendMessageToConversation,
    sendFile,
    sendVoice,
    sendGameFrame,
    sendQuickAlert,
    sendQuickAlertFeedback,
    resetQuickAlertCredibility,
    sendAdminDiscoMode,
    sendAdminAlertMode,
    repairNetwork,
    setDebugEnabled,
    clearDebugLogs,
  };
});
















