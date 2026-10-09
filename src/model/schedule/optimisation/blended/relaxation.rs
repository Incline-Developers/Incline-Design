//! A proven upper bound on the blended model's value: its linear relaxation,
//! solved by HiGHS's interior-point method.
//!
//! SCIP's first LP on a long horizon is slow, because its bundled LP solver,
//! SoPlex, has only simplex methods; a week of hourly intervals needed
//! 110-120 s before SCIP had any bound at all. The same LP took HiGHS's
//! interior-point method 36 s. SCIP cannot use that method - its HiGHS LP
//! interface sends a barrier request to dual simplex - so the relaxation is
//! solved here instead, beside the SCIP run, and only its value is used.
//!
//! The relaxation is the model [`formulate`] builds, with three changes, each
//! of which can only enlarge the feasible set and so keeps the value an upper
//! bound on every schedule the model allows:
//!
//! - every binary may take any value in `0..=1`;
//! - an indicator is posted as its big-M row, which the formulation's own
//!   `big_m` makes valid at any flag value in `0..=1`;
//! - the perfect-mixing equality, `Q_recl * T_open = T_recl * Q_open`, is left
//!   out. The grade-box rows around it stay, so a reclaim's grade is still
//!   held inside the pile's grade ceilings, but not to the pile's blend.
//!
//! The last makes the bound weaker on models that blend, not wrong.

use std::{
    ffi::{c_char, c_int, c_void},
    time::{Duration, Instant},
};

use highs_sys::{HighsCallbackDataIn, HighsCallbackDataOut, HighsInt};

use super::{
    formulation::{BlendColumns, BlendSizes, Rows, formulate},
    input::BlendInput,
};
use crate::model::schedule::optimisation::StockpileId;

/// What the relaxation proved.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelaxationBound {
    /// No schedule the model allows is worth more than this.
    pub(crate) value: f64,
    /// Building and solving the relaxation.
    pub(crate) elapsed: Duration,
    /// Perfect-mixing equalities left out; zero means the relaxation is the
    /// model's exact LP relaxation.
    pub(crate) dropped_mixing: usize,
}

/// `stop` is polled while the model is built and between interior-point
/// iterations; once it returns true the solve ends without a bound.
pub(crate) fn relaxation_bound(input: &BlendInput, limit: Option<Duration>, stop: &(dyn Fn() -> bool + Sync)) -> Result<RelaxationBound, String> {
    let started = Instant::now();
    let mut rows = LpRows {
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        stop,
        cost: Vec::new(),
        upper: Vec::new(),
        row_lower: Vec::new(),
        row_upper: Vec::new(),
        starts: Vec::new(),
        index: Vec::new(),
        value: Vec::new(),
        dropped_mixing: 0,
        merge: Vec::new(),
    };
    formulate(&mut rows, input).map_err(|_| "stopped while building the relaxation".to_owned())?;
    let value = solve(&rows, limit, stop)?;
    Ok(RelaxationBound {
        value,
        elapsed: started.elapsed(),
        dropped_mixing: rows.dropped_mixing,
    })
}

/// Rows kept in HiGHS's row-wise sparse form, ready for one `Highs_passLp`.
struct LpRows<'a> {
    columns: BlendColumns<usize>,
    sizes: BlendSizes,
    stop: &'a (dyn Fn() -> bool + Sync),
    cost: Vec<f64>,
    upper: Vec<f64>,
    row_lower: Vec<f64>,
    row_upper: Vec<f64>,
    starts: Vec<usize>,
    index: Vec<usize>,
    value: Vec<f64>,
    dropped_mixing: usize,
    /// Scratch for merging a row's repeated columns, which HiGHS rejects.
    merge: Vec<(usize, f64)>,
}

impl LpRows<'_> {
    fn column(&mut self, upper: f64, value: f64) -> usize {
        self.sizes.variables += 1;
        self.cost.push(value);
        self.upper.push(if upper.is_finite() { upper } else { f64::INFINITY });
        self.cost.len() - 1
    }
}

impl Rows for LpRows<'_> {
    type Var = usize;

    fn columns(&mut self) -> &mut BlendColumns<usize> {
        &mut self.columns
    }

    fn cancelled(&self) -> bool {
        (self.stop)()
    }

    fn valued(&mut self, upper: f64, value: f64, _name: &str) -> usize {
        self.column(upper, value)
    }

    /// Relaxed to `0..=1`.
    fn binary(&mut self, _name: &str) -> usize {
        self.sizes.binaries += 1;
        self.column(1.0, 0.0)
    }

    fn linear(&mut self, terms: Vec<(usize, f64)>, lhs: f64, rhs: f64, _name: &str) {
        if terms.is_empty() {
            return;
        }
        self.sizes.linear_constraints += 1;
        self.merge.clear();
        self.merge.extend(terms.into_iter().filter(|(_, coefficient)| *coefficient != 0.0));
        self.merge.sort_unstable_by_key(|(column, _)| *column);
        self.starts.push(self.index.len());
        let first = self.index.len();
        for &(column, coefficient) in &self.merge {
            if self.index.len() > first && self.index.last() == Some(&column) {
                *self.value.last_mut().expect("an entry was pushed") += coefficient;
            } else {
                self.index.push(column);
                self.value.push(coefficient);
            }
        }
        self.sizes.linear_coefficient_entries += self.index.len() - first;
        self.row_lower.push(lhs);
        self.row_upper.push(rhs);
    }

    /// Left out; see the module docs.
    fn mix(&mut self, _pile: StockpileId, _interval: usize, _grade: usize, _recl_q: usize, _open_t: usize, _recl_t: usize, _open_q: usize, _name: &str) {
        self.dropped_mixing += 1;
    }
}

