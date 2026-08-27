use super::*;
use glib::translate::{ToGlibPtrMut, from_glib};

pub(super) fn video_content_rect(
    surface_width: i32,
    surface_height: i32,
    canvas_width: u32,
    canvas_height: u32,
    padding_px: u32,
) -> Rect {
    let surface_width = surface_width.max(1) as f32;
    let surface_height = surface_height.max(1) as f32;
    let padding = padding_px as f32;
    let available_width = (surface_width - padding * 2.0).max(1.0);
    let available_height = (surface_height - padding * 2.0).max(1.0);
    let available_aspect = available_width / available_height;
    let canvas_aspect = canvas_width.max(1) as f32 / canvas_height.max(1) as f32;
    let (width, height) = if available_aspect > canvas_aspect {
        (available_height * canvas_aspect, available_height)
    } else {
        (available_width, available_width / canvas_aspect)
    };
    Rect::from_min_size(
        vec2(
            (surface_width - width) * 0.5,
            (surface_height - height) * 0.5,
        ),
        vec2(width, height),
    )
}

pub(super) fn surface_viewport(
    area: &gtk::GLArea,
    project: &Project,
    state: &VideoSurfaceState,
) -> PreviewViewport {
    PreviewViewport::new(
        GlamVec2::new(
            project.canvas_size.width.max(1) as f32,
            project.canvas_size.height.max(1) as f32,
        ),
        video_content_rect(
            area.width().max(1),
            area.height().max(1),
            project.canvas_size.width,
            project.canvas_size.height,
            state.padding_px(),
        ),
    )
}

pub(super) fn theme_window_color(area: &gtk::GLArea) -> Color {
    unsafe {
        let context =
            gtk::ffi::gtk_widget_get_style_context(area.as_ptr() as *mut gtk::ffi::GtkWidget);
        let mut color = gdk::RGBA::TRANSPARENT;
        let found: bool = from_glib(gtk::ffi::gtk_style_context_lookup_color(
            context,
            c"window_bg_color".as_ptr(),
            color.to_glib_none_mut().0,
        ));
        assert!(found, "Adwaita theme does not define window_bg_color");
        color.into()
    }
}
