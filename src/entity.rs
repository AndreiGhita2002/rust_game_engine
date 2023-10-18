use std::mem;
use std::ops::DerefMut;

use event::{GameEvent, Response};
use space::{NoSpaceComponent, NoSpaceMaster, SpaceComponent};

use crate::entity::component::Component;
use crate::GlobalContext;
use crate::render::{NoRender, RenderCommand, RenderComponent};
use crate::util::{IdManager, SharedCell};

pub mod system;
pub mod space;
pub mod component;
pub mod event;

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

    pub fn new_entity(
        &mut self,
        context: &GlobalContext,
        mut entity_desc: EntityDesc,
    ) -> SharedCell<Entity>
    {
        // creating the new entity
        let parent_entity = match entity_desc.parent_id {
            Some(r) => r.into_entity(&self.id_manager),
            None => 0u64.into_entity(&self.id_manager),
        };
        let p_id = parent_entity.get_id();
        let entity = SharedCell::new(Entity {
            id: self.id_manager.next_id(),
            parent_id: p_id,
            children: vec![],
            render_component: entity_desc.get_render_component().unwrap_or(NoRender::new()),
            space_component: entity_desc.get_space_component().unwrap_or(NoSpaceComponent::new()),
            components: entity_desc.get_components(),
        });
        // registering the new entity:
        self.id_manager.register_entity(entity.clone());
        self.entities.push(entity.clone());
        parent_entity.borrow_mut().add_child(entity.clone());

        // going through all of the entities parents and letting them init the new entity:
        let mut parent_id = p_id;
        let mut depth = 0;
        while parent_id != 0 {
            let current_parent = self.id_manager.get(parent_id).unwrap().to_entity().unwrap();
            let current_parent_b = current_parent.borrow();
            current_parent_b.init_child(context, entity.clone(), &entity_desc, depth);
            depth += 1;
            parent_id = current_parent_b.parent_id;
        }
        // initialising the entity:
        {
            let mut entity_b = entity.borrow_mut();
            entity_b.init(context);
            // todo this should be in space master init
            //  maybe it should take the EntityDesc as an argument and figure out the position from there
            entity_b.space_component.translate(&entity_desc.position)
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
        for (i, entity_cell) in self.entities.iter().enumerate() {
            let entity = entity_cell.borrow();
            let id = entity.get_id();
            println!("[{i}:{id}]")
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

    pub fn init_child(
        &self,
        context: &GlobalContext,
        child: SharedCell<Entity>,
        entity_desc: &EntityDesc,
        depth: i32
    ) {
        println!("Entity:{} is initialising child Entity:{}", self.get_id(), child.get_id());
        // first the space component:
        self.space_component.init_child_entity(context, child.clone(), entity_desc, depth);
        // all the components get the chance to edit the new child:
        for component in self.components.iter() {
            component.init_child_entity(context, child.clone(), entity_desc, depth)
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
    pub parent_id: Option<u64>,
    pub position: Vec<f32>,
    pub rotation: Vec<f32>,
    pub components: Vec<Component>,
    pub space_component: Option<Box<dyn SpaceComponent>>,
    pub render_component: Option<Box<dyn RenderComponent>>,
}
impl EntityDesc {
    fn get_space_component(&mut self) -> Option<Box<dyn SpaceComponent>> {
        let mut comp = None;
        mem::swap(&mut self.space_component, &mut comp);
        comp
    }

    fn get_render_component(&mut self) -> Option<Box<dyn RenderComponent>> {
        let mut comp = None;
        mem::swap(&mut self.render_component, &mut comp);
        comp
    }

    fn get_components(&mut self) -> Vec<Component> {
        //todo better to do a clone, surely?
        let mut comps = vec![];
        mem::swap(&mut self.components, &mut comps);
        comps
    }
}
impl Default for EntityDesc {
    fn default() -> Self {
        EntityDesc {
            parent_id: None,
            position: vec![0.0, 0.0, 0.0],
            rotation: vec![1.0, 0.0, 0.0, 0.0],
            components: vec![],
            space_component: None,
            render_component: None,
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
