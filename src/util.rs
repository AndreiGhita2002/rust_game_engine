use std::cell::{Ref, RefCell, RefMut};
use std::ops::Deref;
use std::rc::Rc;

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
