//! Translation catalog: the Fluent message bundles, the active-language loader,
//! and the [`tr!`] macro the UI uses in place of string literals.
//!
//! # How it fits together
//!
//! - Strings live in `i18n/<lang>/incline_design.ftl` — the file is named after
//!   the cargo package (`CARGO_PKG_NAME`), so renaming the package means renaming
//!   these files. **English (`i18n/en`) is canonical** — every message id and
//!   every argument the code uses is checked against it at compile time by
//!   [`i18n_embed_fl::fl!`]. Other languages may be incomplete; a missing
//!   message falls back to English.
//! - The `.ftl` files are embedded in the binary at build time (`rust-embed`), so
//!   nothing is read from disk and the wasm build needs no extra bundling step —
//!   same model as the fonts in [`crate::ui::fonts`].
//! - The active language is a [`LanguageChoice`] persisted in the config (see
//!   [`crate::app::io::Config`]) and installed on [`LOADER`] by
//!   [`select_language`]. There is no "follow the system" state: the OS locale
//!   is consulted once, by [`LanguageChoice::default`], to seed the config on
//!   first launch, and from then on the stored choice is what runs.
//! - Switching is **live**. egui rebuilds the interface from scratch every
//!   frame, so re-selecting the language and asking for a redraw is all it takes
//!   — nothing caches a translated string across frames, and no restart is
//!   needed. The picker lives in the status bar
//!   ([`crate::ui::elements::status_bar`]).
//!
//! # Adding a string
//!
//! Add `my-message = English text` to `i18n/en/incline_design.ftl` (and ideally
//! the other languages), then call `tr!("my-message")` where the literal was.
//! For interpolated values: `greeting = Hello, { $name }` → `tr!("greeting", name = who)`.
//! `cargo check` enforces both: `tr!` fails the build on an unknown id or a
//! missing argument.

use std::sync::LazyLock;

use i18n_embed::{
    LanguageLoader,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use unic_langid::{LanguageIdentifier, langid};

/// The embedded `i18n/` tree (`<lang>/incline_design.ftl`).
#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

/// Process-wide Fluent loader. Loads the English fallback bundle on first use;
/// [`select_language`] then installs the language the user is running.
pub(crate) static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader.load_fallback_language(&Localizations).expect("i18n: the English fallback bundle must load");
    // Fluent isolates interpolated values with Unicode FSI/PDI control marks by
    // default; egui renders those as tofu, so switch the isolation off.
    loader.set_use_isolating(false);
    loader
});

/// UI language, as stored in `config.toml`.
///
/// `Copy` so it can live in `PreferencesDraft`. Serialises to a short tag
/// (`"en"`, `"es"`, and so on). Every value names a bundle in `i18n/`; there is
/// deliberately no "system" value — see [`Self::default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LanguageChoice {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "zh")]
    ChineseSimplified,
    #[serde(rename = "id")]
    Indonesian,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "ar")]
    Arabic,
    #[serde(rename = "fa")]
    Farsi,
    #[serde(rename = "hi")]
    Hindi,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "vi")]
    Vietnamese,
    #[serde(rename = "mn")]
    Mongolian,
    #[serde(rename = "sw")]
    Swahili,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "uk")]
    Ukrainian,
}

impl Default for LanguageChoice {
    /// The OS locale, or English when it names no language we bundle.
    ///
    /// This is what a missing `language` key in the config resolves to, so the
    /// system locale decides the language on the first launch and the stored
    /// choice decides it on every launch after that.
    fn default() -> Self {
        Self::from_system_locale()
    }
}

impl LanguageChoice {
    /// Every value, in the order the status bar's picker lists them.
    pub(crate) const ALL: [Self; 20] = [
        Self::English,
        Self::Spanish,
        Self::Portuguese,
        Self::French,
        Self::ChineseSimplified,
        Self::Indonesian,
        Self::Russian,
        Self::Arabic,
        Self::Farsi,
        Self::Hindi,
        Self::German,
        Self::Italian,
        Self::Polish,
        Self::Turkish,
        Self::Vietnamese,
        Self::Mongolian,
        Self::Swahili,
        Self::Japanese,
        Self::Korean,
        Self::Ukrainian,
    ];

