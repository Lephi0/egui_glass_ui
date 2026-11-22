use crate::style::GlassPalette;
use egui::{
    Align2, Color32, Context, CornerRadius, CursorIcon, FontFamily, FontId, Frame, Id, Key,
    Margin, Painter, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
    ViewportCommand,
};
use std::hash::Hash;
use std::ops::{Range, RangeInclusive};

pub const GLASS_TITLEBAR_HEIGHT: f32 = 46.0;

pub use crate::fonts::install_system_font;

/// Renders crisp text with an optional multi-pass soft drop shadow for legibility in Clear and Glass modes.
pub fn draw_shadowed_text(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    color: Color32,
    shadow: bool,
) {
    if shadow {
        painter.text(
            pos + Vec2::new(0.0, 2.0),
            anchor,
            text,
            font.clone(),
            Color32::from_black_alpha(115),
        );
        painter.text(
            pos + Vec2::new(0.8, 1.0),
            anchor,
            text,
            font.clone(),
            Color32::from_black_alpha(80),
        );
    }
    painter.text(pos, anchor, text, font, color);
}

/// Draws a frameless glass titlebar with an optional custom badge, drag region, and close button.
/// Returns `true` if the close button was clicked.
pub fn draw_glass_titlebar(
    ctx: &Context,
    ui: &mut Ui,
    palette: &GlassPalette,
    title: &str,
    draw_badge: Option<&dyn Fn(&Painter, Rect)>,
) -> bool {
    let (title_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), GLASS_TITLEBAR_HEIGHT),
        Sense::hover(),
    );

    let close_size = 28.0;
    let close_rect = Rect::from_center_size(
        Pos2::new(
            title_rect.max.x - 16.0 - close_size * 0.5,
            title_rect.center().y,
        ),
        Vec2::splat(close_size),
    );
    let drag_rect = Rect::from_min_max(
        title_rect.min,
        Pos2::new(close_rect.min.x - 6.0, title_rect.max.y),
    );

    let close_resp = ui
        .interact(
            close_rect,
            Id::new(("glass_titlebar_close", title)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);

    let start_drag = ctx.input(|i| {
        i.pointer.button_pressed(PointerButton::Primary)
            && i.pointer
                .interact_pos()
                .map_or(false, |p| drag_rect.contains(p))
    });
    if start_drag {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let painter = ui.painter();

    let title_x = if let Some(draw_badge) = draw_badge {
        let badge_rect = Rect::from_center_size(
            Pos2::new(title_rect.min.x + 16.0 + 11.0, title_rect.center().y),
            Vec2::splat(22.0),
        );
        draw_badge(painter, badge_rect);
        badge_rect.max.x + 10.0
    } else {
        title_rect.min.x + 16.0
    };

    painter.text(
        Pos2::new(title_x, title_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::new(14.5, FontFamily::Proportional),
        palette.primary_text,
    );

    let close_hovered = close_resp.hovered();
    if close_hovered {
        painter.rect_filled(close_rect, CornerRadius::same(8), palette.danger_hover_fill);
    }
    let cross_color = if close_hovered {
        palette.item_hover_text
    } else {
        palette.secondary_text
    };
    let c = close_rect.center();
    let r = 4.4;
    let cross_stroke = Stroke::new(1.5_f32, cross_color);
    painter.line_segment(
        [Pos2::new(c.x - r, c.y - r), Pos2::new(c.x + r, c.y + r)],
        cross_stroke,
    );
    painter.line_segment(
        [Pos2::new(c.x + r, c.y - r), Pos2::new(c.x - r, c.y + r)],
        cross_stroke,
    );

    painter.line_segment(
        [
            Pos2::new(title_rect.min.x, title_rect.max.y),
            Pos2::new(title_rect.max.x, title_rect.max.y),
        ],
        Stroke::new(1.0_f32, palette.divider),
    );

    close_resp.clicked()
}

/// Draws a text-only recessed glass pill tab switcher. Returns `true` if `active_tab` changed.
pub fn draw_glass_tab_bar<T: Copy + PartialEq>(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    tabs: &[(T, &str)],
    active_tab: &mut T,
) -> bool {
    draw_glass_tab_bar_inner(ui, id_salt, palette, tabs, active_tab, None::<fn(&Painter, Pos2, T, Color32)>)
}

/// Draws a recessed glass pill tab switcher with a 13px custom icon beside each tab label.
/// Returns `true` if `active_tab` changed.
pub fn draw_glass_tab_bar_with_icons<T: Copy + PartialEq>(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    tabs: &[(T, &str)],
    active_tab: &mut T,
    draw_icon: impl Fn(&Painter, Pos2, T, Color32),
) -> bool {
    draw_glass_tab_bar_inner(ui, id_salt, palette, tabs, active_tab, Some(draw_icon))
}

fn draw_glass_tab_bar_inner<T: Copy + PartialEq>(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    tabs: &[(T, &str)],
    active_tab: &mut T,
    draw_icon: Option<impl Fn(&Painter, Pos2, T, Color32)>,
) -> bool {
    if tabs.is_empty() {
        return false;
    }

    let (track_rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());

    ui.painter().rect_filled(
        track_rect,
        CornerRadius::same(10),
        palette.segment_track_fill,
    );
    ui.painter().rect_stroke(
        track_rect,
        CornerRadius::same(10),
        Stroke::new(1.0_f32, palette.divider),
        egui::StrokeKind::Inside,
    );

    let inner = track_rect.shrink(3.0);
    let gap = 3.0;
    let count = tabs.len() as f32;
    let tab_w = ((inner.width() - gap * (count - 1.0)) / count).max(40.0);

    let mut changed = false;
    let base_id = ui.make_persistent_id(id_salt);
    for (idx, (tab, label)) in tabs.iter().enumerate() {
        let min_x = inner.min.x + idx as f32 * (tab_w + gap);
        let tab_rect =
            Rect::from_min_size(Pos2::new(min_x, inner.min.y), Vec2::new(tab_w, inner.height()));
        let resp = ui
            .interact(tab_rect, base_id.with(idx), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        let is_active = *active_tab == *tab;
        let is_hovered = resp.hovered();
        let painter = ui.painter();

        if is_active {
            painter.rect_filled(tab_rect, CornerRadius::same(8), palette.segment_active_fill);
            painter.rect_stroke(
                tab_rect,
                CornerRadius::same(8),
                Stroke::new(1.0_f32, palette.card_stroke),
                egui::StrokeKind::Inside,
            );
        } else if is_hovered {
            painter.rect_filled(tab_rect, CornerRadius::same(8), palette.control_hover_fill);
        }

        let text_color = if is_active || is_hovered {
            palette.primary_text
        } else {
            palette.secondary_text
        };

        let font = FontId::new(12.5, FontFamily::Proportional);
        let galley = painter.layout_no_wrap((*label).to_string(), font, text_color);

        if let Some(ref icon_fn) = draw_icon {
            let icon_w = 13.0;
            let icon_gap = 6.0;
            let total_w = icon_w + icon_gap + galley.size().x;
            let start_x = tab_rect.center().x - total_w * 0.5;
            let icon_center = Pos2::new(start_x + icon_w * 0.5, tab_rect.center().y);

            icon_fn(painter, icon_center, *tab, text_color);
            painter.galley(
                Pos2::new(
                    start_x + icon_w + icon_gap,
                    tab_rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                text_color,
            );
        } else {
            painter.galley(
                Pos2::new(
                    tab_rect.center().x - galley.size().x * 0.5,
                    tab_rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                text_color,
            );
        }

        if resp.clicked() && !is_active {
            *active_tab = *tab;
            changed = true;
        }
    }
    changed
}

/// Renders a translucent glass section card with consistent inner padding and subtle hairline border.
pub fn glass_card(
    ui: &mut Ui,
    palette: &GlassPalette,
    title: &str,
    add_contents: impl FnOnce(&mut Ui),
) {
    Frame::NONE
        .fill(palette.card_fill)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 12))
        .stroke(Stroke::new(1.0_f32, palette.card_stroke))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .size(12.5)
                    .strong()
                    .color(palette.secondary_text),
            );
            ui.add_space(6.0);
            add_contents(ui);
        });
}

