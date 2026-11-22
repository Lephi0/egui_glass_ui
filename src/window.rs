use crate::dwm::{
    bring_hwnd_to_front, configure_frameless_window, configure_tray_only_window,
    find_own_window_by_title, get_monitor_work_area_for_hwnd, get_window_pos_px,
    hide_popup_window, position_popup_window_px, resolve_window_pos_in_work_area,
    trim_process_working_set,
};
use crate::style::{
    apply_glass_visuals, draw_glass_surface, BackgroundMode, GlassPalette, GlassStyle,
    GlassWindowBackdrop,
};
use egui::{
    Color32, Context, Frame, IconData, Key, Margin, Painter, Pos2, Rect, Vec2, ViewportBuilder,
    ViewportClass, ViewportId, WindowLevel,
};
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Configuration for a secondary frameless Acrylic glass window viewport.
#[derive(Clone)]
pub struct GlassWindowConfig {
    pub viewport_id: ViewportId,
    pub title: String,
    pub default_size: Vec2,
    pub min_height: f32,
    pub always_on_top: bool,
    pub taskbar: bool,
    pub icon: Option<IconData>,
}

impl GlassWindowConfig {
    pub fn new(
        viewport_id_salt: impl Hash,
        title: impl Into<String>,
        default_size: Vec2,
    ) -> Self {
        Self {
            viewport_id: ViewportId::from_hash_of(viewport_id_salt),
            title: title.into(),
            default_size,
            min_height: 400.0,
            always_on_top: true,
            taskbar: true,
            icon: None,
        }
    }

    pub fn with_min_height(mut self, min_height: f32) -> Self {
        self.min_height = min_height.max(120.0);
        self
    }

    pub fn with_always_on_top(mut self, always_on_top: bool) -> Self {
        self.always_on_top = always_on_top;
        self
    }

    pub fn with_taskbar(mut self, taskbar: bool) -> Self {
        self.taskbar = taskbar;
        self
    }

