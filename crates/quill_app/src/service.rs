//! Type-keyed service registry shared by plugins and the app.
//!
//! A plugin registers the values it owns during [`Plugin::build`](crate::Plugin::build);
//! the runtime and the application read them back by concrete type. There is
//! deliberately no dependency graph: a plugin registers what it provides, and a
//! consumer asks for the type it needs. A missing required service is the
//! consumer's problem (the platform plugins panic with their name in 31.2).

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A set of values keyed by their concrete type.
#[derive(Default)]
pub struct ServiceMap {
    services: HashMap<TypeId, Box<dyn Any>>,
}

impl ServiceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `service`, returning any previous value of the same type.
    pub fn insert<T: Any>(&mut self, service: T) -> Option<T> {
        self.services
            .insert(TypeId::of::<T>(), Box::new(service))
            .and_then(|previous| previous.downcast::<T>().ok())
            .map(|previous| *previous)
    }

    /// The value registered for `T`, if any.
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.services.get(&TypeId::of::<T>())?.downcast_ref()
    }

    /// Mutable access to the value registered for `T`, if any.
    pub fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.services.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    /// Removes and returns the value registered for `T`, if any.
    pub fn remove<T: Any>(&mut self) -> Option<T> {
        self.services
            .remove(&TypeId::of::<T>())
            .and_then(|service| service.downcast::<T>().ok())
            .map(|service| *service)
    }

    pub fn contains<T: Any>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_round_trip_by_type() {
        let mut services = ServiceMap::new();
        assert!(services.is_empty());

        assert_eq!(services.insert(7u32), None);
        assert_eq!(services.insert("hello"), None);
        assert_eq!(services.len(), 2);
        assert!(services.contains::<u32>());
        assert_eq!(services.get::<u32>(), Some(&7));
        assert_eq!(services.get::<&str>(), Some(&"hello"));

        *services.get_mut::<u32>().unwrap() = 9;
        assert_eq!(services.get::<u32>(), Some(&9));

        // Re-inserting a type replaces the value.
        assert_eq!(services.insert(1u32), Some(9));
        assert_eq!(services.get::<u32>(), Some(&1));

        // A different type is a miss, not a wrong downcast.
        assert_eq!(services.get::<u8>(), None);

        assert_eq!(services.remove::<u32>(), Some(1));
        assert!(!services.contains::<u32>());
        assert_eq!(services.remove::<u32>(), None);
    }
}
