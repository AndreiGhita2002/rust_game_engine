use cgmath::{Matrix4, Vector3, Zero};

use crate::entity::Entity;
use crate::event::{GameEvent, Response, ValueType};
use crate::GlobalContext;
use crate::render::{RenderCommand, Single3DInstance};
use crate::render::instance::{InstanceDesc, InstanceRef, InstanceType};
use crate::util::SharedCell;

pub trait SpaceComponent {
    fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        depth: i32,
    );

    fn translate(&mut self, vector: &[f32]);

    fn rotate(&mut self, vector: &[f32]);

    fn set_pos(&mut self, vector: &[f32]);

    fn set_rot(&mut self, vector: &[f32]);

    fn transform(&mut self, vector: &[f32]) {
        self.translate(vector);
        self.rotate(vector);
    }

    fn transform_render(&self, command: &mut RenderCommand);

    fn input(&mut self, event: GameEvent) -> Response;
}

// -----------------------
//    Implementations:
// -----------------------

// Abstract (root) Space:
// does nothing, is in root
pub struct NoSpaceMaster {}
impl SpaceComponent for NoSpaceMaster {
    fn init_child_entity(
        &self,
        _context: &GlobalContext,
        _child_entity: SharedCell<Entity>,
        _depth: i32,
    ) {}

    fn translate(&mut self, _vector: &[f32]) {}

    fn rotate(&mut self, _vector: &[f32]) {}

    fn set_pos(&mut self, _vector: &[f32]) {}

    fn set_rot(&mut self, _vector: &[f32]) {}

    fn transform_render(&self, _command: &mut RenderCommand) {}

    fn input(&mut self, _event: GameEvent) -> Response {
        Response::No
    }
}
pub struct NoSpaceComponent {}
impl SpaceComponent for NoSpaceComponent {
    fn init_child_entity(
        &self,
        _context: &GlobalContext,
        _child_entity: SharedCell<Entity>,
        _depth: i32,
    ) {}

    fn translate(&mut self, _vector: &[f32]) {}

    fn rotate(&mut self, _vector: &[f32]) {}

    fn set_pos(&mut self, _vector: &[f32]) {}

    fn set_rot(&mut self, _vector: &[f32]) {}

    fn transform_render(&self, _command: &mut RenderCommand) {}

    fn input(&mut self, _event: GameEvent) -> Response {
        Response::No
    }
}

// Game (3D) Space:
pub struct GameSpaceMaster {
    pub total_displacement: SharedCell<Vector3<f32>>, // should be the same as the camera coordinates
}
impl SpaceComponent for GameSpaceMaster {
    fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        _depth: i32,
    ) {
        // creating the instance
        let mut instance_manager = context.instance_manager.borrow_mut();
        let instance = instance_manager.register_instance(InstanceDesc {
            instance_type: InstanceType::Model,
            ..Default::default()
        });
        let mut entity = child_entity.borrow_mut();

        println!("GameSpaceMaster is initialising Entity:{}", entity.get_id());

        // space component:
        entity.space_component = Box::new(GameSpaceComponent {
            total_displacement: self.total_displacement.clone(),
            instance: instance.clone(),
        });
        // render component:
        entity.render_component = Single3DInstance::new("cube", instance)
    }

    fn translate(&mut self, _vector: &[f32]) {}

    fn rotate(&mut self, _vector: &[f32]) {}

    fn set_pos(&mut self, _vector: &[f32]) {}

    fn set_rot(&mut self, _vector: &[f32]) {}

    fn transform_render(&self, _command: &mut RenderCommand) {}

    fn input(&mut self, _event: GameEvent) -> Response {
        Response::No
    }
}
impl Default for GameSpaceMaster {
    fn default() -> Self {
        GameSpaceMaster {
            total_displacement: SharedCell::new(Vector3::zero()),
        }
    }
}


pub struct GameSpaceComponent {
    total_displacement: SharedCell<Vector3<f32>>,
    instance: InstanceRef,
}
impl SpaceComponent for GameSpaceComponent {
    fn init_child_entity(
        &self,
        _context: &GlobalContext,
        _child_entity: SharedCell<Entity>,
        _depth: i32,
    ) {}

    fn translate(&mut self, vector: &[f32]) {
        if vector.len() == 3 {
            self.instance.add_pos((vector[0], vector[1], vector[2]))
        } else {
            println!(
                "[ERR] GameSpaceComponent of instance:{} received vector of wrong size for the \
                method 'translate()';\n  vector.len={}, 3 was expected!",
                self.instance.get_instance_id(),
                vector.len()
            )
        }
    }

    fn rotate(&mut self, vector: &[f32]) {
        if vector.len() == 4 {
            self.instance.add_rot((vector[0], vector[1], vector[2], vector[3]))
        } else {
            println!(
                "[ERR] GameSpaceComponent of instance:{} received vector of wrong size for \
                the method 'rotate()';\n  vector.len={}, 4 was expected!",
                self.instance.get_instance_id(),
                vector.len()
            )
        }
    }

    fn set_pos(&mut self, vector: &[f32]) {
        if vector.len() == 3 {
            self.instance.set_pos((vector[0], vector[1], vector[2]))
        } else {
            println!(
                "[ERR] GameSpaceComponent of instance:{} received vector of wrong size for the \
                method 'set_pos()';\n  vector.len={}, 3 was expected!",
                self.instance.get_instance_id(),
                vector.len()
            )
        }
    }

    fn set_rot(&mut self, vector: &[f32]) {
        if vector.len() == 4 {
            self.instance.set_rot((vector[0], vector[1], vector[2], vector[3]))
        } else {
            println!(
                "[ERR] GameSpaceComponent of instance:{} received vector of wrong size for \
                the method 'set_rot()';\n  vector.len={}, 4 was expected!",
                self.instance.get_instance_id(),
                vector.len()
            )
        }
    }

    fn transform_render(&self, command: &mut RenderCommand) {
        let matrix = Matrix4::from_translation(self.total_displacement.borrow().clone());
        command.transform = Some(matrix);
    }

    fn input(&mut self, event: GameEvent) -> Response {
        match event {
            GameEvent::SendValueWith { value, string } => match &string as &str {
                "set" | "set pos" => match value {
                    ValueType::Float3(pos) => {
                        self.instance.set_pos(pos);
                        Response::Strong
                    }
                    _ => Response::No,
                },
                _ => Response::No,
            },
            _ => Response::No,
        }
    }
}
