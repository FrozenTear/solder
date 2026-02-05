use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A saved patchbay preset containing connection configurations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preset {
    pub name: String,
    pub version: u32,
    pub connections: Vec<PresetConnection>,
    #[serde(default)]
    pub node_renames: HashMap<String, String>,
    #[serde(default)]
    pub pinned_connections: Vec<PresetConnection>,
}

impl Preset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: 1,
            connections: Vec::new(),
            node_renames: HashMap::new(),
            pinned_connections: Vec::new(),
        }
    }
}

/// A connection between two ports, stored by node/port identifiers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PresetConnection {
    pub output_node: NodeMatcher,
    pub output_port: String,
    pub input_node: NodeMatcher,
    pub input_port: String,
    #[serde(default)]
    pub pinned: bool,
}

/// Identifies a node by various properties for matching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeMatcher {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_path: Option<String>,
    #[serde(default)]
    pub use_regex: bool,
}

impl NodeMatcher {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            app_name: None,
            object_path: None,
            use_regex: false,
        }
    }

    pub fn with_app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    pub fn with_object_path(mut self, path: impl Into<String>) -> Self {
        self.object_path = Some(path.into());
        self
    }

    /// Check if this matcher matches a given node
    pub fn matches(&self, name: &str, app_name: Option<&str>, object_path: Option<&str>) -> bool {
        // If object_path is specified and matches, that's the strongest identifier
        if let (Some(matcher_path), Some(node_path)) = (&self.object_path, object_path) {
            if self.use_regex {
                if let Ok(re) = regex::Regex::new(matcher_path) {
                    return re.is_match(node_path);
                }
            }
            return matcher_path == node_path;
        }

        // Match by name (and optionally app_name)
        let name_matches = if self.use_regex {
            regex::Regex::new(&self.name)
                .map(|re| re.is_match(name))
                .unwrap_or(false)
        } else {
            self.name == name
        };

        if !name_matches {
            return false;
        }

        // If app_name is specified in matcher, it must also match
        if let Some(matcher_app) = &self.app_name {
            match app_name {
                Some(node_app) => matcher_app == node_app,
                None => false,
            }
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortTypeFilter {
    #[default]
    All,
    Audio,
    Midi,
    Video,
}
