//! The pieces the mock (docs/mock/minato.html) is built from: its tones, the
//! repository tags, the label and kind chips, the 受信箱 badge, the keys and
//! the boxes of the detail. Each draws at a left edge and a middle line and
//! gives back where it ends, so a row is laid out from left to right.

use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use minato_core::list::{Kind, Label};
use minato_core::notes::Tone;

pub use crate::ui::ACCENT;

pub const OK: Color32 = Color32::from_rgb(0x4F, 0xBF, 0x7F);
pub const BAD: Color32 = Color32::from_rgb(0xE5, 0x58, 0x6B);
pub const RUN: Color32 = Color32::from_rgb(0xE8, 0xB0, 0x4A);
pub const ASK: Color32 = Color32::from_rgb(0xA0, 0x7C, 0xF0);
pub const IDLE: Color32 = Color32::from_rgb(0x8A, 0x93, 0xA0);
/// A pull request's mark.
pub const PR: Color32 = Color32::from_rgb(0xB4, 0x8C, 0xF5);
/// The text on an accent button.
const ON_ACCENT: Color32 = Color32::from_rgb(0x0E, 0x17, 0x22);
/// The selected row's background on a dark theme.
const SEL_DARK: Color32 = Color32::from_rgb(0x26, 0x40, 0x5A);

pub fn tone(t: Tone) -> Color32 {
    match t {
        Tone::Bad => BAD,
        Tone::Ok => OK,
        Tone::Ask => ASK,
        Tone::Run => RUN,
        Tone::Idle => IDLE,
        Tone::Accent => ACCENT,
    }
}

/// The selected row's background.
pub fn sel() -> Color32 {
    let c = ito_theme::colors();
    if c.light { c.selection() } else { SEL_DARK }
}

/// The background of a card (the mock's `--card`).
pub fn card() -> Color32 {
    ito_theme::colors().panel
}

/// The background of a head or a key (the mock's `--head`).
pub fn head() -> Color32 {
    ito_theme::colors().raised()
}

/// A colour by the repository's name without the owner.
pub fn repo_color(repo: &str) -> Color32 {
    match repo.rsplit('/').next().unwrap_or(repo) {
        "kura" => Color32::from_rgb(0x7A, 0x3B, 0x66),
        "tsumugi" => Color32::from_rgb(0x8A, 0x3A, 0x5E),
        "yagura" => Color32::from_rgb(0x3C, 0x5E, 0x9A),
        "ito" => Color32::from_rgb(0x8A, 0x6A, 0x1E),
        _ => Color32::from_rgb(0x4A, 0x4F, 0x60),
    }
}

/// Text drawn twice, a hair apart, for the mock's weight 600 (egui has one weight).
pub fn bold(p: &Painter, at: Pos2, align: Align2, text: impl ToString, font: FontId, color: Color32) -> Rect {
    let text = text.to_string();
    p.text(at + vec2(0.45, 0.0), align, &text, font.clone(), color);
    p.text(at, align, text, font, color)
}

/// A filled pill with `text` on it from `x`, centred on `y`; its right edge.
#[allow(clippy::too_many_arguments)]
fn pill(p: &Painter, x: f32, y: f32, text: &str, font: FontId, pad: f32, h: f32, radius: u8, fill: Color32, color: Color32) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), font, color);
    let r = Rect::from_min_size(pos2(x, y - h / 2.0), vec2(g.size().x + pad * 2.0, h));
    p.rect_filled(r, CornerRadius::same(radius), fill);
    p.galley(pos2(r.left() + pad, y - g.size().y / 2.0), g, color);
    r.right()
}

/// A bordered chip; its right edge.
#[allow(clippy::too_many_arguments)]
pub fn chip(p: &Painter, x: f32, y: f32, text: &str, font: FontId, pad: f32, h: f32, radius: u8, border: Color32, fill: Color32, color: Color32) -> f32 {
    let g = p.layout_no_wrap(text.to_owned(), font, color);
    let r = Rect::from_min_size(pos2(x, y - h / 2.0), vec2(g.size().x + pad * 2.0, h));
    p.rect_filled(r, CornerRadius::same(radius), fill);
    p.rect_stroke(r, CornerRadius::same(radius), Stroke::new(1.0, border), StrokeKind::Inside);
    p.galley(pos2(r.left() + pad, y - g.size().y / 2.0), g, color);
    r.right()
}

/// The repository tag; `text` is what is written on it (`kura` or `uchmk/kura`).
pub fn tag(p: &Painter, x: f32, y: f32, repo: &str, text: &str) -> f32 {
    pill(p, x, y, text, FontId::proportional(11.5), 8.0, 18.0, 5, repo_color(repo), Color32::WHITE)
}

