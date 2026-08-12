include!(concat!(env!("OUT_DIR"), "/native_main_ui.rs"));

use crate::native_app::{NativeAppServices, NativeUiSettings, PetWindow, TextKey, Translator};
use crate::storage::DEFAULT_GROUP_ID;
use crate::{
    desktop_pet::{DesktopPetManager, DesktopPetPackage, PetPackageSource, PetResourceRoot, PetStateKind},
    native_app::initial_idle_frame,
};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativePage {
    Chat,
    Devices,
    Games,
    Alerts,
    Settings,
}

const NAVIGATION_PAGES: [NativePage; 5] = [
    NativePage::Chat,
    NativePage::Devices,
    NativePage::Games,
    NativePage::Alerts,
    NativePage::Settings,
];

fn native_page_title(page: NativePage) -> &'static str {
    match page {
        NativePage::Chat => "局域网频道",
        NativePage::Devices => "设备通讯录",
        NativePage::Games => "内置游戏",
        NativePage::Alerts => "告警与通知",
        NativePage::Settings => "设置",
    }
}

impl Default for NativePage {
    fn default() -> Self {
        Self::Chat
    }
}

pub fn run() -> Result<(), String> {
    let _initial_page = NativePage::default();
    let _available_pages = NAVIGATION_PAGES;
    let services = NativeAppServices::open_default()?;
    let ui_settings = NativeUiSettings::load(
        NativeAppServices::default_app_data_dir().join("native-ui-settings.json"),
    );
    let translator = Translator::new(ui_settings.locale);
    let sidebar = services.load_sidebar()?;
    let window = MainWindow::new().map_err(|error| format!("创建原生主窗口失败：{error}"))?;
    window.set_app_title(SharedString::from("LanChat"));
    window.set_nav_chat(SharedString::from(translator.text(TextKey::Chat)));
    window.set_nav_devices(SharedString::from(translator.text(TextKey::Devices)));
    window.set_nav_games(SharedString::from(translator.text(TextKey::Games)));
    window.set_nav_alerts(SharedString::from(translator.text(TextKey::Alerts)));
    window.set_nav_settings(SharedString::from(translator.text(TextKey::Settings)));
    window.set_search_chats(SharedString::from(translator.text(TextKey::SearchChats)));
    window.set_lan_channel(SharedString::from(translator.text(TextKey::LanChannel)));
    window.set_channel_broadcast(SharedString::from(translator.text(TextKey::ChannelBroadcast)));
    window.set_input_message(SharedString::from(translator.text(TextKey::InputMessage)));
    window.set_send_label(SharedString::from(translator.text(TextKey::Send)));
    window.set_input_hint(SharedString::from(translator.text(TextKey::InputHint)));
    let messages = services.load_messages(DEFAULT_GROUP_ID, None)?;
    let rows = messages
        .into_iter()
        .map(|message| ChatMessage {
            author: SharedString::from(if message.outgoing {
                sidebar.profile.nickname.clone()
            } else {
                message.sender_device_id
            }),
            content: SharedString::from(message.content),
            outgoing: message.outgoing,
        })
        .collect::<Vec<_>>();
    window.set_messages(ModelRc::new(VecModel::from(rows)));
    let conversations = sidebar
        .conversations
        .iter()
        .map(|conversation| Conversation {
            id: SharedString::from(conversation.id.clone()),
            title: SharedString::from(conversation.title.clone()),
            unread_count: SharedString::from(
                (conversation.unread_count > 0)
                    .then(|| conversation.unread_count.to_string())
                    .unwrap_or_default(),
            ),
            subtitle: SharedString::from(if conversation.is_group {
                "频道"
            } else {
                "私聊"
            }),
        })
        .collect::<Vec<_>>();
    window.set_conversations(ModelRc::new(VecModel::from(conversations)));
    let peers = sidebar
        .peers
        .iter()
        .map(|peer| Device {
            nickname: SharedString::from(peer.display_name.clone()),
            address: SharedString::from(peer.address.clone()),
            status: SharedString::from(if peer.online { "在线" } else { "离线" }),
            capability: SharedString::from(if peer.supports_chat {
                "可聊天"
            } else {
                "仅告警"
            }),
        })
        .collect::<Vec<_>>();
    window.set_peers(ModelRc::new(VecModel::from(peers)));
    window.set_page_title(SharedString::from(native_page_title(NativePage::Chat)));
    window.set_page(0);
    let notification_rows = services
        .load_local_notification_history()?
        .into_iter()
        .map(|notification| AdminNotification {
            title: SharedString::from(notification.title),
            content: SharedString::from(notification.content),
            status: SharedString::from(notification.status),
        })
        .collect::<Vec<_>>();
    window.set_notifications(ModelRc::new(VecModel::from(notification_rows)));
    let weak_window = window.as_weak();
    window.on_select_page(move |page| {
        let page = match page {
            1 => NativePage::Devices,
            2 => NativePage::Games,
            3 => NativePage::Alerts,
            4 => NativePage::Settings,
            _ => NativePage::Chat,
        };
        let _ = weak_window.upgrade_in_event_loop(move |window| {
            window.set_page(page as i32);
            window.set_page_title(SharedString::from(native_page_title(page)));
        });
    });
    let message_services = services.clone();
    let message_sidebar = sidebar.clone();
    let message_window = window.as_weak();
    window.on_select_conversation(move |conversation_id| {
        let conversation_id = conversation_id.to_string();
        let Ok(messages) = message_services.load_messages(&conversation_id, None) else {
            return;
        };
        let rows = messages
            .into_iter()
            .map(|message| ChatMessage {
                author: SharedString::from(if message.outgoing {
                    message_sidebar.profile.nickname.clone()
                } else {
                    message.sender_device_id
                }),
                content: SharedString::from(message.content),
                outgoing: message.outgoing,
            })
            .collect::<Vec<_>>();
        let title = message_sidebar
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| native_page_title(NativePage::Chat).to_string());
        let _ = message_window.upgrade_in_event_loop(move |window| {
            window.set_messages(ModelRc::new(VecModel::from(rows)));
            window.set_page(0);
            window.set_page_title(SharedString::from(title));
        });
    });
    let pet_window = create_pet_window()?;
    window
        .show()
        .map_err(|error| format!("显示原生主窗口失败：{error}"))?;
    if let Some(pet_window) = &pet_window {
        pet_window
            .show()
            .map_err(|error| format!("显示原生桌宠窗口失败：{error}"))?;
    }
    slint::run_event_loop().map_err(|error| format!("运行原生界面事件循环失败：{error}"))
}

