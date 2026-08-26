use shrimply_ui_foundation::tr;
use std::rc::Rc;

use adw::prelude::{ActionRowExt, PreferencesRowExt};
use gtk::prelude::*;
use shrimply_core::timeline_value::TimelineBase;
use shrimply_pdf::PageSize;
use shrimply_project::project::{Asset, PdfItem, Transform, VideoItemContent};

use crate::InspectorContext;
use crate::item::{DefaultInspectorItem, InspectorListItem};
use crate::player_state::{self, ProjectChange};

pub(super) fn item(pdf: &PdfItem, source: Asset, _context: &InspectorContext) -> InspectorListItem {
    let controls_source = source.clone();
    DefaultInspectorItem::new(
        "pdf",
        "PDF",
        pdf.clone(),
        move |pdf, context| controls(pdf, controls_source.clone(), context),
        move |context, _: PdfItem| {
            let result = source
                .snapshot()
                .and_then(|snapshot| snapshot.read())
                .and_then(shrimply_pdf::page_sizes);
            match result.and_then(|pages| {
                pages
                    .into_iter()
                    .next()
                    .ok_or_else(|| "PDF contains no pages".to_string())
            }) {
                Ok(size) => update_page(context, 0, size, "reset-pdf-page"),
                Err(error) => tracing::warn!(%error, "could not reset PDF page"),
            }
        },
    )
    .boxed()
}

fn controls(pdf: &PdfItem, source: Asset, context: &InspectorContext) -> Vec<gtk::Widget> {
    let out = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(18, 18);
    out.append(&spinner);

    let snapshot = match source.snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            show_error(&out, &error);
            return vec![out.upcast()];
        }
    };
    let (sender, receiver) = async_channel::bounded(1);
    let inspect_snapshot = snapshot.clone();
    std::thread::spawn(move || {
        let result = inspect_snapshot.read().and_then(shrimply_pdf::page_sizes);
        let _ = sender.send_blocking(result);
    });

    let out_weak = out.downgrade();
    let listener_scope = Rc::downgrade(&context.listener_scope);
    let context = context.clone();
    let selected_page = pdf.page;
    gtk::glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else {
            return;
        };
        let Some(out) = out_weak.upgrade() else {
            return;
        };
        if listener_scope.upgrade().is_none() {
            return;
        }
        if !snapshot.is_current() {
            (context.refresh)();
            return;
        }
        while let Some(child) = out.first_child() {
            out.remove(&child);
        }
        match result {
            Ok(pages) => add_page_control(&out, pages, selected_page, &context),
            Err(error) => show_error(&out, &error),
        }
    });
    vec![out.upcast()]
}

fn add_page_control(
    out: &gtk::Box,
    pages: Vec<PageSize>,
    selected_page: u32,
    context: &InspectorContext,
) {
    let last_page = u32::try_from(pages.len() - 1).expect("PDF page count must fit u32");
    let selected_page = selected_page.min(last_page);
    if let Some(size) = pages.get(selected_page as usize).copied() {
        let current = context.selected_item.as_ref().and_then(|key| {
            let project = context.project.borrow();
            let item = project.video_item(key)?;
            let VideoItemContent::Pdf(pdf) = &item.content else {
                return None;
            };
            Some((pdf.page, item.source_width, item.source_height))
        });
        if current != Some((selected_page, size.width, size.height)) {
            update_page(context, selected_page, size, "normalize-pdf-page");
        }
    }

    let pages = Rc::new(pages);
    let page = adw::SpinRow::with_range(1.0, pages.len() as f64, 1.0);
    page.set_title(tr!("Page").as_ref());
    let page_count = if pages.len() == 1 {
        tr!("1 page").into_owned()
    } else {
        shrimply_ui_foundation::i18n::text_args(
            "%{count} pages",
            &[("count", pages.len().to_string())],
        )
    };
    page.set_subtitle(&page_count);
    page.set_digits(0);
    page.set_value(f64::from(selected_page + 1));
    let context = context.clone();
    page.connect_value_notify(move |row| {
        let index = row.value().round().max(1.0) as u32 - 1;
        let Some(size) = pages.get(index as usize).copied() else {
            return;
        };
        update_page(&context, index, size, "pdf-page");
    });
    out.append(&page);
}

fn update_page(context: &InspectorContext, page: u32, size: PageSize, commit_name: &'static str) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let VideoItemContent::Pdf(pdf) = &item.content else {
        return;
    };
    if pdf.page == page && item.source_width == size.width && item.source_height == size.height {
        return;
    }
    let old_center = glam::Vec2::new(item.source_width as f32, item.source_height as f32) * 0.5;
    let new_center = glam::Vec2::new(size.width as f32, size.height as f32) * 0.5;
    recenter_default_anchor(&mut item.transform, old_center, new_center);
    if let Some(transform) = &mut item.default_transform {
        recenter_default_anchor(transform, old_center, new_center);
    }
    let VideoItemContent::Pdf(pdf) = &mut item.content else {
        unreachable!("PDF item changed content while updating its page")
    };
    pdf.page = page;
    item.source_width = size.width;
    item.source_height = size.height;
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

fn recenter_default_anchor(transform: &mut Transform, old: glam::Vec2, new: glam::Vec2) {
    if transform.anchor.expression.is_none()
        && let TimelineBase::Const(anchor) = &mut transform.anchor.base
        && *anchor == old
    {
        *anchor = new;
    }
}

fn show_error(out: &gtk::Box, error: &str) {
    while let Some(child) = out.first_child() {
        out.remove(&child);
    }
    out.append(
        &gtk::Label::builder()
            .label(error)
            .wrap(true)
            .xalign(0.0)
            .build(),
    );
}
