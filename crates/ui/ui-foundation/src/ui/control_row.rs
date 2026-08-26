use gtk::prelude::*;

const LABEL_WIDTH_CHARS: i32 = 13;

pub fn control_row(label: &str, child: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let label = crate::i18n::text(label);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    row.append(
        &gtk::Label::builder()
            .label(label.as_ref())
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .width_chars(LABEL_WIDTH_CHARS)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    child.set_hexpand(true);
    row.append(child);
    row.upcast()
}