/// A label's colour: its own, else the mock's for the usual names.
fn label_color(l: &Label) -> Color32 {
    if let Some(c) = crate::list_ui::parse_hex(&l.color) {
        return c;
    }
    let rgb = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    rgb(match l.name.as_str() {
        "bug" => 0xB33A4C,
        "finding" => 0x6B7280,
        "needs-pc" => 0x2F6E9E,
        "question" => 0x7E57C2,
        "vote" => 0x5E7D3A,
        "deps" | "dependencies" => 0x1F6F68,
        "enhancement" => 0x3A7BBF,
        _ => 0x4A4F60,
    })
}

/// One label; its right edge.
pub fn label(p: &Painter, x: f32, y: f32, l: &Label) -> f32 {
    let fill = label_color(l);
    // Dark text on a bright label, as GitHub does.
    let [r, g, b, _] = fill.to_array();
    let bright = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 165.0;
    pill(p, x, y, &l.name, FontId::proportional(11.0), 7.0, 18.0, 9, fill, if bright { ON_ACCENT } else { Color32::WHITE })
}

/// The labels from `x` until `right`; where they end.
pub fn labels(p: &Painter, x: f32, y: f32, ls: &[Label], right: f32) -> f32 {
    let mut x = x;
    for l in ls {
        let w = p.layout_no_wrap(l.name.clone(), FontId::proportional(11.0), Color32::WHITE).size().x + 14.0;
        if x + w > right {
            break;
        }
        x = label(p, x, y, l) + 4.0;
    }
    x
}

/// The mark and the colour of a kind.
pub fn kind_mark(k: Kind) -> (&'static str, Color32) {
    match k {
        Kind::Issue => ("◎", OK),
        Kind::Pr => ("⇄", PR),
        Kind::Discussion => ("◇", ACCENT),
    }
}

/// The kind chip (`◎ Issue`); its right edge.
pub fn kind_chip(p: &Painter, x: f32, y: f32, k: Kind) -> f32 {
    let (mark, color) = kind_mark(k);
    let word = match k {
        Kind::Issue => "Issue",
        Kind::Pr => "PR",
        Kind::Discussion => "Discussion",
    };
    run_chip(p, x, y, &format!("{mark} {word}"), color)
}

/// A chip in the kind's frame with any text (the run's `▶ Actions`).
pub fn run_chip(p: &Painter, x: f32, y: f32, text: &str, color: Color32) -> f32 {
    chip(p, x, y, text, FontId::proportional(11.0), 6.0, 17.0, 4, color, Color32::TRANSPARENT, color)
}

/// The 受信箱 badge; its right edge.
pub fn badge(p: &Painter, x: f32, y: f32, text: &str) -> f32 {
    chip(p, x, y, text, FontId::proportional(10.5), 5.0, 16.0, 4, ACCENT, Color32::TRANSPARENT, ACCENT)
}

/// The width the badge takes.
pub fn badge_w(p: &Painter, text: &str) -> f32 {
    p.layout_no_wrap(text.to_owned(), FontId::proportional(10.5), ACCENT).size().x + 10.0
}

/// A key cap; its right edge.
pub fn kbd(p: &Painter, x: f32, y: f32, key: &str) -> f32 {
    let c = ito_theme::colors();
    chip(p, x, y, key, FontId::monospace(11.0), 5.0, 18.0, 4, c.border, head(), c.fg)
}

/// One line cut short with `…` at `width`.
pub fn truncated(p: &Painter, text: String, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(10.0));
    p.layout_job(job)
}

/// A filled dot.
pub fn dot(p: &Painter, at: Pos2, r: f32, color: Color32) {
    p.circle_filled(at, r, color);
}

/// A section heading of the detail (`検査`, `本文`, `経過`); the y after it.
pub fn heading(p: &Painter, x: f32, y: f32, text: &str) -> f32 {
    let c = ito_theme::colors();
    let y = y + 18.0;
    bold(p, pos2(x, y + 8.0), Align2::LEFT_CENTER, text, FontId::proportional(11.5), c.dim);
    y + 22.0
}

/// What the next box says.
pub struct Next<'a> {
    pub tone: Color32,
    /// The small words over the label (「次にやること」).
    pub lead: &'a str,
    pub label: &'a str,
    pub note: &'a str,
    /// The key of the button at the right, if any.
    pub key: Option<&'a str>,
}

