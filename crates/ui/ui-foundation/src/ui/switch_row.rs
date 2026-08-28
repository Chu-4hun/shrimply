use gtk::prelude::*;

use super::control_row;

pub fn switch_row(
    label: &str,
    tooltip: Option<&str>,
    active: bool,
    changed: impl Fn(bool) + 'static,
) -> gtk::Widget {
    let switch = gtk::Switch::builder()
        .active(active)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    if let Some(tooltip) = tooltip {
        switch.set_tooltip_text(Some(crate::i18n::text(tooltip).as_ref()));
    }
    switch.connect_active_notify(move |switch| changed(switch.is_active()));
    control_row(label, &switch)
}
