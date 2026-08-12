use crate::native_app::models::{
    NativeConversationRow, NativeMessageRow, NativeNotificationRow, NativePeerRow, NativeProfile,
    NativePetRow, NativeSidebar,
};
use crate::desktop_pet::{DesktopPetManager, PetPackageSource, PetResourceRoot};
use crate::storage::{
    AdminNotificationRecord, ConversationKind, Message, MessageStatus, MessageType, Peer, Profile,
    Storage,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct NativeAppServices {
    storage: Arc<Storage>,
}

impl NativeAppServices {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn open_default() -> Result<Self, String> {
        let app_data = Self::default_app_data_dir();
        let storage = Storage::open(Self::storage_path_for_app_data(app_data))?;
        Ok(Self::new(Arc::new(storage)))
    }

    pub fn default_app_data_dir() -> PathBuf {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("com.lanchat.desktop"))
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from(".lanchat"))
                    .join(".lanchat")
            })
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

    pub fn load_local_notification_history(&self) -> Result<Vec<NativeNotificationRow>, String> {
        let profile = self.storage.get_or_create_profile()?;
        Ok(self
            .storage
            .list_admin_notifications()?
            .into_iter()
            .filter(|record| record.target_device_id.eq_ignore_ascii_case(&profile.device_id))
            .map(map_notification)
            .collect())
    }

    pub fn update_profile_nickname(&self, nickname: &str) -> Result<NativeProfile, String> {
        let profile = self.storage.get_or_create_profile()?;
        let nickname = nickname.trim();
        if nickname.is_empty() {
            return Err("昵称不能为空".to_string());
        }
        self.storage
            .update_profile(nickname, profile.listen_port, profile.avatar)?;
        self.storage
            .get_or_create_profile()
            .map(map_profile)
    }

    pub fn list_desktop_pets(&self) -> Vec<NativePetRow> {
        let manager = self.desktop_pet_manager();
        let selected_id = manager.settings().selected_pet_id;
        manager
            .snapshot()
            .packages
            .into_iter()
            .map(|package| {
                let id = package.id().to_string();
                NativePetRow {
                    selected: selected_id.as_deref() == Some(id.as_str()),
                    name: package.manifest.name,
                    id,
                }
            })
            .collect()
    }

    pub fn select_desktop_pet(&self, id: &str) -> Result<(), String> {
        self.desktop_pet_manager().select(id).map(|_| ())
    }

    fn desktop_pet_manager(&self) -> DesktopPetManager {
        let app_data_dir = Self::default_app_data_dir();
        let mut roots = vec![PetResourceRoot::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("desktop-pets"),
            PetPackageSource::BuiltIn,
        )];
        if let Ok(executable) = std::env::current_exe() {
            if let Some(parent) = executable.parent() {
                roots.push(PetResourceRoot::new(
                    parent.join("desktop-pets"),
                    PetPackageSource::Portable,
                ));
                roots.push(PetResourceRoot::new(
                    parent.join("resources").join("desktop-pets"),
                    PetPackageSource::Portable,
                ));
            }
        }
        DesktopPetManager::new(
            roots,
            app_data_dir.join("desktop-pets"),
            app_data_dir.join("desktop-pet-settings.json"),
        )
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

fn map_notification(record: AdminNotificationRecord) -> NativeNotificationRow {
    NativeNotificationRow {
        id: record.notification_id,
        title: record.title,
        content: record.content,
        issued_by_nickname: record.issued_by_nickname,
        status: record.status,
        created_at: record.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::NativeAppServices;
    use crate::protocol::AdminNotificationFrame;
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

    #[test]
    fn loads_only_notifications_received_by_the_local_device() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Arc::new(Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage"));
        let profile = storage.get_or_create_profile().expect("profile");
        for (id, target, title) in [
            ("received", profile.device_id.as_str(), "本机公告"),
            ("sent", "other-device", "其他设备公告"),
        ] {
            storage
                .upsert_admin_notification(&AdminNotificationFrame {
                    notification_id: id.to_string(),
                    target_device_id: target.to_string(),
                    title: title.to_string(),
                    content: "通知内容".to_string(),
                    template: "source".to_string(),
                    support_url: None,
                    display_mode: "dismissible".to_string(),
                    deadline_at: None,
                    timeout_policy: "manual_review".to_string(),
                    force_open_main_window: false,
                    issued_by_device_id: "admin".to_string(),
                    issued_by_nickname: "管理员".to_string(),
                    created_at: 10,
                })
                .expect("notification saved");
        }

        let history = NativeAppServices::new(storage)
            .load_local_notification_history()
            .expect("notification history");

        assert_eq!(1, history.len());
        assert_eq!("本机公告", history[0].title);
    }

    #[test]
    fn updates_the_local_profile_nickname() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Arc::new(Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage"));
        let services = NativeAppServices::new(storage);

        let profile = services
            .update_profile_nickname("原生版昵称")
            .expect("profile updated");

        assert_eq!(profile.nickname, "原生版昵称");
        assert!(services.update_profile_nickname("   ").is_err());
    }
}
