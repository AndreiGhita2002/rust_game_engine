use std::ops::DerefMut;

use cgmath::Vector3;

use crate::event::{GameEvent, Response};
use crate::GlobalContext;
use crate::render::{NoRender, RenderCommand, RenderComponent};
use crate::space::{GameSpaceMaster, NoSpaceComponent, NoSpaceMaster, SpaceComponent};
use crate::util::{IdManager, SharedCell};

pub struct EntityManager {
    id_manager: IdManager,
    // assume the first entity is the root:
    entities: Vec<SharedCell<Entity>>,
}
impl EntityManager {
    pub fn new(id_manager: IdManager) -> Self {
        let root = Entity::make_root(id_manager.clone());
        let game_space_master = Entity::make_space_master(id_manager.clone());
        {
            root.borrow_mut().add_child(game_space_master.clone());
        }
        EntityManager {
            id_manager,
            entities: vec![root, game_space_master],
        }
    }

    pub fn init(&mut self, context: &GlobalContext) {
        let space_master_id = self.entities.get(1).unwrap().get_id();
        // first cube
        self.new_entity(
            EntityDesc {
                position: Vector3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                ..Default::default()
            },
            space_master_id,
            &context,
        );
        // second cube
        self.new_entity(
            EntityDesc {
                position: Vector3 {
                    x: -1.0,
                    y: -2.0,
                    z: 0.0,
                },
                ..Default::default()
            },
            space_master_id,
            &context,
        );
        // third cube:
        self.new_entity(
            EntityDesc {
                position: Vector3 {
                    x: 1.0,
                    y: 2.0,
                    z: 0.0,
                },
                ..Default::default()
            },
            space_master_id,
            &context,
        );
    }

    pub fn new_entity<T>(
        &mut self,
        entity_desc: EntityDesc,
        parent: T,
        context: &GlobalContext,
    ) -> SharedCell<Entity>
    where
        T: EntityRef,
    {
        // creating the new entity
        let p_id = parent.get_id();
        let (position, components) = entity_desc.unpack();
        let entity = SharedCell::new(Entity {
            id: self.id_manager.next_id(),
            parent_id: p_id,
            children: vec![],
            render_component: NoRender::new(),
            space_component: Box::new(NoSpaceComponent{}),
            components,
        });
        // registering the new entity:
        self.id_manager.register_entity(entity.clone());
        self.entities.push(entity.clone());
        let parent_entity = parent.into_entity(&self.id_manager);
        parent_entity.borrow_mut().add_child(entity.clone());

        // going through all of the entities parents and letting them init the new entity:
        let mut parent_id = p_id;
        let mut depth = 0;
        while parent_id != 0 {
            let current_parent = self.id_manager.get(parent_id).unwrap().to_entity().unwrap();
            let current_parent_b = current_parent.borrow();
            current_parent_b.init_child(context, entity.clone(), depth);
            depth += 1;
            parent_id = current_parent_b.parent_id;
        }
        // initialising the entity:
        {
            let mut entity_b = entity.borrow_mut();
            entity_b.init(context);
            // todo this should be in space master init
            //  maybe it should take the EntityDesc as an argument and figure out the position from there
            let pos = [position.x, position.y, position.z];
            entity_b.space_component.translate(&pos)
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
        self.entities.get(0).expect(
            "No root entity!!\n\
         (at space 0 in the EntityManager vector)",
        )
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn print_entities(&self) {
        println!("ENTITIES:");
        for entity_cell in self.entities.iter() {
            let entity = entity_cell.borrow();
            let id = entity.get_id();
            println!("[0:{}]", id)
        }
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
    pub space_component: Box<dyn SpaceComponent>,
    pub components: Vec<Component>, // also todo: make these not public
}
impl Entity {
    pub fn init(&mut self, context: &GlobalContext) {
        self.render_component.init(context, &self.components);
        for component in self.components.iter_mut() {
            component.init(context);
        }
    }

    pub fn init_child(&self, context: &GlobalContext, child: SharedCell<Entity>, depth: i32) {
        println!("Entity:{} is initialising child Entity:{}", self.get_id(), child.get_id());
        // first the space component:
        self.space_component.init_child_entity(context, child.clone(), depth);
        // all the components get the chance to edit the new child:
        for component in self.components.iter() {
            component.init_child_entity(context, child.clone(), depth)
        }
    }

    pub fn make_root(id_manager: IdManager) -> SharedCell<Self> {
        let root = Entity {
            id: 0,
            parent_id: 0,
            render_component: NoRender::new(),
            space_component: Box::new(NoSpaceMaster{}),
            components: vec![],
            children: vec![],
        };
        let cell = SharedCell::new(root);
        id_manager.register_entity(cell.clone());
        cell
    }

    // todo remove and generalize this with EntityDesc (somehow)
    pub fn make_space_master(id_manager: IdManager) -> SharedCell<Self> {
        let master = Entity {
            id: id_manager.next_id(),
            parent_id: 0,
            render_component: NoRender::new(),
            space_component: Box::new(GameSpaceMaster::default()),
            components: vec![],
            children: vec![],
        };
        let cell = SharedCell::new(master);
        id_manager.register_entity(cell.clone());
        cell
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

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

    pub fn space_component(&mut self) -> &mut dyn SpaceComponent {
        self.space_component.deref_mut()
    }

    pub fn render(&self, commands: &mut Vec<RenderCommand>) {
        // rendering self
        self.render_component.render(&self, commands);
        //todo add the transform thing:
        // self.space_component.transform_render(commands);

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
    pub position: Vector3<f32>, //todo replace with space location
    pub components: Vec<Component>,
}
impl EntityDesc {
    pub fn unpack(self) -> (Vector3<f32>, Vec<Component>) {
        (self.position, self.components)
    }
}
impl Default for EntityDesc {
    fn default() -> Self {
        EntityDesc {
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
        id_manager
            .get(self)
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
    component_obj: Box<dyn ComponentObject>,
}
impl Component {
    pub fn get_id(&self) -> u64 {
        self.id
    }

    fn init(&mut self, context: &GlobalContext) {
        self.component_obj.init(context)
    }

    fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        depth: i32,
    ) {
        self.component_obj
            .init_child_entity(context, child_entity, depth)
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

    fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        depth: i32,
    );

    fn input(&mut self, event: GameEvent) -> Response;

    fn tick(&mut self);
}