/// Renders a full-width setting row with label, optional subtitle, and an animated pill toggle switch.
pub fn glass_toggle_row(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    title: &str,
    subtitle: Option<&str>,
    value: &mut bool,
) -> bool {
    let row_h = if subtitle.is_some() { 38.0 } else { 28.0 };
    let (row_rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);

    let mut changed = false;
    if resp.clicked() {
        *value = !*value;
        changed = true;
    }

    let id = ui.make_persistent_id(id_salt);
    let anim = ui.ctx().animate_bool(id, *value);

    let switch_size = Vec2::new(36.0, 20.0);
    let switch_rect = Rect::from_center_size(
        Pos2::new(row_rect.max.x - switch_size.x * 0.5, row_rect.center().y),
        switch_size,
    );
    let text_max_x = switch_rect.min.x - 10.0;

    let painter = ui.painter();
    if let Some(sub) = subtitle {
        painter.text(
            Pos2::new(row_rect.min.x, row_rect.center().y - 7.5),
            Align2::LEFT_CENTER,
            title,
            FontId::new(13.0, FontFamily::Proportional),
            palette.primary_text,
        );
        let sub_galley = painter.layout(
            sub.to_string(),
            FontId::new(11.0, FontFamily::Proportional),
            palette.muted_text,
            (text_max_x - row_rect.min.x).max(60.0),
        );
        painter.galley(
            Pos2::new(row_rect.min.x, row_rect.center().y + 2.5),
            sub_galley,
            palette.muted_text,
        );
    } else {
        painter.text(
            Pos2::new(row_rect.min.x, row_rect.center().y),
            Align2::LEFT_CENTER,
            title,
            FontId::new(13.0, FontFamily::Proportional),
            palette.primary_text,
        );
    }

    let off = palette.toggle_off_fill;
    let on = if resp.hovered() {
        palette.accent_hover_fill
    } else {
        palette.accent_fill
    };
    let track_color = lerp_color32(off, on, anim);

    painter.rect_filled(switch_rect, CornerRadius::same(10), track_color);
    if !*value && resp.hovered() {
        painter.rect_stroke(
            switch_rect,
            CornerRadius::same(10),
            Stroke::new(1.0_f32, palette.control_stroke),
            egui::StrokeKind::Inside,
        );
    }

    let knob_x = egui::lerp((switch_rect.min.x + 10.0)..=(switch_rect.max.x - 10.0), anim);
    let knob_center = Pos2::new(knob_x, switch_rect.center().y);
    painter.circle_filled(
        knob_center + Vec2::new(0.0, 1.0),
        7.0,
        Color32::from_black_alpha(38),
    );
    painter.circle_filled(knob_center, 7.0, palette.toggle_knob_fill);

    changed
}

