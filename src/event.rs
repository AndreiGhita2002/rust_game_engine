use std::collections::HashMap;

use winit::dpi::PhysicalPosition;
use winit::event::KeyboardInput;

use crate::util::{IdManager, SharedCell};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum GameEvent {
    KeyboardInput {
        input: KeyboardInput,
    },
    CursorMoved {
        position: PhysicalPosition<f64>,
    },
    CommandString {
        target: String,
        command: String,
        args: String,
    },
    SendValue(ValueType),
    SendValueWith {
        string: String,
        value: ValueType,
    },
    AttachListener(Listener), // todo add a 'where' field
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum ValueType {
    Int(i32),
    Int2((i32, i32)),
    Int3((i32, i32, i32)),
    Float(f32),
    Float2((f32, f32)),
    Float3((f32, f32, f32)),
    String(String),
}

#[derive(PartialEq, Debug)]
pub enum Response {
    No,
    Weak,
    Strong,
}
impl Response {
    pub fn with(self, other: Response) -> Response {
        if self == Response::Strong || other == Response::Strong {
            return Response::Strong;
        }
        if self == Response::Weak || other == Response::Weak {
            return Response::Weak;
        }
        Response::No
    }

    pub fn no_response(&self) -> bool {
        match self {
            Response::No => true,
            _ => false,
        }
    }

    pub fn at_most_weak(&self) -> bool {
        match self {
            Response::Weak | Response::No => true,
            _ => false,
        }
    }

    pub fn at_least_weak(&self) -> bool {
        match self {
            Response::Weak | Response::Strong => true,
            _ => false,
        }
    }

    pub fn is_strong(&self) -> bool {
        match self {
            Response::Strong => true,
            _ => false,
        }
    }
}

pub struct EventDispatcher {
    event_queue: SharedCell<Vec<(String, GameEvent)>>,
    // destination name -> its hash
    destinations: SharedCell<HashMap<String, Vec<u64>>>,
    id_finder: IdManager,
}

impl EventDispatcher {
    pub fn new(id_finder: IdManager) -> Self {
        Self {
            event_queue: SharedCell::new(Vec::new()),
            destinations: SharedCell::new(HashMap::new()),
            id_finder,
        }
    }

    // Public Methods:
    pub fn register_destination(&self, destination: &str, id: u64) {
        let mut destinations = self.destinations.borrow_mut();
        if let Some(v) = destinations.get_mut(destination) {
            v.push(id);
        } else {
            destinations.insert(destination.to_string(), vec![id]);
        }
    }

    pub fn send_event(&self, destination: &str, event: GameEvent) {
        let mut queue = self.event_queue.borrow_mut();
        queue.push((destination.to_string(), event));
    }

    pub fn process_events(&mut self) {
        let mut queue = self.event_queue.borrow_mut();
        let destinations = self.destinations.borrow_mut();

        while let Some((destination, event)) = queue.pop() {
            // println!("[EVENT] processing event: {event:?}\n   to destination: {destination}");
            if !destinations.contains_key(&*destination) {
                println!("[Event] Event destination not found: {destination}");
                continue;
            }
            for id in destinations.get(&*destination).unwrap().iter() {
                if let Some(thing) = self.id_finder.get(*id) {
                    thing.input(event.clone());
                } else {
                    println!("Thing with id:{id} not found!")
                }
            }
        }
    }
}

impl Clone for EventDispatcher {
    fn clone(&self) -> Self {
        EventDispatcher {
            destinations: self.destinations.clone(),
            event_queue: self.event_queue.clone(),
            id_finder: self.id_finder.clone(),
        }
    }
}

impl GameEvent {
    pub fn from_winit_event(event: &winit::event::WindowEvent) -> Option<GameEvent> {
        match event {
            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                Some(GameEvent::KeyboardInput { input: *input })
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                Some(GameEvent::CursorMoved {
                    position: *position,
                })
            }
            _ => None,
        }
    }
}

pub trait EventConsumer {
    fn input(&mut self, event: GameEvent);
}

#[derive(Clone, Debug)]
pub struct Listener {
    destination: String,
}

impl Listener {
    pub fn new(destination: &str) -> Self {
        Listener {
            destination: destination.to_string(),
        }
    }

    pub fn update(&self, value: ValueType, event_dispatcher: &mut EventDispatcher) {
        event_dispatcher.send_event(&self.destination, GameEvent::SendValue(value));
    }
}
