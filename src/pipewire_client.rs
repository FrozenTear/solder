use iced::futures::channel::mpsc;
use iced::futures::StreamExt;
use iced::Subscription;
use pipewire::context::ContextBox;
use pipewire::main_loop::MainLoopBox;
use pipewire as pw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::graph::{PortDirection, PortType};

#[derive(Debug, Clone)]
pub enum PipewireEvent {
    NodeAdded {
        id: u32,
        name: String,
        app_name: Option<String>,
        serial: Option<String>,
        object_path: Option<String>,
    },
    NodeRemoved {
        id: u32,
    },
    PortAdded {
        node_id: u32,
        port_id: u32,
        name: String,
        direction: PortDirection,
        port_type: PortType,
    },
    PortRemoved {
        node_id: u32,
        port_id: u32,
    },
    LinkAdded {
        id: u32,
        output_node: u32,
        output_port: u32,
        input_node: u32,
        input_port: u32,
    },
    LinkRemoved {
        id: u32,
    },
}

pub fn connect() -> Subscription<PipewireEvent> {
    Subscription::run(|| {
        iced::stream::channel(100, |mut output: iced::futures::channel::mpsc::Sender<PipewireEvent>| async move {
            let (tx, mut rx) = mpsc::channel::<PipewireEvent>(100);

            std::thread::spawn(move || {
                if let Err(e) = run_pipewire_loop(tx) {
                    eprintln!("PipeWire error: {}", e);
                }
            });

            while let Some(event) = rx.next().await {
                use iced::futures::SinkExt;
                let _ = output.send(event).await;
            }
        })
    })
}

fn run_pipewire_loop(tx: mpsc::Sender<PipewireEvent>) -> Result<(), pw::Error> {
    let mainloop = MainLoopBox::new(None)?;
    let context = ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect(None)?;
    let registry = core.get_registry()?;

    // Track port -> node mapping
    let port_to_node: Rc<RefCell<HashMap<u32, u32>>> = Rc::new(RefCell::new(HashMap::new()));
    let tx = Rc::new(RefCell::new(tx));

    let _listener = registry
        .add_listener_local()
        .global({
            let tx = tx.clone();
            let port_to_node = port_to_node.clone();
            move |global| {
                let mut tx = tx.borrow_mut();
                match global.type_ {
                    pw::types::ObjectType::Node => {
                        let props = global.props.as_ref();
                        let name = props
                            .and_then(|p| p.get("node.description"))
                            .or_else(|| props.and_then(|p| p.get("node.nick")))
                            .or_else(|| props.and_then(|p| p.get("node.name")))
                            .unwrap_or("Unknown")
                            .to_string();
                        let app_name = props
                            .and_then(|p| p.get("application.name"))
                            .map(String::from);
                        let serial = props
                            .and_then(|p| p.get("object.serial"))
                            .map(String::from);
                        let object_path = props
                            .and_then(|p| p.get("object.path"))
                            .map(String::from);

                        let _ = tx.try_send(PipewireEvent::NodeAdded {
                            id: global.id,
                            name,
                            app_name,
                            serial,
                            object_path,
                        });
                    }
                    pw::types::ObjectType::Port => {
                        let props = global.props.as_ref();
                        let name = props
                            .and_then(|p| p.get("port.name"))
                            .unwrap_or("port")
                            .to_string();
                        let node_id: u32 = props
                            .and_then(|p| p.get("node.id"))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let direction = props
                            .and_then(|p| p.get("port.direction"))
                            .map(|d| {
                                if d == "in" {
                                    PortDirection::Input
                                } else {
                                    PortDirection::Output
                                }
                            })
                            .unwrap_or(PortDirection::Output);
                        let port_type = props
                            .and_then(|p| p.get("format.dsp"))
                            .map(|fmt| {
                                if fmt.contains("midi") {
                                    PortType::Midi
                                } else if fmt.contains("video") {
                                    PortType::Video
                                } else {
                                    PortType::Audio
                                }
                            })
                            .unwrap_or(PortType::Audio);

                        port_to_node.borrow_mut().insert(global.id, node_id);

                        let _ = tx.try_send(PipewireEvent::PortAdded {
                            node_id,
                            port_id: global.id,
                            name,
                            direction,
                            port_type,
                        });
                    }
                    pw::types::ObjectType::Link => {
                        let props = global.props.as_ref();
                        let output_port: u32 = props
                            .and_then(|p| p.get("link.output.port"))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let input_port: u32 = props
                            .and_then(|p| p.get("link.input.port"))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let output_node: u32 = props
                            .and_then(|p| p.get("link.output.node"))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let input_node: u32 = props
                            .and_then(|p| p.get("link.input.node"))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);

                        let _ = tx.try_send(PipewireEvent::LinkAdded {
                            id: global.id,
                            output_node,
                            output_port,
                            input_node,
                            input_port,
                        });
                    }
                    _ => {}
                }
            }
        })
        .global_remove({
            let tx = tx.clone();
            let port_to_node = port_to_node.clone();
            move |id| {
                let mut tx = tx.borrow_mut();
                if let Some(node_id) = port_to_node.borrow_mut().remove(&id) {
                    let _ = tx.try_send(PipewireEvent::PortRemoved {
                        node_id,
                        port_id: id,
                    });
                } else {
                    let _ = tx.try_send(PipewireEvent::NodeRemoved { id });
                    let _ = tx.try_send(PipewireEvent::LinkRemoved { id });
                }
            }
        })
        .register();

    mainloop.run();

    Ok(())
}