/// Renders a segmented pill control inside a recessed glass track.
/// Passing `current: None` leaves all options unselected while still allowing clicks.
pub fn glass_segmented_control<T: Copy + PartialEq>(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    options: &[(T, &str)],
    current: Option<T>,
) -> Option<T> {
    if options.is_empty() {
        return None;
    }

    let (track_rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(track_rect, CornerRadius::same(8), palette.segment_track_fill);
    painter.rect_stroke(
        track_rect,
        CornerRadius::same(8),
        Stroke::new(1.0_f32, palette.divider),
        egui::StrokeKind::Inside,
    );

    let inner = track_rect.shrink(2.5);
    let gap = 2.5;
    let count = options.len() as f32;
    let seg_w = ((inner.width() - gap * (count - 1.0)) / count).max(24.0);

    let mut picked = None;
    let base_id = ui.make_persistent_id(id_salt);
    for (idx, (val, label)) in options.iter().enumerate() {
        let min_x = inner.min.x + idx as f32 * (seg_w + gap);
        let seg_rect =
            Rect::from_min_size(Pos2::new(min_x, inner.min.y), Vec2::new(seg_w, inner.height()));
        let resp = ui
            .interact(seg_rect, base_id.with(idx), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        let is_selected = current == Some(*val);
        let is_hovered = resp.hovered();

        if is_selected {
            ui.painter()
                .rect_filled(seg_rect, CornerRadius::same(6), palette.accent_fill);
        } else if is_hovered {
            ui.painter()
                .rect_filled(seg_rect, CornerRadius::same(6), palette.control_hover_fill);
        }

        let text_color = if is_selected {
            palette.accent_text_on_fill
        } else if is_hovered {
            palette.primary_text
        } else {
            palette.secondary_text
        };

        ui.painter().text(
            seg_rect.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::new(12.0, FontFamily::Proportional),
            text_color,
        );

        if resp.clicked() && current != Some(*val) {
            picked = Some(*val);
        }
    }
    picked
}

/// Renders a stacked slider row with label on the left, a compact value pill on the right,
/// and a full-width slider bar underneath.
pub fn glass_slider_row(
    ui: &mut Ui,
    palette: &GlassPalette,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f64,
    suffix: &str,
) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(label)
                    .size(12.5)
                    .color(palette.primary_text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                Frame::NONE
                    .fill(palette.control_fill)
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(7, 2))
                    .stroke(Stroke::new(1.0_f32, palette.card_stroke))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{:.0}{suffix}", *value))
                                .size(11.5)
                                .color(palette.secondary_text),
                        );
                    });
            });
        });

        ui.scope(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 6.0).max(120.0);
            if ui
                .add(
                    egui::Slider::new(value, range)
                        .step_by(step)
                        .show_value(false),
                )
                .changed()
            {
                changed = true;
            }
        });
    });
    changed
}

