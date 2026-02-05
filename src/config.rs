use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    positions: HashMap<String, Position>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct NodeKey {
    pub node_name: String,
    pub app_name: Option<String>,
    pub object_path: Option<String>,
    pub index: Option<u32>,
}

impl NodeKey {
    fn to_string_key(&self) -> String {
        // object_path is the most stable identifier (for hardware devices)
        // Fall back to node_name|app_name for software nodes
        // index is used to distinguish multiple identical nodes
        let base = match (&self.object_path, &self.app_name) {
            (Some(path), _) => format!("path:{}", path),
            (None, Some(app)) => format!("{}|{}", self.node_name, app),
            (None, None) => self.node_name.clone(),
        };
        match self.index {
            Some(idx) if idx > 0 => format!("{}#{}", base, idx),
            _ => base,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Config {
    pub fn load() -> Option<Self> {
        let path = Self::config_path()?;
        let contents = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    pub fn save(&self) -> Option<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok()?;
        }
        let contents = serde_json::to_string_pretty(self).ok()?;
        fs::write(&path, &contents).ok()
    }

    pub fn get_position(&self, key: &NodeKey) -> Option<Position> {
        self.positions.get(&key.to_string_key()).copied()
    }

    pub fn set_position(&mut self, key: NodeKey, pos: Position) {
        self.positions.insert(key.to_string_key(), pos);
        let _ = self.save();
    }

    fn config_path() -> Option<PathBuf> {
        let dirs = ProjectDirs::from("", "", "solder")?;
        Some(dirs.config_dir().join("positions.json"))
    }
}
