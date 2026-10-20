//! Noto Sans setup and bold text helpers.

use crate::fonts::{NOTO_SANS, NOTO_SANS_ARABIC, NOTO_SANS_DEVANAGARI, NOTO_SANS_MONO, NOTO_SANS_SYMBOLS, NOTO_SANS_SYMBOLS_2, cjk_fonts};

const REGULAR_WEIGHT: f32 = 400.0;
const BOLD_WEIGHT: f32 = 700.0;

fn variable_font(font: &'static [u8], weight: f32) -> egui::FontData {
    egui::FontData::from_static(font).tweak(egui::FontTweak {
        coords: egui::epaint::text::VariationCoords::new([(b"wght", weight)]),
        ..Default::default()
    })
}

/// Install the bundled Noto Sans fonts into the egui context.
///
/// The base and mono faces cover Latin and Cyrillic. Script-specific faces are
/// fallbacks for Arabic/Persian, Devanagari, Simplified Chinese (also Japanese),
/// Korean, and UI symbols. The text faces are variable, so regular and bold
/// share files.
///
/// The CJK faces are fetched on the web: until they arrive the browser draws
/// those glyphs, and calling this again once they have swaps the Noto faces in.
/// Anything no bundled face covers - emoji, Thai, Hebrew - comes from the
/// system fonts on native and the browser on the web.
pub(crate) fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    let cjk = cjk_fonts();
    let mut faces = vec![
        ("noto_sans_regular", variable_font(NOTO_SANS, REGULAR_WEIGHT)),
        ("noto_sans_mono_regular", variable_font(NOTO_SANS_MONO, REGULAR_WEIGHT)),
        ("noto_sans_arabic_regular", variable_font(NOTO_SANS_ARABIC, REGULAR_WEIGHT)),
        ("noto_sans_devanagari_regular", variable_font(NOTO_SANS_DEVANAGARI, REGULAR_WEIGHT)),
        ("noto_sans_symbols_regular", variable_font(NOTO_SANS_SYMBOLS, REGULAR_WEIGHT)),
        ("noto_sans_symbols_2", egui::FontData::from_static(NOTO_SANS_SYMBOLS_2)),
        ("noto_sans_bold", variable_font(NOTO_SANS, BOLD_WEIGHT)),
        ("noto_sans_arabic_bold", variable_font(NOTO_SANS_ARABIC, BOLD_WEIGHT)),
        ("noto_sans_devanagari_bold", variable_font(NOTO_SANS_DEVANAGARI, BOLD_WEIGHT)),
        ("noto_sans_symbols_bold", variable_font(NOTO_SANS_SYMBOLS, BOLD_WEIGHT)),
    ];
    if let Some(cjk) = cjk {
        faces.extend([
            ("noto_sans_sc_regular", variable_font(cjk.sc, REGULAR_WEIGHT)),
            ("noto_sans_kr_regular", variable_font(cjk.kr, REGULAR_WEIGHT)),
            ("noto_sans_sc_bold", variable_font(cjk.sc, BOLD_WEIGHT)),
            ("noto_sans_kr_bold", variable_font(cjk.kr, BOLD_WEIGHT)),
        ]);
    }
    for (name, data) in faces {
        fonts.font_data.insert(name.to_owned(), data.into());
    }

    // Each family tries its own face, then the script fallbacks in this order.
    let family = |head: &str, weight: &str| {
        let scripts: &[&str] = if cjk.is_some() {
            &["arabic", "devanagari", "sc", "kr", "symbols"]
        } else {
            &["arabic", "devanagari", "symbols"]
        };
        std::iter::once(head.to_owned())
            .chain(scripts.iter().map(|script| format!("noto_sans_{script}_{weight}")))
            .chain(std::iter::once("noto_sans_symbols_2".to_owned()))
            .collect::<Vec<_>>()
    };
    fonts.families.insert(egui::FontFamily::Proportional, family("noto_sans_regular", "regular"));
    fonts.families.insert(egui::FontFamily::Monospace, family("noto_sans_mono_regular", "regular"));
    fonts.families.insert(bold_family(), family("noto_sans_bold", "bold"));
    ctx.set_fonts(fonts);
    install_fallbacks(ctx);
}

/// Native: the system's own fonts, found on demand.
#[cfg(not(target_arch = "wasm32"))]
fn install_fallbacks(ctx: &egui::Context) {
    ctx.set_font_providers(vec![std::sync::Arc::new(egui_system_font_provider::SystemFontProvider::new())]);
}

/// Web: a CJK character with no face yet starts the fetch, and the browser
/// draws whatever no font covers - CJK included, until its faces arrive.
#[cfg(target_arch = "wasm32")]
fn install_fallbacks(ctx: &egui::Context) {
    let request_cjk = |request: &egui::epaint::text::FallbackRequest<'_>| {
        crate::fonts::request_cjk_fonts_for(request.cluster);
        None
    };
    ctx.set_font_providers(vec![std::sync::Arc::new(request_cjk)]);
    ctx.add_glyph_rasterizer(super::canvas_glyphs::glyph_rasterizer());
}

/// The family the bundled bold faces are registered under.
fn bold_family() -> egui::FontFamily {
    egui::FontFamily::Name("noto_sans_bold".into())
}

/// Return a [`RichText`] styled with the bundled bold font face.
pub(crate) fn bold(label: &str) -> egui::RichText {
    egui::RichText::new(label).family(bold_family())
}

/// The bundled bold face at `size`, for text laid out as an
/// [`egui::text::LayoutJob`] - which carries font ids rather than the family
/// name [`bold`] sets on a [`RichText`].
pub(crate) fn bold_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, bold_family())
}