/// Frees the HiGHS instance on every path out of [`solve`].
struct Instance(*mut c_void);

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: created by `Highs_create` and destroyed once, here.
        unsafe { highs_sys::Highs_destroy(self.0) };
    }
}

fn solve(rows: &LpRows, limit: Option<Duration>, stop: &(dyn Fn() -> bool + Sync)) -> Result<f64, String> {
    let fits = |count: usize| HighsInt::try_from(count).map_err(|_| format!("the relaxation has {count} entries, more than HiGHS can index"));
    let columns = fits(rows.cost.len())?;
    let row_count = fits(rows.row_lower.len())?;
    let entries = fits(rows.index.len())?;
    let starts = rows.starts.iter().map(|&start| fits(start)).collect::<Result<Vec<_>, _>>()?;
    let index = rows.index.iter().map(|&column| fits(column)).collect::<Result<Vec<_>, _>>()?;
    let lower = vec![0.0; rows.cost.len()];

    // SAFETY: a single-threaded use of one HiGHS instance, freed by
    // `Instance`. Every array passed has the length its count says, and
    // `stop` outlives the run that calls back into it.
    unsafe {
        let highs = Instance(highs_sys::Highs_create());
        let ok = |code: HighsInt, what: &str| {
            if code == highs_sys::STATUS_ERROR {
                Err(format!("HiGHS rejected {what}"))
            } else {
                Ok(())
            }
        };
        ok(highs_sys::Highs_setBoolOptionValue(highs.0, c"output_flag".as_ptr(), 0), "output_flag")?;
        ok(highs_sys::Highs_setStringOptionValue(highs.0, c"solver".as_ptr(), c"ipm".as_ptr()), "the ipm solver")?;
        if let Some(limit) = limit {
            ok(
                highs_sys::Highs_setDoubleOptionValue(highs.0, c"time_limit".as_ptr(), limit.as_secs_f64()),
                "the time limit",
            )?;
        }
        ok(
            highs_sys::Highs_passLp(
                highs.0,
                columns,
                row_count,
                entries,
                highs_sys::MATRIX_FORMAT_ROW_WISE,
                highs_sys::OBJECTIVE_SENSE_MAXIMIZE,
                0.0,
                rows.cost.as_ptr(),
                lower.as_ptr(),
                rows.upper.as_ptr(),
                rows.row_lower.as_ptr(),
                rows.row_upper.as_ptr(),
                starts.as_ptr(),
                index.as_ptr(),
                rows.value.as_ptr(),
            ),
            "the relaxation",
        )?;
        let data = &stop as *const &(dyn Fn() -> bool + Sync) as *mut c_void;
        ok(highs_sys::Highs_setCallback(highs.0, Some(interrupt), data), "the interrupt callback")?;
        for kind in [highs_sys::kHighsCallbackIpmInterrupt, highs_sys::kHighsCallbackSimplexInterrupt] {
            ok(highs_sys::Highs_startCallback(highs.0, kind), "the interrupt callback")?;
        }
        ok(highs_sys::Highs_run(highs.0), "the relaxation solve")?;
        let status = highs_sys::Highs_getModelStatus(highs.0);
        if status != highs_sys::MODEL_STATUS_OPTIMAL {
            return Err(format!("HiGHS ended with model status {status}"));
        }
        let value = highs_sys::Highs_getObjectiveValue(highs.0);
        if !value.is_finite() {
            return Err(format!("HiGHS reported a non-finite optimum {value}"));
        }
        Ok(value)
    }
}

/// HiGHS's interrupt callback: `data` is the `stop` closure `solve` passed.
unsafe extern "C" fn interrupt(_kind: c_int, _message: *const c_char, _out: *const HighsCallbackDataOut, input: *mut HighsCallbackDataIn, data: *mut c_void) {
    // SAFETY: `data` points at `solve`'s `stop` reference, alive for the
    // whole `Highs_run`; `input` is HiGHS's own and may be written.
    unsafe {
        let stop = &*(data as *const &(dyn Fn() -> bool + Sync));
        if !input.is_null() && stop() {
            (*input).user_interrupt = 1;
        }
    }
}
