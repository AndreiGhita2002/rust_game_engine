use crate::event::GameEvent;
use crate::GlobalContext;
use crate::render::RenderComponent;
use crate::util::{IdManager, SharedCell};

struct EntityManager {
    id_finder: IdManager,
    entities: Vec<SharedCell<Entity>>,
}

pub struct Entity {
    id: u64,
    render_component: Box<dyn RenderComponent>,
    components: Vec<Component>,
    children: Vec<u64>,
}

impl Entity {
    pub fn new(context: &GlobalContext, render_component: Box<dyn RenderComponent>) -> SharedCell<Entity> {
        let id = context.id_manager.next_id();
        let entity = Entity {
            id,
            render_component,
            components: vec![],
            children: vec![],
        };
        let cell = SharedCell::new(entity);
        context.register_entity(cell.clone());
        cell
    }

    pub fn get_id(&self) -> u64 { self.id }

    pub fn input(&mut self, event: GameEvent) {

    }
}


pub struct Component {
    id: u64,
    component_obj: Box<dyn ComponentObject>
}

impl Component {
    pub fn get_id(&self) -> u64 { self.id }

    pub fn input(&mut self, event: GameEvent) {
        self.input(event);
    }

    pub fn tick(&mut self) {
        self.component_obj.tick()
    }
}

trait ComponentObject {
    fn input(&mut self, event: GameEvent);

    fn tick(&mut self);
}
