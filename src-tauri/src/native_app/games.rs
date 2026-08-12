#[cfg(test)]
mod tests {
    use super::{game_room_from_frame, native_game_catalog, NativeGameRoomRow, NativeGameRoomStore};
    use crate::protocol::GameFrame;

    #[test]
    fn native_catalog_keeps_all_available_lan_games() {
        let games = native_game_catalog();

        assert_eq!(games.len(), 4);
        assert_eq!(games[0].id, "doudizhu");
        assert_eq!(games[3].id, "xiangqi");
        assert!(games.iter().all(|game| game.max_players >= game.min_players));
    }

    #[test]
    fn room_created_frame_becomes_native_room_row() {
        let room = game_room_from_frame(&GameFrame {
            frame_id: "frame-1".to_string(),
            game: "gomoku".to_string(),
            room_id: "gomoku-1".to_string(),
            sender_device_id: "AA-BB".to_string(),
            sender_nickname: "小王".to_string(),
            kind: "room_created".to_string(),
            payload: serde_json::json!({ "roomName": "午休棋局", "players": [{}, {}] }),
            created_at: 1,
        });

        assert_eq!(room.id, "gomoku-1");
        assert_eq!(room.name, "午休棋局");
        assert_eq!(room.players, "2 人");
    }

    #[test]
    fn room_store_keeps_multiple_discovered_rooms() {
        let mut store = NativeGameRoomStore::default();
        store.upsert(NativeGameRoomRow {
            id: "room-a".to_string(), game_id: "gomoku".to_string(), game_name: "五子棋".to_string(), name: "A".to_string(), host: "甲".to_string(), players: "1 人".to_string(),
        });
        store.upsert(NativeGameRoomRow {
            id: "room-b".to_string(), game_id: "xiangqi".to_string(), game_name: "中国象棋".to_string(), name: "B".to_string(), host: "乙".to_string(), players: "2 人".to_string(),
        });

        assert_eq!(store.rows().len(), 2);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeGameDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub min_players: u8,
    pub max_players: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGameRoomRow {
    pub id: String,
    pub game_id: String,
    pub game_name: String,
    pub name: String,
    pub host: String,
    pub players: String,
}

#[derive(Debug, Default)]
pub struct NativeGameRoomStore {
    rooms: BTreeMap<String, NativeGameRoomRow>,
}

impl NativeGameRoomStore {
    pub fn upsert(&mut self, room: NativeGameRoomRow) {
        self.rooms.insert(room.id.clone(), room);
    }

    pub fn rows(&self) -> Vec<NativeGameRoomRow> {
        self.rooms.values().cloned().collect()
    }
}

pub fn native_game_catalog() -> [NativeGameDefinition; 4] {
    [
        NativeGameDefinition {
            id: "doudizhu",
            name: "斗地主",
            description: "三人局域网娱乐房间，支持房间聊天和实时同步。",
            icon: "斗",
            min_players: 3,
            max_players: 3,
        },
        NativeGameDefinition {
            id: "gomoku",
            name: "五子棋",
            description: "双人局域网棋盘对战，黑白轮流落子，五连即胜。",
            icon: "五",
            min_players: 2,
            max_players: 2,
        },
        NativeGameDefinition {
            id: "minesweeper",
            name: "扫雷竞速",
            description: "单人或多人同图竞速扫雷，先清完非雷格获胜。",
            icon: "雷",
            min_players: 1,
            max_players: 6,
        },
        NativeGameDefinition {
            id: "xiangqi",
            name: "中国象棋",
            description: "双人局域网象棋对局，红黑轮流走子。",
            icon: "象",
            min_players: 2,
            max_players: 2,
        },
    ]
}

pub fn game_room_from_frame(frame: &crate::protocol::GameFrame) -> NativeGameRoomRow {
    let players = frame
        .payload
        .get("players")
        .and_then(serde_json::Value::as_array)
        .map_or(1, Vec::len);
    NativeGameRoomRow {
        id: frame.room_id.clone(),
        game_id: frame.game.clone(),
        game_name: native_game_catalog()
            .into_iter()
            .find(|game| game.id == frame.game)
            .map(|game| game.name)
            .unwrap_or("局域网游戏")
            .to_string(),
        name: frame
            .payload
            .get("roomName")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("未命名房间")
            .to_string(),
        host: frame.sender_nickname.clone(),
        players: format!("{players} 人"),
    }
}
use std::collections::BTreeMap;
