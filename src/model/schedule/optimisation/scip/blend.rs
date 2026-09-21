//! The SCIP side of the blended model: a [`Rows`] implementation that posts
//! the mixing relationship as the nonconvex bilinear equality it actually is.
//!
//! The model itself is not here. It is built once, for both experimental
//! solvers, by [`super::super::blended::formulation::formulate`]; this file
//! supplies columns, linear rows and the one operation the two methods do
//! differently.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use russcip::{Model, ProblemCreated, Variable, prelude::*};

use super::super::blended::{
    formulation::{BlendColumns, BlendSizes, FormulationCancelled, Rows, formulate},
    input::BlendInput,
};
use crate::model::schedule::optimisation::StockpileId;

pub(crate) struct BlendFormulation {
    pub(crate) model: Model<ProblemCreated>,
    pub(crate) columns: BlendColumns<Variable>,
    pub(crate) sizes: BlendSizes,
}

struct ScipRows<'a> {
    model: Model<ProblemCreated>,
    columns: BlendColumns<Variable>,
    sizes: BlendSizes,
    cancel: Option<&'a AtomicBool>,
    checks: Option<&'a AtomicU64>,
}

impl Rows for ScipRows<'_> {
    type Var = Variable;

    fn columns(&mut self) -> &mut BlendColumns<Variable> {
        &mut self.columns
    }

    fn sizes(&mut self) -> &mut BlendSizes {
        &mut self.sizes
    }

    fn cancelled(&self) -> bool {
        if let Some(checks) = self.checks {
            checks.fetch_add(1, Ordering::Relaxed);
        }
        self.cancel.is_some_and(|flag| flag.load(Ordering::Acquire))
    }

    /// russcip fixes a column's objective coefficient at creation, so a column
    /// that earns value must be created knowing it - there is no
    /// `set_obj_coeff`.
    fn valued(&mut self, upper: f64, value: f64, name: &str) -> Variable {
        self.sizes.variables += 1;
        let upper = if upper.is_finite() { upper } else { 1e20 };
        self.model.add(var().cont(0.0..=upper).obj(value).name(name))
    }

    fn binary(&mut self, name: &str) -> Variable {
        self.sizes.variables += 1;
        self.sizes.binaries += 1;
        self.model.add(var().bin().name(name))
    }

    fn linear(&mut self, terms: Vec<(Variable, f64)>, lhs: f64, rhs: f64, name: &str) {
        if terms.is_empty() {
            return;
        }
        self.sizes.linear_constraints += 1;
        let vars: Vec<&Variable> = terms.iter().map(|(v, _)| v).collect();
        let coefs: Vec<f64> = terms.iter().map(|(_, c)| *c).collect();
        self.model.add_cons(vars, &coefs, lhs, rhs, name);
    }

    /// `Q_recl * T_open - T_recl * Q_open = 0`, posted through
    /// `SCIPcreateConsBasicQuadraticNonlinear`. Nonconvex, and the reason this
    /// backend was investigated at all.
    fn mix(&mut self, _pile: StockpileId, _interval: usize, _grade: usize, recl_q: Variable, open_t: Variable, recl_t: Variable, open_q: Variable, name: &str) {
        self.sizes.nonlinear_constraints += 1;
        let q1: Vec<&Variable> = vec![&recl_q, &recl_t];
        let q2: Vec<&Variable> = vec![&open_t, &open_q];
        let mut coefs = [1.0, -1.0];
        self.model.add_cons_quadratic(Vec::new(), &mut [], q1, q2, &mut coefs, 0.0, 0.0, name);
    }
}

/// Build the blended model for SCIP.
pub(crate) fn formulate_scip(input: &BlendInput) -> BlendFormulation {
    formulate_scip_with_cancel(input, None, None).expect("uncancelled formulation")
}

pub(crate) fn formulate_scip_with_cancel(input: &BlendInput, cancel: Option<&AtomicBool>, checks: Option<&AtomicU64>) -> Result<BlendFormulation, FormulationCancelled> {
    let model = Model::new()
        .hide_output()
        .include_default_plugins()
        .create_prob("blended_stockpile")
        .set_obj_sense(ObjSense::Maximize);
    let mut rows = ScipRows {
        model,
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        cancel,
        checks,
    };
    formulate(&mut rows, input)?;
    Ok(BlendFormulation {
        model: rows.model,
        columns: rows.columns,
        sizes: rows.sizes,
    })
}
