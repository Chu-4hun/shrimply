use shrimply_ui_foundation::tr;
use std::cell::RefCell;
use std::rc::Rc;

use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange, SharedPlayerState};
use crate::ui::NumberPicker;
use adw::prelude::*;
use shrimply_project::project::{
    CaptionEdgeStyle, CaptionFont, CaptionItem, CaptionWritingDirection, Color, HorizontalAlign,
    Project, VerticalAlign,
};
use shrimply_ui_foundation::ui::{ColorPicker, MultilineTextInput, switch_row};

use super::{
    Inspectable, InspectorContext,
    item::{DefaultInspectorItem, HeaderToggle},
    list,
    section::InspectorSection,
    selector::selector,
};

impl Inspectable for CaptionItem {
    fn title(&self) -> &'static str {
        "Caption"
    }

    fn add_rows(&self, section: &InspectorSection, context: &InspectorContext) {
        let Some(key) = context.selected_item.clone() else {
            return;
        };
        add_text_editor(section, context, key.clone(), &self.text);
        add_enabled_editor(
            section,
            context,
            key.clone(),
            "Enable layout",
            self.layout_enabled,
            |item, enabled| item.layout_enabled = enabled,
        );
        add_enabled_editor(
            section,
            context,
            key.clone(),
            "Enable styling",
            self.styling_enabled,
            |item, enabled| item.styling_enabled = enabled,
        );
        add_h_align_editor(section, context, key.clone(), self.h_align);
        add_v_align_editor(section, context, key.clone(), self.v_align);
        add_number_editor(
            section,
            context,
            key.clone(),
            "Position X",
            self.position_x.into(),
            0..=100,
            |item, value| item.position_x = value as u8,
        );
        add_number_editor(
            section,
            context,
            key.clone(),
            "Position Y",
            self.position_y.into(),
            0..=100,
            |item, value| item.position_y = value as u8,
        );
        add_number_editor(
            section,
            context,
            key.clone(),
            "Font size",
            self.font_scale,
            75..=300,
            |item, value| item.font_scale = value,
        );
        add_font_editor(section, context, key.clone(), self.font);
        add_direction_editor(section, context, key.clone(), self.writing_direction);
        add_edge_editor(section, context, key.clone(), self.edge_style);
        add_color_editor(
            section,
            context,
            key.clone(),
            "Text color",
            self.text_color,
            |item, value| item.text_color = value,
        );
        add_color_editor(
            section,
            context,
            key.clone(),
            "Background",
            self.background_color,
            |item, value| item.background_color = value,
        );
        add_color_editor(
            section,
            context,
            key.clone(),
            "Edge color",
            self.edge_color,
            |item, value| item.edge_color = value,
        );
    }

    fn inspect(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        let info_items = vec![super::info::item(
            context,
            super::info::ItemInfo {
                leading: Vec::new(),
                kind: "Caption",
                natural_duration: None,
                start: self.start,
                end: self.end,
                source_offset: None,
                dimensions: None,
                file: None,
                source_metadata: super::info::SourceMetadata::None,
            },
        )];
        let text_items = vec![
            DefaultInspectorItem::new(
                "caption-text",
                "Text",
                CaptionText {
                    text: self.text.clone(),
                    writing_direction: self.writing_direction,
                },
                |value, context| {
                    let section = InspectorSection::controls();
                    if let Some(key) = context.selected_item.clone() {
                        add_text_editor(&section, context, key.clone(), &value.text);
                        add_direction_editor(
                            &section,
                            context,
                            key.clone(),
                            value.writing_direction,
                        );
                    }
                    vec![section.into_widget()]
                },
                |context, value: CaptionText| {
                    apply_caption_reset(context, "reset-caption-text", move |item| {
                        item.writing_direction = value.writing_direction
                    });
                },
            )
            .boxed(),
        ];
        let visual_items = vec![
            DefaultInspectorItem::new(
                "caption-layout",
                "Layout",
                CaptionLayout::from(self.clone()),
                caption_layout_controls,
                |context, value: CaptionLayout| {
                    apply_caption_reset(context, "reset-caption-layout", move |item| {
                        value.apply(item)
                    });
                },
            )
            .toggle(enabled_header_toggle(
                context,
                self.layout_enabled,
                "Enable layout",
                |item, enabled| item.layout_enabled = enabled,
            ))
            .boxed(),
            DefaultInspectorItem::new(
                "caption-appearance",
                "Appearance",
                CaptionAppearance::from(self.clone()),
                caption_appearance_controls,
                |context, value: CaptionAppearance| {
                    apply_caption_reset(context, "reset-caption-appearance", move |item| {
                        value.apply(item)
                    });
                },
            )
            .toggle(enabled_header_toggle(
                context,
                self.styling_enabled,
                "Enable styling",
                |item, enabled| item.styling_enabled = enabled,
            ))
            .boxed(),
        ];
        list::render_categories(
            vec![
                list::InspectorCategory {
                    key: "text",
                    label: "Text",
                    icon: "insert-text-symbolic",
                    items: text_items,
                },
                list::InspectorCategory {
                    key: "visual",
                    label: "Visual",
                    icon: "blend-tool-symbolic",
                    items: visual_items,
                },
                list::InspectorCategory {
                    key: "info",
                    label: "Info",
                    icon: "info-outline-symbolic",
                    items: info_items,
                },
            ],
            context,
        )
    }
}

