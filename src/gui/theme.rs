use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

/// Hiveory-inspired near-black / zinc palette with a quiet cyan accent.
pub struct Theme;

impl Theme {
    // Surfaces — hiveory.vercel.app
    pub const BG: Color32 = Color32::from_rgb(0x06, 0x06, 0x08);
    pub const PANEL: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x10);
    pub const GLASS: Color32 = Color32::from_rgb(0x12, 0x12, 0x16);
    pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x14, 0x18);
    pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x22);
    pub const SELECTED: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x20);

    // Borders — white/10 and white/20 approximations
    pub const BORDER: Color32 = Color32::from_rgb(0x3f, 0x3f, 0x46);
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2e);
    pub const BORDER_BRIGHT: Color32 = Color32::from_rgb(0x52, 0x52, 0x5b);

    // Text — zinc
    pub const TEXT: Color32 = Color32::from_rgb(0xf4, 0xf4, 0xf5);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xa1, 0xa1, 0xaa);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(0x71, 0x71, 0x7a);

    // Quiet cyan accent (selection, prompt, primary)
    pub const ACCENT: Color32 = Color32::from_rgb(0x2d, 0xc4, 0xb4);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x24, 0x9e, 0x90);

    // Soft semantic
    pub const SUCCESS: Color32 = Color32::from_rgb(0x00, 0xb9, 0x5c);
    pub const WARN: Color32 = Color32::from_rgb(0xd2, 0x99, 0x22);
    pub const ERROR: Color32 = Color32::from_rgb(0xfc, 0x41, 0x3d);
    pub const INFO: Color32 = Color32::from_rgb(0x74, 0x9b, 0xff);

    // Terminal
    pub const TERM_BG: Color32 = Color32::from_rgb(0x07, 0x07, 0x0a);
    pub const TERM_STDOUT: Color32 = Color32::from_rgb(0xf0, 0xf2, 0xf5);
    pub const TERM_STDERR: Color32 = Color32::from_rgb(0xfc, 0x41, 0x3d);
    pub const TERM_TIMESTAMP: Color32 = Color32::from_rgb(0x52, 0x52, 0x5b);

    // Diff
    pub const DIFF_ADD: Color32 = Color32::from_rgb(0x00, 0xb9, 0x5c);
    pub const DIFF_ADD_BG: Color32 = Color32::from_rgba_premultiplied(0x00, 0x5c, 0x2e, 0x40);
    pub const DIFF_DEL: Color32 = Color32::from_rgb(0xfc, 0x41, 0x3d);
    pub const DIFF_DEL_BG: Color32 = Color32::from_rgba_premultiplied(0x5a, 0x14, 0x14, 0x45);
    pub const DIFF_HUNK: Color32 = Color32::from_rgb(0x74, 0x9b, 0xff);
    pub const DIFF_HUNK_BG: Color32 = Color32::from_rgba_premultiplied(0x1a, 0x2a, 0x4c, 0x40);
    pub const STDERR_BG: Color32 = Color32::from_rgba_premultiplied(0x5a, 0x14, 0x14, 0x30);

    pub fn hairline() -> Stroke {
        Stroke::new(1.0_f32, Self::BORDER)
    }

    pub fn hairline_subtle() -> Stroke {
        Stroke::new(1.0_f32, Self::BORDER_SUBTLE)
    }

    pub fn hairline_bright() -> Stroke {
        Stroke::new(1.0_f32, Self::BORDER_BRIGHT)
    }

    pub fn apply(ctx: &egui::Context) {
        Self::apply_fonts(ctx);

        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::PANEL;
        visuals.window_fill = Self::SURFACE;
        visuals.window_stroke = Self::hairline();
        visuals.extreme_bg_color = Self::BG;
        visuals.faint_bg_color = Self::SURFACE;
        visuals.code_bg_color = Self::TERM_BG;
        visuals.override_text_color = Some(Self::TEXT);
        visuals.hyperlink_color = Self::ACCENT;
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(0x2d, 0xc4, 0xb4, 0x35);
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::ACCENT_DIM);
        visuals.widgets.noninteractive.bg_fill = Self::SURFACE;
        visuals.widgets.noninteractive.bg_stroke = Self::hairline_subtle();
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_MUTED);
        visuals.widgets.inactive.bg_fill = Self::SURFACE;
        visuals.widgets.inactive.bg_stroke = Self::hairline_subtle();
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_MUTED);
        visuals.widgets.hovered.bg_fill = Self::SURFACE_HOVER;
        visuals.widgets.hovered.bg_stroke = Self::hairline();
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Self::TEXT);
        visuals.widgets.active.bg_fill = Self::SELECTED;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_BRIGHT);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Self::TEXT);
        visuals.widgets.open.bg_fill = Self::SURFACE;
        visuals.widgets.open.bg_stroke = Self::hairline();
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Self::TEXT);
        visuals.window_corner_radius = 10.0.into();
        visuals.menu_corner_radius = 8.0.into();
        visuals.button_frame = true;
        visuals.striped = false;

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(14);
        style.visuals = visuals;
        ctx.set_style(style);
    }

    fn apply_fonts(ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();
        let home = std::env::var("HOME").unwrap_or_default();

        let sans_paths: Vec<String> = vec![
            format!("{home}/Library/Fonts/Inter.ttf"),
            format!("{home}/Library/Fonts/Inter-Regular.ttf"),
            format!("{home}/.local/share/fonts/Inter-Regular.ttf"),
            "/Library/Fonts/Inter.ttf".into(),
            "/Library/Fonts/Inter-Regular.ttf".into(),
            "/System/Library/Fonts/Supplemental/Arial.ttf".into(),
            "/Library/Fonts/Arial.ttf".into(),
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf".into(),
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
            r"C:\Windows\Fonts\arial.ttf".into(),
            r"C:\Windows\Fonts\segoeui.ttf".into(),
        ];

        if let Some(data) = load_first_font(&sans_paths) {
            fonts.font_data.insert("HiveSans".to_owned(), data.into());
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "HiveSans".to_owned());
            }
        }

        let mono_paths: Vec<String> = vec![
            format!("{home}/Library/Fonts/JetBrainsMono-Regular.ttf"),
            format!("{home}/Library/Fonts/JetBrainsMonoNL-Regular.ttf"),
            format!("{home}/.local/share/fonts/JetBrainsMono-Regular.ttf"),
            "/Library/Fonts/JetBrainsMono-Regular.ttf".into(),
            "/Library/Fonts/JetBrainsMonoNL-Regular.ttf".into(),
            "/System/Library/Fonts/Supplemental/Courier New.ttf".into(),
            "/Library/Fonts/SF-Mono-Regular.otf".into(),
            "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf".into(),
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".into(),
            r"C:\Windows\Fonts\consola.ttf".into(),
            r"C:\Windows\Fonts\cour.ttf".into(),
        ];

        if let Some(data) = load_first_font(&mono_paths) {
            fonts.font_data.insert("HiveMono".to_owned(), data.into());
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "HiveMono".to_owned());
            }
        }

        ctx.set_fonts(fonts);
    }
}

fn load_first_font(paths: &[String]) -> Option<FontData> {
    for path in paths {
        if path.is_empty() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path) {
            return Some(FontData::from_owned(bytes));
        }
    }
    None
}
