use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeUiSettings {
    pub locale: Locale,
    pub theme: ThemePreference,
}

impl Default for NativeUiSettings {
    fn default() -> Self {
        Self {
            locale: Locale::ZhCn,
            theme: ThemePreference::System,
        }
    }
}

impl NativeUiSettings {
    pub fn load(path: impl AsRef<Path>) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建原生界面设置目录失败：{error}"))?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("序列化原生界面设置失败：{error}"))?;
        std::fs::write(path, bytes).map_err(|error| format!("保存原生界面设置失败：{error}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKey {
    Chat,
    Devices,
    Games,
    Alerts,
    Settings,
    SearchChats,
    LanChannel,
    ChannelBroadcast,
    InputMessage,
    Send,
    InputHint,
}

#[derive(Debug, Clone, Copy)]
pub struct Translator {
    locale: Locale,
}

impl Translator {
    pub fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub fn text(self, key: TextKey) -> &'static str {
        match (self.locale, key) {
            (Locale::ZhCn, TextKey::Chat) => "聊天",
            (Locale::ZhCn, TextKey::Devices) => "设备",
            (Locale::ZhCn, TextKey::Games) => "游戏",
            (Locale::ZhCn, TextKey::Alerts) => "狼来了",
            (Locale::ZhCn, TextKey::Settings) => "设置",
            (Locale::ZhCn, TextKey::SearchChats) => "搜索聊天",
            (Locale::ZhCn, TextKey::LanChannel) => "局域网频道",
            (Locale::ZhCn, TextKey::ChannelBroadcast) => "频道广播",
            (Locale::ZhCn, TextKey::InputMessage) => "输入消息",
            (Locale::ZhCn, TextKey::Send) => "发送",
            (Locale::ZhCn, TextKey::InputHint) => "Enter 发送 · Shift+Enter 换行",
            (Locale::EnUs, TextKey::Chat) => "Chats",
            (Locale::EnUs, TextKey::Devices) => "Devices",
            (Locale::EnUs, TextKey::Games) => "Games",
            (Locale::EnUs, TextKey::Alerts) => "Wolf Alert",
            (Locale::EnUs, TextKey::Settings) => "Settings",
            (Locale::EnUs, TextKey::SearchChats) => "Search chats",
            (Locale::EnUs, TextKey::LanChannel) => "LAN Channel",
            (Locale::EnUs, TextKey::ChannelBroadcast) => "Channel broadcast",
            (Locale::EnUs, TextKey::InputMessage) => "Type a message",
            (Locale::EnUs, TextKey::Send) => "Send",
            (Locale::EnUs, TextKey::InputHint) => "Enter to send · Shift+Enter for a new line",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Locale, NativeUiSettings, ThemePreference};

    #[test]
    fn settings_default_to_simplified_chinese_and_system_theme() {
        let settings = NativeUiSettings::default();

        assert_eq!(Locale::ZhCn, settings.locale);
        assert_eq!(ThemePreference::System, settings.theme);
    }

    #[test]
    fn settings_round_trip_and_invalid_file_falls_back_to_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("native-ui-settings.json");
        let settings = NativeUiSettings {
            locale: Locale::EnUs,
            theme: ThemePreference::Dark,
        };

        settings.save(&path).expect("settings saved");
        assert_eq!(settings, NativeUiSettings::load(&path));

        std::fs::write(&path, "not-json").expect("invalid file written");
        assert_eq!(NativeUiSettings::default(), NativeUiSettings::load(&path));
    }
}
