use gtk::prelude::*;
use gtk::{gdk, glib};

pub fn attach_space_play_toggle(
    widget: &impl IsA<gtk::Widget>,
    toggle_playing: impl Fn() + 'static,
    step_playback_speed_forward: impl Fn() + 'static,
) {
    let widget = widget.as_ref();
    widget.set_focusable(true);

    let click = gtk::GestureClick::new();
    let focus_widget = widget.clone();
    click.connect_pressed(move |_, _, _, _| {
        focus_widget.grab_focus();
    });
    widget.add_controller(click);

    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    key.connect_key_pressed(move |_, key, _, _| {
        match key {
            gdk::Key::space => toggle_playing(),
            gdk::Key::l | gdk::Key::L => step_playback_speed_forward(),
            _ => return glib::Propagation::Proceed,
        }

        glib::Propagation::Stop
    });
    widget.add_controller(key);
}
