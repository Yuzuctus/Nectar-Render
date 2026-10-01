//! L'interface porte le design Agrume v3 : papier « Sapin », encre, filets,
//! rayon 0, IBM Plex ; le jaune yuzu pour l'état rempli, le vert identité
//! pour le lieu courant. Clair et sombre composés, pas inversés.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, Visuals,
};

/// Les jetons Agrume d'un schéma (clair ou sombre).
#[derive(Clone, Copy)]
pub struct Tokens {
    pub paper: Color32,
    pub surface: Color32,
    pub sunken: Color32,
    pub raised: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub rule: Color32,
    pub rule_strong: Color32,
    pub identity: Color32,
    pub accent: Color32,
    pub accent_ink: Color32,
    pub marker: Color32,
    pub focus: Color32,
    pub danger: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Tokens = Tokens {
    paper: hex(0xf3f6ea),
    surface: hex(0xeaeedd),
    sunken: hex(0xe0e5d2),
    raised: hex(0xfbfcf5),
    ink: hex(0x141c17),
    muted: hex(0x4b554f),
    faint: hex(0x5f6761),
    rule: hex(0xd2d8c6),
    rule_strong: hex(0x7c8274),
    identity: hex(0x0e6b4c),
    accent: hex(0xf2e285),
    accent_ink: hex(0x1d1a05),
    marker: hex(0xffe696),
    focus: hex(0x156b85),
    danger: hex(0xa52638),
};

pub const DARK: Tokens = Tokens {
    paper: hex(0x131c17),
    surface: hex(0x1a251e),
    sunken: hex(0x222d27),
    raised: hex(0x25312a),
    ink: hex(0xeeede4),
    muted: hex(0xb9c0b8),
    faint: hex(0x99a199),
    rule: hex(0x2d3932),
    rule_strong: hex(0x707d74),
    identity: hex(0x86e3b4),
    accent: hex(0xf2de6e),
    accent_ink: hex(0x171403),
    marker: hex(0x584e0f),
    focus: hex(0x87cfe6),
    danger: hex(0xff9aa9),
};

pub fn tokens(ctx: &egui::Context) -> Tokens {
    if ctx.theme() == egui::Theme::Dark { DARK } else { LIGHT }
}

/// Famille de la voix mono des étiquettes (kickers).
pub fn mono() -> FontFamily {
    FontFamily::Monospace
}

/// Famille grasse (Plex Sans 600).
pub fn strong() -> FontFamily {
    FontFamily::Name("plex-strong".into())
}

/// Famille des titres (Plex Sans resserrée 600).
pub fn title() -> FontFamily {
    FontFamily::Name("plex-title".into())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), FontData::from_static(bytes).into());
    };
    add("plex-sans", include_bytes!("../../../assets/fonts/IBMPlexSans-Regular.ttf"));
    add("plex-sans-600", include_bytes!("../../../assets/fonts/IBMPlexSans-SemiBold.ttf"));
    add("plex-cond-600", include_bytes!("../../../assets/fonts/IBMPlexSansCondensed-SemiBold.ttf"));
    add("plex-mono-500", include_bytes!("../../../assets/fonts/IBMPlexMono-Medium.ttf"));

    let fallbacks: Vec<String> = fonts.families[&FontFamily::Proportional].clone();
    let family = |first: &str| {
        let mut list = vec![first.to_string()];
        list.extend(fallbacks.iter().cloned());
        list
    };
    fonts.families.insert(FontFamily::Proportional, family("plex-sans"));
    fonts.families.insert(FontFamily::Monospace, family("plex-mono-500"));
    fonts.families.insert(strong(), family("plex-sans-600"));
    fonts.families.insert(title(), family("plex-cond-600"));
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, title())),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.combo_width = 150.0;
    });
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK, true));
}

fn visuals(t: &Tokens, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let square = CornerRadius::ZERO;
    v.override_text_color = Some(t.ink);
    v.weak_text_color = Some(t.faint);
    v.hyperlink_color = t.identity;
    v.panel_fill = t.paper;
    v.window_fill = t.raised;
    v.faint_bg_color = t.surface;
    v.extreme_bg_color = t.raised;
    v.text_edit_bg_color = Some(t.raised);
    v.code_bg_color = t.surface;
    v.warn_fg_color = if dark { hex(0xffe696) } else { hex(0x6b5a00) };
    v.error_fg_color = t.danger;
    v.window_corner_radius = square;
    v.menu_corner_radius = square;
    v.window_stroke = Stroke::new(1.0, t.ink);
    // Pas d'ombre, sauf pour un panneau flottant.
    v.window_shadow =
        Shadow { offset: [0, 14], blur: 36, spread: 0, color: Color32::from_black_alpha(if dark { 160 } else { 60 }) };
    v.popup_shadow =
        Shadow { offset: [0, 8], blur: 20, spread: 0, color: Color32::from_black_alpha(if dark { 140 } else { 45 }) };
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, t.accent_ink);
    v.text_cursor.stroke = Stroke::new(2.0, t.focus);
    v.slider_trailing_fill = true;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.paper;
    w.noninteractive.weak_bg_fill = t.paper;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.rule);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.ink);
    w.noninteractive.corner_radius = square;

    w.inactive.bg_fill = t.raised;
    w.inactive.weak_bg_fill = t.raised;
    w.inactive.bg_stroke = Stroke::new(1.0, t.rule_strong);
    w.inactive.fg_stroke = Stroke::new(1.0, t.ink);
    w.inactive.corner_radius = square;

    w.hovered.bg_fill = t.surface;
    w.hovered.weak_bg_fill = t.surface;
    w.hovered.bg_stroke = Stroke::new(1.0, t.ink);
    w.hovered.fg_stroke = Stroke::new(1.0, t.ink);
    w.hovered.corner_radius = square;
    w.hovered.expansion = 0.0;

    // Le jaune yuzu « balaie » l'action pressée.
    w.active.bg_fill = t.accent;
    w.active.weak_bg_fill = t.accent;
    w.active.bg_stroke = Stroke::new(1.0, t.ink);
    w.active.fg_stroke = Stroke::new(1.0, t.accent_ink);
    w.active.corner_radius = square;
    w.active.expansion = 0.0;

    w.open.bg_fill = t.surface;
    w.open.weak_bg_fill = t.surface;
    w.open.bg_stroke = Stroke::new(1.0, t.ink);
    w.open.fg_stroke = Stroke::new(1.0, t.ink);
    w.open.corner_radius = square;
    v
}

/// L'étiquette mono en capitales : la seule voix en capitales d'Agrume.
pub fn kicker(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let t = tokens(ui.ctx());
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(FontId::new(10.5, mono()))
            .color(t.muted)
            .extra_letter_spacing(0.9),
    )
}

/// Un filet horizontal (`hair` 1 px ou `rule` 2 px).
pub fn rule(ui: &mut egui::Ui, width: f32, strong: bool) {
    let t = tokens(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), width), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, if strong { t.ink } else { t.rule });
}
