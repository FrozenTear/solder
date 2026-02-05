mod config;
mod graph;
mod layout;
mod pipewire_client;

use iced::widget::canvas;
use iced::{Element, Length, Subscription, Task, Theme};

use config::Config;
use graph::{Graph, GraphMessage};
use pipewire_client::PipewireEvent;

fn main() -> iced::Result {
    iced::application(init, update, view)
        .title("Solder")
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
