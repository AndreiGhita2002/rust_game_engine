use cgmath::{Quaternion, Vector3};

use crate::event::{GameEvent, Response};
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
        let root = Entity::make_root(id_manager.clone());
        EntityManager {
            id_manager,
            entities: vec![root],
        }
    }

    pub fn init(&mut self, context: &GlobalContext) {
        // first cube
        self.new_entity(EntityDesc{
            render_component: Single3DInstance::new(),
            position: Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            ..Default::default()
        }, 0, &context);
        // second cube
        self.new_entity(EntityDesc{
            render_component: Single3DInstance::new(),
            position: Vector3 {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            },
            ..Default::default()
        }, 0, &context);
    }

    pub fn new_entity<T>(&mut self, entity_desc: EntityDesc, parent: T, context: &GlobalContext)
        -> SharedCell<Entity> where T: EntityRef
    {
        // creating the new entity
        let p_id = parent.get_id();
        let (render_component, position, components) = entity_desc.unpack();
        let entity = SharedCell::new(Entity {
            id: self.id_manager.next_id(),
            parent_id: p_id,
            children: vec![],
            render_component,
            instance: SharedCell::new(Instance3D {
                position,
                rotation: Quaternion::new(1.0, 0.5, 0.0, 0.0),
                model_name: "cube".to_string(),
            }),
            components,
        });
        // registering the new entity:
        self.id_manager.register_entity(entity.clone());
        self.entities.push(entity.clone());
        let parent_entity = parent.into_entity(&self.id_manager);
        parent_entity.borrow_mut().add_child(entity.clone());

        // going through all of the entities parents and letting them init the new entity:
        let mut parent_id = p_id;
        while parent_id != 0 {
            let current_parent = self.id_manager.get(parent_id).unwrap().to_entity().unwrap();
            let current_parent_b = current_parent.borrow();
            current_parent_b.init_child(context, entity.clone());
            parent_id = current_parent_b.parent_id;
        }
        // initialising the entity:
        {
            let mut entity_b = entity.borrow_mut();
            entity_b.init(context);
        }
        entity
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

    #[allow(dead_code)]
    // used for sending a GameEvent to all Entities
    pub fn input_root(&self, event: GameEvent) -> Response {
        self.get_root().borrow_mut().input(event)
    }

    #[allow(dead_code)]
    pub fn get_root(&self) -> &SharedCell<Entity> {
        self.entities.get(0)
            .expect("No root entity!!\n\
         (at space 0 in the EntityManager vector)")
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entities.len()
    }
}


#[allow(dead_code)]
pub struct Entity {
    // the self, the parent and the children
    id: u64,
    parent_id: u64,
    pub children: Vec<SharedCell<Entity>>,
    // components:
    pub render_component: Box<dyn RenderComponent>,
    pub instance: SharedCell<Instance3D>, //todo: replace this with the space thing
    pub components: Vec<Component>,        // also todo: make these not public
}
impl Entity {
    pub fn init(&mut self, context: &GlobalContext) {
        self.render_component.init(context, &self.instance, &self.components);
        for component in self.components.iter_mut() {
            component.init(context);
        }
    }

    pub fn init_child(&self, context: &GlobalContext, child: SharedCell<Entity>) {
        // all the components get the chance to edit the new child:
        for component in self.components.iter() {
            component.init_child_entity(context, child.clone())
        }
    }

    pub fn make_root(id_manager: IdManager) -> SharedCell<Self> {
        let root = Entity{
            id: 0,
            parent_id: 0,
            render_component: NoRender::new(),
            instance: SharedCell::new(Instance3D::default()),
            components: vec![],
            children: vec![],
        };
        let cell = SharedCell::new(root);
        id_manager.register_entity(cell.clone());
        cell
    }

    pub fn get_id(&self) -> u64 { self.id }

    pub fn input(&mut self, event: GameEvent) -> Response {
        let mut response = Response::No;
        for component in self.components.iter_mut() {
            response = response.with(component.input(event.clone()));
        }
        response
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

    // todo: redo this when you implement spaces
    pub fn instance_set(&mut self, new_pos: Vector3<f32>) {
        let mut instance = self.instance.borrow_mut();
        instance.position = new_pos;
    }

    //todo add the transform things
    pub fn render(&self, commands: &mut Vec<RenderCommand>) {
        // rendering self
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


pub struct EntityDesc {
    pub render_component: Box<dyn RenderComponent>,
    pub position: Vector3<f32>,   //todo replace with space location
    pub components: Vec<Component>,
}
impl EntityDesc {
    pub fn unpack(self) -> (Box<dyn RenderComponent>, Vector3<f32>, Vec<Component>) {
        (self.render_component, self.position, self.components)
    }
}
impl Default for EntityDesc {
    fn default() -> Self {
        EntityDesc {
            render_component: NoRender::new(),
            position: Vector3::new(0.0, 0.0, 0.0),
            components: vec![],
        }
    }
}


pub trait EntityRef {
    fn into_entity(self, id_manager: &IdManager) -> SharedCell<Entity>;

    fn get_id(&self) -> u64;
}
impl EntityRef for SharedCell<Entity> {
    fn into_entity(self, _id_manager: &IdManager) -> SharedCell<Entity> {
        self.clone()
    }

    fn get_id(&self) -> u64 {
        self.borrow().id
    }
}
impl EntityRef for &SharedCell<Entity> {
    fn into_entity(self, _id_manager: &IdManager) -> SharedCell<Entity> {
        self.clone()
    }

    fn get_id(&self) -> u64 {
        self.borrow().id
    }
}
impl EntityRef for u64 {
    fn into_entity(self, id_manager: &IdManager) -> SharedCell<Entity> {
        id_manager.get(self)
            .expect("Could not find entity with id:{self}")
            .to_entity()
            .expect("Object with id:{self} was requested as an Entity, but it's not!")
    }

    fn get_id(&self) -> u64 {
        *self
    }
}


// todo implement some of these:
pub struct Component {
    id: u64,
    component_obj: Box<dyn ComponentObject>
}
impl Component {
    pub fn get_id(&self) -> u64 { self.id }

    fn init(&mut self, context: &GlobalContext) {
        self.component_obj.init(context)
    }

    fn init_child_entity(&self, context: &GlobalContext, child_entity: SharedCell<Entity>) {
        self.component_obj.init_child_entity(context, child_entity)
    }

    pub fn input(&mut self, event: GameEvent) -> Response {
        self.component_obj.input(event)
    }

    pub fn tick(&mut self) {
        self.component_obj.tick()
    }
}

trait ComponentObject {
    fn init(&mut self, context: &GlobalContext);

    fn init_child_entity(&self, context: &GlobalContext, child_entity: SharedCell<Entity>);

    fn input(&mut self, event: GameEvent) -> Response;

    fn tick(&mut self);
}
