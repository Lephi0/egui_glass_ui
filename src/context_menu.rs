use crate::dwm::{
    clamp_popup_pos_to_monitor_px, configure_popup_window, find_own_window_by_title,
    get_cursor_pos_px, get_window_pos_px, hide_popup_window, position_popup_window_px,
    trim_process_working_set,
};
use crate::style::{
    draw_glass_surface_with_alpha, BackgroundMode, GlassStyle, GlassWindowBackdrop,
};
use egui::{
    Align2, Color32, Context, CornerRadius, FontFamily, FontId, Frame, Id, Key, PointerButton,
    Pos2, Rect, Sense, Stroke, Vec2, ViewportBuilder, ViewportClass, ViewportId, WindowLevel,
};
use std::hash::Hash;
use std::time::{Duration, Instant};

pub const DEFAULT_CONTEXT_MENU_WIDTH: f32 = 236.0;
pub const CONTEXT_MENU_FADE_IN_SECS: f32 = 0.15;
pub const CONTEXT_MENU_FADE_OUT_SECS: f32 = 0.12;

/// An entry in a native glass popup context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlassMenuEntry<A> {
    Item {
        action: A,
        label: String,
        checked: bool,
        shortcut: Option<String>,
        danger: bool,
    },
    Divider,
}

impl<A> GlassMenuEntry<A> {
    pub fn item(action: A, label: impl Into<String>) -> Self {
        Self::Item {
            action,
            label: label.into(),
            checked: false,
            shortcut: None,
            danger: false,
        }
    }

    pub fn toggle(action: A, label: impl Into<String>, checked: bool) -> Self {
        Self::Item {
            action,
            label: label.into(),
            checked,
            shortcut: None,
            danger: false,
        }
    }

    pub fn danger(action: A, label: impl Into<String>) -> Self {
        Self::Item {
            action,
            label: label.into(),
            checked: false,
            shortcut: None,
            danger: true,
        }
    }

    pub fn divider() -> Self {
        Self::Divider
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        if let Self::Item {
            checked: ref mut c, ..
        } = self
        {
            *c = checked;
        }
        self
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        if let Self::Item {
            shortcut: ref mut s,
            ..
        } = self
        {
            *s = Some(shortcut.into());
        }
        self
    }

    pub fn with_danger(mut self, danger: bool) -> Self {
        if let Self::Item {
            danger: ref mut d, ..
        } = self
        {
            *d = danger;
        }
        self
    }
}

/// Computes the exact logical height for a list of [`GlassMenuEntry`] items and dividers.
pub fn compute_menu_height<A>(entries: &[GlassMenuEntry<A>]) -> f32 {
    let inner_margin = 12.0; // 6.0 top + 6.0 bottom
    let row_h = 23.0;
    let item_gap = 2.0;
    let divider_h = 11.0;

    let mut content_h = 0.0_f32;
    for (idx, entry) in entries.iter().enumerate() {
        match entry {
            GlassMenuEntry::Divider => {
                content_h += divider_h;
            }
            GlassMenuEntry::Item { .. } => {
                let next_is_item = matches!(entries.get(idx + 1), Some(GlassMenuEntry::Item { .. }));
                content_h += row_h + if next_is_item { item_gap } else { 0.0 };
            }
        }
    }

    (inner_margin + content_h).max(35.0)
}

