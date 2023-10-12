use crate::camera::{Camera, CameraController};
use crate::entity::Entity;
use crate::entity::event::{GameEvent, Response};
use crate::GlobalContext;
use crate::util::{IdManager, SharedCell};

pub struct SystemManager {
    id_manager: IdManager,
    systems: Vec<SharedCell<GameSystem>>,
}
impl SystemManager {
    pub fn new(id_manager: IdManager) -> Self {
        Self { id_manager, systems: vec![] }
    }

    pub fn input(&mut self, event: GameEvent) -> Response {
        let mut output = Response::No;
        for system in self.systems.iter_mut() {
            output = output.with(system.borrow_mut().input(event.clone()));
        }
        output
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        for system in self.systems.iter_mut() {
            system.borrow_mut().tick(context);
        }
    }

    pub fn new_system(&mut self, sys_obj: Box<dyn SystemObject>) {
        let id = sys_obj.get_id();
        let new_system = SharedCell::new(GameSystem {
            // id,
            object: sys_obj,
        });
        self.id_manager.register_system(new_system.clone());
        self.systems.push(new_system);
    }
}

pub struct GameSystem {
    // id: u64,
    object: Box<dyn SystemObject>,
}
impl GameSystem {
    pub fn input(&mut self, event: GameEvent) -> Response {
        self.object.input(event)
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        self.object.tick(context);
    }

    pub fn get_id(&self) -> u64 {
        self.object.get_id()
    }
}

pub trait SystemObject {
    fn input(&mut self, event: GameEvent) -> Response;

    fn tick(&mut self, context: &GlobalContext);

    fn get_id(&self) -> u64;
}

pub struct PlayerControllerSystem {
    id: u64,
    camera: Camera,
    controller: Box<dyn CameraController>,
    player_entity: SharedCell<Entity>,
}
impl PlayerControllerSystem {
    pub fn new(
        id_manager: &IdManager,
        camera: Camera,
        controller: Box<dyn CameraController>,
        player_entity: SharedCell<Entity>,
    ) -> Box<PlayerControllerSystem> {
        Box::new(Self {
            id: id_manager.next_id(),
            camera,
            controller,
            player_entity,
        })
    }
}
impl SystemObject for PlayerControllerSystem {
    fn input(&mut self, event: GameEvent) -> Response {
        if match event {
            GameEvent::ScreenResize { new_size } => {
                self.camera.aspect = new_size.width as f32 / new_size.height as f32;
                true
            }
            _ => {
                self.controller.input(event.clone())
            }
        } {
            Response::Strong
        } else {
            Response::No
        }
    }

    fn tick(&mut self, context: &GlobalContext) {
        self.controller.update_camera(&mut self.camera, context.size);

        // changing the player instance:
        let point = self.camera.get_pos();
        let pos = [point.x, point.y, point.z];
        self.player_entity.borrow_mut().space_component().set_pos(&pos);

        context.update_camera_uniform(&self.camera);
    }

    fn get_id(&self) -> u64 {
        self.id
    }
}
