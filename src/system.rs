use crate::camera::{Camera, CameraController};
use crate::entity::Entity;
use crate::event::{GameEvent, ValueType};
use crate::GlobalContext;
use crate::util::SharedCell;

struct SystemManager {
    systems: Vec<System>,
}
impl SystemManager {
    pub fn new() -> Self {
        Self { systems: vec![] }
    }

    pub fn add_system(&mut self, system: System) {
        self.systems.push(system);
    }

    pub fn input(&mut self, event: GameEvent) {
        for system in self.systems.iter_mut() {
            system.input(event.clone());
        }
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        for system in self.systems.iter_mut() {
            system.tick(context);
        }
    }
}

struct System {
    object: Box<dyn SystemObject>,
}
impl System {
    fn input(&mut self, event: GameEvent) {
        self.object.input(event);
    }

    fn tick(&mut self, context: &GlobalContext) {
        self.object.tick(context);
    }
}

trait SystemObject {
    fn input(&mut self, event: GameEvent);

    fn tick(&mut self, context: &GlobalContext);
}

trait IntoSystem {
    fn make_system(self) -> System;
}
impl IntoSystem for Box<dyn SystemObject> {
    fn make_system(self) -> System {
        System {
            object: self,
        }
    }
}

pub struct PlayerControllerSystem {
    camera: Camera,
    controller: Box<dyn CameraController>,
    player_entity: SharedCell<Entity>,
}
impl PlayerControllerSystem {
    pub fn new(
        camera: Camera,
        controller: Box<dyn CameraController>,
        player_entity: SharedCell<Entity>
    ) -> Box<Self> {
        Box::new(Self { camera, controller, player_entity })
    }
}
impl SystemObject for PlayerControllerSystem {
    fn input(&mut self, event: GameEvent) {
        self.controller.input(event);
    }

    fn tick(&mut self, context: &GlobalContext) {
        self.controller.update_camera(&mut self.camera, context.size);
        self.player_entity.borrow_mut().input(GameEvent::SendValueWith {
            string: "set position".to_string(),
            value: ValueType::Float3(self.camera.get_pos().into()),
        })
    }
}