fn enabled_header_toggle(
    context: &InspectorContext,
    active: bool,
    tooltip: &'static str,
    set: fn(&mut CaptionItem, bool),
) -> HeaderToggle {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    let key = context.selected_item.clone().unwrap();
    HeaderToggle {
        active,
        tooltip,
        activate: Rc::new(move |enabled| {
            update_caption(&project, &player_state, key.clone(), |item| {
                set(item, enabled)
            });
            refresh();
        }),
    }
}

struct CaptionText {
    text: String,
    writing_direction: CaptionWritingDirection,
}

impl Default for CaptionText {
    fn default() -> Self {
        let item = default_caption();
        Self {
            text: item.text,
            writing_direction: item.writing_direction,
        }
    }
}

struct CaptionLayout {
    layout_enabled: bool,
    h_align: HorizontalAlign,
    v_align: VerticalAlign,
    position_x: u8,
    position_y: u8,
}

impl Default for CaptionLayout {
    fn default() -> Self {
        Self::from(default_caption())
    }
}

impl From<CaptionItem> for CaptionLayout {
    fn from(item: CaptionItem) -> Self {
        Self {
            layout_enabled: item.layout_enabled,
            h_align: item.h_align,
            v_align: item.v_align,
            position_x: item.position_x,
            position_y: item.position_y,
        }
    }
}

impl CaptionLayout {
    fn apply(self, item: &mut CaptionItem) {
        item.layout_enabled = self.layout_enabled;
        item.h_align = self.h_align;
        item.v_align = self.v_align;
        item.position_x = self.position_x;
        item.position_y = self.position_y;
    }
}

struct CaptionAppearance {
    styling_enabled: bool,
    font_scale: u16,
    font: CaptionFont,
    edge_style: CaptionEdgeStyle,
    text_color: Color<u8>,
    background_color: Color<u8>,
    edge_color: Color<u8>,
}

impl Default for CaptionAppearance {
    fn default() -> Self {
        Self::from(default_caption())
    }
}

impl From<CaptionItem> for CaptionAppearance {
    fn from(item: CaptionItem) -> Self {
        Self {
            styling_enabled: item.styling_enabled,
            font_scale: item.font_scale,
            font: item.font,
            edge_style: item.edge_style,
            text_color: item.text_color,
            background_color: item.background_color,
            edge_color: item.edge_color,
        }
    }
}

impl CaptionAppearance {
    fn apply(self, item: &mut CaptionItem) {
        item.styling_enabled = self.styling_enabled;
        item.font_scale = self.font_scale;
        item.font = self.font;
        item.edge_style = self.edge_style;
        item.text_color = self.text_color;
        item.background_color = self.background_color;
        item.edge_color = self.edge_color;
    }
}

fn caption_layout_controls(value: &CaptionLayout, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let Some(key) = context.selected_item.clone() else {
        return vec![section.into_widget()];
    };
    add_h_align_editor(&section, context, key.clone(), value.h_align);
    add_v_align_editor(&section, context, key.clone(), value.v_align);
    add_number_editor(
        &section,
        context,
        key.clone(),
        "Position X",
        value.position_x.into(),
        0..=100,
        |item, value| item.position_x = value as u8,
    );
    add_number_editor(
        &section,
        context,
        key.clone(),
        "Position Y",
        value.position_y.into(),
        0..=100,
        |item, value| item.position_y = value as u8,
    );
    let controls = section.into_widget();
    controls.set_sensitive(value.layout_enabled);
    vec![controls]
}

