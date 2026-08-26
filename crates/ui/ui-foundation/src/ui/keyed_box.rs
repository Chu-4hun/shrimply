use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use gtk::prelude::*;

struct Child<Value> {
    value: Value,
    widget: gtk::Widget,
}

pub struct KeyedBox<Key, Value> {
    container: gtk::Box,
    children: HashMap<Key, Child<Value>>,
}

impl<Key, Value> KeyedBox<Key, Value>
where
    Key: Clone + Eq + Hash,
    Value: Clone + Eq,
{
    pub fn new(orientation: gtk::Orientation, spacing: i32) -> Self {
        Self {
            container: gtk::Box::new(orientation, spacing),
            children: HashMap::new(),
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn child(&self, key: &Key) -> Option<&gtk::Widget> {
        self.children.get(key).map(|child| &child.widget)
    }

    pub fn reconcile(
        &mut self,
        values: impl IntoIterator<Item = (Key, Value)>,
        mut create: impl FnMut(&Key, &Value) -> gtk::Widget,
    ) {
        let values = values.into_iter().collect::<Vec<_>>();
        let mut keys = HashSet::with_capacity(values.len());
        for (key, _) in &values {
            assert!(
                keys.insert(key.clone()),
                "keyed UI contains a duplicate key"
            );
        }

        let container = self.container.clone();
        self.children.retain(|key, child| {
            if keys.contains(key) {
                true
            } else {
                container.remove(&child.widget);
                false
            }
        });

        for (key, value) in &values {
            let unchanged = self
                .children
                .get(key)
                .is_some_and(|child| child.value == *value);
            if unchanged {
                continue;
            }
            if let Some(previous) = self.children.remove(key) {
                self.container.remove(&previous.widget);
            }
            let widget = create(key, value);
            self.container.append(&widget);
            self.children.insert(
                key.clone(),
                Child {
                    value: value.clone(),
                    widget,
                },
            );
        }

        let mut previous = None;
        for (key, _) in values {
            let widget = self
                .children
                .get(&key)
                .expect("reconciled key must have a widget")
                .widget
                .clone();
            self.container
                .reorder_child_after(&widget, previous.as_ref());
            previous = Some(widget);
        }
    }
}
