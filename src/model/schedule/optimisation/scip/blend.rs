//! The SCIP side of the blended model: a [`Rows`] implementation that posts
//! the mixing relationship as the nonconvex bilinear equality it actually is.
//!
//! The model itself is not here. It is built once, for both experimental
//! solvers, by [`super::super::blended::formulation::formulate`]; this file
//! supplies columns, linear rows and the one operation the two methods do
//! differently.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

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
    /// Each pile's capacity, which its mixing rows are divided by.
    capacities: BTreeMap<StockpileId, f64>,
}

impl Rows for ScipRows<'_> {
    type Var = Variable;

    fn columns(&mut self) -> &mut BlendColumns<Variable> {
        &mut self.columns
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
        self.sizes.linear_coefficient_entries += terms.iter().filter(|(_, coefficient)| *coefficient != 0.0).count();
        let vars: Vec<&Variable> = terms.iter().map(|(v, _)| v).collect();
        let coefs: Vec<f64> = terms.iter().map(|(_, c)| *c).collect();
        self.model.add_cons(vars, &coefs, lhs, rhs, name);
    }

    /// SCIP's own indicator constraint rather than a big-M row: the linear
    /// part is then checked at its own scale, so the implication cannot be
    /// bent by `M x feastol`.
    fn implies(&mut self, flag: Variable, terms: Vec<(Variable, f64)>, rhs: f64, _big_m: f64, name: &str) {
        if terms.is_empty() {
            return;
        }
        self.sizes.linear_constraints += 1;
        self.sizes.linear_coefficient_entries += terms.iter().filter(|(_, coefficient)| *coefficient != 0.0).count();
        let vars: Vec<&Variable> = terms.iter().map(|(v, _)| v).collect();
        let mut coefs: Vec<f64> = terms.iter().map(|(_, c)| *c).collect();
        self.model.add_cons_indicator(&flag, vars, &mut coefs, rhs, name);
    }

    /// `Q_recl * T_open - T_recl * Q_open = 0`, posted through
    /// `SCIPcreateConsBasicQuadraticNonlinear`. Nonconvex, and the reason this
    /// backend was investigated at all.
    ///
    /// Divided through by the pile's capacity. SCIP checks the row against an
    /// absolute tolerance of 1e-6, but unscaled its two products reach about
    /// 3e10 on a full 200,000 t pile reclaimed at 3,000 t/h, where double
    /// precision alone leaves residuals several times it: a reclaim at exactly
    /// the pile's blend could not be certified, and on such a pile the
    /// hourly dispatch schedule was never completed into a seed. Scaled,
    /// rounding sits far inside the tolerance, while the reclaimed blend the
    /// tolerance admits is still within `1e-6 x capacity / (T_recl x T_open)`
    /// grade units of the pile's: a millionth for a tonne from a full pile.
    fn mix(&mut self, pile: StockpileId, _interval: usize, _grade: usize, recl_q: Variable, open_t: Variable, recl_t: Variable, open_q: Variable, name: &str) {
        self.sizes.nonlinear_constraints += 1;
        let q1: Vec<&Variable> = vec![&recl_q, &recl_t];
        let q2: Vec<&Variable> = vec![&open_t, &open_q];
        let scale = self.capacities.get(&pile).copied().filter(|capacity| *capacity > 1.0).map_or(1.0, f64::recip);
        let mut coefs = [scale, -scale];
        self.model.add_cons_quadratic(Vec::new(), &mut [], q1, q2, &mut coefs, 0.0, 0.0, name);
    }
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
        capacities: input.piles.iter().map(|pile| (pile.id, pile.capacity_t)).collect(),
    };
    formulate(&mut rows, input)?;
    Ok(BlendFormulation {
        model: rows.model,
        columns: rows.columns,
        sizes: rows.sizes,
    })
}
