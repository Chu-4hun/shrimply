use shrimply_ui_foundation::tr;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

pub(super) fn widget() -> gtk::Widget {
    let status = adw::ExpanderRow::builder()
        .title(tr!("Live Performance").as_ref())
        .build();
    let clear = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text(tr!("Clear").as_ref())
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(tr!("Copy JSON").as_ref())
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    status.add_suffix(&clear);
    status.add_suffix(&copy);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("card");
    content.append(&status);

    let rows = Rc::new(RefCell::new(Vec::<adw::ActionRow>::new()));

    clear.connect_clicked({
        let status = status.clone();
        let rows = rows.clone();
        move |_| {
            shrimply_benchmarking::clear();
            if status.is_expanded() {
                refresh(&status, &rows);
            }
        }
    });
    copy.connect_clicked(|button| {
        button
            .display()
            .clipboard()
            .set_text(&shrimply_benchmarking::report_json());
        shrimply_ui_foundation::toast::show_confirmation_for_widget(
            button,
            "Benchmark report copied",
        );
    });

    status.connect_expanded_notify({
        let rows = rows.clone();
        move |status| {
            if status.is_expanded() {
                refresh(status, &rows);
            } else {
                for row in rows.take() {
                    status.remove(&row);
                }
            }
        }
    });

    let status_weak = status.downgrade();
    let status_timer = status.clone();
    glib::timeout_add_local(REFRESH_INTERVAL, move || {
        if status_weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        if status_timer.is_expanded() {
            refresh(&status_timer, &rows);
        }
        glib::ControlFlow::Continue
    });

    content.upcast()
}

fn refresh(status: &adw::ExpanderRow, rows: &RefCell<Vec<adw::ActionRow>>) {
    for row in rows.take() {
        status.remove(&row);
    }

    let mut snapshot = shrimply_benchmarking::snapshot();
    snapshot
        .timings
        .sort_by_key(|timing| std::cmp::Reverse(timing.average));
    snapshot.counters.sort_by_key(|counter| counter.name);
    let frame = snapshot
        .timings
        .iter()
        .find(|timing| timing.name == "Video / Render request")
        .map(|timing| timing.average);

    let mut next_rows = Vec::with_capacity(snapshot.timings.len() + snapshot.counters.len());
    for timing in snapshot.timings {
        let percentage = frame
            .filter(|frame| !frame.is_zero())
            .map(|frame| {
                let tenths = timing
                    .average
                    .as_nanos()
                    .saturating_mul(1_000)
                    .checked_div(frame.as_nanos())
                    .unwrap_or_default();
                format!(" · {}.{}% frame", tenths / 10, tenths % 10)
            })
            .unwrap_or_default();
        let row = adw::ActionRow::builder()
            .title(timing.name)
            .subtitle(shrimply_ui_foundation::i18n::text_args(
                "Last %{last} · Avg %{average} · Min %{minimum} · Max %{maximum} · %{samples} samples%{percentage}",
                &[
                    ("last", duration_label(timing.last)),
                    ("average", duration_label(timing.average)),
                    ("minimum", duration_label(timing.minimum)),
                    ("maximum", duration_label(timing.maximum)),
                    ("samples", timing.samples.to_string()),
                    ("percentage", percentage),
                ],
            ))
            .build();
        status.add_row(&row);
        next_rows.push(row);
    }
    for counter in snapshot.counters {
        let row = adw::ActionRow::builder()
            .title(counter.name)
            .subtitle(counter.value.to_string())
            .build();
        status.add_row(&row);
        next_rows.push(row);
    }

    *rows.borrow_mut() = next_rows;
}

fn duration_label(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros >= 1_000_000 {
        format!(
            "{}.{:02} s",
            micros / 1_000_000,
            micros % 1_000_000 / 10_000
        )
    } else if micros >= 1_000 {
        format!("{}.{:02} ms", micros / 1_000, micros % 1_000 / 10)
    } else {
        format!("{micros} µs")
    }
}
