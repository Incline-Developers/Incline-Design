//! One block-model column, held in a fraction of the memory of a `Vec<f64>`.
//!
//! Survey-sized models run to hundreds of millions of rows, and most of what
//! they hold is repetitive: category codes, a grade written to four decimals,
//! a coordinate with a few hundred distinct values in any stretch of the file.
//! Each run of [`SEGMENT_ROWS`] rows is therefore stored in whichever form is
//! smallest for its own values, and every form gives back exactly the bits it
//! was given (a `NaN` of any payload comes back as [`f64::NAN`]), so nothing
//! computed from a column can differ from what the raw numbers would give.
//!
//! Rows are read one at a time with [`BlockColumn::get`]; a column is never
//! exposed as a slice, so nothing can come to depend on it being one.

use rayon::prelude::*;

const SEGMENT_SHIFT: usize = 16;
pub(crate) const SEGMENT_ROWS: usize = 1 << SEGMENT_SHIFT;
const SEGMENT_MASK: usize = SEGMENT_ROWS - 1;
/// The most decimal places a [`Segment::Decimal16`] or `Decimal32` is tried at.
const MAX_DECIMALS: usize = 12;
const POWERS_OF_TEN: [f64; MAX_DECIMALS + 1] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12];
/// Integers beyond this are not all representable, so scaling stops being exact.
const MAX_SCALED: f64 = 9_007_199_254_740_992.0;
const NAN_16: u16 = u16::MAX;
const NAN_32: u32 = u32::MAX;

#[derive(Clone, Debug)]
enum Segment {
    /// Every row holds the same value.
    Constant(f64),
    /// Up to 256 distinct values.
    Table8 {
        table: Box<[f64]>,
        codes: Box<[u8]>,
    },
    /// Up to 65,536 distinct values, when that beats the decimal forms.
    Table16 {
        table: Box<[f64]>,
        codes: Box<[u16]>,
    },
    /// `(base + raw) / divisor`; a CSV's `12.3456` is exactly `123456 / 1e4`,
    /// as both are correctly rounded from the same real number.
    Decimal16 {
        base: i64,
        divisor: f64,
        raw: Box<[u16]>,
    },
    Decimal32 {
        base: i64,
        divisor: f64,
        raw: Box<[u32]>,
    },
    Raw(Box<[f64]>),
}

impl Segment {
    #[inline]
    fn get(&self, row: usize) -> f64 {
        match self {
            Self::Constant(value) => *value,
            Self::Table8 { table, codes } => table[usize::from(codes[row])],
            Self::Table16 { table, codes } => table[usize::from(codes[row])],
            Self::Decimal16 { base, divisor, raw } => match raw[row] {
                NAN_16 => f64::NAN,
                raw => (base + i64::from(raw)) as f64 / divisor,
            },
            Self::Decimal32 { base, divisor, raw } => match raw[row] {
                NAN_32 => f64::NAN,
                raw => (base + i64::from(raw)) as f64 / divisor,
            },
            Self::Raw(values) => values[row],
        }
    }

    /// Every row of the segment into `out`, as [`Self::get`] gives each.
    fn decode(&self, out: &mut [f64]) {
        match self {
            Self::Constant(value) => out.fill(*value),
            Self::Table8 { table, codes } => out.iter_mut().zip(codes).for_each(|(out, code)| *out = table[usize::from(*code)]),
            Self::Table16 { table, codes } => out.iter_mut().zip(codes).for_each(|(out, code)| *out = table[usize::from(*code)]),
            Self::Decimal16 { base, divisor, raw } => out.iter_mut().zip(raw).for_each(|(out, raw)| {
                *out = match *raw {
                    NAN_16 => f64::NAN,
                    raw => (base + i64::from(raw)) as f64 / divisor,
                }
            }),
            Self::Decimal32 { base, divisor, raw } => out.iter_mut().zip(raw).for_each(|(out, raw)| {
                *out = match *raw {
                    NAN_32 => f64::NAN,
                    raw => (base + i64::from(raw)) as f64 / divisor,
                }
            }),
            Self::Raw(values) => out.copy_from_slice(&values[..out.len()]),
        }
    }

