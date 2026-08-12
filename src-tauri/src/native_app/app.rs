include!(concat!(env!("OUT_DIR"), "/native_main_ui.rs"));

use crate::native_app::{
    game_room_from_frame, native_game_catalog, Locale, NativeAppServices, NativeEventBus,
    NativeGameRoomStore, NativeUiSettings, TextKey, Translator,
};
use crate::storage::DEFAULT_GROUP_ID;
use crate::{
    desktop_pet::{
        DesktopPetManager, PetEvent, PetPackageSource, PetResourceRoot, PetStateMachine,
    },
    desktop_pet_runtime::{DesktopPetController, DesktopPetRuntimeState},
    network::Network,
    runtime_events::NetworkEventSink,
};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
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

fn locale_code(locale: Locale) -> &'static str {
    match locale {
        Locale::ZhCn => "zh-CN",
        Locale::EnUs => "en-US",
    }
}

fn locale_from_code(value: &str) -> Option<Locale> {
    match value {
        "zh-CN" => Some(Locale::ZhCn),
        "en-US" => Some(Locale::EnUs),
        _ => None,
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
    let network_events = NativeEventBus::default();
    let room_store = Rc::new(RefCell::new(NativeGameRoomStore::default()));
    let network = Network::new_with_desktop_pet(services.storage(), native_desktop_pet_manager());
    network.start_native(NetworkEventSink::native(network_events.clone()))?;
    let pet_controller = Rc::new(RefCell::new(start_native_desktop_pet(
        network_events.clone(),
    )?));
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
    window.set_channel_broadcast(SharedString::from(
        translator.text(TextKey::ChannelBroadcast),
    ));
    window.set_input_message(SharedString::from(translator.text(TextKey::InputMessage)));
    window.set_send_label(SharedString::from(translator.text(TextKey::Send)));
    window.set_input_hint(SharedString::from(translator.text(TextKey::InputHint)));
    window.set_current_locale(SharedString::from(locale_code(ui_settings.locale)));
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
            kind: SharedString::from(message.message_type),
            outgoing: message.outgoing,
        })
        .collect::<Vec<_>>();
    window.set_messages(ModelRc::new(VecModel::from(rows)));
    window.set_profile_nickname(SharedString::from(sidebar.profile.nickname.clone()));
    let pets = services
        .list_desktop_pets()
        .into_iter()
        .map(|pet| PetOption {
            id: SharedString::from(pet.id),
            name: SharedString::from(pet.name),
            selected: pet.selected,
        })
        .collect::<Vec<_>>();
    window.set_pets(ModelRc::new(VecModel::from(pets)));
    window.set_pet_enabled(services.desktop_pet_enabled());
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
            id: SharedString::from(peer.device_id.clone()),
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
    window.set_selected_device(DeviceDetail::default());
    window.set_page_title(SharedString::from(native_page_title(NativePage::Chat)));
    window.set_page(0);
    window.set_active_conversation_is_group(true);
    window.set_active_peer_online(true);
    window.set_active_peer_address(SharedString::from("局域网频道 · 设备广播"));
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
    window.set_alerts(ModelRc::new(VecModel::from(Vec::<AlertItem>::new())));
    window.set_games(ModelRc::new(VecModel::from(
        native_game_catalog()
            .into_iter()
            .map(|game| GameOption {
                id: SharedString::from(game.id),
                name: SharedString::from(game.name),
                description: SharedString::from(game.description),
                icon: SharedString::from(game.icon),
                players: SharedString::from(format!(
                    "{}-{} 人",
                    game.min_players, game.max_players
                )),
            })
            .collect::<Vec<_>>(),
    )));
    window.set_game_rooms(ModelRc::new(VecModel::from(Vec::<GameRoom>::new())));
    window.set_channel_members(ModelRc::new(VecModel::from(channel_member_rows(
        services
            .load_channel_members(DEFAULT_GROUP_ID)
            .unwrap_or_default(),
    ))));
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
                kind: SharedString::from(message.message_type),
                outgoing: message.outgoing,
            })
            .collect::<Vec<_>>();
        let title = message_sidebar
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| native_page_title(NativePage::Chat).to_string());
        let is_group = conversation_id == DEFAULT_GROUP_ID
            || message_sidebar
                .conversations
                .iter()
                .find(|conversation| conversation.id == conversation_id)
                .is_some_and(|conversation| conversation.is_group);
        let peer = (!is_group)
            .then(|| {
                message_sidebar
                    .peers
                    .iter()
                    .find(|peer| peer.device_id == conversation_id)
            })
            .flatten();
        let peer_online = peer.map(|peer| peer.online).unwrap_or(true);
        let peer_address = peer
            .map(|peer| peer.address.clone())
            .unwrap_or_else(|| "局域网频道 · 设备广播".to_string());
        let members = channel_member_rows(
            message_services
                .load_channel_members(&conversation_id)
                .unwrap_or_default(),
        );
        let _ = message_window.upgrade_in_event_loop(move |window| {
            window.set_messages(ModelRc::new(VecModel::from(rows)));
            window.set_channel_members(ModelRc::new(VecModel::from(members)));
            window.set_page(0);
            window.set_page_title(SharedString::from(title));
            window.set_active_conversation_is_group(is_group);
            window.set_active_peer_online(peer_online);
            window.set_active_peer_address(SharedString::from(peer_address));
        });
    });
    let device_services = services.clone();
    let device_window = window.as_weak();
    window.on_select_device(move |device_id| {
        let detail = device_services.load_peer_detail(&device_id).ok().flatten();
        let _ = device_window.upgrade_in_event_loop(move |window| {
            if let Some(detail) = detail {
                window.set_selected_device(DeviceDetail {
                    nickname: SharedString::from(detail.nickname),
                    device_id: SharedString::from(detail.device_id),
                    address: SharedString::from(detail.address),
                    status: SharedString::from(if detail.online { "在线" } else { "离线" }),
                    capability: SharedString::from(if detail.supports_chat {
                        "可聊天"
                    } else {
                        "仅告警"
                    }),
                    client_kind: SharedString::from(detail.client_kind),
                    build_version: SharedString::from(detail.build_version),
                });
            }
        });
    });
    let profile_services = services.clone();
    let profile_window = window.as_weak();
    window.on_save_profile(move |nickname| {
        let result = profile_services.update_profile_nickname(&nickname);
        let _ = profile_window.upgrade_in_event_loop(move |window| match result {
            Ok(profile) => {
                window.set_profile_nickname(SharedString::from(profile.nickname));
                window.set_settings_feedback(SharedString::from("昵称已保存"));
            }
            Err(error) => window.set_settings_feedback(SharedString::from(error)),
        });
    });
    let locale_window = window.as_weak();
    let locale_path = NativeAppServices::default_app_data_dir().join("native-ui-settings.json");
    window.on_set_locale(move |locale| {
        let locale = locale_from_code(&locale).unwrap_or(Locale::ZhCn);
        let mut settings = NativeUiSettings::load(&locale_path);
        settings.locale = locale;
        let result = settings.save(&locale_path);
        let translator = Translator::new(locale);
        let _ = locale_window.upgrade_in_event_loop(move |window| match result {
            Ok(()) => {
                window.set_current_locale(SharedString::from(locale_code(locale)));
                window.set_nav_chat(SharedString::from(translator.text(TextKey::Chat)));
                window.set_nav_devices(SharedString::from(translator.text(TextKey::Devices)));
                window.set_nav_games(SharedString::from(translator.text(TextKey::Games)));
                window.set_nav_alerts(SharedString::from(translator.text(TextKey::Alerts)));
                window.set_nav_settings(SharedString::from(translator.text(TextKey::Settings)));
                window.set_search_chats(SharedString::from(translator.text(TextKey::SearchChats)));
                window.set_lan_channel(SharedString::from(translator.text(TextKey::LanChannel)));
                window.set_channel_broadcast(SharedString::from(
                    translator.text(TextKey::ChannelBroadcast),
                ));
                window
                    .set_input_message(SharedString::from(translator.text(TextKey::InputMessage)));
                window.set_send_label(SharedString::from(translator.text(TextKey::Send)));
                window.set_input_hint(SharedString::from(translator.text(TextKey::InputHint)));
                window.set_settings_feedback(SharedString::from("语言设置已保存"));
            }
            Err(error) => window.set_settings_feedback(SharedString::from(error)),
        });
    });
    let pet_services = services.clone();
    let pet_window = window.as_weak();
    let pet_selection_controller = pet_controller.clone();
    window.on_select_pet(move |pet_id| {
        let result = pet_services.select_desktop_pet(&pet_id);
        if result.is_ok() {
            if let Some(controller) = pet_selection_controller.borrow().as_ref() {
                controller.set_package(native_desktop_pet_manager().selected_package());
            }
        }
        let pets = pet_services
            .list_desktop_pets()
            .into_iter()
            .map(|pet| PetOption {
                id: SharedString::from(pet.id),
                name: SharedString::from(pet.name),
                selected: pet.selected,
            })
            .collect::<Vec<_>>();
        let _ = pet_window.upgrade_in_event_loop(move |window| {
            window.set_pets(ModelRc::new(VecModel::from(pets)));
            let feedback = match result {
                Ok(()) => "桌宠已切换".to_string(),
                Err(error) => error,
            };
            window.set_settings_feedback(SharedString::from(feedback));
        });
    });
    let message_network = network.clone();
    let message_services = services.clone();
    let message_events = network_events.clone();
    let send_window = window.as_weak();
    window.on_send_message(move |content| {
        let content = content.trim().to_string();
        if content.is_empty() {
            return;
        }
        let profile = match message_services.load_sidebar() {
            Ok(sidebar) => sidebar.profile,
            Err(error) => {
                let _ = send_window.upgrade_in_event_loop(move |window| {
                    window.set_send_feedback(SharedString::from(error));
                });
                return;
            }
        };
        let message = crate::storage::Message {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: send_window
                .upgrade()
                .map(|window| window.get_active_conversation_id().to_string())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GROUP_ID.to_string()),
            sender_device_id: profile.device_id,
            content,
            message_type: crate::storage::MessageType::Text,
            file_meta: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            status: crate::storage::MessageStatus::Sending,
            simulation: None,
        };
        let result = tauri::async_runtime::block_on(
            message_network.send_message(NetworkEventSink::native(message_events.clone()), message),
        );
        let _ = send_window.upgrade_in_event_loop(move |window| match result {
            Ok(()) => {
                window.set_message_input(SharedString::default());
                window.set_send_feedback(SharedString::from("已发送"));
            }
            Err(error) => window.set_send_feedback(SharedString::from(error)),
        });
    });
    let pet_toggle_services = services.clone();
    let pet_toggle_window = window.as_weak();
    let pet_toggle_controller = pet_controller.clone();
    let pet_toggle_events = network_events.clone();
    window.on_toggle_pet(move |enabled| {
        let result = pet_toggle_services.set_desktop_pet_enabled(enabled);
        if result.is_ok() {
            if enabled && pet_toggle_controller.borrow().is_none() {
                if let Ok(controller) = start_native_desktop_pet(pet_toggle_events.clone()) {
                    *pet_toggle_controller.borrow_mut() = controller;
                }
            }
            if let Some(controller) = pet_toggle_controller.borrow().as_ref() {
                controller.set_enabled(enabled);
            }
        }
        let _ = pet_toggle_window.upgrade_in_event_loop(move |window| match result {
            Ok(()) => {
                window.set_pet_enabled(enabled);
                window.set_settings_feedback(SharedString::from(if enabled {
                    "桌宠已开启"
                } else {
                    "桌宠已关闭"
                }));
            }
            Err(error) => window.set_settings_feedback(SharedString::from(error)),
        });
    });
    let alert_network = network.clone();
    let alert_services = services.clone();
    let alert_events = network_events.clone();
    window.on_send_alert(move || {
        let Ok(profile) = alert_services.load_sidebar().map(|sidebar| sidebar.profile) else {
            return;
        };
        let frame = crate::protocol::QuickAlertFrame {
            alert_id: uuid::Uuid::new_v4().to_string(),
            sender_device_id: profile.device_id,
            sender_nickname: profile.nickname,
            sender_address: Some(crate::network::local_ip_address()),
            content: "呱呱~呱~~".to_string(),
            mode: "normal".to_string(),
            simulation: None,
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let _ = tauri::async_runtime::block_on(alert_network.broadcast_quick_alert(
            NetworkEventSink::native(alert_events.clone()),
            frame.clone(),
        ));
        alert_events.publish(
            "quick_alert_received",
            serde_json::to_value(frame).unwrap_or_default(),
        );
    });
    let feedback_network = network.clone();
    let feedback_services = services.clone();
    let feedback_events = network_events.clone();
    window.on_feedback_alert(move |alert_id, sender_device_id, result| {
        let Ok(profile) = feedback_services
            .load_sidebar()
            .map(|sidebar| sidebar.profile)
        else {
            return;
        };
        let _ = tauri::async_runtime::block_on(feedback_network.broadcast_quick_alert_feedback(
            NetworkEventSink::native(feedback_events.clone()),
            crate::protocol::QuickAlertFeedbackFrame {
                alert_id: alert_id.to_string(),
                alert_sender_device_id: sender_device_id.to_string(),
                responder_device_id: profile.device_id,
                responder_nickname: profile.nickname,
                result: result.to_string(),
                created_at: chrono::Utc::now().timestamp_millis(),
            },
        ));
    });
    let game_network = network.clone();
    let game_services = services.clone();
    let game_events = network_events.clone();
    let game_window = window.as_weak();
    window.on_create_game_room(move |game_id| {
        let game_id = game_id.to_string();
        let Ok(profile) = game_services.load_sidebar().map(|sidebar| sidebar.profile) else {
            return;
        };
        let room_id = format!("{}-{}", game_id, uuid::Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "roomId": room_id,
            "gameType": game_id,
            "roomName": format!("{}房间", game_name(&game_id)),
            "hostDeviceId": profile.device_id,
            "hostName": profile.nickname,
            "players": [{ "deviceId": profile.device_id, "nickname": profile.nickname, "online": true, "ready": false }],
            "createdAt": chrono::Utc::now().timestamp_millis(),
        });
        let frame = crate::protocol::GameFrame {
            frame_id: uuid::Uuid::new_v4().to_string(),
            game: game_id.clone(),
            room_id,
            sender_device_id: profile.device_id,
            sender_nickname: profile.nickname,
            kind: "room_created".to_string(),
            payload,
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let result = tauri::async_runtime::block_on(game_network.send_game_frame(
            NetworkEventSink::native(game_events.clone()),
            None,
            frame.clone(),
        ));
        game_events.publish("game_frame_received", serde_json::to_value(frame).unwrap_or_default());
        let _ = game_window.upgrade_in_event_loop(move |window| {
            window.set_page(2);
            window.set_page_title(SharedString::from(match result {
                Ok(()) => "房间已创建，已向在线设备广播",
                Err(_) => "房间已创建，等待设备上线后邀请",
            }));
        });
    });
    let join_network = network.clone();
    let join_services = services.clone();
    let join_events = network_events.clone();
    window.on_join_game_room(move |room_id, game_id| {
        let Ok(profile) = join_services.load_sidebar().map(|sidebar| sidebar.profile) else {
            return;
        };
        let frame = crate::protocol::GameFrame {
            frame_id: uuid::Uuid::new_v4().to_string(),
            game: game_id.to_string(),
            room_id: room_id.to_string(),
            sender_device_id: profile.device_id,
            sender_nickname: profile.nickname,
            kind: "room_action".to_string(),
            payload: serde_json::json!({ "roomId": room_id.to_string(), "action": "join" }),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let _ = tauri::async_runtime::block_on(join_network.send_game_frame(
            NetworkEventSink::native(join_events.clone()),
            None,
            frame,
        ));
    });
    let pet_state = Rc::new(RefCell::new(PetStateMachine::new()));
    start_native_network_refresh(
        &window,
        services.clone(),
        network_events,
        network.clone(),
        sidebar.profile.nickname.clone(),
        sidebar.profile.device_id.clone(),
        room_store,
        pet_state,
        pet_controller,
    );
    window
        .show()
        .map_err(|error| format!("显示原生主窗口失败：{error}"))?;
    slint::run_event_loop().map_err(|error| format!("运行原生界面事件循环失败：{error}"))
}

fn channel_member_rows(
    members: Vec<crate::native_app::NativeChannelMemberRow>,
) -> Vec<ChannelMember> {
    members
        .into_iter()
        .map(|member| ChannelMember {
            nickname: SharedString::from(member.nickname),
            detail: SharedString::from(if member.is_owner {
                "群主"
            } else if member.muted {
                "已禁言"
            } else if member.online {
                "在线"
            } else {
                "离线"
            }),
            online: member.online,
        })
        .collect()
}

fn game_name(game_id: &str) -> &'static str {
    native_game_catalog()
        .into_iter()
        .find(|game| game.id == game_id)
        .map(|game| game.name)
        .unwrap_or("局域网游戏")
}