/// The box of the next step; the y under it.
pub fn next_box(p: &Painter, x: f32, y: f32, w: f32, n: &Next) -> f32 {
    let c = ito_theme::colors();
    let y = y + 14.0;
    let pad = 14.0;
    // The button at the right, measured first so the words wrap before it.
    let go = n.key.map(|k| {
        let kg = p.layout_no_wrap(k.to_owned(), FontId::monospace(11.0), ON_ACCENT);
        let lg = p.layout_no_wrap(n.label.to_owned(), FontId::proportional(12.5), ON_ACCENT);
        (kg.size().x + 10.0 + 8.0 + lg.size().x + 24.0, k)
    });
    let words_w = (w - pad * 2.0 - go.map_or(0.0, |(gw, _)| gw + 14.0)).max(80.0);
    let lead = p.layout_no_wrap(n.lead.to_owned(), FontId::proportional(12.5), c.fg);
    let label = p.layout(n.label.to_owned(), FontId::proportional(14.0), c.fg, words_w);
    let note = (!n.note.is_empty()).then(|| p.layout(n.note.to_owned(), FontId::proportional(12.5), c.dim, words_w));
    let h = 9.0 + lead.size().y + label.size().y + note.as_ref().map_or(0.0, |g| g.size().y) + 9.0;
    let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
    p.rect_filled(r, CornerRadius::same(8), card());
    p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, c.border), StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(r.min, vec2(4.0, h)), CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, n.tone);
    let mut ty = r.top() + 9.0;
    let lh = lead.size().y;
    p.galley(pos2(x + pad, ty), lead, c.fg);
    ty += lh;
    let bh = label.size().y;
    p.galley(pos2(x + pad + 0.45, ty), label.clone(), c.fg);
    p.galley(pos2(x + pad, ty), label, c.fg);
    ty += bh;
    if let Some(g) = note {
        p.galley(pos2(x + pad, ty), g, c.dim);
    }
    if let Some((gw, k)) = go {
        let b = Rect::from_min_size(pos2(r.right() - pad - gw, r.center().y - 14.0), vec2(gw, 28.0));
        p.rect_filled(b, CornerRadius::same(6), ACCENT);
        let kx = chip(p, b.left() + 12.0, b.center().y, k, FontId::monospace(11.0), 5.0, 18.0, 4, Color32::TRANSPARENT, Color32::from_white_alpha(128), ON_ACCENT);
        bold(p, pos2(kx + 8.0, b.center().y), Align2::LEFT_CENTER, n.label, FontId::proportional(12.5), ON_ACCENT);
    }
    r.bottom() + 6.0
}

/// The other keys under the next box (`d 済み  s あとで`); the y under them.
pub fn alt_keys(p: &Painter, x: f32, y: f32, right: f32, keys: &[(&str, &str)]) -> f32 {
    let c = ito_theme::colors();
    let mut kx = x + 2.0;
    let mut y = y + 10.0;
    for (k, word) in keys {
        let w = p.layout_no_wrap(word.to_string(), FontId::proportional(12.5), c.dim).size().x;
        let kw = p.layout_no_wrap(k.to_string(), FontId::monospace(11.0), c.fg).size().x + 10.0;
        if kx + kw + 4.0 + w > right && kx > x + 2.0 {
            kx = x + 2.0;
            y += 22.0;
        }
        let e = kbd(p, kx, y, k);
        let t = p.text(pos2(e + 4.0, y), Align2::LEFT_CENTER, *word, FontId::proportional(12.5), c.dim);
        kx = t.right() + 16.0;
    }
    y + 16.0
}

/// One row of the checks (`✓ name   time`), a bad one tinted; the y under it.
#[allow(clippy::too_many_arguments)]
pub fn check_row(p: &Painter, x: f32, y: f32, w: f32, mark: &str, color: Color32, name: &str, time: &str) -> f32 {
    let c = ito_theme::colors();
    let r = Rect::from_min_size(pos2(x, y), vec2(w, 26.0));
    if color == BAD {
        p.rect_filled(r, CornerRadius::same(6), BAD.gamma_multiply(0.10));
    }
    p.text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, mark, FontId::proportional(13.0), color);
    let t = p.text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, time, FontId::monospace(12.0), c.dim);
    p.with_clip_rect(Rect::from_min_max(r.min, pos2(t.left() - 8.0, r.bottom()))).text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(13.0), c.fg);
    r.bottom()
}

/// `⋯ 上に 3 件` / `⋯ 3 below` where folded rows were left out.
pub fn more(p: &Painter, ja: bool, side: &str, x: f32, y: f32, n: usize) {
    let c = ito_theme::colors();
    let words = if ja { format!("⋯ {side}{n} 件") } else { format!("⋯ {n} {side}") };
    p.text(pos2(x, y), Align2::LEFT_CENTER, words, FontId::proportional(11.5), c.dim);
}