    fn heap_bytes(&self) -> usize {
        match self {
            Self::Constant(_) => 0,
            Self::Table8 { table, codes } => size_of_val(&**table) + size_of_val(&**codes),
            Self::Table16 { table, codes } => size_of_val(&**table) + size_of_val(&**codes),
            Self::Decimal16 { raw, .. } => size_of_val(&**raw),
            Self::Decimal32 { raw, .. } => size_of_val(&**raw),
            Self::Raw(values) => size_of_val(&**values),
        }
    }

    fn encode(values: &[f64]) -> Self {
        let first = canonical(values[0]);
        if values.iter().all(|value| canonical(*value) == first) {
            return Self::Constant(f64::from_bits(first));
        }
        if let Some(segment) = Self::small_table(values) {
            return segment;
        }
        let mut scaled = Vec::new();
        let decimal = decimal_form(values, &mut scaled);
        let decimal_bytes = decimal.as_ref().map_or(size_of_val(values), |form| form.bytes(values.len()));
        // Two bytes a row is already what the larger table would cost before
        // its table, so it only ever has the wider forms to beat.
        if decimal_bytes > values.len() * 2
            && let Some(segment) = Self::large_table(values, decimal_bytes)
        {
            return segment;
        }
        match decimal {
            Some(form) => form.encode(values, &scaled),
            None => Self::Raw(values.into()),
        }
    }

    fn small_table(values: &[f64]) -> Option<Self> {
        // Open addressing over four times the table's limit, so a segment
        // that has too many values finds out cheaply.
        const SLOTS: usize = 1024;
        let mut slots = [u16::MAX; SLOTS];
        let mut table: Vec<u64> = Vec::with_capacity(256);
        let mut codes = Vec::with_capacity(values.len());
        for value in values {
            let bits = canonical(*value);
            let mut slot = (bits.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54) as usize;
            let code = loop {
                match slots[slot] {
                    u16::MAX => {
                        if table.len() == 256 {
                            return None;
                        }
                        slots[slot] = table.len() as u16;
                        table.push(bits);
                        break table.len() - 1;
                    }
                    code if table[usize::from(code)] == bits => break usize::from(code),
                    _ => slot = (slot + 1) % SLOTS,
                }
            };
            codes.push(code as u8);
        }
        Some(Self::Table8 {
            table: table.into_iter().map(f64::from_bits).collect(),
            codes: codes.into_boxed_slice(),
        })
    }

    fn large_table(values: &[f64], to_beat: usize) -> Option<Self> {
        // Open addressing over twice the most rows a segment holds, as
        // `small_table` does: sorting the segment to find its distinct values
        // was most of what encoding a large model cost.
        const SLOTS: usize = 2 * SEGMENT_ROWS;
        let mut slots = vec![u32::MAX; SLOTS];
        let mut table: Vec<u64> = Vec::new();
        let mut codes: Vec<u16> = Vec::with_capacity(values.len());
        for value in values {
            let bits = canonical(*value);
            let mut slot = (bits.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - (SEGMENT_SHIFT + 1))) as usize;
            let code = loop {
                match slots[slot] {
                    u32::MAX => {
                        if table.len() > usize::from(u16::MAX) {
                            return None;
                        }
                        slots[slot] = table.len() as u32;
                        table.push(bits);
                        break table.len() - 1;
                    }
                    code if table[code as usize] == bits => break code as usize,
                    _ => slot = (slot + 1) % SLOTS,
                }
            };
            codes.push(code as u16);
        }
        if table.len() * size_of::<f64>() + values.len() * 2 >= to_beat {
            return None;
        }
        Some(Self::Table16 {
            table: table.into_iter().map(f64::from_bits).collect(),
            codes: codes.into_boxed_slice(),
        })
    }
}

/// A value's bits, with every `NaN` the same one so they compare equal.
#[inline]
fn canonical(value: f64) -> u64 {
    if value.is_nan() { f64::NAN.to_bits() } else { value.to_bits() }
}

struct DecimalForm {
    decimals: usize,
    min: i64,
    range: u64,
}

impl DecimalForm {
    fn bytes(&self, rows: usize) -> usize {
        rows * if self.range < u64::from(NAN_16) { 2 } else { 4 }
    }

    /// `values` in this form, from the integers [`decimal_form`] scaled them to.
    fn encode(&self, values: &[f64], scaled: &[i64]) -> Segment {
        let divisor = POWERS_OF_TEN[self.decimals];
        let offset = |scaled: i64| (scaled - self.min) as u64;
        let rows = values.iter().zip(scaled);
        if self.range < u64::from(NAN_16) {
            Segment::Decimal16 {
                base: self.min,
                divisor,
                raw: rows.map(|(value, scaled)| if value.is_nan() { NAN_16 } else { offset(*scaled) as u16 }).collect(),
            }
        } else {
            Segment::Decimal32 {
                base: self.min,
                divisor,
                raw: rows.map(|(value, scaled)| if value.is_nan() { NAN_32 } else { offset(*scaled) as u32 }).collect(),
            }
        }
    }
}

