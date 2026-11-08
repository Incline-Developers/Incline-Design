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