    /// How this language names itself, in its own script. Never translated:
    /// someone who cannot read the running language has to be able to find
    /// their own in the list.
    pub(crate) fn endonym(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
            Self::Portuguese => "Português",
            Self::French => "Français",
            Self::ChineseSimplified => "简体中文",
            Self::Indonesian => "Bahasa Indonesia",
            Self::Russian => "Русский",
            Self::Arabic => "العربية",
            Self::Farsi => "فارسی",
            Self::Hindi => "हिन्दी",
            Self::German => "Deutsch",
            Self::Italian => "Italiano",
            Self::Polish => "Polski",
            Self::Turkish => "Türkçe",
            Self::Vietnamese => "Tiếng Việt",
            Self::Mongolian => "Монгол",
            Self::Swahili => "Kiswahili",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::Ukrainian => "Українська",
        }
    }

    /// The bundle this choice selects.
    fn lang_id(self) -> LanguageIdentifier {
        match self {
            Self::English => langid!("en"),
            Self::Spanish => langid!("es"),
            Self::Portuguese => langid!("pt"),
            Self::French => langid!("fr"),
            Self::ChineseSimplified => langid!("zh"),
            Self::Indonesian => langid!("id"),
            Self::Russian => langid!("ru"),
            Self::Arabic => langid!("ar"),
            Self::Farsi => langid!("fa"),
            Self::Hindi => langid!("hi"),
            Self::German => langid!("de"),
            Self::Italian => langid!("it"),
            Self::Polish => langid!("pl"),
            Self::Turkish => langid!("tr"),
            Self::Vietnamese => langid!("vi"),
            Self::Mongolian => langid!("mn"),
            Self::Swahili => langid!("sw"),
            Self::Japanese => langid!("ja"),
            Self::Korean => langid!("ko"),
            Self::Ukrainian => langid!("uk"),
        }
    }

    /// The first bundled language the OS asks for, ignoring region and script:
    /// `ru-RU` and `ru` both pick Russian.
    fn from_system_locale() -> Self {
        for requested in system_languages() {
            if let Some(choice) = Self::ALL.into_iter().find(|choice| choice.lang_id().language == requested.language) {
                return choice;
            }
        }
        Self::English
    }
}

/// Install `choice` on [`LOADER`].
///
/// Called at startup with the config's language and again on every switch from
/// the status bar's picker. Safe to call mid-session: the caller only has to
/// ask for a redraw, since the next frame rebuilds every string.
pub(crate) fn select_language(choice: LanguageChoice) {
    if let Err(error) = i18n_embed::select(&*LOADER, &Localizations, &[choice.lang_id()]) {
        // Not fatal: the English fallback bundle is already loaded.
        log::warn!("{}", crate::i18n::tr!("i18n-could-not-select-language-error", error = error.to_string()));
    }
    log::info!(
        "{}",
        crate::i18n::tr!(
            "i18n-active-language-language-bundled-bun",
            language = LOADER.current_language().to_string(),
            bundled = (format!("{:?}", available_languages())).to_string()
        )
    );
}

/// Available `<lang>` bundles, for logging / diagnostics.
pub(crate) fn available_languages() -> Vec<LanguageIdentifier> {
    LOADER.available_languages(&Localizations).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn system_languages() -> Vec<LanguageIdentifier> {
    sys_locale::get_locales().filter_map(|locale| parse_system_language(&locale)).collect()
}

/// Convert platform locale spellings such as `en_US.UTF-8` into Unicode
/// language identifiers. `C` and `POSIX` deliberately mean no language; the
/// caller will use English when they are the only configured locales.
#[cfg(not(target_arch = "wasm32"))]
fn parse_system_language(locale: &str) -> Option<LanguageIdentifier> {
    let locale = locale.split(['.', '@']).next().unwrap_or(locale);
    if locale.eq_ignore_ascii_case("c") || locale.eq_ignore_ascii_case("posix") {
        return None;
    }
    locale.replace('_', "-").parse().ok()
}

#[cfg(target_arch = "wasm32")]
fn system_languages() -> Vec<LanguageIdentifier> {
    // The browser locale is not negotiated yet, so the web build's first launch
    // lands on English; the picker still switches it from there.
    Vec::new()
}

/// Translate a message id, with optional named Fluent arguments. A thin wrapper
/// over [`i18n_embed_fl::fl!`], which checks the id and its arguments against
/// `i18n/en/incline_design.ftl` at compile time.
///
/// ```ignore
/// tr!("menu-file");
/// tr!("tri-count-polylines", count = n.to_string());
/// ```
macro_rules! tr {
    ($($tail:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $($tail)*)
    };
}
pub(crate) use tr;
