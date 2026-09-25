//! Record reading shared by the mapped CSV importers.

use csv::{ReaderBuilder, StringRecord};

/// Visit each record with the 1-based line it starts on, stopping after
/// `limit` records. Rows may differ in length, blank lines are skipped, and a
/// UTF-8 byte-order mark is ignored.
pub(crate) fn for_each_record<E: From<csv::Error>>(bytes: &[u8], limit: Option<usize>, mut visit: impl FnMut(usize, &StringRecord) -> Result<(), E>) -> Result<(), E> {
    let mut reader = ReaderBuilder::new().has_headers(false).flexible(true).from_reader(bytes);
    let mut record = StringRecord::new();
    let mut read = 0;
    // The reader's own line count drifts across CRLF and blank lines, and a
    // record's byte position sits on the terminator before it, so count
    // newlines up to its first byte instead.
    let (mut line, mut counted) = (1, 0);
    while limit.is_none_or(|limit| read < limit) && reader.read_record(&mut record)? {
        read += 1;
        let mut start = record.position().map_or(counted, |position| position.byte() as usize).clamp(counted, bytes.len());
        while bytes.get(start).is_some_and(|byte| matches!(byte, b'\r' | b'\n')) {
            start += 1;
        }
        line += bytes[counted..start].iter().filter(|&&byte| byte == b'\n').count();
        counted = start;
        visit(line, &record)?;
    }
    Ok(())
}

pub(crate) fn owned_fields(record: &StringRecord) -> Vec<String> {
    record.iter().map(str::to_owned).collect()
}
