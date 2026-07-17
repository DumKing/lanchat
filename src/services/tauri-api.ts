import { invoke } from "@tauri-apps/api/core";
import type { AdminAlertMode, AdminDiscoMode, ChannelMember, Conversation, DesktopPetRuntimeState, GameFrame, Message, Peer, PetAlertMode, PrivateChannelInvitePayload, Profile, QuickAlert, QuickAlertFeedback, QuickAlertTrustReset, TrayAttentionItem } from "../types/lanchat";
import type { DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings, PetStatePlaybackConfig } from "../types/desktop-pet";

export const api = {
  getProfile: () => invoke<Profile>("get_profile"),
  updateProfile: (nickname: string, listenPort: number, avatar?: string | null) =>
    invoke<Profile>("update_profile", { nickname, listenPort, avatar }),
  listPeers: () => invoke<Peer[]>("list_peers"),
  deletePeer: (deviceId: string) => invoke<void>("delete_peer", { deviceId }),
  adminRenamePeer: (targetDeviceId: string, nickname: string) =>
    invoke<Peer>("admin_rename_peer", { targetDeviceId, nickname }),
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
  listMessages: (conversationId: string) =>
    invoke<Message[]>("list_messages", { conversationId }),
  sendMessage: (conversationId: string, content: string) =>
    invoke<Message>("send_message", { conversationId, content }),
  saveSystemNotice: (conversationId: string, content: string) =>
    invoke<Message>("save_system_notice", { conversationId, content }),
  recallMessage: (messageId: string) =>
    invoke<Message>("recall_message", { messageId }),
  sendFileMessage: (conversationId: string, path: string) =>
    invoke<Message>("send_file_message", { conversationId, path }),
  sendVoiceMessage: (conversationId: string, fileName: string, bytes: number[], durationMs: number) =>
    invoke<Message>("send_voice_message", { conversationId, fileName, bytes, durationMs }),
  sendGameFrame: (targetDeviceId: string | null, frame: GameFrame) =>
    invoke<void>("send_game_frame", { targetDeviceId, frame }),
  sendQuickAlert: (content: string, mode: PetAlertMode = "normal") =>
    invoke<QuickAlert>("send_quick_alert", { content, mode }),
  sendQuickAlertFeedback: (alertId: string, alertSenderDeviceId: string, result: "real" | "false") =>
    invoke<QuickAlertFeedback>("send_quick_alert_feedback", { alertId, alertSenderDeviceId, result }),
  resetQuickAlertCredibility: (targetDeviceId: string) =>
    invoke<QuickAlertTrustReset>("send_quick_alert_trust_reset", { targetDeviceId }),
  sendAdminDiscoMode: (targetDeviceId: string, durationMs = 120_000) =>
    invoke<AdminDiscoMode>("send_admin_disco_mode", { targetDeviceId, durationMs }),
  sendAdminAlertMode: (targetDeviceId: string, mode: PetAlertMode) =>
    invoke<AdminAlertMode>("send_admin_alert_mode", { targetDeviceId, mode }),
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



