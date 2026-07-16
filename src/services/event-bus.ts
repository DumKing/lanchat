import { listen } from "@tauri-apps/api/event";
import type { AdminAlertMode, AdminDiscoMode, ChannelNoticePayload, Conversation, DebugLog, GameFrame, Message, Peer, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset } from "../types/lanchat";

export async function registerLanChatEvents(handlers: {
  onPeerOnline: (peer: Peer) => void;
  onPeerOffline: (deviceId: string) => void;
  onProfileUpdated: (profile: Profile) => void;
  onMessageReceived: (message: Message) => void;
  onMessageStatusChanged: (payload: unknown) => void;
  onDebugLog: (entry: DebugLog) => void;
  onGameFrame: (frame: GameFrame) => void;
  onPrivateChannelInvited: (conversation: Conversation) => void;
  onPrivateChannelChanged: (conversationId: string) => void;
  onChannelNoticeUpdated: (payload: ChannelNoticePayload) => void;
  onMessageRecalled: (message: Message) => void;
  onQuickAlertReceived: (alert: QuickAlert) => void;
  onQuickAlertFeedbackReceived: (feedback: QuickAlertFeedback) => void;
  onQuickAlertTrustResetReceived: (reset: QuickAlertTrustReset) => void;
  onAdminDiscoModeReceived: (mode: AdminDiscoMode) => void;
  onAdminAlertModeReceived: (mode: AdminAlertMode) => void;
}) {
  const unlistenPeerOnline = await listen<Peer>("peer_online", (event) => {
    handlers.onPeerOnline(event.payload);
  });
  const unlistenPeerOffline = await listen<string>("peer_offline", (event) => {
    handlers.onPeerOffline(event.payload);
  });
  const unlistenProfileUpdated = await listen<Profile>("profile_updated", (event) => {
    handlers.onProfileUpdated(event.payload);
  });
  const unlistenMessageReceived = await listen<Message>("message_received", (event) => {
    handlers.onMessageReceived(event.payload);
  });
  const unlistenMessageStatus = await listen<unknown>("message_status_changed", (event) => {
    handlers.onMessageStatusChanged(event.payload);
  });
  const unlistenDebugLog = await listen<DebugLog>("debug_log", (event) => {
    handlers.onDebugLog(event.payload);
  });
  const unlistenGameFrame = await listen<GameFrame>("game_frame_received", (event) => {
    handlers.onGameFrame(event.payload);
  });
  const unlistenPrivateChannelInvited = await listen<Conversation>("private_channel_invited", (event) => {
    handlers.onPrivateChannelInvited(event.payload);
  });
  const unlistenPrivateChannelChanged = await listen<string>("private_channel_changed", (event) => {
    handlers.onPrivateChannelChanged(event.payload);
  });
  const unlistenChannelNoticeUpdated = await listen<ChannelNoticePayload>("channel_notice_updated", (event) => {
    handlers.onChannelNoticeUpdated(event.payload);
  });
  const unlistenMessageRecalled = await listen<Message>("message_recalled", (event) => {
    handlers.onMessageRecalled(event.payload);
  });
  const unlistenQuickAlert = await listen<QuickAlert>("quick_alert_received", (event) => {
    handlers.onQuickAlertReceived(event.payload);
  });
  const unlistenQuickAlertFeedback = await listen<QuickAlertFeedback>("quick_alert_feedback_received", (event) => {
    handlers.onQuickAlertFeedbackReceived(event.payload);
  });
  const unlistenQuickAlertTrustReset = await listen<QuickAlertTrustReset>("quick_alert_trust_reset_received", (event) => {
    handlers.onQuickAlertTrustResetReceived(event.payload);
  });
  const unlistenAdminDiscoMode = await listen<AdminDiscoMode>("admin_disco_mode_received", (event) => {
    handlers.onAdminDiscoModeReceived(event.payload);
  });
  const unlistenAdminAlertMode = await listen<AdminAlertMode>("admin_alert_mode_received", (event) => {
    handlers.onAdminAlertModeReceived(event.payload);
  });

  return () => {
    unlistenPeerOnline();
    unlistenPeerOffline();
    unlistenProfileUpdated();
    unlistenMessageReceived();
    unlistenMessageStatus();
    unlistenDebugLog();
    unlistenGameFrame();
    unlistenPrivateChannelInvited();
    unlistenPrivateChannelChanged();
    unlistenChannelNoticeUpdated();
    unlistenMessageRecalled();
    unlistenQuickAlert();
    unlistenQuickAlertFeedback();
    unlistenQuickAlertTrustReset();
    unlistenAdminDiscoMode();
    unlistenAdminAlertMode();
  };
}

