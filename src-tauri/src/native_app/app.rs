slint::include_modules!();

use crate::native_app::{NativeAppServices, NativeUiSettings, TextKey, Translator};
use crate::storage::DEFAULT_GROUP_ID;
use slint::{ModelRc, SharedString, VecModel};

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
    window
        .run()
        .map_err(|error| format!("运行原生主窗口失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::NativePage;

    #[test]
    fn native_shell_opens_on_chat_page() {
        assert_eq!(NativePage::default(), NativePage::Chat);
    }
}