fn create_pet_window() -> Result<Option<PetWindow>, String> {
    let app_data_dir = NativeAppServices::default_app_data_dir();
    let manager = DesktopPetManager::new(
        native_pet_resource_roots(&app_data_dir),
        app_data_dir.join("desktop-pets"),
        app_data_dir.join("desktop-pet-settings.json"),
    );
    if !manager.settings().enabled {
        return Ok(None);
    }
    let Some(package) = manager.selected_package() else {
        return Ok(None);
    };
    let Some(path) = initial_idle_frame(&package) else {
        return Ok(None);
    };
    let image = slint::Image::load_from_path(&path)
        .map_err(|error| format!("加载桌宠首帧失败：{error}"))?;
    let pet_window = PetWindow::new().map_err(|error| format!("创建原生桌宠窗口失败：{error}"))?;
    pet_window.set_pet_image(image);
    start_native_pet_animation(&pet_window, package);
    Ok(Some(pet_window))
}

fn start_native_pet_animation(pet_window: &PetWindow, package: DesktopPetPackage) {
    let window = pet_window.as_weak();
    let clip_index = Rc::new(Cell::new(0usize));
    let elapsed_seconds = Rc::new(Cell::new(0.0f32));
    let timer = Box::leak(Box::new(Timer::default()));
    timer.start(TimerMode::Repeated, Duration::from_millis(80), move || {
        let candidates = package.clip_candidates(PetStateKind::Idle, None);
        if candidates.is_empty() {
            return;
        }
        let current_index = clip_index.get() % candidates.len();
        let clip = candidates[current_index];
        let elapsed = elapsed_seconds.get() + 0.08;
        if elapsed >= DesktopPetPackage::clip_cycle_seconds(clip) {
            clip_index.set((current_index + 1 + fastrand::usize(..candidates.len())) % candidates.len());
            elapsed_seconds.set(0.0);
            return;
        }
        elapsed_seconds.set(elapsed);
        let Some(frame) = DesktopPetPackage::frame_in_clip(clip, elapsed) else {
            return;
        };
        let Ok(image) = slint::Image::load_from_path(&frame.path) else {
            return;
        };
        if let Some(window) = window.upgrade() {
            window.set_pet_image(image);
        }
    });
}

fn native_pet_resource_roots(app_data_dir: &Path) -> Vec<PetResourceRoot> {
    let mut roots = vec![PetResourceRoot::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("desktop-pets"),
        PetPackageSource::BuiltIn,
    )];
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(PetResourceRoot::new(
                parent.join("resources").join("desktop-pets"),
                PetPackageSource::Portable,
            ));
        }
    }
    roots.push(PetResourceRoot::new(
        app_data_dir.join("desktop-pets"),
        PetPackageSource::User,
    ));
    roots
}

#[cfg(test)]
mod tests {
    use super::{native_page_title, NativePage};

    #[test]
    fn native_shell_opens_on_chat_page() {
        assert_eq!(NativePage::default(), NativePage::Chat);
    }

    #[test]
    fn notification_page_has_a_localized_title() {
        assert_eq!(native_page_title(NativePage::Alerts), "告警与通知");
    }
}