/// Computes the pure fade-in / fade-out alpha scale and completion flag for the right-click context menu.
pub fn context_menu_fade_alpha(is_closing: bool, elapsed_secs: f32, start_alpha: f32) -> (f32, bool) {
    if is_closing {
        let t = (elapsed_secs / CONTEXT_MENU_FADE_OUT_SECS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(2);
        let alpha = (start_alpha * (1.0 - eased)).clamp(0.0, 1.0);
        (alpha, t >= 1.0)
    } else {
        let t = (elapsed_secs / CONTEXT_MENU_FADE_IN_SECS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        let alpha = (start_alpha + (1.0 - start_alpha) * eased).clamp(0.0, 1.0);
        (alpha, t >= 1.0)
    }
}

/// Controller for a standalone Windows 11 Acrylic popup context menu viewport with smooth fade-in/out.
pub struct GlassContextMenu {
    viewport_id: ViewportId,
    window_title: String,
    width: f32,
    menu_size: Vec2,
    open: bool,
    closing: bool,
    revealed: bool,
    activated: bool,
    locked_pos_px: [i32; 2],
    locked_size_px: [i32; 2],
    locked_logical_pos: Pos2,
    embedded_pos: Pos2,
    hwnd: Option<isize>,
    glass_backdrop: GlassWindowBackdrop,
    needs_reposition: bool,
    had_focus: bool,
    opened_at: Option<Instant>,
    fade_started_at: Option<Instant>,
    fade_from_alpha: f32,
    current_alpha: f32,
}

impl GlassContextMenu {
    pub fn new(viewport_id_salt: impl Hash, window_title: impl Into<String>) -> Self {
        Self {
            viewport_id: ViewportId::from_hash_of(viewport_id_salt),
            window_title: window_title.into(),
            width: DEFAULT_CONTEXT_MENU_WIDTH,
            menu_size: Vec2::new(DEFAULT_CONTEXT_MENU_WIDTH, 100.0),
            open: false,
            closing: false,
            revealed: false,
            activated: false,
            locked_pos_px: [0, 0],
            locked_size_px: [0, 0],
            locked_logical_pos: Pos2::ZERO,
            embedded_pos: Pos2::ZERO,
            hwnd: None,
            glass_backdrop: GlassWindowBackdrop::new(),
            needs_reposition: false,
            had_focus: false,
            opened_at: None,
            fade_started_at: None,
            fade_from_alpha: 0.0,
            current_alpha: 0.0,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width.max(120.0);
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    pub fn is_animating(&self) -> bool {
        self.open
            && (!self.revealed || !self.activated || self.closing || self.current_alpha < 0.999)
    }

    pub fn open_for_entries<A>(
        &mut self,
        root_ctx: &Context,
        owner_hwnd: Option<isize>,
        click_pos: Pos2,
        entries: &[GlassMenuEntry<A>],
    ) {
        let height = compute_menu_height(entries);
        self.open_with_height(root_ctx, owner_hwnd, click_pos, height);
    }

    pub fn open_with_height(
        &mut self,
        root_ctx: &Context,
        owner_hwnd: Option<isize>,
        click_pos: Pos2,
        height: f32,
    ) {
        self.menu_size = Vec2::new(self.width, height);
        let ppp = root_ctx.pixels_per_point().max(1.0);
        let menu_w_px = (self.menu_size.x * ppp).round() as i32;
        let menu_h_px = (self.menu_size.y * ppp).round() as i32;
        let anchor_px = get_cursor_pos_px()
            .or_else(|| {
                get_window_pos_px(owner_hwnd).map(|[wx, wy]| {
                    [
                        wx + (click_pos.x * ppp).round() as i32,
                        wy + (click_pos.y * ppp).round() as i32,
                    ]
                })
            })
            .unwrap_or([240, 180]);
        let pos_px = clamp_popup_pos_to_monitor_px(anchor_px, menu_w_px, menu_h_px);
        let logical_pos = Pos2::new(pos_px[0] as f32 / ppp, pos_px[1] as f32 / ppp);

        self.locked_pos_px = pos_px;
        self.locked_size_px = [menu_w_px, menu_h_px];
        self.locked_logical_pos = logical_pos;
        self.embedded_pos = click_pos;

        if self.open {
            self.closing = false;
            self.needs_reposition = true;
            self.had_focus = false;
            self.opened_at = Some(Instant::now());
            self.fade_started_at = Some(Instant::now());
            self.fade_from_alpha = 0.0;
            self.current_alpha = 0.0;
        } else {
            self.open = true;
            self.closing = false;
            self.revealed = false;
            self.activated = false;
            self.needs_reposition = true;
            self.had_focus = false;
            self.opened_at = None;
            self.fade_started_at = None;
            self.fade_from_alpha = 0.0;
            self.current_alpha = 0.0;
        }
        root_ctx.request_repaint();
    }

    pub fn begin_close(&mut self, root_ctx: &Context) {
        if self.open && !self.closing {
            self.closing = true;
            self.fade_from_alpha = self.current_alpha;
            self.fade_started_at = Some(Instant::now());
            root_ctx.request_repaint();
        }
    }

    pub fn close_immediately(&mut self) {
        if self.open {
            hide_popup_window(self.hwnd);
            self.reset_runtime_state();
            trim_process_working_set();
        }
    }

    fn reset_runtime_state(&mut self) {
        self.open = false;
        self.closing = false;
        self.revealed = false;
        self.activated = false;
        self.locked_pos_px = [0, 0];
        self.locked_size_px = [0, 0];
        self.locked_logical_pos = Pos2::ZERO;
        self.embedded_pos = Pos2::ZERO;
        self.hwnd = None;
        self.glass_backdrop = GlassWindowBackdrop::new();
        self.needs_reposition = false;
        self.had_focus = false;
        self.opened_at = None;
        self.fade_started_at = None;
        self.fade_from_alpha = 0.0;
        self.current_alpha = 0.0;
    }

    /// Dismisses the context menu when Escape is pressed or the root surface is clicked while open.
    pub fn handle_root_dismiss(&mut self, root_ctx: &Context) {
        if self.open && !self.closing {
            let dismiss_from_root = root_ctx.input(|i| {
                i.key_pressed(Key::Escape) || i.pointer.button_pressed(PointerButton::Primary)
            });
            if dismiss_from_root {
                self.begin_close(root_ctx);
            }
        }
    }

    /// Renders the context menu viewport when open and returns `Some(action)` when an item is clicked.
    pub fn show<A: Copy>(
        &mut self,
        root_ctx: &Context,
        glass_style: &GlassStyle,
        embedded_bounds: Vec2,
        entries: &[GlassMenuEntry<A>],
    ) -> Option<A> {
        if !self.open {
            return None;
        }

        let menu_size = self.menu_size;
        let pos_px = self.locked_pos_px;
        let size_px = self.locked_size_px;
        let logical_pos = self.locked_logical_pos;

        let builder = ViewportBuilder::default()
            .with_title(self.window_title.as_str())
            .with_inner_size([menu_size.x, menu_size.y])
            .with_min_inner_size([menu_size.x, menu_size.y])
            .with_max_inner_size([menu_size.x, menu_size.y])
            .with_position(logical_pos)
            .with_visible(self.revealed)
            .with_decorations(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_close_button(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let mut picked_action = None;

        root_ctx.show_viewport_immediate(self.viewport_id, builder, |menu_ctx, class| {
            if class == ViewportClass::Embedded {
                if !self.revealed || !self.activated {
                    self.revealed = true;
                    self.activated = true;
                    self.fade_started_at = Some(Instant::now());
                    self.fade_from_alpha = 0.0;
                }

                let bounds_rect = Rect::from_min_size(Pos2::ZERO, embedded_bounds);
                let max_x = (bounds_rect.max.x - menu_size.x - 4.0).max(bounds_rect.min.x + 4.0);
                let max_y = (bounds_rect.max.y - menu_size.y - 4.0).max(bounds_rect.min.y + 4.0);
                let min_x = self.embedded_pos.x.clamp(bounds_rect.min.x + 4.0, max_x);
                let min_y = self.embedded_pos.y.clamp(bounds_rect.min.y + 4.0, max_y);
                let menu_rect = Rect::from_min_size(Pos2::new(min_x, min_y), menu_size);

                let elapsed_secs = self
                    .fade_started_at
                    .map_or(0.0, |t| t.elapsed().as_secs_f32());
                let (alpha, anim_done) =
                    context_menu_fade_alpha(self.closing, elapsed_secs, self.fade_from_alpha);
                self.current_alpha = alpha;

                if self.closing && anim_done {
                    self.close_immediately();
                    root_ctx.request_repaint();
                    return;
                }

                if !self.closing {
                    let close_outside = menu_ctx.input(|i| {
                        i.key_pressed(Key::Escape)
                            || (i.pointer.button_pressed(PointerButton::Primary)
                                && i.pointer
                                    .interact_pos()
                                    .map_or(false, |p| !menu_rect.contains(p)))
                    });
                    if close_outside {
                        self.begin_close(root_ctx);
                    }
                }

                if !anim_done || self.closing {
                    menu_ctx.request_repaint();
                    root_ctx.request_repaint();
                }

                egui::Area::new(Id::new("embedded_ctx_menu_area"))
                    .fixed_pos(menu_rect.min)
                    .order(egui::Order::Foreground)
                    .show(menu_ctx, |ui| {
                        let (rect, _) = ui.allocate_exact_size(menu_size, Sense::hover());
                        if let Some(action) = self.render_surface(ui, rect, glass_style, alpha, entries) {
                            self.begin_close(root_ctx);
                            picked_action = Some(action);
                            root_ctx.request_repaint();
                        }
                    });
                return;
            }

            // 1. Discover popup HWND owned by this process
            if self.hwnd.is_none() {
                self.hwnd = find_own_window_by_title(&self.window_title);
                if self.hwnd.is_none() {
                    root_ctx.request_repaint();
                    return;
                }
            }

            // 2. Frame 1 (window still hidden): disable DWM transitions, set tool-window style,
            // sync Acrylic backdrop, and place at exact physical coordinates before ever showing.
            if !self.revealed {
                configure_popup_window(self.hwnd);
                self.glass_backdrop.sync(
                    self.hwnd,
                    BackgroundMode::FrostedGlass,
                    glass_style.is_light(),
                );
                position_popup_window_px(
                    self.hwnd,
                    pos_px[0],
                    pos_px[1],
                    size_px[0],
                    size_px[1],
                    false,
                );

                egui::CentralPanel::default()
                    .frame(Frame::NONE.fill(Color32::TRANSPARENT))
                    .show(menu_ctx, |ui| {
                        let menu_rect = ui.max_rect();
                        let _ = self.render_surface(ui, menu_rect, glass_style, 0.0, entries);
                    });

                self.revealed = true;
                self.needs_reposition = false;
                self.current_alpha = 0.0;
                menu_ctx.request_repaint();
                root_ctx.request_repaint();
                return;
            }

            // 3. Frame 2+: keep DWM backdrop synced and reveal/reposition in place without SW_RESTORE
            self.glass_backdrop.sync(
                self.hwnd,
                BackgroundMode::FrostedGlass,
                glass_style.is_light(),
            );

            if !self.activated || self.needs_reposition {
                configure_popup_window(self.hwnd);
                position_popup_window_px(
                    self.hwnd,
                    pos_px[0],
                    pos_px[1],
                    size_px[0],
                    size_px[1],
                    true,
                );
                self.glass_backdrop.invalidate();
                self.glass_backdrop.sync(
                    self.hwnd,
                    BackgroundMode::FrostedGlass,
                    glass_style.is_light(),
                );
                if !self.activated {
                    self.activated = true;
                    self.opened_at = Some(Instant::now());
                    self.fade_started_at = Some(Instant::now());
                    self.fade_from_alpha = 0.0;
                }
                self.needs_reposition = false;
            }

            // 4. Advance pure fade-in / fade-out curve
            let elapsed_secs = self
                .fade_started_at
                .map_or(0.0, |t| t.elapsed().as_secs_f32());
            let (alpha, anim_done) =
                context_menu_fade_alpha(self.closing, elapsed_secs, self.fade_from_alpha);
            self.current_alpha = alpha;

            if self.closing && anim_done {
                self.close_immediately();
                root_ctx.request_repaint();
                return;
            }

            // 5. Track focus and begin fade-out on close request, Escape, or focus loss
            if !self.closing {
                let (close_req, esc_pressed, focused) = menu_ctx.input(|i| {
                    (
                        i.viewport().close_requested(),
                        i.key_pressed(Key::Escape),
                        i.viewport().focused,
                    )
                });
                if focused == Some(true) {
                    self.had_focus = true;
                }
                let grace_elapsed = self
                    .opened_at
                    .map_or(false, |t| t.elapsed() >= Duration::from_millis(250));
                let lost_focus = focused == Some(false) && (self.had_focus || grace_elapsed);

                if close_req || esc_pressed || lost_focus {
                    self.begin_close(root_ctx);
                }
            }

            if !anim_done || self.closing {
                menu_ctx.request_repaint();
                root_ctx.request_repaint();
            }

            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(Color32::TRANSPARENT))
                .show(menu_ctx, |ui| {
                    let menu_rect = ui.max_rect();
                    if let Some(action) = self.render_surface(ui, menu_rect, glass_style, alpha, entries) {
                        self.begin_close(root_ctx);
                        picked_action = Some(action);
                        root_ctx.request_repaint();
                    }
                });
        });

        picked_action
    }

    fn render_surface<A: Copy>(
        &self,
        ui: &mut egui::Ui,
        menu_rect: Rect,
        glass_style: &GlassStyle,
        fade_alpha: f32,
        entries: &[GlassMenuEntry<A>],
    ) -> Option<A> {
        let fade_alpha = fade_alpha.clamp(0.0, 1.0);
        let palette = glass_style.palette();
        let painter = ui.painter();

        draw_glass_surface_with_alpha(painter, menu_rect, glass_style, fade_alpha);

        let inner = menu_rect.shrink(6.0);
        let row_h = 23.0;
        let interactive = !self.closing;

        let mut picked = None;
        let mut cur_y = inner.min.y;
        let mut item_idx = 0usize;

        for (idx, entry) in entries.iter().enumerate() {
            match entry {
                GlassMenuEntry::Divider => {
                    let div_y = cur_y + 5.5;
                    painter.line_segment(
                        [
                            Pos2::new(inner.min.x + 8.0, div_y),
                            Pos2::new(inner.max.x - 8.0, div_y),
                        ],
                        Stroke::new(1.0_f32, palette.divider.gamma_multiply(fade_alpha)),
                    );
                    cur_y += 11.0;
                }
                GlassMenuEntry::Item {
                    action,
                    label,
                    checked,
                    shortcut,
                    danger,
                } => {
                    let item_rect = Rect::from_min_size(
                        Pos2::new(inner.min.x, cur_y),
                        Vec2::new(inner.width(), row_h),
                    );
                    let next_is_item =
                        matches!(entries.get(idx + 1), Some(GlassMenuEntry::Item { .. }));
                    cur_y += row_h + if next_is_item { 2.0 } else { 0.0 };

                    let resp = ui.interact(
                        item_rect,
                        Id::new((self.viewport_id, "ctx_menu_row", item_idx)),
                        Sense::click(),
                    );
                    item_idx += 1;

                    let is_hovered = interactive && resp.hovered();

                    if is_hovered {
                        let hover_fill = if *danger {
                            palette.danger_hover_fill
                        } else {
                            palette.item_hover_fill
                        };
                        painter.rect_filled(
                            item_rect,
                            CornerRadius::same(7),
                            hover_fill.gamma_multiply(fade_alpha),
                        );
                    }

                    let text_color = if is_hovered {
                        palette.item_hover_text
                    } else if *danger {
                        palette.danger_text
                    } else {
                        palette.primary_text
                    }
                    .gamma_multiply(fade_alpha);

                    if *checked {
                        let cx = item_rect.min.x + 13.0;
                        let cy = item_rect.center().y;
                        let stroke = Stroke::new(1.6_f32, text_color);
                        painter.line_segment(
                            [Pos2::new(cx - 4.0, cy + 0.2), Pos2::new(cx - 1.2, cy + 3.0)],
                            stroke,
                        );
                        painter.line_segment(
                            [Pos2::new(cx - 1.2, cy + 3.0), Pos2::new(cx + 4.2, cy - 3.0)],
                            stroke,
                        );
                    }

                    painter.text(
                        Pos2::new(item_rect.min.x + 26.0, item_rect.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        FontId::new(12.5, FontFamily::Proportional),
                        text_color,
                    );

                    if let Some(badge) = shortcut {
                        let badge_color = if is_hovered {
                            palette.item_hover_secondary_text
                        } else {
                            palette.secondary_text
                        }
                        .gamma_multiply(fade_alpha);
                        painter.text(
                            Pos2::new(item_rect.max.x - 8.0, item_rect.center().y),
                            Align2::RIGHT_CENTER,
                            badge,
                            FontId::new(10.5, FontFamily::Proportional),
                            badge_color,
                        );
                    }

                    if interactive && resp.clicked() {
                        picked = Some(*action);
                    }
                }
            }
        }

        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_fade_alpha() {
        let (start_in, done_start) = context_menu_fade_alpha(false, 0.0, 0.0);
        assert!((start_in - 0.0).abs() < 1e-6);
        assert!(!done_start);

        let (mid_in, done_mid) =
            context_menu_fade_alpha(false, CONTEXT_MENU_FADE_IN_SECS * 0.5, 0.0);
        assert!(mid_in > 0.5 && mid_in < 1.0);
        assert!(!done_mid);

        let (end_in, done_in) = context_menu_fade_alpha(false, CONTEXT_MENU_FADE_IN_SECS, 0.0);
        assert!((end_in - 1.0).abs() < 1e-6);
        assert!(done_in);

        let (start_out, done_out_start) = context_menu_fade_alpha(true, 0.0, 1.0);
        assert!((start_out - 1.0).abs() < 1e-6);
        assert!(!done_out_start);

        let (end_out, done_out) = context_menu_fade_alpha(true, CONTEXT_MENU_FADE_OUT_SECS, 1.0);
        assert!((end_out - 0.0).abs() < 1e-6);
        assert!(done_out);

        // Mid-fade reversal from partial alpha (0.6 -> 0.0)
        let (rev_start, rev_done_start) = context_menu_fade_alpha(true, 0.0, 0.6);
        assert!((rev_start - 0.6).abs() < 1e-6);
        assert!(!rev_done_start);

        let (rev_mid, _) = context_menu_fade_alpha(true, CONTEXT_MENU_FADE_OUT_SECS * 0.5, 0.6);
        assert!(rev_mid > 0.0 && rev_mid < 0.6);

        let (rev_end, rev_done_end) =
            context_menu_fade_alpha(true, CONTEXT_MENU_FADE_OUT_SECS, 0.6);
        assert!((rev_end - 0.0).abs() < 1e-6);
        assert!(rev_done_end);
    }

    #[test]
    fn test_compute_menu_height() {
        let entries = vec![
            GlassMenuEntry::toggle(0, "Compact Mode", false),
            GlassMenuEntry::toggle(1, "Transparent Mode", false),
            GlassMenuEntry::toggle(2, "Show Seconds", false),
            GlassMenuEntry::toggle(3, "Always on Top", false).with_shortcut("Alt+Shift+T"),
            GlassMenuEntry::divider(),
            GlassMenuEntry::item(4, "Options..."),
            GlassMenuEntry::divider(),
            GlassMenuEntry::danger(5, "Close"),
        ];

        assert_eq!(compute_menu_height(&entries), 178.0);
    }
}
