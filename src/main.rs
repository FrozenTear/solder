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
            state.graph.update(msg, &mut state.config);
        }
        Message::Pipewire(event) => {
            state.graph.handle_pipewire_event(event, &state.config);
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