pub fn glass_button(ui: &mut Ui, palette: &GlassPalette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(palette.primary_text))
            .fill(palette.control_fill)
            .stroke(Stroke::new(1.0_f32, palette.control_stroke))
            .corner_radius(CornerRadius::same(8)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn glass_primary_button(ui: &mut Ui, palette: &GlassPalette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(palette.accent_text_on_fill),
        )
        .fill(palette.accent_fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn glass_danger_button(ui: &mut Ui, palette: &GlassPalette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(palette.item_hover_text),
        )
        .fill(palette.danger_hover_fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(8)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

/// Renders a translucent status pill with a 6px status indicator dot.
pub fn draw_status_pill(
    ui: &mut Ui,
    palette: &GlassPalette,
    dot_color: Color32,
    text: &str,
) {
    Frame::NONE
        .fill(palette.control_fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .stroke(Stroke::new(1.0_f32, palette.card_stroke))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter()
                    .circle_filled(dot_rect.center(), 3.2, dot_color);
                ui.label(
                    RichText::new(text)
                        .size(11.5)
                        .color(palette.primary_text),
                );
            });
        });
}

/// Renders a full-width hoverable glass list row with a primary label and optional right-aligned metadata.
pub fn glass_list_row(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    label: &str,
    secondary_right: Option<&str>,
) -> bool {
    ui.push_id(id_salt, |ui| {
        let (row_rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        let hovered = resp.hovered();

        let painter = ui.painter();
        painter.rect_filled(
            row_rect,
            CornerRadius::same(8),
            if hovered {
                palette.control_hover_fill
            } else {
                palette.control_fill
            },
        );
        painter.rect_stroke(
            row_rect,
            CornerRadius::same(8),
            Stroke::new(
                1.0_f32,
                if hovered {
                    palette.accent_fill
                } else {
                    palette.card_stroke
                },
            ),
            egui::StrokeKind::Inside,
        );

        let right_galley = secondary_right.map(|meta| {
            painter.layout_no_wrap(
                meta.to_string(),
                FontId::new(11.0, FontFamily::Proportional),
                palette.muted_text,
            )
        });

        let max_label_w = if let Some(ref rg) = right_galley {
            (row_rect.width() - rg.size().x - 28.0).max(60.0)
        } else {
            (row_rect.width() - 20.0).max(60.0)
        };

        let label_galley = painter.layout(
            label.to_string(),
            FontId::new(12.0, FontFamily::Proportional),
            palette.primary_text,
            max_label_w,
        );

        painter.galley(
            Pos2::new(
                row_rect.min.x + 10.0,
                row_rect.center().y - label_galley.size().y * 0.5,
            ),
            label_galley,
            palette.primary_text,
        );

        if let Some(rg) = right_galley {
            painter.galley(
                Pos2::new(
                    row_rect.max.x - 10.0 - rg.size().x,
                    row_rect.center().y - rg.size().y * 0.5,
                ),
                rg,
                palette.muted_text,
            );
        }

        resp.clicked()
    })
    .inner
}

/// Action returned by [`glass_keybind_row`] in a single frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindRowAction {
    None,
    Apply(String),
    ToggleRecord,
    Reset,
}

