//! A system font behind egui's own, for the Japanese (and the CJK in titles
//! of issues, whatever the language). The candidates are every uchmk app's
//! (`ito_common::fonts::JAPANESE`): the first that reads is used. Behind it,
//! the system's emoji face, for the emoji egui's own small set lacks (🤖 in
//! a body written by Claude Code).

/// The system emoji faces, in order. Their outlines draw in one colour, as
/// egui's own emoji do. macOS has only a bitmap face, which egui cannot read.
#[cfg(windows)]
const EMOJI: &[&str] = &[r"C:\Windows\Fonts\seguiemj.ttf"];
#[cfg(target_os = "macos")]
const EMOJI: &[&str] = &[];
#[cfg(not(any(windows, target_os = "macos")))]
const EMOJI: &[&str] = &["/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf", "/usr/share/fonts/noto/NotoEmoji-Regular.ttf"];

/// Read the candidates on a thread (the files are tens of megabytes) and put
/// them behind the monospace and proportional faces. Until they are in, a CJK
/// character draws as a box for a frame or two.
pub fn load_in_background(ctx: egui::Context, mono: ito_common::MonoFont) {
    let _ = std::thread::Builder::new().name("fonts".into()).spawn(move || {
        let found: Vec<_> = [ito_common::fonts::JAPANESE, EMOJI].into_iter().filter_map(ito_common::fonts::read_first).collect();
        let mut fonts = egui::FontDefinitions::default();
        // common.toml's `mono_font`: the bundled face goes first in monospace unless it says `system`.
        ito_fonts::register(&mut fonts, mono);
        if found.is_empty() && mono != ito_common::MonoFont::Bundled {
            return;
        }
        for (path, bytes) in found {
            let name = format!("system:{}", path.display());
            fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
                fonts.families.entry(family).or_default().push(name.clone());
            }
        }
        ctx.set_fonts(fonts);
        ctx.request_repaint();
    });
}