/// `value` rounded half away from zero, as [`f64::round`] rounds it, when its
/// magnitude is below [`MAX_SCALED`], where every integer is exact; `None`
/// otherwise. Without SSE4.1, `round` is a library call, and it ran twice for
/// every value of a decimal segment.
#[inline]
fn round_scaled(value: f64) -> Option<i64> {
    if value.abs().partial_cmp(&MAX_SCALED).is_none_or(std::cmp::Ordering::is_ge) {
        return None;
    }
    let whole = value as i64;
    // Exact: `whole` is `value` with its fraction cut off.
    let rest = value - whole as f64;
    Some(if rest >= 0.5 {
        whole + 1
    } else if rest <= -0.5 {
        whole - 1
    } else {
        whole
    })
}

/// The fewest decimal places that give back every value's exact bits, if any
/// do within a range four bytes a row can hold, with each value scaled to its
/// integer in `scaled` (anything in a `NaN`'s place).
fn decimal_form(values: &[f64], scaled: &mut Vec<i64>) -> Option<DecimalForm> {
    let mut decimals = 0;
    'places: while decimals <= MAX_DECIMALS {
        let divisor = POWERS_OF_TEN[decimals];
        let (mut min, mut max) = (i64::MAX, i64::MIN);
        scaled.clear();
        for value in values {
            if value.is_nan() {
                scaled.push(0);
                continue;
            }
            let whole = round_scaled(value * divisor)?;
            // An integer has no negative zero to give back.
            if whole == 0 && value.is_sign_negative() {
                return None;
            }
            if ((whole as f64) / divisor).to_bits() != value.to_bits() {
                decimals += 1;
                continue 'places;
            }
            min = min.min(whole);
            max = max.max(whole);
            scaled.push(whole);
        }
        if min > max {
            return None;
        }
        let range = max.abs_diff(min);
        return (range < u64::from(NAN_32)).then_some(DecimalForm { decimals, min, range });
    }
    None
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockColumn {
    len: usize,
    segments: Vec<Segment>,
}