/// Smoothly animates a hover-revealed toolbar or overlay opacity (`0.0..=1.0`).
pub fn hover_chrome_opacity(ctx: &Context, id: Id, visible: bool) -> f32 {
    ctx.animate_bool_with_time(id, visible, 0.14)
}

/// Renders a backgroundless icon button that reveals a darker rounded fill only on hover/press,
/// with opacity scaled by `alpha` (e.g. for hover chrome).
pub fn glass_icon_button(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    size: f32,
    active: bool,
    alpha: f32,
    tooltip: &str,
    draw_icon: impl FnOnce(&Painter, Pos2, Color32),
) -> egui::Response {
    let alpha = alpha.clamp(0.0, 1.0);
    let resp = ui
        .push_id(id_salt, |ui| {
            let sense = if alpha > 0.04 {
                Sense::click()
            } else {
                Sense::hover()
            };
            let (rect, mut resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
            if alpha > 0.04 {
                resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            }

            if alpha > 0.01 {
                let hovered = alpha > 0.04 && resp.hovered();
                let pressed = hovered && resp.is_pointer_button_down_on();
                let painter = ui.painter();

                if pressed {
                    painter.rect_filled(
                        rect,
                        CornerRadius::same(8),
                        palette.icon_button_pressed_fill.gamma_multiply(alpha),
                    );
                } else if hovered {
                    painter.rect_filled(
                        rect,
                        CornerRadius::same(8),
                        palette.icon_button_hover_fill.gamma_multiply(alpha),
                    );
                }

                let icon_color = if active {
                    if hovered {
                        palette.accent_hover_fill
                    } else {
                        palette.accent_fill
                    }
                } else if hovered {
                    palette.primary_text
                } else {
                    palette.secondary_text
                }
                .gamma_multiply(alpha);

                draw_icon(painter, rect.center(), icon_color);
            }
            resp
        })
        .inner;

    if alpha > 0.5 && !tooltip.is_empty() {
        resp.on_hover_text(tooltip)
    } else {
        resp
    }
}

/// Computes the slice of pagination indices to display when `count` exceeds `max_visible`.
pub fn visible_dot_window(count: usize, active_index: usize, max_visible: usize) -> Range<usize> {
    if count <= max_visible || max_visible == 0 {
        return 0..count;
    }
    let clamped_active = active_index.min(count.saturating_sub(1));
    let half = max_visible / 2;
    let start = clamped_active
        .saturating_sub(half)
        .min(count - max_visible);
    start..(start + max_visible)
}

/// Renders horizontal pill/dot pagination indicators. Returns `Some(index)` when a dot is clicked.
pub fn glass_pagination_dots(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    count: usize,
    active_index: usize,
    alpha: f32,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let alpha = alpha.clamp(0.0, 1.0);
    let range = visible_dot_window(count, active_index, 11);
    let mut clicked = None;
    ui.push_id(id_salt, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let sense = if alpha > 0.04 {
                Sense::click()
            } else {
                Sense::hover()
            };
            for idx in range {
                let is_active = idx == active_index;
                let dot_w = if is_active { 16.0 } else { 6.0 };
                let hit_size = Vec2::new(dot_w + 4.0, 14.0);
                let (hit_rect, mut resp) = ui.allocate_exact_size(hit_size, sense);
                if alpha > 0.04 {
                    resp = resp.on_hover_cursor(CursorIcon::PointingHand);
                }
                if alpha > 0.01 {
                    let visual_rect =
                        Rect::from_center_size(hit_rect.center(), Vec2::new(dot_w, 6.0));
                    let color = if is_active {
                        palette.accent_fill
                    } else if alpha > 0.04 && resp.hovered() {
                        palette.primary_text
                    } else {
                        palette.muted_text.gamma_multiply(0.75)
                    }
                    .gamma_multiply(alpha);
                    ui.painter()
                        .rect_filled(visual_rect, CornerRadius::same(3), color);
                }
                if alpha > 0.04 && resp.clicked() {
                    clicked = Some(idx);
                }
            }
        });
    });

    clicked
}

