use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::path::Path;
use std::sync::Arc;

pub const DEFAULT_EDITOR_FONT: &str = "Courier New";
pub const EDITOR_FONT_FAMILY: &str = "editor";
pub const EDITOR_BOLD_FONT_FAMILY: &str = "editor_bold";
pub const EDITOR_ITALIC_FONT_FAMILY: &str = "editor_italic";
pub const EDITOR_BOLD_ITALIC_FONT_FAMILY: &str = "editor_bold_italic";

/// Metadata for an installed system font usable as the editor typeface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemFontOption {
    pub name: String,
    pub regular_path: String,
    pub bold_path: Option<String>,
    pub italic_path: Option<String>,
    pub bold_italic_path: Option<String>,
    pub is_monospace: bool,
}

struct FontCandidate {
    name: &'static str,
    regular: &'static str,
    bold: Option<&'static str>,
    italic: Option<&'static str>,
    bold_italic: Option<&'static str>,
    is_monospace: bool,
}

const FONT_CANDIDATES: &[FontCandidate] = &[
    FontCandidate {
        name: "Courier New",
        regular: r"C:\Windows\Fonts\cour.ttf",
        bold: Some(r"C:\Windows\Fonts\courbd.ttf"),
        italic: Some(r"C:\Windows\Fonts\couri.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\courbi.ttf"),
        is_monospace: true,
    },
    FontCandidate {
        name: "Consolas",
        regular: r"C:\Windows\Fonts\consola.ttf",
        bold: Some(r"C:\Windows\Fonts\consolab.ttf"),
        italic: Some(r"C:\Windows\Fonts\consolai.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\consolaz.ttf"),
        is_monospace: true,
    },
    FontCandidate {
        name: "Cascadia Mono",
        regular: r"C:\Windows\Fonts\CascadiaMono.ttf",
        bold: None,
        italic: None,
        bold_italic: None,
        is_monospace: true,
    },
    FontCandidate {
        name: "Cascadia Code",
        regular: r"C:\Windows\Fonts\CascadiaCode.ttf",
        bold: None,
        italic: None,
        bold_italic: None,
        is_monospace: true,
    },
    FontCandidate {
        name: "Lucida Console",
        regular: r"C:\Windows\Fonts\lucon.ttf",
        bold: None,
        italic: None,
        bold_italic: None,
        is_monospace: true,
    },
    FontCandidate {
        name: "Segoe UI",
        regular: r"C:\Windows\Fonts\segoeui.ttf",
        bold: Some(r"C:\Windows\Fonts\segoeuib.ttf"),
        italic: Some(r"C:\Windows\Fonts\segoeuii.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\segoeuiz.ttf"),
        is_monospace: false,
    },
    FontCandidate {
        name: "Arial",
        regular: r"C:\Windows\Fonts\arial.ttf",
        bold: Some(r"C:\Windows\Fonts\arialbd.ttf"),
        italic: Some(r"C:\Windows\Fonts\ariali.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\arialbi.ttf"),
        is_monospace: false,
    },
    FontCandidate {
        name: "Georgia",
        regular: r"C:\Windows\Fonts\georgia.ttf",
        bold: Some(r"C:\Windows\Fonts\georgiab.ttf"),
        italic: Some(r"C:\Windows\Fonts\georgiai.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\georgiaz.ttf"),
        is_monospace: false,
    },
    FontCandidate {
        name: "Calibri",
        regular: r"C:\Windows\Fonts\calibri.ttf",
        bold: Some(r"C:\Windows\Fonts\calibrib.ttf"),
        italic: Some(r"C:\Windows\Fonts\calibrii.ttf"),
        bold_italic: Some(r"C:\Windows\Fonts\calibriz.ttf"),
        is_monospace: false,
    },
];

fn existing_path(path: Option<&'static str>) -> Option<String> {
    path.filter(|p| Path::new(p).is_file())
        .map(ToOwned::to_owned)
}

/// Discovers installed system fonts suitable for glass UI and editor typography.
pub fn discover_installed_fonts() -> Vec<SystemFontOption> {
    FONT_CANDIDATES
        .iter()
        .filter(|c| Path::new(c.regular).is_file())
        .map(|c| SystemFontOption {
            name: c.name.to_owned(),
            regular_path: c.regular.to_owned(),
            bold_path: existing_path(c.bold),
            italic_path: existing_path(c.italic),
            bold_italic_path: existing_path(c.bold_italic),
            is_monospace: c.is_monospace,
        })
        .collect()
}

fn load_ui_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(bytes);
        }
    }
    None
}