impl BlockColumn {
    pub(crate) fn from_values(values: &[f64]) -> Self {
        Self {
            len: values.len(),
            segments: values.par_chunks(SEGMENT_ROWS).map(Segment::encode).collect(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The value of `row`. Panics past the end, as indexing a `Vec` would.
    #[inline]
    pub(crate) fn get(&self, row: usize) -> f64 {
        assert!(row < self.len, "block column has {} rows, asked for {row}", self.len);
        self.segments[row >> SEGMENT_SHIFT].get(row & SEGMENT_MASK)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(|row| self.get(row))
    }

    pub(crate) fn range(&self, start: usize, end: usize) -> Vec<f64> {
        (start..end).map(|row| self.get(row)).collect()
    }

    /// How many runs [`Self::decode_run`] reads the column in: every one
    /// [`SEGMENT_ROWS`] long but the last.
    pub(crate) fn run_count(&self) -> usize {
        self.segments.len()
    }

    /// Run `run`'s values into `out`, which is resized to fit: rows
    /// `run * SEGMENT_ROWS` onward, as [`Self::get`] gives each. A pass over a
    /// whole column reads this way rather than a row at a time.
    pub(crate) fn decode_run(&self, run: usize, out: &mut Vec<f64>) {
        let start = run << SEGMENT_SHIFT;
        out.resize(SEGMENT_ROWS.min(self.len - start), 0.0);
        self.segments[run].decode(out);
    }

    /// Every value, a run at a time and the runs side by side: `map` is given
    /// each run's first row and its values.
    pub(crate) fn par_map_runs<T: Send>(&self, map: impl Fn(usize, &[f64]) -> T + Sync + Send) -> Vec<T> {
        (0..self.run_count())
            .into_par_iter()
            .map_init(Vec::new, |values, run| {
                self.decode_run(run, values);
                map(run << SEGMENT_SHIFT, values)
            })
            .collect()
    }

    /// The rows `rows`, which must ascend, as a column of their own.
    pub(crate) fn select(&self, rows: &[usize]) -> Self {
        let mut values = Vec::with_capacity(rows.len());
        let (mut decoded, mut current) = (Vec::new(), usize::MAX);
        for &row in rows {
            let run = row >> SEGMENT_SHIFT;
            if run != current {
                self.decode_run(run, &mut decoded);
                current = run;
            }
            values.push(decoded[row & SEGMENT_MASK]);
        }
        Self::from_values(&values)
    }

    /// The column as it is held, a segment at a time, for a cache file: each
    /// piece of bytes goes to `put` in order. [`Self::read`] reads it back.
    pub(crate) fn write(&self, put: &mut impl FnMut(&[u8])) {
        put(&(self.len as u64).to_le_bytes());
        put(&(self.segments.len() as u64).to_le_bytes());
        for segment in &self.segments {
            match segment {
                Segment::Constant(value) => {
                    put(&[0]);
                    put(&value.to_le_bytes());
                }
                Segment::Table8 { table, codes } => {
                    put(&[1]);
                    write_values(put, table);
                    put(&(codes.len() as u64).to_le_bytes());
                    put(codes);
                }
                Segment::Table16 { table, codes } => {
                    put(&[2]);
                    write_values(put, table);
                    put(&(codes.len() as u64).to_le_bytes());
                    put(&codes.iter().flat_map(|code| code.to_le_bytes()).collect::<Vec<u8>>());
                }
                Segment::Decimal16 { base, divisor, raw } => {
                    put(&[3]);
                    put(&base.to_le_bytes());
                    put(&divisor.to_le_bytes());
                    put(&(raw.len() as u64).to_le_bytes());
                    put(&raw.iter().flat_map(|raw| raw.to_le_bytes()).collect::<Vec<u8>>());
                }
                Segment::Decimal32 { base, divisor, raw } => {
                    put(&[4]);
                    put(&base.to_le_bytes());
                    put(&divisor.to_le_bytes());
                    put(&(raw.len() as u64).to_le_bytes());
                    put(&raw.iter().flat_map(|raw| raw.to_le_bytes()).collect::<Vec<u8>>());
                }
                Segment::Raw(values) => {
                    put(&[5]);
                    write_values(put, values);
                }
            }
        }
    }

    /// A column [`Self::write`] wrote, from `take`, which hands back the next
    /// `n` bytes or `None` when fewer are left. `None` for anything that is
    /// not a whole, well-formed column: every segment is a full one but the
    /// last, and every table code names a table entry.
    pub(crate) fn read(take: &mut impl FnMut(usize) -> Option<Vec<u8>>) -> Option<Self> {
        let len = usize::try_from(read_u64(take)?).ok()?;
        let count = usize::try_from(read_u64(take)?).ok()?;
        if count != len.div_ceil(SEGMENT_ROWS) {
            return None;
        }
        let mut segments = Vec::with_capacity(count);
        for index in 0..count {
            let rows = SEGMENT_ROWS.min(len - index * SEGMENT_ROWS);
            let tag = take(1)?[0];
            let segment = match tag {
                0 => Segment::Constant(f64::from_le_bytes(take(8)?.try_into().ok()?)),
                1 | 2 => {
                    let table = read_values(take)?;
                    let codes = usize::try_from(read_u64(take)?).ok()?;
                    if codes != rows {
                        return None;
                    }
                    if tag == 1 {
                        let codes = take(rows)?;
                        if table.len() > 256 || codes.iter().any(|code| usize::from(*code) >= table.len()) {
                            return None;
                        }
                        Segment::Table8 {
                            table: table.into(),
                            codes: codes.into(),
                        }
                    } else {
                        let codes: Box<[u16]> = take(rows * 2)?.as_chunks::<2>().0.iter().map(|code| u16::from_le_bytes(*code)).collect();
                        if table.len() > usize::from(u16::MAX) + 1 || codes.iter().any(|code| usize::from(*code) >= table.len()) {
                            return None;
                        }
                        Segment::Table16 { table: table.into(), codes }
                    }
                }
                3 | 4 => {
                    let base = i64::from_le_bytes(take(8)?.try_into().ok()?);
                    let divisor = f64::from_le_bytes(take(8)?.try_into().ok()?);
                    if usize::try_from(read_u64(take)?).ok()? != rows || !POWERS_OF_TEN.iter().any(|power| power.to_bits() == divisor.to_bits()) {
                        return None;
                    }
                    if tag == 3 {
                        Segment::Decimal16 {
                            base,
                            divisor,
                            raw: take(rows * 2)?.as_chunks::<2>().0.iter().map(|raw| u16::from_le_bytes(*raw)).collect(),
                        }
                    } else {
                        Segment::Decimal32 {
                            base,
                            divisor,
                            raw: take(rows * 4)?.as_chunks::<4>().0.iter().map(|raw| u32::from_le_bytes(*raw)).collect(),
                        }
                    }
                }
                5 => {
                    let values = read_values(take)?;
                    if values.len() != rows {
                        return None;
                    }
                    Segment::Raw(values.into())
                }
                _ => return None,
            };
            segments.push(segment);
        }
        Some(Self { len, segments })
    }

    /// The value of `row`, or `None` past the end.
    #[inline]
    pub(crate) fn checked(&self, row: usize) -> Option<f64> {
        (row < self.len).then(|| self.get(row))
    }

    /// What the column holds on the heap.
    pub(crate) fn heap_bytes(&self) -> usize {
        self.segments.iter().map(|segment| size_of::<Segment>() + segment.heap_bytes()).sum()
    }
}

fn write_values(put: &mut impl FnMut(&[u8]), values: &[f64]) {
    put(&(values.len() as u64).to_le_bytes());
    put(&values.iter().flat_map(|value| value.to_le_bytes()).collect::<Vec<u8>>());
}

fn read_u64(take: &mut impl FnMut(usize) -> Option<Vec<u8>>) -> Option<u64> {
    Some(u64::from_le_bytes(take(8)?.try_into().ok()?))
}

fn read_values(take: &mut impl FnMut(usize) -> Option<Vec<u8>>) -> Option<Vec<f64>> {
    let count = usize::try_from(read_u64(take)?).ok()?;
    let bytes = take(count.checked_mul(8)?)?;
    Some(bytes.as_chunks::<8>().0.iter().map(|value| f64::from_le_bytes(*value)).collect())
}

/// Builds a column from values that arrive a batch at a time, so a model read
/// from disk never has its columns in full at once.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlockColumnBuilder {
    column: BlockColumn,
    pending: Vec<f64>,
}

impl BlockColumnBuilder {
    pub(crate) fn push(&mut self, value: f64) {
        self.pending.push(value);
        if self.pending.len() == SEGMENT_ROWS {
            self.flush();
        }
    }

    /// Add `values`, encoding whole segments side by side.
    pub(crate) fn extend_slice(&mut self, mut values: &[f64]) {
        if !self.pending.is_empty() {
            let (head, rest) = values.split_at((SEGMENT_ROWS - self.pending.len()).min(values.len()));
            self.pending.extend_from_slice(head);
            values = rest;
            if self.pending.len() == SEGMENT_ROWS {
                self.flush();
            }
        }
        let full = values.len() - values.len() % SEGMENT_ROWS;
        let (whole, rest) = values.split_at(full);
        self.column.segments.par_extend(whole.par_chunks_exact(SEGMENT_ROWS).map(Segment::encode));
        self.column.len += whole.len();
        self.pending.extend_from_slice(rest);
    }

    /// Add a column built on its own, as if its values were added here: its
    /// segments are kept where they start on a segment boundary, which is
    /// where adding its values would have put them, and its values are added
    /// again otherwise. A last part-filled segment goes back to waiting for
    /// the rows after it, as it would have.
    pub(crate) fn append(&mut self, mut column: BlockColumn) {
        if !self.pending.is_empty() {
            let mut values = Vec::with_capacity(column.len);
            let mut run = Vec::new();
            for index in 0..column.run_count() {
                column.decode_run(index, &mut run);
                values.extend_from_slice(&run);
            }
            self.extend_slice(&values);
            return;
        }
        let tail = column.len & SEGMENT_MASK;
        if tail != 0 {
            column.decode_run(column.run_count() - 1, &mut self.pending);
            column.segments.pop();
            column.len -= tail;
        }
        self.column.len += column.len;
        self.column.segments.append(&mut column.segments);
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.column.segments.push(Segment::encode(&self.pending));
            self.column.len += self.pending.len();
            self.pending.clear();
        }
    }

    pub(crate) fn finish(mut self) -> BlockColumn {
        self.flush();
        self.column.segments.shrink_to_fit();
        self.column
    }
}
