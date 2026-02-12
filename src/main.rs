mod config;
mod graph;
mod icon;
mod layout;
mod pipewire_client;
mod preset;

use iced::widget::canvas;
use iced::{Element, Length, Subscription, Task, Theme};

use config::Config;
use graph::{Graph, GraphMessage};
use pipewire_client::PipewireEvent;

fn main() -> iced::Result {
    let mut settings = iced::window::Settings::default();
    settings.icon = icon::app_icon();
    settings.platform_specific.application_id = "solder".to_string();

    iced::application(init, update, view)
        .title("Solder")
        .window(settings)
        .subscription(subscription)
        .theme(theme)
        .antialiasing(true)
        .run()
}

fn theme(_state: &Solder) -> Theme {
    Theme::Dark
}

fn init() -> (Solder, Task<Message>) {
    let config = Config::load().unwrap_or_default();
    let graph = Graph::new(&config);
    (Solder { graph, config }, Task::none())
}

#[derive(Debug, Clone)]
pub enum Message {
    Graph(GraphMessage),
    Pipewire(PipewireEvent),
}

struct Solder {
    graph: Graph,
    config: Config,
}

fn update(state: &mut Solder, message: Message) -> Task<Message> {
    match message {
        Message::Graph(msg) => {
            return state.graph.update(msg, &mut state.config);
        }
        Message::Pipewire(event) => {
            state.graph.handle_pipewire_event(event, &mut state.config);
        }
    }
    Task::none()
}

fn view(state: &Solder) -> Element<'_, Message> {
    canvas(&state.graph)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn subscription(_state: &Solder) -> Subscription<Message> {
    pipewire_client::connect().map(Message::Pipewire)
}

/// Connect two ports via pw-link
pub fn pipewire_connect(output_port: u32, input_port: u32) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("pw-link")
            .arg(output_port.to_string())
            .arg(input_port.to_string())
            .output();
    });
}

/// Disconnect two ports via pw-link -d
pub fn pipewire_disconnect(output_port: u32, input_port: u32) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("pw-link")
            .arg("-d")
            .arg(output_port.to_string())
            .arg(input_port.to_string())
            .output();
    });
}

/// Set device profile via wpctl
pub fn set_device_profile(device_id: u32, profile_index: u32) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("wpctl")
            .arg("set-profile")
            .arg(device_id.to_string())
            .arg(profile_index.to_string())
            .output();
    });
}

/// Load device profiles via pw-dump (async, runs in background thread)
pub async fn load_device_profiles(device_id: u32) -> Vec<graph::DeviceProfile> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();

    std::thread::spawn(move || {
        let result = parse_device_profiles(device_id);
        let _ = tx.send(result);
    });

    rx.await.unwrap_or_default()
}

fn parse_device_profiles(device_id: u32) -> Vec<graph::DeviceProfile> {
    let output = match std::process::Command::new("pw-dump")
        .arg(device_id.to_string())
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    let json_str = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    // Parse EnumProfile entries from pw-dump output
    let mut profiles = Vec::new();
    if let Some(arr) = json.as_array() {
        for obj in arr {
            if let Some(enum_profiles) = obj.pointer("/info/params/EnumProfile") {
                if let Some(profile_arr) = enum_profiles.as_array() {
                    for p in profile_arr {
                        let index = p.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let description = p.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

                        // Skip "Off" profile (index 0) - that's what deactivation uses
                        if index == 0 {
                            continue;
                        }

                        profiles.push(graph::DeviceProfile {
                            index,
                            name,
                            description,
                        });
                    }
                }
            }
        }
    }

    profiles
}
