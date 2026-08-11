use crate::native_app::models::{
    NativeConversationRow, NativeMessageRow, NativePeerRow, NativeProfile, NativeSidebar,
};
use crate::storage::{ConversationKind, Message, MessageStatus, MessageType, Peer, Profile, Storage};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct NativeAppServices {
    storage: Arc<Storage>,
}

impl NativeAppServices {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn open_default() -> Result<Self, String> {
        let app_data = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("com.lanchat.desktop"))
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from(".lanchat"))
                    .join(".lanchat")
            });
        let storage = Storage::open(Self::storage_path_for_app_data(app_data))?;
        Ok(Self::new(Arc::new(storage)))
    }

    pub fn storage_path_for_app_data(app_data_dir: impl AsRef<Path>) -> PathBuf {
        app_data_dir.as_ref().join("lanchat.sqlite3")
    }

    pub fn load_sidebar(&self) -> Result<NativeSidebar, String> {
        let profile = self.storage.get_or_create_profile()?;
        let peers = self.storage.list_peers()?;
        let conversations = self.storage.list_conversations()?;

        Ok(NativeSidebar {
            profile: map_profile(profile),
            peers: peers.into_iter().map(map_peer).collect(),
            conversations: conversations.into_iter().map(map_conversation).collect(),
        })
    }

    pub fn load_messages(
        &self,
        conversation_id: &str,
        before_created_at: Option<i64>,
    ) -> Result<Vec<NativeMessageRow>, String> {
        let profile = self.storage.get_or_create_profile()?;
        self.storage
            .list_messages_page(conversation_id, before_created_at, 20)?
            .into_iter()
            .map(|message| Ok(map_message(message, &profile.device_id)))
            .collect()
    }
}

fn map_profile(profile: Profile) -> NativeProfile {
    NativeProfile {
        device_id: profile.device_id,
        nickname: profile.nickname,
        avatar: profile.avatar,
    }
}

fn map_peer(peer: Peer) -> NativePeerRow {
    let display_name = peer
        .note
        .filter(|note| !note.trim().is_empty())
        .unwrap_or(peer.nickname);
    NativePeerRow {
        device_id: peer.device_id,
        display_name,
        avatar: peer.avatar,
        address: peer.address,
        online: peer.online,
        supports_chat: peer.supports_chat,
    }
}

fn map_conversation(conversation: crate::storage::Conversation) -> NativeConversationRow {
    NativeConversationRow {
        id: conversation.id,
        title: conversation.title,
        unread_count: conversation.unread_count,
        is_group: matches!(conversation.kind, ConversationKind::Group),
        is_private: conversation.is_private,
        updated_at: conversation.updated_at,
    }
}

fn map_message(message: Message, local_device_id: &str) -> NativeMessageRow {
    NativeMessageRow {
        id: message.id,
        sender_device_id: message.sender_device_id.clone(),
        content: message.content,
        message_type: match message.message_type {
            MessageType::Text => "text".to_string(),
            MessageType::File => "file".to_string(),
            MessageType::Voice => "voice".to_string(),
            MessageType::System => "system".to_string(),
        },
        status: match message.status {
            MessageStatus::Sending => "sending".to_string(),
            MessageStatus::Sent => "sent".to_string(),
            MessageStatus::Delivered => "delivered".to_string(),
            MessageStatus::Failed => "failed".to_string(),
        },
        created_at: message.created_at,
        outgoing: message.sender_device_id.eq_ignore_ascii_case(local_device_id),
        has_attachment: message.file_meta.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::NativeAppServices;
    use crate::storage::{Message, MessageStatus, MessageType, Peer, Storage, DEFAULT_GROUP_ID};
    use std::sync::Arc;

    #[test]
    fn resolves_the_existing_tauri_database_name_under_app_data() {
        let path = NativeAppServices::storage_path_for_app_data("C:/Users/test/AppData/Roaming/com.lanchat.desktop");

        assert_eq!(
            std::path::PathBuf::from("C:/Users/test/AppData/Roaming/com.lanchat.desktop")
                .join("lanchat.sqlite3"),
            path
        );
    }

    #[test]
    fn loads_existing_profile_peer_and_conversation_into_native_sidebar() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Arc::new(Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage"));
        storage
            .update_profile("本机", 18145, None)
            .expect("profile updated");
        storage
            .upsert_peer(&Peer {
                device_id: "AA-BB-CC-DD-EE-FF".to_string(),
                nickname: "开发同事".to_string(),
                note: Some("小王".to_string()),
                avatar: Some("data:image/png;base64,abc".to_string()),
                address: "192.168.1.20".to_string(),
                port: 18145,
                online: true,
                last_seen_at: 100,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: false,
                build_version: "0.4.2".to_string(),
                build_timestamp: 100,
            })
            .expect("peer saved");
        storage
            .update_peer_note("AA-BB-CC-DD-EE-FF", "小王")
            .expect("peer note saved");

        let sidebar = NativeAppServices::new(storage)
            .load_sidebar()
            .expect("native sidebar");

        assert_eq!("本机", sidebar.profile.nickname);
        assert_eq!(1, sidebar.peers.len());
        assert_eq!("小王", sidebar.peers[0].display_name);
        assert!(sidebar.peers[0].online);
        assert!(sidebar
            .conversations
            .iter()
            .any(|item| item.id == "aa:bb:cc:dd:ee:ff" && item.title == "开发同事"));
    }

    #[test]
    fn loads_twenty_most_recent_messages_for_native_timeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Arc::new(Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage"));
        for index in 1..=23 {
            storage
                .save_message(&Message {
                    id: format!("native-msg-{index}"),
                    conversation_id: DEFAULT_GROUP_ID.to_string(),
                    sender_device_id: "peer-1".to_string(),
                    content: format!("消息 {index}"),
                    message_type: MessageType::Text,
                    file_meta: None,
                    status: MessageStatus::Delivered,
                    simulation: None,
                    created_at: index,
                })
                .expect("message saved");
        }

        let messages = NativeAppServices::new(storage)
            .load_messages(DEFAULT_GROUP_ID, None)
            .expect("native messages");

        assert_eq!(20, messages.len());
        assert_eq!("消息 4", messages.first().expect("first message").content);
        assert_eq!("消息 23", messages.last().expect("last message").content);
    }
}