/// Renders a clickable glass card for zoomed-out grid overviews, clipping contents to its inner rect.
pub fn glass_grid_card(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    size: Vec2,
    selected: bool,
    add_contents: impl FnOnce(&mut Ui, Rect, bool),
) -> egui::Response {
    let id = ui.make_persistent_id(id_salt);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let hovered = resp.hovered();

    let fill = if selected {
        palette.control_hover_fill
    } else if hovered {
        palette.control_hover_fill
    } else {
        palette.card_fill
    };
    let stroke = if selected {
        Stroke::new(1.5_f32, palette.accent_fill)
    } else if hovered {
        Stroke::new(1.2_f32, palette.accent_fill.gamma_multiply(0.72))
    } else {
        Stroke::new(1.0_f32, palette.card_stroke)
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(11), fill);
    painter.rect_stroke(rect, CornerRadius::same(11), stroke, egui::StrokeKind::Inside);

    let inner = rect.shrink2(Vec2::new(12.0, 10.0));
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id.with("body"))
            .max_rect(inner),
    );
    child_ui.set_clip_rect(inner);
    add_contents(&mut child_ui, inner, hovered);

    resp
}

/// Renders a reusable shortcut-editing row with a text field, Apply button, Record toggle, and Reset button.
pub fn glass_keybind_row(
    ui: &mut Ui,
    id_salt: impl Hash,
    palette: &GlassPalette,
    label: &str,
    subtitle: Option<&str>,
    buf: &mut String,
    is_recording: bool,
    default_val: &str,
) -> KeybindRowAction {
    let mut action = KeybindRowAction::None;
    ui.push_id(id_salt, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new(label)
                            .size(12.5)
                            .color(palette.primary_text),
                    );
                    if let Some(sub) = subtitle {
                        ui.label(RichText::new(sub).size(10.5).color(palette.muted_text));
                    }
                });
            });

            ui.horizontal(|ui| {
                let edit_w = (ui.available_width() - 186.0).max(96.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(buf)
                        .desired_width(edit_w)
                        .hint_text(default_val),
                );
                let enter_apply = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if enter_apply || glass_button(ui, palette, "Apply").clicked() {
                    action = KeybindRowAction::Apply(buf.trim().to_string());
                }

                let rec_clicked = if is_recording {
                    glass_danger_button(ui, palette, "Press... (Esc)").clicked()
                } else {
                    glass_button(ui, palette, "Record").clicked()
                };
                if rec_clicked {
                    action = KeybindRowAction::ToggleRecord;
                }

                let is_default = buf.trim().eq_ignore_ascii_case(default_val);
                if !is_default && glass_button(ui, palette, "Reset").clicked() {
                    action = KeybindRowAction::Reset;
                }
            });
        });
    });
    action
}

/// Renders a floating translucent toast pill centered on `anchor_center` scaled by `alpha`.
pub fn glass_toast_pill(
    painter: &Painter,
    anchor_center: Pos2,
    palette: &GlassPalette,
    text: &str,
    alpha: f32,
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.01 || text.is_empty() {
        return;
    }
    let font = FontId::new(11.5, FontFamily::Proportional);
    let galley = painter.layout_no_wrap(text.to_string(), font, palette.primary_text);
    let size = galley.size() + Vec2::new(22.0, 12.0);
    let rect = Rect::from_center_size(anchor_center, size);
    let bg = if palette.is_light {
        Color32::from_rgba_unmultiplied(245, 248, 255, 232)
    } else {
        Color32::from_rgba_unmultiplied(22, 26, 36, 232)
    }
    .gamma_multiply(alpha);

    painter.rect_filled(rect, CornerRadius::same(12), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0_f32, palette.control_stroke.gamma_multiply(alpha)),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        palette.primary_text.gamma_multiply(alpha),
    );
}

fn lerp_color32(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp_u8 = |c0: u8, c1: u8| -> u8 {
        ((c0 as f32) + ((c1 as f32) - (c0 as f32)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        lerp_u8(a.r(), b.r()),
        lerp_u8(a.g(), b.g()),
        lerp_u8(a.b(), b.b()),
        lerp_u8(a.a(), b.a()),
    )
}

#[cfg(test)]
mod tests {
    use super::visible_dot_window;

    #[test]
    fn test_visible_dot_window() {
        assert_eq!(visible_dot_window(5, 2, 11), 0..5);
        assert_eq!(visible_dot_window(20, 0, 11), 0..11);
        assert_eq!(visible_dot_window(20, 10, 11), 5..16);
        assert_eq!(visible_dot_window(20, 19, 11), 9..20);
    }
}
