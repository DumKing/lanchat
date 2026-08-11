include!(concat!(env!("OUT_DIR"), "/native_main_ui.rs"));

use crate::native_app::{NativeAppServices, NativeUiSettings, PetWindow, TextKey, Translator};
use crate::storage::DEFAULT_GROUP_ID;
use crate::{
    desktop_pet::{DesktopPetManager, PetPackageSource, PetResourceRoot},
    native_app::initial_idle_frame,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};

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
    Ok(Some(pet_window))
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
    use super::NativePage;

    #[test]
    fn native_shell_opens_on_chat_page() {
        assert_eq!(NativePage::default(), NativePage::Chat);
    }
}