fn caption_appearance_controls(
    value: &CaptionAppearance,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let Some(key) = context.selected_item.clone() else {
        return vec![section.into_widget()];
    };
    add_number_editor(
        &section,
        context,
        key.clone(),
        "Font size",
        value.font_scale,
        75..=300,
        |item, value| item.font_scale = value,
    );
    add_font_editor(&section, context, key.clone(), value.font);
    add_edge_editor(&section, context, key.clone(), value.edge_style);
    add_color_editor(
        &section,
        context,
        key.clone(),
        "Text color",
        value.text_color,
        |item, value| item.text_color = value,
    );
    add_color_editor(
        &section,
        context,
        key.clone(),
        "Background",
        value.background_color,
        |item, value| item.background_color = value,
    );
    add_color_editor(
        &section,
        context,
        key.clone(),
        "Edge color",
        value.edge_color,
        |item, value| item.edge_color = value,
    );
    let controls = section.into_widget();
    controls.set_sensitive(value.styling_enabled);
    vec![controls]
}

fn default_caption() -> CaptionItem {
    CaptionItem::new(
        shrimply_project::project::Time::ZERO,
        shrimply_project::project::Time::ZERO,
        String::new(),
    )
}

fn apply_caption_reset(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut CaptionItem),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.caption_item_mut(&key) else {
        return;
    };
    update(item);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            captions: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

fn add_text_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    text: &str,
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let commit_project = context.project.clone();
    let editor = MultilineTextInput::builder(text)
        .on_change(move |text| update_caption_text_live(&project, &player_state, key.clone(), text))
        .on_commit(move || {
            shrimply_project::project::commit_edit(&commit_project.borrow(), "caption");
        })
        .build();
    editor.widget().set_tooltip_text(Some(
        "Caption markup: **bold**, *italic*, __underline__, {milliseconds} karaoke, [base/ruby]",
    ));
    section.add_control_row("Text", editor.widget());
}

fn add_enabled_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    label: &str,
    enabled: bool,
    set: fn(&mut CaptionItem, bool),
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    let row = switch_row(label, None, enabled, move |enabled| {
        update_caption(&project, &player_state, key.clone(), |item| {
            set(item, enabled);
        });
        refresh();
    });
    section.add_wide_control(&row);
}

fn add_number_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    label: &str,
    value: u16,
    range: std::ops::RangeInclusive<u16>,
    set: fn(&mut CaptionItem, u16),
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let picker = NumberPicker::builder(f64::from(value))
        .drag_step(1.0)
        .digits(0)
        .width_chars(6)
        .minimum(f64::from(*range.start()))
        .accepted_range(f64::from(*range.start()), f64::from(*range.end()))
        .unit_name("%")
        .on_change(move |value| {
            update_caption(&project, &player_state, key.clone(), |item| {
                set(item, value as u16)
            });
        })
        .build();
    section.add_control_row(label, &picker);
}

fn add_font_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    value: CaptionFont,
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let dropdown = selector(
        "Font",
        value,
        [
            (CaptionFont::Roboto, "Roboto"),
            (CaptionFont::MonospaceSerif, "Monospace Serif"),
            (CaptionFont::Serif, "Serif"),
            (CaptionFont::MonospaceSans, "Monospace Sans"),
            (CaptionFont::Casual, "Casual"),
            (CaptionFont::Cursive, "Cursive"),
            (CaptionFont::SmallCapitals, "Small Capitals"),
        ],
        move |value| {
            update_caption(&project, &player_state, key.clone(), |item| {
                item.font = value
            })
        },
    );
    section.add_wide_control(&dropdown);
}

fn add_direction_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    value: CaptionWritingDirection,
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let dropdown = selector(
        "Writing",
        value,
        [
            (CaptionWritingDirection::Horizontal, "Horizontal"),
            (CaptionWritingDirection::VerticalRightToLeft, "Vertical RTL"),
            (CaptionWritingDirection::VerticalLeftToRight, "Vertical LTR"),
            (CaptionWritingDirection::RotatedLeftToRight, "Rotated LTR"),
            (CaptionWritingDirection::RotatedRightToLeft, "Rotated RTL"),
        ],
        move |value| {
            update_caption(&project, &player_state, key.clone(), |item| {
                item.writing_direction = value
            });
        },
    );
    section.add_wide_control(&dropdown);
}

fn add_edge_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    value: CaptionEdgeStyle,
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let dropdown = selector(
        "Edge",
        value,
        [
            (CaptionEdgeStyle::None, "None"),
            (CaptionEdgeStyle::HardShadow, "Hard shadow"),
            (CaptionEdgeStyle::Bevel, "Bevel"),
            (CaptionEdgeStyle::Glow, "Glow / outline"),
            (CaptionEdgeStyle::SoftShadow, "Soft shadow"),
        ],
        move |value| {
            update_caption(&project, &player_state, key.clone(), |item| {
                item.edge_style = value
            });
        },
    );
    section.add_wide_control(&dropdown);
}

