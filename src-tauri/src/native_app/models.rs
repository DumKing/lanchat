#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProfile {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePeerRow {
    pub device_id: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub address: String,
    pub online: bool,
    pub supports_chat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConversationRow {
    pub id: String,
    pub title: String,
    pub unread_count: u32,
    pub is_group: bool,
    pub is_private: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMessageRow {
    pub id: String,
    pub sender_device_id: String,
    pub content: String,
    pub message_type: String,
    pub status: String,
    pub created_at: i64,
    pub outgoing: bool,
    pub has_attachment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSidebar {
    pub profile: NativeProfile,
    pub peers: Vec<NativePeerRow>,
    pub conversations: Vec<NativeConversationRow>,
}
