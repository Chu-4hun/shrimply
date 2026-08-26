use gtk::prelude::Cast;
use shrimply_core::timeline_value::TimelineStep;

pub(super) use crate::ui::{dropdown, enum_selector, selector, string_selector};

pub(super) fn step_editor<T: TimelineStep>(value: T, changed: impl Fn(T) + 'static) -> gtk::Widget {
    let variants = T::variants();
    if variants.iter().all(|variant| variant.icon.is_some()) {
        return step_button_editor(value, changed);
    }
    dropdown(
        value,
        variants
            .iter()
            .map(|variant| (variant.value, variant.label)),
        changed,
    )
    .upcast()
}

pub(super) fn step_button_editor<T: TimelineStep>(
    value: T,
    changed: impl Fn(T) + 'static,
) -> gtk::Widget {
    let variants = T::variants();
    let group = adw::ToggleGroup::builder()
        .halign(gtk::Align::End)
        .homogeneous(true)
        .build();
    for variant in variants {
        let mut toggle = adw::Toggle::builder().name(variant.key);
        toggle = if let Some(icon) = variant.icon {
            toggle.icon_name(icon).tooltip(variant.label)
        } else {
            toggle.label(variant.label)
        };
        group.add(toggle.build());
    }
    group.set_active(
        variants
            .iter()
            .position(|variant| variant.value == value)
            .expect("timeline step value must be a declared variant") as u32,
    );
    group.connect_active_notify(move |group| {
        if let Some(variant) = variants.get(group.active() as usize) {
            changed(variant.value);
        }
    });
    group.upcast()
}
