slint::include_modules!();

use crate::native_app::NativeAppServices;
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
    let sidebar = services.load_sidebar()?;
    let window = MainWindow::new().map_err(|error| format!("创建原生主窗口失败：{error}"))?;
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
