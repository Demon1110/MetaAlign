//! Visual identity: palette, egui style and typography.

use std::collections::BTreeMap;

use egui::style::WidgetVisuals;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Vec2, Visuals,
};

/// The two themes the app can render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "深色",
            Self::Light => "浅色",
        }
    }
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

/// All colours used by the custom widgets.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub sunken: Color32,
    pub border: Color32,
    pub border_soft: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub overlay: Color32,
}

const fn rgb(value: u32) -> Color32 {
    Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

/// Graphite + teal, comfortable for long editing sessions.
pub fn dark() -> Palette {
    Palette {
        bg: rgb(0x0E1116),
        panel: rgb(0x121820),
        card: rgb(0x171F2A),
        card_hover: rgb(0x1E2834),
        sunken: rgb(0x0B0F14),
        border: rgb(0x27333F),
        border_soft: rgb(0x1B2430),
        text: rgb(0xE7EDF5),
        text_muted: rgb(0x9CABBA),
        text_dim: rgb(0x6D7B8A),
        accent: rgb(0x3FD3B0),
        accent_soft: Color32::from_rgba_unmultiplied(0x3F, 0xD3, 0xB0, 38),
        on_accent: rgb(0x04211B),
        ok: rgb(0x4ADE80),
        warn: rgb(0xF5B955),
        error: rgb(0xF87171),
        overlay: Color32::from_black_alpha(180),
    }
}

/// Paper + deep teal, for bright offices and projectors.
pub fn light() -> Palette {
    Palette {
        bg: rgb(0xF1F4F9),
        panel: rgb(0xFFFFFF),
        card: rgb(0xFFFFFF),
        card_hover: rgb(0xF6F9FC),
        sunken: rgb(0xE9EEF5),
        border: rgb(0xDCE4EE),
        border_soft: rgb(0xECF1F7),
        text: rgb(0x16202B),
        text_muted: rgb(0x5A6A79),
        text_dim: rgb(0x8A97A6),
        accent: rgb(0x0FA37F),
        accent_soft: Color32::from_rgba_unmultiplied(0x0F, 0xA3, 0x7F, 28),
        on_accent: rgb(0xFFFFFF),
        ok: rgb(0x15803D),
        warn: rgb(0xB45309),
        error: rgb(0xB91C1C),
        overlay: Color32::from_black_alpha(120),
    }
}

pub fn palette(mode: ThemeMode) -> Palette {
    match mode {
        ThemeMode::Dark => dark(),
        ThemeMode::Light => light(),
    }
}

fn widget(
    weak_bg: Color32,
    bg: Color32,
    border: Color32,
    fg: Color32,
    radius: u8,
) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: weak_bg,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: CornerRadius::same(radius),
        fg_stroke: Stroke::new(1.0, fg),
        expansion: 0.0,
    }
}

fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let mut styles = BTreeMap::new();
    styles.insert(TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional));
    styles.insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    styles.insert(TextStyle::Button, FontId::new(13.5, FontFamily::Proportional));
    styles.insert(TextStyle::Small, FontId::new(11.5, FontFamily::Proportional));
    styles.insert(TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
    styles.insert(
        TextStyle::Name("H1".into()),
        FontId::new(26.0, FontFamily::Proportional),
    );
    styles.insert(
        TextStyle::Name("H2".into()),
        FontId::new(17.0, FontFamily::Proportional),
    );
    styles.insert(
        TextStyle::Name("Caption".into()),
        FontId::new(11.5, FontFamily::Proportional),
    );
    styles.insert(
        TextStyle::Name("Eyebrow".into()),
        FontId::new(10.5, FontFamily::Proportional),
    );
    styles
}

/// Apply the theme to the whole context.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_theme(match mode {
        ThemeMode::Dark => egui::ThemePreference::Dark,
        ThemeMode::Light => egui::ThemePreference::Light,
    });

    let p = palette(mode);
    let mut visuals = if mode.is_dark() {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.dark_mode = mode.is_dark();
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.panel;
    visuals.faint_bg_color = p.card;
    visuals.extreme_bg_color = p.sunken;
    visuals.code_bg_color = p.card;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.accent;
    visuals.warn_fg_color = p.warn;
    visuals.error_fg_color = p.error;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.striped = false;
    visuals.widgets.noninteractive = widget(p.card, p.card, p.border_soft, p.text_muted, 8);
    visuals.widgets.inactive = widget(p.card, p.card, p.border, p.text, 8);
    visuals.widgets.hovered = widget(p.card_hover, p.card_hover, p.accent, p.text, 8);
    visuals.widgets.active = widget(p.accent_soft, p.card_hover, p.accent, p.text, 8);
    visuals.widgets.open = widget(p.card_hover, p.card_hover, p.border, p.text, 8);
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles();
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(13.0, 7.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.indent = 16.0;
        style.spacing.scroll.bar_width = 6.0;
        style.interaction.selectable_labels = false;
    });
}