fn start_native_network_refresh(
    window: &MainWindow,
    services: NativeAppServices,
    event_bus: NativeEventBus,
    network: Network,
    local_nickname: String,
    local_device_id: String,
    room_store: Rc<RefCell<NativeGameRoomStore>>,
    pet_state: Rc<RefCell<PetStateMachine>>,
    pet_controller: Rc<RefCell<Option<DesktopPetController>>>,
) {
    let window = window.as_weak();
    let alert_frames = Rc::new(RefCell::new(HashMap::new()));
    let timer = Box::leak(Box::new(Timer::default()));
    timer.start(TimerMode::Repeated, Duration::from_millis(300), move || {
        let events = event_bus.drain();
        if events.is_empty() {
            return;
        }
        let quick_alerts = events
            .iter()
            .filter(|event| event.name == "quick_alert_received")
            .filter_map(|event| {
                serde_json::from_value::<crate::protocol::QuickAlertFrame>(event.payload.clone())
                    .ok()
            })
            .collect::<Vec<_>>();
        for alert in &quick_alerts {
            alert_frames
                .borrow_mut()
                .insert(alert.alert_id.clone(), alert.clone());
        }
        let alerts = quick_alerts
            .iter()
            .cloned()
            .map(|alert| alert_item_from_frame(alert, &local_device_id))
            .collect::<Vec<_>>();
        if !alerts.is_empty() {
            pet_state.borrow_mut().handle(PetEvent::AlertRaised);
            if let (Some(controller), Some(alert)) =
                (pet_controller.borrow().as_ref(), quick_alerts.last())
            {
                controller.update(DesktopPetRuntimeState {
                    enabled: true,
                    pending_count: alerts.len() as u32,
                    temperature: 100,
                    latest_alert_id: Some(alert.alert_id.clone()),
                    latest_sender: Some(alert.sender_nickname.clone()),
                    latest_sender_address: alert.sender_address.clone(),
                    latest_content: Some(alert.content.clone()),
                    latest_created_at: Some(alert.created_at),
                    feedbackable: alert.sender_device_id != local_device_id,
                    flashing: true,
                    disco: alert.mode.eq_ignore_ascii_case("disco"),
                    ..Default::default()
                });
            }
        }
        for action in events
            .iter()
            .filter(|event| event.name == "desktop_pet_action")
            .filter_map(|event| {
                serde_json::from_value::<crate::desktop_pet_runtime::DesktopPetAction>(
                    event.payload.clone(),
                )
                .ok()
            })
        {
            handle_native_pet_action(
                &action,
                &window,
                &services,
                &network,
                &event_bus,
                &alert_frames,
                &pet_controller,
            );
        }
        for room in events
            .iter()
            .filter(|event| event.name == "game_frame_received")
            .filter_map(|event| {
                serde_json::from_value::<crate::protocol::GameFrame>(event.payload.clone()).ok()
            })
            .filter(|frame| frame.kind == "room_created")
            .map(|frame| game_room_from_frame(&frame))
        {
            room_store.borrow_mut().upsert(room);
        }
        let rooms = room_store
            .borrow()
            .rows()
            .into_iter()
            .map(|room| GameRoom {
                id: SharedString::from(room.id),
                game_id: SharedString::from(room.game_id),
                game_name: SharedString::from(room.game_name),
                name: SharedString::from(room.name),
                host: SharedString::from(room.host),
                players: SharedString::from(room.players),
            })
            .collect::<Vec<_>>();
        let Ok(sidebar) = services.load_sidebar() else {
            return;
        };
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
        let peers = sidebar
            .peers
            .iter()
            .map(|peer| Device {
                id: SharedString::from(peer.device_id.clone()),
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
        let conversation_id = window
            .upgrade()
            .map(|window| window.get_active_conversation_id().to_string())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GROUP_ID.to_string());
        let active_conversation = sidebar
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id);
        let active_title = active_conversation
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| native_page_title(NativePage::Chat).to_string());
        let active_is_group = conversation_id == DEFAULT_GROUP_ID
            || active_conversation.is_some_and(|conversation| conversation.is_group);
        let active_peer = (!active_is_group)
            .then(|| {
                sidebar
                    .peers
                    .iter()
                    .find(|peer| peer.device_id == conversation_id)
            })
            .flatten();
        let active_peer_online = active_peer.map(|peer| peer.online).unwrap_or(true);
        let active_peer_address = active_peer
            .map(|peer| peer.address.clone())
            .unwrap_or_else(|| "局域网频道 · 设备广播".to_string());
        let members = channel_member_rows(
            services
                .load_channel_members(&conversation_id)
                .unwrap_or_default(),
        );
        let Ok(messages) = services.load_messages(&conversation_id, None) else {
            return;
        };
        let rows = messages
            .into_iter()
            .map(|message| ChatMessage {
                author: SharedString::from(if message.outgoing {
                    local_nickname.clone()
                } else {
                    message.sender_device_id
                }),
                content: SharedString::from(message.content),
                kind: SharedString::from(message.message_type),
                outgoing: message.outgoing,
            })
            .collect::<Vec<_>>();
        if let Some(window) = window.upgrade() {
            window.set_conversations(ModelRc::new(VecModel::from(conversations)));
            window.set_peers(ModelRc::new(VecModel::from(peers)));
            window.set_messages(ModelRc::new(VecModel::from(rows)));
            window.set_page_title(SharedString::from(active_title));
            window.set_active_conversation_is_group(active_is_group);
            window.set_active_peer_online(active_peer_online);
            window.set_active_peer_address(SharedString::from(active_peer_address));
            window.set_channel_members(ModelRc::new(VecModel::from(members)));
            if !alerts.is_empty() {
                window.set_alerts(ModelRc::new(VecModel::from(alerts)));
            }
            if !rooms.is_empty() {
                window.set_game_rooms(ModelRc::new(VecModel::from(rooms)));
            }
        }
    });
}

