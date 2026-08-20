import { invoke } from "@tauri-apps/api/core";
import type { AdminAlertMode, AdminAlertPushPolicy, AdminDiscoMode, AdminNotification, AdminRemoteUpdate, AppVersionInfo, CallSignal, ChannelMember, Conversation, DesktopPetRuntimeState, GameFrame, Message, Nudge, Peer, PetAlertMode, PlatformInfo, PreviewMediaCacheInfo, PrivateChannelInvitePayload, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset, TrayAttentionItem, UpdateCheckResult, UpdateGithubTokenInfo } from "../types/lanchat";
import type { DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings, PetStatePlaybackConfig } from "../types/desktop-pet";
import type { CameraFaceAlert, CameraMonitorSettings, FaceMonitorPolicy, FaceMonitorRuntimeStatus, FacePersonPolicy } from "../types/face-monitor";
import type { VisionFrameSample, VisionProfileSummary, VisionRuntimeDiagnostics, VisionRuntimeSnapshot } from "../types/vision";
import { encodeVisionFrameEnvelope } from "./visionFrameTransport";

export const api = {
  getPlatformInfo: () => invoke<PlatformInfo>("get_platform_info"),
  getFaceMonitorStatus: () => invoke<FaceMonitorRuntimeStatus>("get_face_monitor_status"),
  updateFaceMonitorLocalSettings: (settings: CameraMonitorSettings) => invoke<CameraMonitorSettings>("update_face_monitor_local_settings", { settings }),
  submitVisionFrameRaw: (sample: VisionFrameSample) =>
    invoke<void>("submit_vision_frame_raw", { frame: encodeVisionFrameEnvelope(sample) }),
  getVisionRuntimeDiagnostics: () => invoke<VisionRuntimeDiagnostics>("get_vision_runtime_diagnostics"),
  getVisionRuntimeSnapshot: () => invoke<VisionRuntimeSnapshot>("get_vision_runtime_snapshot"),
  listVisionModelProfiles: () => invoke<VisionProfileSummary[]>("list_vision_model_profiles"),
  refreshVisionModelCatalog: () => invoke<VisionProfileSummary[]>("refresh_vision_model_catalog"),
  installVisionModelProfile: (profileId: string, profileVersion: string) =>
    invoke<VisionProfileSummary[]>("install_vision_model_profile", { profileId, profileVersion }),
  activateVisionModelProfile: (profileId: string, profileVersion: string) =>
    invoke<VisionProfileSummary[]>("activate_vision_model_profile", { profileId, profileVersion }),
  setVisionRuntimePaused: (paused: boolean) => invoke<VisionRuntimeSnapshot>("set_vision_runtime_paused", { paused }),
  // 过渡兼容：旧调用方继续使用原方法名，但实际只会进入新的 Raw RGBA 通道。
  submitFaceMonitorFrame: (sample: VisionFrameSample) =>
    invoke<void>("submit_vision_frame_raw", { frame: encodeVisionFrameEnvelope(sample) }),
  listFacePeople: () => invoke<FacePersonPolicy[]>("list_face_people"),
  deleteFacePersonLocal: (personId: string) => invoke<void>("delete_face_person_local", { personId }),
  saveFaceReferencePhoto: (bytes: Uint8Array) => invoke<string>("save_face_reference_photo", { bytes: Array.from(bytes) }),
  createLocalFacePerson: (personId: string, displayName: string, photoPaths: string[]) =>
    invoke<FacePersonPolicy>("create_local_face_person", { personId, displayName, photoPaths }),
  getEffectiveFaceMonitorPolicy: () => invoke<FaceMonitorPolicy | null>("get_effective_face_monitor_policy"),
  sendFaceMonitorPolicy: (targetDeviceId: string, minConfidence: number, bodyMinConfidence: number, sampleFps: number, consecutiveHits: number, faceCooldownSeconds: number, bodyCooldownSeconds: number, settingsLocked: boolean, version: number) =>
    invoke<FaceMonitorPolicy>("send_face_monitor_policy", { targetDeviceId, minConfidence, bodyMinConfidence, sampleFps, consecutiveHits, faceCooldownSeconds, bodyCooldownSeconds, settingsLocked, version }),
  sendFacePersonPolicy: (targetDeviceId: string, personId: string, displayName: string, photoPaths: string[], expiresAt: number | null, enabled: boolean, action: "upsert" | "disable" | "delete", version: number) =>
    invoke<FacePersonPolicy>("send_face_person_policy", { targetDeviceId, personId, displayName, photoPaths, expiresAt, enabled, action, version }),
  listCameraFaceAlerts: () => invoke<CameraFaceAlert[]>("list_camera_face_alerts"),
  clearCameraFaceAlerts: () => invoke<void>("clear_camera_face_alerts"),
  sendCameraFaceAlertFeedback: (alertId: string, sourceDeviceId: string, result: "real" | "false") =>
    invoke<CameraFaceAlert>("send_camera_face_alert_feedback", { alertId, sourceDeviceId, result }),
  getAppVersionInfo: () => invoke<AppVersionInfo>("get_app_version_info"),
  refreshUpdateProxy: () => invoke<void>("refresh_update_proxy"),
  checkForUpdate: () => invoke<UpdateCheckResult>("check_for_update"),
  getUpdateGithubTokenInfo: () => invoke<UpdateGithubTokenInfo>("get_update_github_token_info"),
  saveUpdateGithubToken: (token: string) => invoke<UpdateGithubTokenInfo>("save_update_github_token", { token }),
  clearUpdateGithubToken: () => invoke<UpdateGithubTokenInfo>("clear_update_github_token"),
  isPortableRuntime: () => invoke<boolean>("is_portable_runtime"),
  installPortableUpdate: (downloadUrl: string, sha256: string) => invoke<void>("install_portable_update", { downloadUrl, sha256 }),
  sendAdminRemoteUpdate: (targetDeviceId: string, targetVersion: string, packagePath?: string | null) =>
    invoke<AdminRemoteUpdate>("send_admin_remote_update", { targetDeviceId, targetVersion, packagePath: packagePath || null }),
  executeAdminRemoteUpdate: (command: AdminRemoteUpdate) => invoke<void>("execute_admin_remote_update", { command }),
  authenticateSuperAdmin: (password: string) => invoke<boolean>("authenticate_super_admin", { password }),
  clearSuperAdminSession: () => invoke<void>("clear_super_admin_session"),
  isSuperAdminAuthenticated: () => invoke<boolean>("is_super_admin_authenticated"),
  openUpdateUrl: (url: string) => invoke<void>("open_update_url", { url }),
  getProfile: () => invoke<Profile>("get_profile"),
  updateProfile: (nickname: string, listenPort: number, avatar?: string | null) =>
    invoke<Profile>("update_profile", { nickname, listenPort, avatar }),
  listPeers: () => invoke<Peer[]>("list_peers"),
  updatePeerNote: (deviceId: string, note: string) => invoke<Peer>("update_peer_note", { deviceId, note }),
  deletePeer: (deviceId: string) => invoke<void>("delete_peer", { deviceId }),
  adminRenamePeer: (targetDeviceId: string, nickname: string, nicknameLocked?: boolean | null, useSystemUsername = false) =>
    invoke<Peer>("admin_rename_peer", { targetDeviceId, nickname, nicknameLocked, useSystemUsername }),
  connectPeer: (address: string, port: number) =>
    invoke<Peer>("connect_peer", { address, port }),
  listConversations: () => invoke<Conversation[]>("list_conversations"),
  listChannelMembers: (conversationId: string) =>
    invoke<ChannelMember[]>("list_channel_members", { conversationId }),
  isChannelMuted: (conversationId: string) =>
    invoke<boolean>("is_channel_muted", { conversationId }),
  createPrivateChannel: (title: string, memberDeviceIds: string[]) =>
    invoke<Conversation>("create_private_channel", { title, memberDeviceIds }),
  invitePrivateChannelMembers: (conversationId: string, memberDeviceIds: string[], superAdmin = false) =>
    invoke<ChannelMember[]>("invite_private_channel_members", { conversationId, memberDeviceIds, superAdmin }),
  removePrivateChannelMember: (conversationId: string, memberDeviceId: string, superAdmin = false) =>
    invoke<ChannelMember[]>("remove_private_channel_member", { conversationId, memberDeviceId, superAdmin }),
  leavePrivateChannel: (conversationId: string) =>
    invoke<void>("leave_private_channel", { conversationId }),
  setPrivateChannelMemberMuted: (conversationId: string, memberDeviceId: string, muted: boolean, superAdmin = false) =>
    invoke<ChannelMember[]>("set_private_channel_member_muted", { conversationId, memberDeviceId, muted, superAdmin }),
  dissolvePrivateChannel: (conversationId: string, superAdmin = false) =>
    invoke<void>("dissolve_private_channel", { conversationId, superAdmin }),
  adminMuteChannelMember: (conversationId: string, targetDeviceId: string, muted: boolean) =>
    invoke<void>("admin_mute_channel_member", { conversationId, targetDeviceId, muted }),
  buildPrivateChannelInviteCard: (conversationId: string, superAdmin = false) =>
    invoke<PrivateChannelInvitePayload>("build_private_channel_invite_card", { conversationId, superAdmin }),
  acceptPrivateChannelInvite: (invite: PrivateChannelInvitePayload) =>
    invoke<Conversation>("accept_private_channel_invite", { invite }),
  broadcastChannelNotice: (conversationId: string, notice: string) =>
    invoke<void>("broadcast_channel_notice", { conversationId, notice }),
  listMessages: (conversationId: string, beforeCreatedAt?: number, limit = 60) =>
    invoke<Message[]>("list_messages", { conversationId, beforeCreatedAt, limit }),
  sendMessage: (conversationId: string, content: string) =>
    invoke<Message>("send_message", { conversationId, content }),
  simulateMessage: (simulatedDeviceId: string, conversationId: string, content: string, displaySimulationLabel: boolean) =>
    invoke<Message>("simulate_message", { simulatedDeviceId, conversationId, content, displaySimulationLabel }),
  saveSystemNotice: (conversationId: string, content: string) =>
    invoke<Message>("save_system_notice", { conversationId, content }),
  recallMessage: (messageId: string) =>
    invoke<Message>("recall_message", { messageId }),
  sendFileMessage: (conversationId: string, path: string) =>
    invoke<Message>("send_file_message", { conversationId, path }),
  sendPastedImageMessage: (conversationId: string, fileName: string, bytes: number[], mimeType: string) =>
    invoke<Message>("send_pasted_image_message", { conversationId, fileName, bytes, mimeType }),
  cachePreviewMedia: (messageId: string, url: string, fileName: string) =>
    invoke<string>("cache_preview_media", { messageId, url, fileName }),
  getPreviewMediaCacheInfo: () => invoke<PreviewMediaCacheInfo>("get_preview_media_cache_info"),
  clearPreviewMediaCache: () => invoke<PreviewMediaCacheInfo>("clear_preview_media_cache"),
  sendVoiceMessage: (conversationId: string, fileName: string, bytes: number[], durationMs: number) =>
    invoke<Message>("send_voice_message", { conversationId, fileName, bytes, durationMs }),
  sendGameFrame: (targetDeviceId: string | null, frame: GameFrame) =>
    invoke<void>("send_game_frame", { targetDeviceId, frame }),
  sendCallSignal: (targetDeviceId: string, frame: CallSignal) =>
    invoke<void>("send_call_signal", { targetDeviceId, frame }),
  sendNudge: (targetDeviceId: string) => invoke<Nudge>("send_nudge", { targetDeviceId }),
  revealAndShakeMainWindow: () => invoke<void>("reveal_and_shake_main_window"),
  sendQuickAlert: (content: string, mode: PetAlertMode = "normal", senderCredibility?: number) =>
    invoke<QuickAlert>("send_quick_alert", { content, mode, senderCredibility }),
  simulateQuickAlert: (simulatedDeviceId: string, content: string, mode: PetAlertMode, displaySimulationLabel: boolean) =>
    invoke<QuickAlert>("simulate_quick_alert", { simulatedDeviceId, content, mode, displaySimulationLabel }),
  sendQuickAlertFeedback: (alertId: string, alertSenderDeviceId: string, result: "real" | "false") =>
    invoke<QuickAlertFeedback>("send_quick_alert_feedback", { alertId, alertSenderDeviceId, result }),
  resetQuickAlertCredibility: (targetDeviceId: string) =>
    invoke<QuickAlertTrustReset>("send_quick_alert_trust_reset", { targetDeviceId }),
  sendAdminDiscoMode: (targetDeviceId: string, durationMs = 60_000) =>
    invoke<AdminDiscoMode>("send_admin_disco_mode", { targetDeviceId, durationMs }),
  sendAdminAlertMode: (targetDeviceId: string, mode: PetAlertMode) =>
    invoke<AdminAlertMode>("send_admin_alert_mode", { targetDeviceId, mode }),
  sendAdminAlertPushPolicy: (targetDeviceId: string, minCredibility: number, minCredibilityLocked: boolean) =>
    invoke<AdminAlertPushPolicy>("send_admin_alert_push_policy", { targetDeviceId, minCredibility, minCredibilityLocked }),
  listAdminNotifications: () => invoke<AdminNotification[]>("list_admin_notifications"),
  sendAdminNotification: (targetDeviceId: string | null, targetScope: "device" | "all_online", title: string, content: string, template: string, supportUrl: string | null, displayMode: string, deadlineAt: number | null, timeoutPolicy: string, forceOpenMainWindow: boolean) =>
    invoke<AdminNotification[]>("send_admin_notification", { targetDeviceId, targetScope, title, content, template, supportUrl, displayMode, deadlineAt, timeoutPolicy, forceOpenMainWindow }),
  submitAdminNotification: (notificationId: string) =>
    invoke<AdminNotification>("submit_admin_notification", { notificationId }),
  decideAdminNotification: (notificationId: string, decision: "approved" | "rejected" | "revoked") =>
    invoke<AdminNotification>("decide_admin_notification", { notificationId, decision }),
  listDesktopPets: () => invoke<DesktopPetRegistrySnapshot>("list_desktop_pets"),
  refreshDesktopPets: () => invoke<DesktopPetRegistrySnapshot>("refresh_desktop_pets"),
  importDesktopPet: (sourcePath: string) =>
    invoke<DesktopPetPackage>("import_desktop_pet", { sourcePath }),
  removeDesktopPet: (petId: string) => invoke<void>("remove_desktop_pet", { petId }),
  selectDesktopPet: (petId: string) =>
    invoke<DesktopPetSettings>("select_desktop_pet", { petId }),
  getDesktopPetSettings: () => invoke<DesktopPetSettings>("get_desktop_pet_settings"),
  updateDesktopPetSettings: (settings: DesktopPetSettings) =>
    invoke<DesktopPetSettings>("update_desktop_pet_settings", { settings }),
  updateDesktopPetPlaybackConfig: (petId: string, configs: Record<string, PetStatePlaybackConfig>) =>
    invoke<DesktopPetPackage>("update_desktop_pet_playback_config", { petId, configs }),
  openDesktopPetFolder: () => invoke<void>("open_desktop_pet_folder"),
  setDesktopPetEnabled: (enabled: boolean) => invoke<void>("set_desktop_pet_enabled", { enabled }),
  updateDesktopPetState: (petState: DesktopPetRuntimeState) =>
    invoke<void>("update_desktop_pet_state", { petState }),
  registerDesktopPetSendHotkey: (hotkey: string) => invoke<void>("register_desktop_pet_send_hotkey", { hotkey }),
  registerDesktopPetStopHotkey: (hotkey: string) => invoke<void>("register_desktop_pet_stop_hotkey", { hotkey }),
  startMainWindowDrag: () => invoke<void>("start_main_window_drag"),
  minimizeMainWindow: () => invoke<void>("minimize_main_window"),
  toggleMainWindowMaximized: () => invoke<void>("toggle_main_window_maximized"),
  repairWindowsFirewall: () => invoke<string>("repair_windows_firewall"),
  hideToTray: () => invoke<void>("hide_to_tray"),
  showFromTray: () => invoke<void>("show_from_tray"),
  updateTrayAttention: (items: TrayAttentionItem[]) => invoke<void>("update_tray_attention", { items }),
  quitApp: () => invoke<void>("quit_app"),
};



