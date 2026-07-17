export type Profile = {
  device_id: string;
  nickname: string;
  listen_port: number;
  avatar?: string | null;
};

export type Peer = {
  device_id: string;
  nickname: string;
  avatar?: string | null;
  address: string;
  port: number;
  online: boolean;
  last_seen_at: number;
};

export type ConversationKind = "direct" | "group";

export type Conversation = {
  id: string;
  title: string;
  kind: ConversationKind;
  peer_device_id?: string | null;
  updated_at: number;
  unread_count: number;
  is_private: boolean;
  owner_device_id?: string | null;
};

export type ChannelMember = {
  channel_id: string;
  device_id: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  last_seen_at: number;
  is_owner: boolean;
  muted: boolean;
};

export type PrivateChannelMemberSeed = {
  device_id: string;
  nickname: string;
  avatar?: string | null;
};

export type PrivateChannelInvitePayload = {
  channel_id: string;
  title: string;
  owner_device_id: string;
  owner_nickname: string;
  channel_key: string;
  key_version: number;
  members?: PrivateChannelMemberSeed[];
  created_at: number;
};

export type TrayAttentionItem = {
  id: string;
  kind: "chat" | "game" | string;
  title: string;
  count: number;
};
export type MessageStatus = "sending" | "sent" | "delivered" | "failed";
export type MessageType = "text" | "file" | "voice" | "system";

export type FileMeta = {
  name: string;
  size: number;
  url: string;
  mime_type?: string | null;
  duration_ms?: number | null;
};

export type Message = {
  id: string;
  conversation_id: string;
  sender_device_id: string;
  content: string;
  message_type: MessageType;
  file_meta?: FileMeta | null;
  status: MessageStatus;
  created_at: number;
};


export type DebugLog = {
  ts: number;
  level: "info" | "warn" | "error" | string;
  scope: string;
  message: string;
  detail?: string | null;
};

export type GameFrame = {
  frame_id: string;
  game: string;
  room_id: string;
  sender_device_id: string;
  sender_nickname: string;
  kind: string;
  payload: unknown;
  created_at: number;
};

export type PetAlertMode = "normal" | "disco";

export type QuickAlert = {
  alert_id: string;
  sender_device_id: string;
  sender_nickname: string;
  content: string;
  mode: PetAlertMode;
  created_at: number;
};

export type QuickAlertFeedback = {
  alert_id: string;
  alert_sender_device_id: string;
  responder_device_id: string;
  responder_nickname: string;
  result: "real" | "false" | string;
  created_at: number;
};

export type QuickAlertTrustReset = {
  target_device_id: string;
  issued_by_device_id: string;
  issued_by_nickname: string;
  created_at: number;
};

export type AdminDiscoMode = {
  target_device_id: string;
  duration_ms: number;
  issued_by_device_id: string;
  issued_by_nickname: string;
  created_at: number;
};

export type AdminAlertMode = {
  target_device_id: string;
  mode: PetAlertMode;
  issued_by_device_id: string;
  issued_by_nickname: string;
  created_at: number;
};

export type DesktopPetRuntimeState = {
  enabled: boolean;
  pending_count: number;
  temperature: number;
  latest_alert_id?: string | null;
  latest_sender?: string | null;
  latest_sender_address?: string | null;
  latest_content?: string | null;
  latest_created_at?: number | null;
  feedbackable: boolean;
  flashing: boolean;
  disco: boolean;
  theme_accent?: string | null;
  random_move_enabled?: boolean;
  random_life_enabled?: boolean;
  disco_movement_mode?: "linear" | "jump";
};

export type ChannelNoticePayload = {
  conversation_id: string;
  notice: string;
  updated_by_device_id: string;
  updated_by_nickname: string;
  updated_at: number;
};