fn alert_item_from_frame(
    alert: crate::protocol::QuickAlertFrame,
    local_device_id: &str,
) -> AlertItem {
    let feedback_allowed = alert.sender_device_id != local_device_id;
    AlertItem {
        id: SharedString::from(alert.alert_id),
        sender_device_id: SharedString::from(alert.sender_device_id),
        sender: SharedString::from(alert.sender_nickname),
        source: SharedString::from(
            alert
                .sender_address
                .unwrap_or_else(|| "未知 IP".to_string()),
        ),
        content: SharedString::from(alert.content),
        feedback_allowed,
    }
}

fn handle_native_pet_action(
    action: &crate::desktop_pet_runtime::DesktopPetAction,
    window: &slint::Weak<MainWindow>,
    services: &NativeAppServices,
    network: &Network,
    events: &NativeEventBus,
    alerts: &Rc<RefCell<HashMap<String, crate::protocol::QuickAlertFrame>>>,
    pet_controller: &Rc<RefCell<Option<DesktopPetController>>>,
) {
    match action.action.as_str() {
        "open_main_window" => {
            let _ = window.upgrade_in_event_loop(|window| {
                let _ = window.show();
            });
        }
        "stop_visuals" => {
            if let Some(controller) = pet_controller.borrow().as_ref() {
                controller.update(DesktopPetRuntimeState {
                    enabled: true,
                    ..Default::default()
                });
            }
        }
        "quick_alert" | "broadcast_disco_alert" => {
            let Ok(profile) = services.load_sidebar().map(|sidebar| sidebar.profile) else {
                return;
            };
            let frame = crate::protocol::QuickAlertFrame {
                alert_id: uuid::Uuid::new_v4().to_string(),
                sender_device_id: profile.device_id,
                sender_nickname: profile.nickname,
                sender_address: Some(crate::network::local_ip_address()),
                content: "呱呱~呱~~".to_string(),
                mode: if action.action == "broadcast_disco_alert" {
                    "disco".to_string()
                } else {
                    "normal".to_string()
                },
                simulation: None,
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            let _ = tauri::async_runtime::block_on(
                network
                    .broadcast_quick_alert(NetworkEventSink::native(events.clone()), frame.clone()),
            );
            events.publish(
                "quick_alert_received",
                serde_json::to_value(frame).unwrap_or_default(),
            );
        }
        "feedback_real" | "feedback_false" => {
            let Some(alert_id) = action.alert_id.as_ref() else {
                return;
            };
            let Some(alert) = alerts.borrow().get(alert_id).cloned() else {
                return;
            };
            let Ok(profile) = services.load_sidebar().map(|sidebar| sidebar.profile) else {
                return;
            };
            let result = if action.action == "feedback_real" {
                "real"
            } else {
                "false"
            };
            let _ = tauri::async_runtime::block_on(network.broadcast_quick_alert_feedback(
                NetworkEventSink::native(events.clone()),
                crate::protocol::QuickAlertFeedbackFrame {
                    alert_id: alert.alert_id,
                    alert_sender_device_id: alert.sender_device_id,
                    responder_device_id: profile.device_id,
                    responder_nickname: profile.nickname,
                    result: result.to_string(),
                    created_at: chrono::Utc::now().timestamp_millis(),
                },
            ));
            if let Some(controller) = pet_controller.borrow().as_ref() {
                controller.update(DesktopPetRuntimeState {
                    enabled: true,
                    ..Default::default()
                });
            }
        }
        _ => {}
    }
}

fn start_native_desktop_pet(
    event_bus: NativeEventBus,
) -> Result<Option<DesktopPetController>, String> {
    let manager = native_desktop_pet_manager();
    if !manager.settings().enabled {
        return Ok(None);
    }
    let Some(package) = manager.selected_package() else {
        return Ok(None);
    };
    let controller = DesktopPetController::start_for_native_ui(move |action| {
        event_bus.publish(
            "desktop_pet_action",
            serde_json::to_value(action).unwrap_or_default(),
        );
    });
    controller.set_package(Some(package));
    controller.set_enabled(true);
    Ok(Some(controller))
}

fn native_desktop_pet_manager() -> DesktopPetManager {
    let app_data_dir = NativeAppServices::default_app_data_dir();
    DesktopPetManager::new(
        native_pet_resource_roots(&app_data_dir),
        app_data_dir.join("desktop-pets"),
        app_data_dir.join("desktop-pet-settings.json"),
    )
}

fn native_pet_resource_roots(app_data_dir: &Path) -> Vec<PetResourceRoot> {
    let mut roots = vec![PetResourceRoot::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("desktop-pets"),
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
    use super::{alert_item_from_frame, native_page_title, NativePage};
    use crate::protocol::QuickAlertFrame;

    #[test]
    fn native_shell_opens_on_chat_page() {
        assert_eq!(NativePage::default(), NativePage::Chat);
    }

    #[test]
    fn notification_page_has_a_localized_title() {
        assert_eq!(native_page_title(NativePage::Alerts), "告警与通知");
    }

    #[test]
    fn alert_item_keeps_sender_identity_for_feedback() {
        let item = alert_item_from_frame(
            QuickAlertFrame {
                alert_id: "alert-1".to_string(),
                sender_device_id: "AA-BB-CC".to_string(),
                sender_nickname: "测试设备".to_string(),
                sender_address: Some("192.168.1.12".to_string()),
                content: "呱呱~呱~~".to_string(),
                mode: "normal".to_string(),
                simulation: None,
                created_at: 1,
            },
            "本机设备",
        );

        assert_eq!(item.sender_device_id, "AA-BB-CC");
        assert!(item.feedback_allowed);
    }
}
