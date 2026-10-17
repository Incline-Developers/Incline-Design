//! Bundled fonts shared by the UI, document text, and plot renderer.

use std::sync::Arc;

use cosmic_text::{FontSystem, fontdb};

pub(crate) const NOTO_SANS: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSans-VF.ttf");
pub(crate) const NOTO_SANS_MONO: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSansMono-VF.ttf");
pub(crate) const NOTO_SANS_ARABIC: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSansArabic-VF.ttf");
pub(crate) const NOTO_SANS_DEVANAGARI: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSansDevanagari-VF.ttf");
pub(crate) const NOTO_SANS_SYMBOLS: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSansSymbols-VF.ttf");
pub(crate) const NOTO_SANS_SYMBOLS_2: &[u8] = include_bytes!("../res/fonts/noto-sans/NotoSansSymbols2-Regular.ttf");

/// Noto Sans SC covers Han ideographs for both Chinese and Japanese, and kana;
/// Noto Sans KR covers Hangul. They are 27 MB between them, so native builds
/// embed them while the web build fetches them the first time CJK text shows
/// up (see [`request_cjk_fonts_for`]).
#[derive(Clone, Copy)]
pub(crate) struct CjkFonts {
    pub(crate) sc: &'static [u8],
    pub(crate) kr: &'static [u8],
}

/// The CJK faces, if this build has them yet: always on native, once fetched
/// on the web.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn cjk_fonts() -> Option<CjkFonts> {
    Some(CjkFonts {
        sc: include_bytes!("../res/fonts/noto-sans/NotoSansSC-VF.ttf"),
        kr: include_bytes!("../res/fonts/noto-sans/NotoSansKR-VF.ttf"),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) use web::{cjk_fonts, install_cjk_font_loader};

/// Start fetching the CJK faces if `text` needs them and they are missing.
/// Native builds embed them, so this only does anything on the web.
pub(crate) fn request_cjk_fonts_for(text: &str) {
    #[cfg(target_arch = "wasm32")]
    if web::cjk_fonts().is_none() && text.chars().any(is_cjk) {
        web::request_cjk_fonts();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

/// Whether `c` needs one of the [`CjkFonts`]: Han, kana, Hangul, and the CJK
/// punctuation and full-width forms that come with them.
#[cfg(target_arch = "wasm32")]
fn is_cjk(c: char) -> bool {
    matches!(
        u32::from(c),
        0x1100..=0x11FF // Hangul Jamo
            | 0x2E80..=0x2FDF // CJK and Kangxi radicals
            | 0x3000..=0x4DBF // CJK punctuation, kana, Bopomofo, Hangul compatibility jamo, enclosed, Extension A
            | 0x4E00..=0x9FFF // CJK Unified Ideographs
            | 0xA960..=0xA97F // Hangul Jamo Extended-A
            | 0xAC00..=0xD7FF // Hangul syllables, Jamo Extended-B
            | 0xF900..=0xFAFF // CJK compatibility ideographs
            | 0xFE30..=0xFE4F // CJK compatibility forms
            | 0xFF00..=0xFFEF // half-width and full-width forms
            | 0x20000..=0x3FFFF // Extensions B onwards
    )
}

/// Create a platform-independent text system with every script supported by
/// Incline. Keep this list in sync with the egui families in `ui::fonts`.
///
/// On the web the CJK faces join once fetched; build a new system then.
pub(crate) fn cosmic_text_font_system() -> FontSystem {
    let cjk = cjk_fonts().map(|fonts| [fonts.sc, fonts.kr]).into_iter().flatten();
    let sources = [NOTO_SANS, NOTO_SANS_MONO, NOTO_SANS_ARABIC, NOTO_SANS_DEVANAGARI, NOTO_SANS_SYMBOLS, NOTO_SANS_SYMBOLS_2]
        .into_iter()
        .chain(cjk)
        .map(|font| fontdb::Source::Binary(Arc::new(font)));
    let mut font_system = FontSystem::new_with_fonts(sources);

    // cosmic-text currently defaults its generic sans-serif family to Open
    // Sans. Incline intentionally bundles Noto Sans instead.
    font_system.db_mut().set_sans_serif_family("Noto Sans");
    font_system
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        cell::{Cell, RefCell},
        sync::OnceLock,
    };

    use wasm_bindgen::JsCast as _;
    use winit::event_loop::EventLoopProxy;

    use super::CjkFonts;
    use crate::app::AppEvent;

    /// Where Trunk copies the faces (`web/index.html`), relative to the page.
    const SC_URL: &str = "fonts/NotoSansSC-VF.ttf";
    const KR_URL: &str = "fonts/NotoSansKR-VF.ttf";

    static FONTS: OnceLock<CjkFonts> = OnceLock::new();

    thread_local! {
        // Fetches run on the page thread, and only it gets the proxy:
        // a request from a worker is left to the page thread to make.
        static PROXY: RefCell<Option<EventLoopProxy<AppEvent>>> = const { RefCell::new(None) };
        static STARTED: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn cjk_fonts() -> Option<CjkFonts> {
        FONTS.get().copied()
    }

    /// Let [`request_cjk_fonts`] fetch from this thread, and announce arrival
    /// as [`AppEvent::CjkFontsLoaded`].
    pub(crate) fn install_cjk_font_loader(proxy: EventLoopProxy<AppEvent>) {
        PROXY.with_borrow_mut(|slot| *slot = Some(proxy));
    }

    pub(super) fn request_cjk_fonts() {
        let Some(proxy) = PROXY.with_borrow(Clone::clone) else {
            return;
        };
        if STARTED.replace(true) {
            return;
        }
        // `fetch` starts each request as it is called, so both download at once.
        let requests = [SC_URL, KR_URL].map(|url| (url, web_sys::window().map(|window| window.fetch_with_str(url))));
        wasm_bindgen_futures::spawn_local(async move {
            let mut faces = Vec::with_capacity(requests.len());
            for (url, request) in requests {
                match read(url, request).await {
                    Ok(face) => faces.push(face),
                    Err(error) => {
                        crate::userspace_warn!("{}", crate::i18n::tr!("text-cjk-fonts-fetch-failed", error = error));
                        return;
                    }
                }
            }
            let [sc, kr] = <[Vec<u8>; 2]>::try_from(faces).expect("one face per request");
            // The faces live as long as the page; leaking them lets egui and
            // cosmic-text share one copy as `&'static`.
            let fonts = CjkFonts {
                sc: Vec::leak(sc),
                kr: Vec::leak(kr),
            };
            if FONTS.set(fonts).is_ok() {
                let _ = proxy.send_event(AppEvent::CjkFontsLoaded);
            }
        });
    }

    async fn read(url: &str, request: Option<js_sys::Promise>) -> Result<Vec<u8>, String> {
        let request = request.ok_or_else(|| format!("{url}: no window"))?;
        let response = wasm_bindgen_futures::JsFuture::from(request).await.map_err(|error| format!("{url}: {error:?}"))?;
        let response: web_sys::Response = response.dyn_into().map_err(|_| format!("{url}: not a response"))?;
        if !response.ok() {
            return Err(format!("{url}: HTTP {}", response.status()));
        }
        let buffer = response.array_buffer().map_err(|error| format!("{url}: {error:?}"))?;
        let buffer = wasm_bindgen_futures::JsFuture::from(buffer).await.map_err(|error| format!("{url}: {error:?}"))?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    }
}
