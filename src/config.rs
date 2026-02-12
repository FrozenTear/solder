use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    positions: HashMap<String, Position>,

    /// Custom display names for nodes (original_key -> custom_name)
    #[serde(default)]
    pub node_renames: HashMap<String, String>,

    /// Last loaded preset path
    #[serde(default)]
    pub last_preset: Option<String>,

    /// Whether exclusive mode is enabled
    #[serde(default)]
    pub exclusive_mode: bool,

    /// Whether to auto-pin new connections
    #[serde(default)]
    pub auto_pin: bool,

    /// Whether ALSA MIDI is enabled
    #[serde(default)]
    pub alsa_midi_enabled: bool,

    /// Last-used profile index per device (device.name → profile index)
    #[serde(default)]
    pub device_profiles: HashMap<String, u32>,

    /// Saved ghost node positions per device (device.name → position)
    #[serde(default)]
    pub device_positions: HashMap<String, Position>,
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
        Some(dirs.config_dir().join("config.json"))
    }

    /// Get custom name for a node
    pub fn get_node_rename(&self, key: &NodeKey) -> Option<&String> {
        self.node_renames.get(&key.to_string_key())
    }

    /// Set custom name for a node
    pub fn set_node_rename(&mut self, key: NodeKey, name: String) {
        self.node_renames.insert(key.to_string_key(), name);
        let _ = self.save();
    }

    /// Clear custom name for a node
    pub fn clear_node_rename(&mut self, key: &NodeKey) {
        self.node_renames.remove(&key.to_string_key());
        let _ = self.save();
    }

    /// Get the presets directory path
    pub fn presets_dir() -> Option<PathBuf> {
        let dirs = ProjectDirs::from("", "", "solder")?;
        Some(dirs.config_dir().join("presets"))
    }

    /// Get last-used profile index for a device
    pub fn get_device_profile(&self, device_name: &str) -> Option<u32> {
        self.device_profiles.get(device_name).copied()
    }

    /// Set last-used profile index for a device
    pub fn set_device_profile(&mut self, device_name: String, profile_index: u32) {
        self.device_profiles.insert(device_name, profile_index);
        let _ = self.save();
    }

    /// Get saved ghost node position for a device
    pub fn get_device_position(&self, device_name: &str) -> Option<Position> {
        self.device_positions.get(device_name).copied()
    }

    /// Set ghost node position for a device
    pub fn set_device_position(&mut self, device_name: String, pos: Position) {
        self.device_positions.insert(device_name, pos);
        let _ = self.save();
    }
}