fn add_h_align_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    align: HorizontalAlign,
) {
    let group = adw::ToggleGroup::builder()
        .active(h_align_index(align))
        .halign(gtk::Align::End)
        .homogeneous(true)
        .build();
    group.add(align_group_toggle(
        "left",
        "text-justify-left-symbolic",
        "Left",
    ));
    group.add(align_group_toggle(
        "center",
        "text-justify-center-symbolic",
        "Center",
    ));
    group.add(align_group_toggle(
        "right",
        "text-justify-right-symbolic",
        "Right",
    ));
    section.add_control_row("H align", &group);

    let project = context.project.clone();
    let player_state = context.player_state.clone();
    group.connect_active_notify(move |group| {
        let Some(align) = h_align_from_index(group.active()) else {
            return;
        };
        update_caption(&project, &player_state, key.clone(), |item| {
            if item.h_align == align {
                return;
            }
            item.h_align = align;
        });
    });
}

fn add_v_align_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    align: VerticalAlign,
) {
    let group = adw::ToggleGroup::builder()
        .active(v_align_index(align))
        .halign(gtk::Align::End)
        .homogeneous(true)
        .build();
    group.add(align_group_toggle("top", "valign-start-symbolic", "Top"));
    group.add(align_group_toggle(
        "middle",
        "valign-center-symbolic",
        "Middle",
    ));
    group.add(align_group_toggle(
        "bottom",
        "valign-end-symbolic",
        "Bottom",
    ));
    section.add_control_row("V align", &group);

    let project = context.project.clone();
    let player_state = context.player_state.clone();
    group.connect_active_notify(move |group| {
        let Some(align) = v_align_from_index(group.active()) else {
            return;
        };
        update_caption(&project, &player_state, key.clone(), |item| {
            if item.v_align == align {
                return;
            }
            item.v_align = align;
        });
    });
}

fn align_group_toggle(name: &str, icon: &str, tooltip: &str) -> adw::Toggle {
    adw::Toggle::builder()
        .name(name)
        .icon_name(icon)
        .tooltip(tooltip)
        .build()
}

fn add_color_editor(
    section: &InspectorSection,
    context: &InspectorContext,
    key: SelectedItem,
    label: &str,
    color: Color<u8>,
    set: fn(&mut CaptionItem, Color<u8>),
) {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let button = ColorPicker::builder(color)
        .title(tr!("Caption text color").as_ref())
        .hexpand(true)
        .on_change(move |color| {
            update_caption(&project, &player_state, key.clone(), |item| {
                set(item, color);
            });
        })
        .build();
    section.add_control_row(label, &button);
}

fn update_caption(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    update: impl FnOnce(&mut CaptionItem),
) {
    let mut project = project.borrow_mut();
    let Some(item) = project.caption_item_mut(&key) else {
        return;
    };
    update(item);
    shrimply_project::project::commit_coalesced_edit(&project, "caption");
    drop(project);

    player_state::refresh_project(
        player_state,
        ProjectChange {
            captions: true,
            ..ProjectChange::default()
        },
    );
}

fn update_caption_text_live(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    text: String,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(item) = project.caption_item_mut(&key) else {
        return false;
    };
    if item.text == text {
        return false;
    }
    item.text = text;
    drop(project);

    player_state::refresh_project(
        player_state,
        ProjectChange {
            captions: true,
            ..ProjectChange::default()
        },
    );
    true
}

fn h_align_index(align: HorizontalAlign) -> u32 {
    match align {
        HorizontalAlign::Left => 0,
        HorizontalAlign::Center => 1,
        HorizontalAlign::Right => 2,
    }
}

fn h_align_from_index(index: u32) -> Option<HorizontalAlign> {
    match index {
        0 => Some(HorizontalAlign::Left),
        1 => Some(HorizontalAlign::Center),
        2 => Some(HorizontalAlign::Right),
        _ => None,
    }
}

fn v_align_index(align: VerticalAlign) -> u32 {
    match align {
        VerticalAlign::Top => 0,
        VerticalAlign::Middle => 1,
        VerticalAlign::Bottom => 2,
    }
}

fn v_align_from_index(index: u32) -> Option<VerticalAlign> {
    match index {
        0 => Some(VerticalAlign::Top),
        1 => Some(VerticalAlign::Middle),
        2 => Some(VerticalAlign::Bottom),
        _ => None,
    }
}
