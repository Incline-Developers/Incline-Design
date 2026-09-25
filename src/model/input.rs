/// A user-selected source independent of native filesystem paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceRef {
    pub(crate) name: String,
    pub(crate) byte_len: usize,
}

/// File contents passed to byte-oriented importers on both native and web.
#[derive(Clone, Debug)]
pub(crate) struct InputFile {
    pub(crate) source: SourceRef,
    pub(crate) bytes: Vec<u8>,
    pub(crate) reservation: Option<crate::app::memory::MemoryReservation>,
}

/// One `[start, end)` byte range of `file`, read through a `Blob` slice:
/// the ends a file's identity is hashed from, a hole's runs, a pump's
/// chunks.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_browser_range(file: &web_sys::File, start: u64, end: u64) -> Result<Vec<u8>, String> {
    let blob = file
        .slice_with_f64_and_f64(start as f64, end as f64)
        .map_err(|error| crate::i18n::tr_format!(literal = "could not slice %name%: %error%", name = file.name(), error = format!("{error:?}")))?;
    let value = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
        .await
        .map_err(|error| crate::i18n::tr_format!(literal = "could not read %name%: %error%", name = file.name(), error = format!("{error:?}")))?;
    let array = js_sys::Uint8Array::new(&value);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    Ok(bytes)
}

/// A file read whole, in chunks so no single `Blob` slice holds more than
/// [`crate::app::memory::FILE_CHUNK_BYTES`]. Refused past the 2 GiB import
/// limit, same as a picked file handle.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_browser_file(file: web_sys::File) -> Result<InputFile, String> {
    let size = file.size();
    if !size.is_finite() || size < 0.0 || size > crate::app::memory::INPUT_FILE_LIMIT as f64 {
        return Err(format!("{} is too large; browser imports are limited to 2 GiB", file.name()));
    }
    let len = size as usize;
    let reservation = crate::app::memory::reserve(len, &format!("input '{}'", file.name()))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|error| format!("could not allocate {len} bytes for {}: {error}", file.name()))?;

    // Copied straight into the reserved buffer: a chunk read on its own
    // would hold a second copy of up to 64 MiB for a moment.
    let mut start = 0usize;
    while start < len {
        let end = start.saturating_add(crate::app::memory::FILE_CHUNK_BYTES).min(len);
        let blob = file
            .slice_with_f64_and_f64(start as f64, end as f64)
            .map_err(|error| format!("could not slice {}: {error:?}", file.name()))?;
        let value = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
            .await
            .map_err(|error| format!("could not read {}: {error:?}", file.name()))?;
        let chunk = js_sys::Uint8Array::new(&value);
        let old_len = bytes.len();
        bytes.resize(old_len + chunk.length() as usize, 0);
        chunk.copy_to(&mut bytes[old_len..]);
        start = end;
    }
    Ok(InputFile {
        source: SourceRef { name: file.name(), byte_len: len },
        bytes,
        reservation: Some(reservation),
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_browser_handle(handle: rfd::FileHandle) -> Result<InputFile, String> {
    read_browser_file(handle.inner().clone()).await
}

/// Only a file's head, up to `max_len` bytes: enough for a mapping preview
/// without holding a bundle's geophysics file, which may run to gigabytes.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_browser_head(file: &web_sys::File, max_len: usize) -> Result<InputFile, String> {
    let size = file.size();
    if !size.is_finite() || size < 0.0 {
        return Err(crate::i18n::tr_format!(literal = "%name% has no readable size", name = file.name()));
    }
    let head_len = (max_len as f64).min(size).max(0.0) as u64;
    let bytes = read_browser_range(file, 0, head_len).await?;
    let reservation = crate::app::memory::reserve(bytes.len(), &format!("input head '{}'", file.name()))?;
    Ok(InputFile {
        source: SourceRef {
            name: file.name(),
            byte_len: bytes.len(),
        },
        bytes,
        reservation: Some(reservation),
    })
}
