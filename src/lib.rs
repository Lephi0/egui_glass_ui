pub mod context_menu;
pub mod dwm;
pub mod style;
pub mod widgets;
pub mod window;

pub use context_menu::{
    compute_menu_height, context_menu_fade_alpha, GlassContextMenu, GlassMenuEntry,
    CONTEXT_MENU_FADE_IN_SECS, CONTEXT_MENU_FADE_OUT_SECS, DEFAULT_CONTEXT_MENU_WIDTH,
};
pub use dwm::{
    bring_hwnd_to_front, clamp_popup_pos_to_monitor_px, clamp_rect_to_work_area,
    configure_frameless_window, configure_popup_window, configure_settings_window, extract_hwnd,
    find_own_window_by_title, get_cursor_pos_px, get_monitor_work_area_for_hwnd,
    get_window_pos_px, hide_popup_window, is_pos_on_any_monitor, is_windows_light_theme,
    needs_caption_suppression, position_popup_window_px, primary_monitor_center_pos_px,
    resize_window_px, resolve_window_pos_in_work_area, set_window_pos_px_checked,
    sync_window_blur, trim_process_working_set,
};
pub use style::{
    apply_glass_visuals, draw_glass_background, draw_glass_surface,
    draw_glass_surface_with_alpha, BackgroundMode, GlassPalette, GlassStyle, GlassTintPreset,
    GlassWindowBackdrop, DEFAULT_GLASS_CORNER_RADIUS,
};
pub use widgets::{
    draw_glass_tab_bar, draw_glass_tab_bar_with_icons, draw_glass_titlebar, draw_shadowed_text,
    draw_status_pill, glass_button, glass_card, glass_danger_button, glass_list_row,
    glass_primary_button, glass_segmented_control, glass_slider_row, glass_toggle_row,
    install_system_font, GLASS_TITLEBAR_HEIGHT,
};
pub use window::{GlassWindow, GlassWindowConfig, GlassWindowResponse};
