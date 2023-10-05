use std::cell::{Ref, RefCell, RefMut};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::entity::{Component, Entity};
use crate::event::{GameEvent, Response};

// ---------------
//   Shared Cell
// ---------------
#[derive(Debug)]
#[allow(dead_code)]
pub struct SharedCell<T> {
    inner: Rc<RefCell<T>>,
}

impl<T> SharedCell<T> {
    pub fn new(inner: T) -> Self {
        SharedCell {
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }

    pub fn set(&self, new_val: T) {
        *self.inner.borrow_mut() = new_val;
    }
}

impl<T> Clone for SharedCell<T> {
    fn clone(&self) -> Self {
        SharedCell {
            inner: self.inner.clone(),
        }
    }
}

impl<T: PartialEq> PartialEq for SharedCell<T> {
    fn eq(&self, other: &Self) -> bool {
        self.borrow().deref().eq(other.borrow().deref())
    }
}

// -------------------
//   Buffer and Refs
// -------------------
//  not multi thread safe
//    such a struct would be possible and v useful at some point
pub struct QueueBuffer<T> {
    inner_ref: QueueBufferRef<T>,
}
pub struct QueueBufferRef<T> {
    buffer: SharedCell<Vec<T>>,
}
impl<T> QueueBuffer<T> {
    pub fn new() -> Self {
        QueueBuffer {
            inner_ref: QueueBufferRef::new(),
        }
    }

    pub fn get_ref(&self) -> QueueBufferRef<T> {
        self.inner_ref.clone()
    }

    pub fn get_buffer(&mut self) -> Vec<T> {
        let mut vec = Vec::new();
        mem::swap(&mut vec, self.inner_ref.buffer.borrow_mut().deref_mut());
        vec
    }
}
impl<T> QueueBufferRef<T> {
    pub fn new() -> Self {
        QueueBufferRef {
            buffer: SharedCell::new(Vec::new()),
        }
    }

    pub fn clone(&self) -> Self {
        QueueBufferRef {
            buffer: self.buffer.clone(),
        }
    }

    pub fn push(&mut self, e: T) {
        self.buffer.borrow_mut().push(e)
    }
}

impl<T: Clone> Clone for QueueBufferRef<T> {
    fn clone(&self) -> Self {
        QueueBufferRef {
            buffer: self.buffer.clone(),
        }
    }
}

// --------------
//   Id Manager
// --------------
pub enum ObjectWrap {
    Entity(SharedCell<Entity>),
    Component(SharedCell<Component>),
}

impl ObjectWrap {
    pub fn get_id(&self) -> u64 {
        match self {
            ObjectWrap::Entity(entity) => entity.borrow().get_id(),
            ObjectWrap::Component(component) => component.borrow().get_id(),
        }
    }

    pub fn input(&self, event: GameEvent) -> Response {
        match self {
            ObjectWrap::Entity(entity) => entity.borrow_mut().input(event),
            ObjectWrap::Component(component) => component.borrow_mut().input(event),
        }
    }

    pub fn to_entity(self) -> Option<SharedCell<Entity>> {
        match self {
            ObjectWrap::Entity(cell) => Some(cell),
            _ => None,
        }
    }

    pub fn to_component(self) -> Option<SharedCell<Component>> {
        match self {
            ObjectWrap::Component(cell) => Some(cell),
            _ => None,
        }
    }
}

impl Clone for ObjectWrap {
    fn clone(&self) -> Self {
        match self {
            ObjectWrap::Entity(cell) => ObjectWrap::Entity(cell.clone()),
            ObjectWrap::Component(cell) => ObjectWrap::Component(cell.clone()),
        }
    }
}

pub struct IdManager {
    map: SharedCell<HashMap<u64, ObjectWrap>>,
    hasher: SharedCell<DefaultHasher>,
}

impl IdManager {
    pub fn new() -> Self {
        Self {
            map: SharedCell::new(HashMap::new()),
            hasher: SharedCell::new(DefaultHasher::new()),
        }
    }

    pub fn get(&self, id: u64) -> Option<ObjectWrap> {
        // self.map.borrow().get(&id)
        let map = self.map.borrow();
        if let Some(t) = map.get(&id) {
            return Some(t.clone());
        }
        None
    }

    pub fn get_mut(&self, id: u64) -> Option<ObjectWrap> {
        // self.map.borrow_mut().get_mut(&id)
        let mut map = self.map.borrow_mut();
        if let Some(t) = map.get_mut(&id) {
            return Some(t.clone());
        }
        None
    }

    pub fn next_id(&self) -> u64 {
        let mut hasher = self.hasher.borrow_mut();
        let hash = hasher.finish();
        hash.hash(hasher.deref_mut());
        hash
    }

    pub fn register_entity(&self, entity: SharedCell<Entity>) {
        let mut map = self.map.borrow_mut();
        let id = entity.borrow().get_id();
        map.insert(id, ObjectWrap::Entity(entity));
    }

    pub fn register_component(&self, component: SharedCell<Component>) {
        let mut map = self.map.borrow_mut();
        let id = component.borrow().get_id();
        map.insert(id, ObjectWrap::Component(component));
    }
}

impl Clone for IdManager {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            hasher: self.hasher.clone(),
        }
    }
}
