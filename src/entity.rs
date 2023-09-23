use cgmath::{Quaternion, Vector3};

use crate::event::GameEvent;
use crate::GlobalContext;
use crate::render::{NoRender, RenderCommand, RenderComponent, Single3DInstance};
use crate::render::instance::Instance3D;
use crate::util::{IdManager, SharedCell};

pub struct EntityManager {
    id_manager: IdManager,
    // assume the first entity is the root:
    entities: Vec<SharedCell<Entity>>,
}
impl EntityManager {
    pub fn new(id_manager: IdManager) -> Self {
        EntityManager {
            id_manager,
            entities: vec![],
        }
    }

    pub fn init(&mut self, context: &GlobalContext) {
        // root entity
        let no_render = NoRender::new();
        let root = Entity::new(self, context, no_render);
        // first cube
        let instance1 = Instance3D {
            position: Vector3::new(10.0, 10.0, -1.0),
            rotation: Quaternion::new(1.0, 0.5, 0.0, 0.0),
            model_name: "cube".to_string(),
        };
        let render_comp1 = Single3DInstance::new(instance1);
        let cube1 = Entity::new(self, context, render_comp1);
        // second cube
        let instance2 = Instance3D {
            position: Vector3::new(1.0, 2.0, 0.0),
            rotation: Quaternion::new(1.0, 0.5, 0.0, 0.0),
            model_name: "cube".to_string(),
        };
        let render_comp2 = Single3DInstance::new(instance2);
        let cube2 = Entity::new(self, context, render_comp2);
        {
            // adding cube2 as a child to cube1
            cube1.borrow_mut().add_child(cube2.clone());
        }
        {
            // adding cube1 as a child to the root:
            root.borrow_mut().add_child(cube1);
        }
    }

    pub fn tick(&mut self) {
        if let Some(root) = self.entities.get(0) {
            root.borrow_mut().tick();
        }
    }

    pub fn render(&self) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        if let Some(root) = self.entities.get(0) {
            root.borrow_mut().render(&mut commands);
        }
        commands
    }

    pub fn register_entity(&mut self, entity: SharedCell<Entity>) {
        self.id_manager.register_entity(entity.clone());
        self.entities.push(entity);
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }
}


pub struct Entity {
    id: u64,
    render_component: Box<dyn RenderComponent>,
    components: Vec<Component>,
    children: Vec<SharedCell<Entity>>,
}
impl Entity {
    pub fn new(
        manager: &mut EntityManager,
        context: &GlobalContext,
        mut render_component: Box<dyn RenderComponent>
    ) -> SharedCell<Entity> {
        let id = manager.id_manager.next_id();
        render_component.init(context);
        let entity = Entity {
            id,
            render_component,
            components: vec![],
            children: vec![],
        };
        let cell = SharedCell::new(entity);
        manager.register_entity(cell.clone());
        cell
    }

    pub fn get_id(&self) -> u64 { self.id }

    pub fn input(&mut self, event: GameEvent) {
        for component in self.components.iter_mut() {
            component.input(event.clone());
        }
    }

    pub fn tick(&mut self) {
        // tick for self
        for component in self.components.iter_mut() {
            component.tick();
        }
        // tick for children
        for child_cell in self.children.iter() {
            child_cell.borrow_mut().tick();
        }
    }

    pub fn render(&self, commands: &mut Vec<RenderCommand>) {
        // rendering self
        println!(" rendering entity: {}", self.id);
        self.render_component.render(&self, commands);

        // rendering children:
        // tick for children
        for child_cell in self.children.iter() {
            child_cell.borrow().render(commands);
        }
    }

    pub fn add_child(&mut self, child: SharedCell<Entity>) {
        self.children.push(child)
    }
}


pub struct Component {
    id: u64,
    component_obj: Box<dyn ComponentObject>
}
impl Component {
    pub fn get_id(&self) -> u64 { self.id }

    pub fn input(&mut self, event: GameEvent) {
        self.component_obj.input(event);
    }

    pub fn tick(&mut self) {
        self.component_obj.tick()
    }
}

trait ComponentObject {
    fn input(&mut self, event: GameEvent);

    fn tick(&mut self);
}