    pub fn with_icon(mut self, icon: IconData) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Lifecycle response emitted each frame by [`GlassWindow`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlassWindowResponse {
    pub closed: bool,
    pub persisted_pos_px: Option<[i32; 2]>,
}

/// Controller for a reusable frameless Windows 11 Acrylic secondary viewport (e.g. Settings / Options).
pub struct GlassWindow {
    config: GlassWindowConfig,
    open: bool,
    revealed: bool,
    activated: bool,
    logical_size: Vec2,
    locked_pos_px: [i32; 2],
    locked_size_px: [i32; 2],
    locked_logical_pos: Pos2,
    hwnd: Option<isize>,
    glass_backdrop: GlassWindowBackdrop,
    last_observed_pos_px: Option<[i32; 2]>,
    pos_dirty_since: Option<Instant>,
}

impl GlassWindow {
    pub fn new(config: GlassWindowConfig) -> Self {
        let default_size = config.default_size;
        Self {
            config,
            open: false,
            revealed: false,
            activated: false,
            logical_size: default_size,
            locked_pos_px: [0, 0],
            locked_size_px: [0, 0],
            locked_logical_pos: Pos2::ZERO,
            hwnd: None,
            glass_backdrop: GlassWindowBackdrop::new(),
            last_observed_pos_px: None,
            pos_dirty_since: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_activating(&self) -> bool {
        self.open && !self.activated
    }

    pub fn has_pending_pos_save(&self) -> bool {
        self.pos_dirty_since.is_some()
    }

    pub fn hwnd(&self) -> Option<isize> {
        self.hwnd
    }

    pub fn viewport_id(&self) -> ViewportId {
        self.config.viewport_id
    }

    pub fn request_repaint(&self, root_ctx: &Context) {
        if self.open {
            root_ctx.request_repaint_of(self.config.viewport_id);
        }
    }

    pub fn current_pos_px(&self) -> Option<[i32; 2]> {
        if self.open && self.activated {
            get_window_pos_px(self.hwnd).or(self.last_observed_pos_px)
        } else {
            None
        }
    }

    pub fn open(
        &mut self,
        root_ctx: &Context,
        owner_hwnd: Option<isize>,
        owner_pos_px: Option<[i32; 2]>,
        saved_pos_px: Option<[i32; 2]>,
    ) {
        if self.open {
            bring_hwnd_to_front(self.hwnd);
            return;
        }

        let work_rect = get_monitor_work_area_for_hwnd(owner_hwnd, owner_pos_px);
        let ppp = root_ctx.pixels_per_point().max(1.0);
        let work_h_logical =
            ((work_rect[3] - work_rect[1]) as f32 / ppp).max(self.config.min_height);
        let logical_h = self
            .config
            .default_size
            .y
            .min((work_h_logical - 32.0).max(self.config.min_height))
            .round();
        let logical_size = Vec2::new(self.config.default_size.x, logical_h);
        let size_px = [
            (logical_size.x * ppp).round() as i32,
            (logical_size.y * ppp).round() as i32,
        ];
        let pos_px = resolve_window_pos_in_work_area(saved_pos_px, size_px, work_rect);
        let logical_pos = Pos2::new(pos_px[0] as f32 / ppp, pos_px[1] as f32 / ppp);

        self.reset_runtime_state();
        self.open = true;
        self.logical_size = logical_size;
        self.locked_pos_px = pos_px;
        self.locked_size_px = size_px;
        self.locked_logical_pos = logical_pos;
        self.last_observed_pos_px = Some(pos_px);
        root_ctx.request_repaint();
    }

    pub fn hide(&self) {
        hide_popup_window(self.hwnd);
    }

    pub fn close(&mut self) -> Option<[i32; 2]> {
        if !self.open {
            return None;
        }
        let pos = self.current_pos_px();
        hide_popup_window(self.hwnd);
        self.reset_runtime_state();
        trim_process_working_set();
        pos
    }

    fn reset_runtime_state(&mut self) {
        self.open = false;
        self.revealed = false;
        self.activated = false;
        self.logical_size = self.config.default_size;
        self.locked_pos_px = [0, 0];
        self.locked_size_px = [0, 0];
        self.locked_logical_pos = Pos2::ZERO;
        self.hwnd = None;
        self.glass_backdrop = GlassWindowBackdrop::new();
        self.last_observed_pos_px = None;
        self.pos_dirty_since = None;
    }

    /// Shows the window with custom interior frame rendering.
    /// `render_frame` returns `true` when the user asks to close the window (e.g. clicking Close).
    pub fn show_raw(
        &mut self,
        root_ctx: &Context,
        glass_style: &GlassStyle,
        allow_esc_close: bool,
        mut render_frame: impl FnMut(&Context, &mut egui::Ui, &GlassStyle, &GlassPalette) -> bool,
    ) -> GlassWindowResponse {
        let mut response = GlassWindowResponse::default();
        if !self.open {
            return response;
        }

        let logical_size = self.logical_size;
        let pos_px = self.locked_pos_px;
        let size_px = self.locked_size_px;

        let mut builder = ViewportBuilder::default()
            .with_title(self.config.title.as_str())
            .with_inner_size([logical_size.x, logical_size.y])
            .with_min_inner_size([logical_size.x, logical_size.y])
            .with_max_inner_size([logical_size.x, logical_size.y])
            .with_visible(self.revealed)
            .with_decorations(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_close_button(false)
            .with_transparent(true)
            .with_taskbar(self.config.taskbar)
            .with_resizable(false)
            .with_window_level(if self.config.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });

        if let Some(ref icon) = self.config.icon {
            builder = builder.with_icon(icon.clone());
        }
        if !self.activated {
            builder = builder.with_position(self.locked_logical_pos);
        }

        root_ctx.show_viewport_immediate(self.config.viewport_id, builder, |win_ctx, class| {
            let palette = glass_style.palette();
            apply_glass_visuals(win_ctx, glass_style);

            if class == ViewportClass::Embedded {
                let mut open = self.open;
                let mut done_clicked = false;
                egui::Window::new(self.config.title.as_str())
                    .open(&mut open)
                    .show(win_ctx, |ui| {
                        done_clicked = render_frame(win_ctx, ui, glass_style, &palette);
                    });
                if !open || done_clicked {
                    response.persisted_pos_px = self.close();
                    response.closed = true;
                    root_ctx.request_repaint();
                }
                return;
            }

            if self.hwnd.is_none() {
                self.hwnd = find_own_window_by_title(&self.config.title);
                if self.hwnd.is_none() {
                    root_ctx.request_repaint();
                    return;
                }
            }

            // Frame 1 (hidden): suppress transitions, apply Acrylic glass backdrop, and place on target monitor.
            if !self.revealed {
                if self.config.taskbar {
                    configure_frameless_window(self.hwnd);
                } else {
                    configure_tray_only_window(self.hwnd);
                }
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
                    .show(win_ctx, |ui| {
                        let _ = render_frame(win_ctx, ui, glass_style, &palette);
                    });

                self.revealed = true;
                win_ctx.request_repaint();
                root_ctx.request_repaint();
                return;
            }

            // Frame 2: reveal and focus cleanly without DWM transition flicker.
            if !self.activated {
                if self.config.taskbar {
                    configure_frameless_window(self.hwnd);
                } else {
                    configure_tray_only_window(self.hwnd);
                }
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
                self.activated = true;

                let actual = get_window_pos_px(self.hwnd).unwrap_or(pos_px);
                self.last_observed_pos_px = Some(actual);
                response.persisted_pos_px = Some(actual);
            } else {
                self.glass_backdrop.sync(
                    self.hwnd,
                    BackgroundMode::FrostedGlass,
                    glass_style.is_light(),
                );
            }

            let (close_req, esc_pressed) = win_ctx.input(|i| {
                (
                    i.viewport().close_requested(),
                    i.key_pressed(Key::Escape),
                )
            });
            let mut should_close = close_req
                || (allow_esc_close && esc_pressed && !win_ctx.wants_keyboard_input());

            // Track physical drag moves via Win32 GetWindowRect and debounce persistence
            if let Some(cur_px) = get_window_pos_px(self.hwnd) {
                let changed = match self.last_observed_pos_px {
                    Some(prev) => (prev[0] - cur_px[0]).abs() > 1 || (prev[1] - cur_px[1]).abs() > 1,
                    None => {
                        self.last_observed_pos_px = Some(cur_px);
                        false
                    }
                };
                if changed {
                    self.last_observed_pos_px = Some(cur_px);
                    self.pos_dirty_since = Some(Instant::now());
                }
            }
            if let Some(dirty_time) = self.pos_dirty_since {
                let is_dragging = win_ctx.input(|i| i.pointer.primary_down());
                if !is_dragging && dirty_time.elapsed() >= Duration::from_millis(350) {
                    if let Some(cur_px) = self.last_observed_pos_px {
                        response.persisted_pos_px = Some(cur_px);
                    }
                    self.pos_dirty_since = None;
                }
            }

            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(Color32::TRANSPARENT))
                .show(win_ctx, |ui| {
                    let frame_close = render_frame(win_ctx, ui, glass_style, &palette);
                    should_close |= frame_close;
                });

            if should_close {
                if let Some(pos) = self.close() {
                    response.persisted_pos_px = Some(pos);
                }
                response.closed = true;
                root_ctx.request_repaint();
            }
        });

        response
    }

    /// Renders a standard glass-framed window with titlebar, optional custom header region (e.g. tabs),
    /// and a padded vertical `ScrollArea` for the body contents.
    pub fn show_framed(
        &mut self,
        root_ctx: &Context,
        glass_style: &GlassStyle,
        allow_esc_close: bool,
        draw_badge: Option<&dyn Fn(&Painter, Rect)>,
        mut add_header: impl FnMut(&Context, &mut egui::Ui, &GlassPalette),
        mut add_scroll_contents: impl FnMut(&Context, &mut egui::Ui, &GlassPalette),
    ) -> GlassWindowResponse {
        let title = self.config.title.clone();
        self.show_raw(root_ctx, glass_style, allow_esc_close, |win_ctx, ui, style, palette| {
            let full_rect = ui.max_rect();
            draw_glass_surface(ui.painter(), full_rect, style);

            ui.spacing_mut().item_spacing.y = 0.0;

            let close_top =
                crate::widgets::draw_glass_titlebar(win_ctx, ui, palette, &title, draw_badge);
            ui.add_space(10.0);

            add_header(win_ctx, ui, palette);

            let scroll_h = ui.available_height().max(120.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), scroll_h), |ui| {
                ui.set_min_size(Vec2::new(ui.available_width(), scroll_h));
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 7.0;
                        Frame::NONE
                            .inner_margin(Margin {
                                left: 16,
                                right: 16,
                                top: 2,
                                bottom: 14,
                            })
                            .show(ui, |ui| {
                                add_scroll_contents(win_ctx, ui, palette);
                            });
                    });
            });

            close_top
        })
    }

    /// Convenience wrapper around [`GlassWindow::show_framed`] with no custom header section.
    pub fn show(
        &mut self,
        root_ctx: &Context,
        glass_style: &GlassStyle,
        allow_esc_close: bool,
        draw_badge: Option<&dyn Fn(&Painter, Rect)>,
        add_scroll_contents: impl FnMut(&Context, &mut egui::Ui, &GlassPalette),
    ) -> GlassWindowResponse {
        self.show_framed(
            root_ctx,
            glass_style,
            allow_esc_close,
            draw_badge,
            |_, _, _| {},
            add_scroll_contents,
        )
    }
}
