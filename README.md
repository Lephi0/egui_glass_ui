# `egui_glass_ui`

Reusable Windows 11 DWM Acrylic glass view layer and translucent `egui` UI frontend kit extracted from `clock_widget`.

## Features

- **Windows 11 DWM Acrylic & Transparent Composition (`dwm`, `style::GlassWindowBackdrop`)**:
  - `extract_hwnd`, `is_windows_light_theme`, `sync_window_blur`, `GlassWindowBackdrop`
  - Caption-control suppression so native `—`, `□`, `✕` buttons do not bleed onto frameless glass windows
  - Multi-monitor position resolution & work-area clamping (`resolve_window_pos_in_work_area`, `clamp_popup_pos_to_monitor_px`, `set_window_pos_px_checked`)
- **Theme & Glass Surfaces (`style`)**:
  - `BackgroundMode` (`FrostedGlass` / `Transparent`), `GlassTintPreset` (`Dark` / `Light` / `System`)
  - `GlassStyle`, `GlassPalette`, `apply_glass_visuals`
  - `draw_glass_surface`, `draw_glass_surface_with_alpha`, `draw_glass_background`
- **Glass Widgets (`widgets`)**:
  - `install_system_font`, `draw_shadowed_text`, `draw_glass_titlebar`
  - `draw_glass_tab_bar`, `draw_glass_tab_bar_with_icons`
  - `glass_card`, `glass_toggle_row`, `glass_segmented_control`, `glass_slider_row`
  - `glass_button`, `glass_primary_button`, `glass_danger_button`, `draw_status_pill`, `glass_list_row`
- **Native Glass Context Menu (`context_menu::GlassContextMenu`)**:
  - Standalone `AlwaysOnTop` Acrylic popup viewport with 2-frame flicker-free reveal, cubic fade-in/quadratic fade-out, dividers, checkmarks, shortcut badges, and danger styling
- **Secondary Frameless Glass Window (`window::GlassWindow`)**:
  - 2-frame flicker-free reveal, multi-monitor work-area placement, debounced physical drag position persistence, titlebar + scroll layout (`show_framed` / `show` / `show_raw`)

## Adding to Another Project

In your `Cargo.toml` (sibling directory):

```toml
[dependencies]
egui = { version = "0.31", default-features = false }
eframe = { version = "0.31", default-features = false, features = ["glow"] }
egui_glass_ui = { path = "../egui_glass_ui" }
```

To disable `serde` derives on `BackgroundMode` and `GlassTintPreset`:

```toml
egui_glass_ui = { path = "../egui_glass_ui", default-features = false }
```

## Quick Example

```rust
use egui::{Pos2, Rect, Vec2};
use egui_glass_ui::{
    draw_glass_background, install_system_font, is_windows_light_theme,
    BackgroundMode, GlassContextMenu, GlassMenuEntry, GlassStyle, GlassTintPreset,
    GlassWindow, GlassWindowBackdrop, GlassWindowConfig,
};
```

1. **Main frameless viewport**:
   - Call `install_system_font(&cc.egui_ctx)` and `extract_hwnd(cc)` in `eframe` setup.
   - Construct `let style = GlassStyle::new(GlassTintPreset::System, 0.52, is_windows_light_theme());`.
   - Keep DWM synced with `backdrop.sync(hwnd, BackgroundMode::FrostedGlass, style.is_light())`.
   - Paint the card surface with `draw_glass_background(painter, rect, BackgroundMode::FrostedGlass, &style)`.
2. **Popup context menu**:
   - Build entries using `GlassMenuEntry::toggle(...)`, `GlassMenuEntry::item(...)`, `GlassMenuEntry::divider()`, `GlassMenuEntry::danger(...)`.
   - Open on right-click via `menu.open_for_entries(ctx, hwnd, click_pos, &entries)` and render with `menu.show(ctx, &style, window_size, &entries)`.
3. **Secondary settings/tool window**:
   - Create `GlassWindow::new(GlassWindowConfig::new("my_settings", "Settings", Vec2::new(468.0, 512.0)))`.
   - Open with `win.open(ctx, owner_hwnd, owner_pos_px, saved_pos_px)` and render with `win.show_framed(...)` or `win.show(...)`.
