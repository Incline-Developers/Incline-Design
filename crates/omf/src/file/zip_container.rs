use std::{
    collections::HashMap,
    io::{Read, Seek, Write},
    sync::{Mutex, OnceLock},
};

use zip::{
    read::{ZipArchive, ZipFile},
    write::{FullFileOptions, ZipWriter},
};

use crate::{FORMAT_NAME, error::Error};

use super::{ReadAt, SubFile};

pub(crate) const INDEX_NAME: &str = "index.json.gz";
pub(crate) const PARQUET_EXT: &str = ".parquet";
pub(crate) const PNG_EXT: &str = ".png";
pub(crate) const JPEG_EXT: &str = ".jpg";

pub(crate) enum FileType {
    Index,
    Parquet,
    Png,
    Jpeg,
}

pub(crate) struct Builder<W: Write + Seek> {
    zip_writer: ZipWriter<W>,
    next_id: u64,
    filenames: Vec<String>,
}

impl<W: Write + Seek> Builder<W> {
    pub fn new(write: W) -> Result<Self, Error> {
        Ok(Self {
            zip_writer: ZipWriter::new(write),
            next_id: 1,
            filenames: Vec::new(),
        })
    }

    fn id(&mut self) -> u64 {
        let i = self.next_id;
        self.next_id += 1;
        i
    }

    pub fn open(&mut self, file_type: FileType) -> Result<SubFileWrite<'_, W>, Error> {
        let name = match file_type {
            FileType::Index => INDEX_NAME.to_owned(),
            FileType::Parquet => format!("{}{PARQUET_EXT}", self.id()),
            FileType::Png => format!("{}{PNG_EXT}", self.id()),
            FileType::Jpeg => format!("{}{JPEG_EXT}", self.id()),
        };
        self.zip_writer.start_file(
            name.clone(),
            FullFileOptions::default()
                .large_file(true)
                .compression_method(zip::CompressionMethod::Stored),
        )?;
        self.filenames.push(name.clone());
        Ok(SubFileWrite {
            name,
            inner: &mut self.zip_writer,
        })
    }

    pub fn filenames(&self) -> impl Iterator<Item = &str> {
        self.filenames.iter().map(|s| &**s)
    }

    pub fn finish(mut self, major: u32, minor: u32, pre_release: Option<&str>) -> Result<W, Error> {
        use std::fmt::Write;
        let mut comment = format!("{FORMAT_NAME} {major}.{minor}");
        if let Some(pre) = pre_release {
            _ = write!(&mut comment, "-{pre}");
        }
        self.zip_writer.set_comment(comment)?;
        Ok(self.zip_writer.finish()?)
    }
}

pub(crate) struct SubFileWrite<'a, W: Write + Seek> {
    name: String,
    inner: &'a mut ZipWriter<W>,
}

impl<W: Write + Seek> SubFileWrite<'_, W> {
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<W: Write + Seek> std::io::Write for SubFileWrite<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FileSpan {
    pub offset: u64,
    pub size: u64,
}

impl<'a, R: Read> TryFrom<ZipFile<'a, R>> for FileSpan {
    type Error = Error;

    fn try_from(f: ZipFile<'a, R>) -> Result<Self, Error> {
        Ok(Self {
            offset: f
                .data_start()
                .ok_or_else(|| Error::ZipError("member data offset is unavailable".into()))?,
            size: f.compressed_size(),
        })
    }
}

pub(crate) struct Archive<R> {
    file: SubFile<R>,
    /// The central directory, consulted again as members are first opened.
    zip: Mutex<ZipArchive<SubFile<R>>>,
    /// Each member's index in `zip`, and its span once something has opened it.
    ///
    /// Locating a member's data means reading its local header, one small read per
    /// member. Deferring that to the first open keeps opening an archive with many
    /// thousands of members - one per small element array - from reading them all when
    /// only a few will be loaded.
    members: HashMap<String, (usize, OnceLock<FileSpan>)>,
    version: [u32; 2],
    pre_release: Option<String>,
}

impl<R: ReadAt> Archive<R> {
    pub fn new(file: SubFile<R>) -> Result<Self, Error> {
        let mut zip_archive = ZipArchive::new(file.sub_file(0, file.len())?)?;
        let members: HashMap<_, _> = zip_archive
            .file_names()
            .map(|name| {
                let index = zip_archive.index_for_name(name).expect("listed member");
                (name.to_owned(), (index, OnceLock::new()))
            })
            .collect();
        let Some((index, _)) = members.get(INDEX_NAME) else {
            return Err(Error::ZipMemberMissing(INDEX_NAME.to_owned()));
        };
        // The index is always read, so check it up front as before.
        let index_span = OnceLock::new();
        _ = index_span.set(member_span(&mut zip_archive, *index)?);
        let Some((version, pre_release)) = get_version(zip_archive.comment()) else {
            return Err(Error::NotOmf(
                String::from_utf8_lossy(zip_archive.comment()).into_owned(),
            ));
        };
        let mut members = members;
        if let Some(entry) = members.get_mut(INDEX_NAME) {
            entry.1 = index_span;
        }
        Ok(Self {
            file,
            zip: Mutex::new(zip_archive),
            members,
            version,
            pre_release,
        })
    }

    pub fn version(&self) -> ([u32; 2], Option<&str>) {
        (self.version, self.pre_release.as_deref())
    }

    pub fn filenames(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(|s| &**s)
    }

    pub fn span(&self, name: &str) -> Result<FileSpan, Error> {
        let (index, span) = self
            .members
            .get(name)
            .ok_or_else(|| Error::ZipMemberMissing(name.to_owned()))?;
        if let Some(span) = span.get() {
            return Ok(*span);
        }
        let mut zip = self
            .zip
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let found = member_span(&mut zip, *index)?;
        Ok(*span.get_or_init(|| found))
    }

    pub fn open(&self, name: &str) -> Result<SubFile<R>, Error> {
        let span = self.span(name)?;
        Ok(self.file.sub_file(span.offset, span.size)?)
    }
}

/// Where a member's data lies, checking that it is stored uncompressed.
fn member_span<R: Read + Seek>(zip: &mut ZipArchive<R>, index: usize) -> Result<FileSpan, Error> {
    let f = zip.by_index_raw(index)?;
    if f.compression() != zip::CompressionMethod::Stored {
        return Err(Error::ZipError("members may not be compressed".into()));
    }
    FileSpan::try_from(f)
}

fn get_version(comment_bytes: &[u8]) -> Option<([u32; 2], Option<String>)> {
    let comment = std::str::from_utf8(comment_bytes).ok()?;
    let mut dash_parts = comment
        .strip_prefix(FORMAT_NAME)?
        .strip_prefix(' ')?
        .split('-');
    let main = dash_parts.next()?;
    let pre_release = dash_parts.next().map(ToOwned::to_owned);
    let mut version_parts = main.split('.');
    let major = version_parts.next()?.parse().ok()?;
    let minor = version_parts.next()?.parse().ok()?;
    if version_parts.next().is_some() {
        return None;
    }
    Some(([major, minor], pre_release))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(
            get_version("Open Mining Format 2.0".as_bytes()),
            Some(([2, 0], None))
        );
        assert_eq!(
            get_version("Open Mining Format 2.0-alpha.1".as_bytes()),
            Some(([2, 0], Some("alpha.1".to_string())))
        );
        assert_eq!(get_version("Something else 1.0".as_bytes()), None);
        assert_eq!(get_version(b"Something not UTF-8 \xff"), None);
    }
}
