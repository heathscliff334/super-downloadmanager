use crate::engine::DownloadState;
use crate::util::FileKind;
use egui::{Color32, Context, CornerRadius, Visuals, vec2};

#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub sidebar: Color32,
    pub bg: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub muted: Color32,
    pub warn_bg: Color32,
    pub warn_fg: Color32,
}

impl Palette {
    pub fn of(dark: bool) -> Self {
        let accent = Color32::from_rgb(76, 139, 245);
        if dark {
            Self {
                accent,
                sidebar: Color32::from_rgb(15, 19, 30),
                bg: Color32::from_rgb(20, 25, 39),
                card: Color32::from_rgb(28, 35, 54),
                card_hover: Color32::from_rgb(33, 41, 63),
                border: Color32::from_rgb(42, 51, 76),
                muted: Color32::from_rgb(146, 156, 180),
                warn_bg: Color32::from_rgb(64, 48, 16),
                warn_fg: Color32::from_rgb(250, 204, 106),
            }
        } else {
            Self {
                accent,
                sidebar: Color32::from_rgb(236, 240, 247),
                bg: Color32::from_rgb(246, 248, 252),
                card: Color32::WHITE,
                card_hover: Color32::from_rgb(241, 245, 252),
                border: Color32::from_rgb(220, 226, 238),
                muted: Color32::from_rgb(100, 110, 132),
                warn_bg: Color32::from_rgb(255, 243, 214),
                warn_fg: Color32::from_rgb(133, 85, 0),
            }
        }
    }

    pub fn state_color(&self, s: &DownloadState) -> Color32 {
        match s {
            DownloadState::Downloading => self.accent,
            DownloadState::Queued => self.muted,
            DownloadState::Paused => Color32::from_rgb(232, 168, 56),
            DownloadState::Completed => Color32::from_rgb(52, 190, 120),
            DownloadState::Failed(_) => Color32::from_rgb(235, 87, 87),
        }
    }
}

pub fn kind_color(k: FileKind) -> Color32 {
    match k {
        FileKind::Video => Color32::from_rgb(236, 100, 140),
        FileKind::Audio => Color32::from_rgb(170, 120, 240),
        FileKind::Archive => Color32::from_rgb(232, 168, 56),
        FileKind::Document => Color32::from_rgb(76, 139, 245),
        FileKind::Program => Color32::from_rgb(52, 190, 120),
        FileKind::Image => Color32::from_rgb(60, 190, 200),
        FileKind::Other => Color32::from_rgb(140, 150, 170),
    }
}

pub fn kind_badge(k: FileKind) -> &'static str {
    match k {
        FileKind::Video => "VID",
        FileKind::Audio => "AUD",
        FileKind::Archive => "ZIP",
        FileKind::Document => "DOC",
        FileKind::Program => "APP",
        FileKind::Image => "IMG",
        FileKind::Other => "FILE",
    }
}

pub fn apply(ctx: &Context, dark: bool) {
    let p = Palette::of(dark);
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke.color = p.border;
    v.extreme_bg_color = if dark { p.sidebar } else { Color32::WHITE };
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.hyperlink_color = p.accent;
    v.window_corner_radius = CornerRadius::same(12);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.corner_radius = CornerRadius::same(8);
    }
    v.widgets.inactive.bg_fill = p.card_hover;
    v.widgets.inactive.weak_bg_fill = p.card_hover;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
    });
}
