use adw::prelude::*;

pub fn split_button(
    label: &str,
    secondary_label: &str,
    primary: impl Fn(&gtk::Widget) + 'static,
    secondary: impl Fn() + 'static,
) -> gtk::Widget {
    let label = crate::i18n::text(label);
    let secondary_label = crate::i18n::text(secondary_label);
    let secondary_button = gtk::Button::builder()
        .label(secondary_label.as_ref())
        .css_classes(["flat"])
        .build();
    let popover = gtk::Popover::builder().child(&secondary_button).build();
    let button = adw::SplitButton::builder()
        .label(label.as_ref())
        .popover(&popover)
        .build();
    button.connect_clicked(move |button| primary(button.upcast_ref()));
    let close_popover = popover.clone();
    secondary_button.connect_clicked(move |_| {
        close_popover.popdown();
        secondary();
    });
    button.upcast()
}
