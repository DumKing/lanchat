import { listen } from "@tauri-apps/api/event";
import type { AdminAlertMode, AdminAlertPushPolicy, AdminDiscoMode, AdminNotification, CallSignal, ChannelNoticePayload, Conversation, DebugLog, GameFrame, Message, Nudge, Peer, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset } from "../types/lanchat";

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
  onCallSignalReceived: (signal: CallSignal) => void;
  onNudgeReceived: (nudge: Nudge) => void;
  onAdminAlertPushPolicyReceived: (policy: AdminAlertPushPolicy) => void;
  onAdminNotificationReceived: (notification: AdminNotification) => void;
  onAdminNotificationSubmissionReceived: (notification: AdminNotification) => void;
  onAdminNotificationDecisionReceived: (notification: AdminNotification) => void;
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
  const unlistenCallSignal = await listen<CallSignal>("call_signal_received", (event) => {
    handlers.onCallSignalReceived(event.payload);
  });
  const unlistenNudge = await listen<Nudge>("nudge_received", (event) => {
    handlers.onNudgeReceived(event.payload);
  });
  const unlistenAdminAlertPushPolicy = await listen<AdminAlertPushPolicy>("admin_alert_push_policy_received", (event) => {
    handlers.onAdminAlertPushPolicyReceived(event.payload);
  });
  const unlistenAdminNotification = await listen<AdminNotification>("admin_notification_received", (event) => {
    handlers.onAdminNotificationReceived(event.payload);
  });
  const unlistenAdminNotificationSubmission = await listen<AdminNotification>("admin_notification_submission_received", (event) => {
    handlers.onAdminNotificationSubmissionReceived(event.payload);
  });
  const unlistenAdminNotificationDecision = await listen<AdminNotification>("admin_notification_decision_received", (event) => {
    handlers.onAdminNotificationDecisionReceived(event.payload);
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
    unlistenCallSignal();
    unlistenNudge();
    unlistenAdminAlertPushPolicy();
    unlistenAdminNotification();
    unlistenAdminNotificationSubmission();
    unlistenAdminNotificationDecision();
  };
}

