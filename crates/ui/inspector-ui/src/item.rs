use std::rc::Rc;

use gtk::prelude::*;

use super::InspectorContext;
use crate::preview_focus::{PreviewFacetKey, PreviewTarget};
use shrimply_project::project::ITEM_PREVIEW_FACET;

pub(super) struct HeaderAction {
    pub(super) icon: &'static str,
    pub(super) tooltip: &'static str,
    pub(super) sensitive: bool,
    pub(super) activate: Rc<dyn Fn()>,
}

pub(super) struct HeaderToggle {
    pub(super) active: bool,
    pub(super) tooltip: &'static str,
    pub(super) activate: Rc<dyn Fn(bool)>,
}

pub(super) struct HeaderButtonToggle {
    pub(super) icon: &'static str,
    pub(super) active: bool,
    pub(super) tooltip: &'static str,
    pub(super) activate: Rc<dyn Fn(bool)>,
}

pub(super) trait InspectorItem {
    fn key(&self) -> &str;
    fn title(&self) -> &str;
    fn controls(&self, context: &InspectorContext) -> Vec<gtk::Widget>;
    fn reset(&self, context: &InspectorContext) -> Rc<dyn Fn()>;
    fn actions(&self) -> &[HeaderAction];
    fn toggle(&self) -> Option<&HeaderToggle>;
    fn button_toggle(&self) -> Option<&HeaderButtonToggle>;
    fn preview_target(&self) -> PreviewFocusTarget;
}

pub(super) enum InspectorListItem {
    Item(Box<dyn InspectorItem>),
    Flat(gtk::Widget),
}

type Controls<T> = dyn Fn(&T, &InspectorContext) -> Vec<gtk::Widget>;
type DefaultValue<T> = dyn Fn(&InspectorContext) -> T;
type Apply<T> = dyn Fn(&InspectorContext, T);

pub(super) struct DefaultInspectorItem<T: Default + 'static> {
    key: String,
    title: String,
    value: T,
    controls: Rc<Controls<T>>,
    default_value: Rc<DefaultValue<T>>,
    apply: Rc<Apply<T>>,
    actions: Vec<HeaderAction>,
    toggle: Option<HeaderToggle>,
    button_toggle: Option<HeaderButtonToggle>,
    preview_target: PreviewFocusTarget,
}

impl<T: Default + 'static> DefaultInspectorItem<T> {
    pub(super) fn new(
        key: impl Into<String>,
        title: impl Into<String>,
        value: T,
        controls: impl Fn(&T, &InspectorContext) -> Vec<gtk::Widget> + 'static,
        apply: impl Fn(&InspectorContext, T) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            value,
            controls: Rc::new(controls),
            default_value: Rc::new(|_| T::default()),
            apply: Rc::new(apply),
            actions: Vec::new(),
            toggle: None,
            button_toggle: None,
            preview_target: PreviewFocusTarget::facet(ITEM_PREVIEW_FACET),
        }
    }

    pub(super) fn default_with(
        mut self,
        default_value: impl Fn(&InspectorContext) -> T + 'static,
    ) -> Self {
        self.default_value = Rc::new(default_value);
        self
    }

    pub(super) fn actions(mut self, actions: Vec<HeaderAction>) -> Self {
        self.actions = actions;
        self
    }

    pub(super) fn toggle(mut self, toggle: HeaderToggle) -> Self {
        self.toggle = Some(toggle);
        self
    }

    pub(super) fn button_toggle(mut self, toggle: HeaderButtonToggle) -> Self {
        self.button_toggle = Some(toggle);
        self
    }

    pub(super) fn preview_facet(mut self, facet: PreviewFacetKey) -> Self {
        self.preview_target = PreviewFocusTarget::facet(facet);
        self
    }

    pub(super) fn preview_target(mut self, target: PreviewTarget) -> Self {
        self.preview_target = PreviewFocusTarget::target(target);
        self
    }

    pub(super) fn boxed(self) -> InspectorListItem {
        InspectorListItem::Item(Box::new(self))
    }
}

impl<T: Default + 'static> InspectorItem for DefaultInspectorItem<T> {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn controls(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        (self.controls)(&self.value, context)
    }

    fn reset(&self, context: &InspectorContext) -> Rc<dyn Fn()> {
        let context = context.detached();
        let default_value = self.default_value.clone();
        let apply = self.apply.clone();
        Rc::new(move || apply(&context, default_value(&context)))
    }

    fn actions(&self) -> &[HeaderAction] {
        &self.actions
    }

    fn toggle(&self) -> Option<&HeaderToggle> {
        self.toggle.as_ref()
    }

    fn button_toggle(&self) -> Option<&HeaderButtonToggle> {
        self.button_toggle.as_ref()
    }

    fn preview_target(&self) -> PreviewFocusTarget {
        self.preview_target
    }
}

#[derive(Clone, Copy)]
pub(super) struct PreviewFocusTarget {
    owner_id: Option<uuid::Uuid>,
    facet: PreviewFacetKey,
}

impl PreviewFocusTarget {
    pub(super) const fn facet(facet: PreviewFacetKey) -> Self {
        Self {
            owner_id: None,
            facet,
        }
    }

    pub(super) const fn target(target: PreviewTarget) -> Self {
        Self {
            owner_id: Some(target.owner_id()),
            facet: target.facet(),
        }
    }

    pub(super) fn resolve(self, item_id: uuid::Uuid) -> PreviewTarget {
        PreviewTarget::new(self.owner_id.unwrap_or(item_id), self.facet)
    }
}

pub(super) fn flat(widget: impl IsA<gtk::Widget>) -> InspectorListItem {
    InspectorListItem::Flat(widget.upcast())
}
