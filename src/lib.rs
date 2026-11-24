pub mod context_menu;
pub mod dwm;
pub mod fonts;
pub mod hotkey_hook;
pub mod style;
pub mod widgets;
pub mod window;

pub use context_menu::{
    compute_menu_height, context_menu_fade_alpha, GlassContextMenu, GlassMenuEntry,
    CONTEXT_MENU_FADE_IN_SECS, CONTEXT_MENU_FADE_OUT_SECS, DEFAULT_CONTEXT_MENU_WIDTH,
};
pub use dwm::{
    bring_hwnd_to_front, center_on_primary_work_area_px, clamp_popup_pos_to_monitor_px,
    clamp_rect_to_work_area, compute_overlay_rect_in_work_area, configure_frameless_window,
    configure_popup_window, configure_settings_window, configure_tray_only_window, extract_hwnd,
    find_own_window_by_title, get_cursor_monitor_work_area_px, get_cursor_pos_px,
    get_foreground_hwnd, get_monitor_work_area_for_hwnd, get_primary_monitor_work_area_px,
    get_window_pos_px, hide_popup_window, hide_window, is_pos_on_any_monitor, is_win_key_pressed,
    is_window_foreground, is_window_visible, is_windows_light_theme, needs_caption_suppression,
    position_popup_window_px, position_window_px_no_activate, primary_monitor_center_pos_px,
    resize_window_px, resolve_window_pos_in_work_area, set_window_always_on_top,
    set_window_cloaked, set_window_pos_px_checked, show_window_at_px, show_window_to_front,
    sync_window_blur,
    trim_process_working_set, wake_cloaked_window,
};
pub use fonts::{
    discover_installed_fonts, install_glass_fonts, install_system_font, SystemFontOption,
    DEFAULT_EDITOR_FONT, EDITOR_BOLD_FONT_FAMILY, EDITOR_BOLD_ITALIC_FONT_FAMILY,
    EDITOR_FONT_FAMILY, EDITOR_ITALIC_FONT_FAMILY,
};
pub use hotkey_hook::{
    is_side_aware_hook_active, read_modifier_side_state, unregister_raw_keyboard_input,
    ModifierRequirement, ModifierSideState, SideAwareHotkeyHook, SideAwareHotkeySpec,
};
pub use style::{
    apply_glass_visuals, draw_glass_background, draw_glass_surface,
    draw_glass_surface_with_alpha, BackgroundMode, GlassPalette, GlassStyle, GlassTintPreset,
    GlassWindowBackdrop, DEFAULT_GLASS_CORNER_RADIUS,
};
pub use widgets::{
    draw_glass_tab_bar, draw_glass_tab_bar_with_icons, draw_glass_titlebar, draw_shadowed_text,
    draw_status_pill, glass_button, glass_card, glass_danger_button, glass_grid_card,
    glass_icon_button, glass_keybind_row, glass_list_row, glass_pagination_dots,
    glass_primary_button, glass_segmented_control, glass_slider_row, glass_toast_pill,
    glass_toggle_row, hover_chrome_opacity, visible_dot_window, KeybindRowAction,
    GLASS_TITLEBAR_HEIGHT,
};
pub use window::{GlassWindow, GlassWindowConfig, GlassWindowResponse};