/// Installs a standard Windows UI sans-serif font (`Segoe UI`, `Arial`, or `Tahoma`) into egui.
pub fn install_system_font(ctx: &Context) {
    if let Some(bytes) = load_ui_font_bytes() {
        let mut fonts = FontDefinitions::empty();
        fonts
            .font_data
            .insert("sys_ui".to_owned(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("sys_ui".to_owned());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("sys_ui".to_owned());
        ctx.set_fonts(fonts);
    }
}

/// Installs the proportional UI font (`Segoe UI` / fallback) along with the configured editor font
/// (regular, bold, italic, bold-italic variants when available) into `Monospace` and named
/// `"editor"`, `"editor_bold"`, `"editor_italic"`, and `"editor_bold_italic"` font families.
/// Returns the resolved font name.
pub fn install_glass_fonts(ctx: &Context, editor_font_name: &str) -> String {
    let installed = discover_installed_fonts();
    let chosen = installed
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(editor_font_name.trim()))
        .or_else(|| {
            installed
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(DEFAULT_EDITOR_FONT))
        })
        .or_else(|| installed.iter().find(|f| f.is_monospace))
        .or_else(|| installed.first())
        .cloned();

    let mut fonts = FontDefinitions::empty();
    let mut ui_fallback_key = None;
    if let Some(ui_bytes) = load_ui_font_bytes() {
        fonts
            .font_data
            .insert("sys_ui".to_owned(), Arc::new(FontData::from_owned(ui_bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("sys_ui".to_owned());
        ui_fallback_key = Some("sys_ui".to_owned());
    }

    let resolved_name = if let Some(opt) = chosen {
        let mut regular_key = None;
        if let Ok(reg_bytes) = std::fs::read(&opt.regular_path) {
            fonts.font_data.insert(
                "editor_regular".to_owned(),
                Arc::new(FontData::from_owned(reg_bytes)),
            );
            regular_key = Some("editor_regular".to_owned());
        }

        let mut push_family = |family: FontFamily, key: Option<String>| {
            let list = fonts.families.entry(family).or_default();
            if let Some(k) = key {
                list.push(k);
            }
            if let Some(ref reg) = regular_key {
                if !list.contains(reg) {
                    list.push(reg.clone());
                }
            }
            if let Some(ref ui) = ui_fallback_key {
                if !list.contains(ui) {
                    list.push(ui.clone());
                }
            }
        };

        let mut load_variant = |slot: &str, path: &Option<String>| -> Option<String> {
            let p = path.as_deref()?;
            let bytes = std::fs::read(p).ok()?;
            fonts
                .font_data
                .insert(slot.to_owned(), Arc::new(FontData::from_owned(bytes)));
            Some(slot.to_owned())
        };

        let bold_key = load_variant("editor_bold_data", &opt.bold_path);
        let italic_key = load_variant("editor_italic_data", &opt.italic_path);
        let bold_italic_key = load_variant("editor_bold_italic_data", &opt.bold_italic_path);

        push_family(FontFamily::Monospace, regular_key.clone());
        push_family(
            FontFamily::Name(EDITOR_FONT_FAMILY.into()),
            regular_key.clone(),
        );
        push_family(
            FontFamily::Name(EDITOR_BOLD_FONT_FAMILY.into()),
            bold_key.or_else(|| regular_key.clone()),
        );
        push_family(
            FontFamily::Name(EDITOR_ITALIC_FONT_FAMILY.into()),
            italic_key.or_else(|| regular_key.clone()),
        );
        push_family(
            FontFamily::Name(EDITOR_BOLD_ITALIC_FONT_FAMILY.into()),
            bold_italic_key,
        );

        opt.name
    } else {
        if let Some(ref ui) = ui_fallback_key {
            for fam in [
                FontFamily::Monospace,
                FontFamily::Name(EDITOR_FONT_FAMILY.into()),
                FontFamily::Name(EDITOR_BOLD_FONT_FAMILY.into()),
                FontFamily::Name(EDITOR_ITALIC_FONT_FAMILY.into()),
                FontFamily::Name(EDITOR_BOLD_ITALIC_FONT_FAMILY.into()),
            ] {
                fonts.families.entry(fam).or_default().push(ui.clone());
            }
        }
        "System UI".to_owned()
    };

    ctx.set_fonts(fonts);
    resolved_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_installed_fonts_windows() {
        let fonts = discover_installed_fonts();
        if cfg!(windows) {
            assert!(!fonts.is_empty());
            assert!(fonts.iter().any(|f| f.name == DEFAULT_EDITOR_FONT));
        }
    }
}
