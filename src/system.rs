use cgmath::Vector3;

use crate::camera::{Camera, CameraController};
use crate::entity::Entity;
use crate::event::{GameEvent, Response};
use crate::GlobalContext;
use crate::util::SharedCell;

pub struct SystemManager {
    systems: Vec<GameSystem>,
}
impl SystemManager {
    pub fn new() -> Self {
        Self { systems: vec![] }
    }

    pub fn add_system<T: Into<GameSystem>>(&mut self, system: T) {
        self.systems.push(system.into());
    }

    pub fn input(&mut self, event: GameEvent) -> Response {
        let mut output = Response::No;
        for system in self.systems.iter_mut() {
            output = output.with(system.input(event.clone()));
        }
        output
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        for system in self.systems.iter_mut() {
            system.tick(context);
        }
    }
}

pub struct GameSystem {
    object: Box<dyn SystemObject>,
}
impl GameSystem {
    fn input(&mut self, event: GameEvent) -> Response {
        self.object.input(event)
    }

    fn tick(&mut self, context: &GlobalContext) {
        self.object.tick(context);
    }
}

pub trait SystemObject {
    fn input(&mut self, event: GameEvent) -> Response;

    fn tick(&mut self, context: &GlobalContext);
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
    ) -> GameSystem {
        GameSystem {
            object: Box::new(Self { camera, controller, player_entity })
        }
    }
}
impl SystemObject for PlayerControllerSystem {
    fn input(&mut self, event: GameEvent) -> Response {
        let used = self.controller.input(event.clone());
        if used {
            Response::Strong
        } else {
            Response::No
        }
    }

    fn tick(&mut self, context: &GlobalContext) {
        self.controller.update_camera(&mut self.camera, context.size);

        // changing the player instance:
        let point = self.camera.get_pos();
        let pos = Vector3 {
            x: point.x,
            y: point.y,
            z: point.z,
        };
        self.player_entity.borrow_mut().instance_set(pos);

        context.update_camera_uniform(&self.camera);

        // self.player_entity.borrow_mut().input(GameEvent::SendValueWith {
        //     string: "set position".to_string(),
        //     value: ValueType::Float3(self.camera.get_pos().into()),
        // }); // we don't care about the response
    }
}