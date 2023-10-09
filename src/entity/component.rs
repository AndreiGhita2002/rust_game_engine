use crate::entity::{Entity, EntityDesc};
use crate::entity::event::{GameEvent, Response};
use crate::GlobalContext;
use crate::util::SharedCell;

// todo implement some of these:
pub struct Component {
    id: u64,
    component_obj: Box<dyn ComponentObject>,
}
impl Component {
    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn init(&mut self, context: &GlobalContext) {
        self.component_obj.init(context)
    }

    pub fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        entity_desc: &EntityDesc,
        depth: i32,
    ) {
        self.component_obj
            .init_child_entity(context, child_entity, entity_desc, depth)
    }

    pub fn input(&mut self, event: GameEvent) -> Response {
        self.component_obj.input(event)
    }

    pub fn tick(&mut self) {
        self.component_obj.tick()
    }
}

pub trait ComponentObject {
    fn init(&mut self, context: &GlobalContext);

    fn init_child_entity(
        &self,
        context: &GlobalContext,
        child_entity: SharedCell<Entity>,
        entity_desc: &EntityDesc,
        depth: i32,
    );

    fn input(&mut self, event: GameEvent) -> Response;

    fn tick(&mut self);
}
