use crate::entity::component::Component;
use crate::entity::Entity;
use crate::GlobalContext;
use crate::render::{RenderComponent, RenderDispatcher};

pub struct NoRender {}
impl NoRender {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}
impl RenderComponent for NoRender {
    fn init(&mut self, _context: &GlobalContext, _components: &Vec<Component>) {}

    fn render(&self, _entity: &Entity, _dispatcher: &mut RenderDispatcher) {}

    fn get_name(&self) -> String {
        "No Render".to_string()
    }
}