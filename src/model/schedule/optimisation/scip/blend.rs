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

use minlp::{
    model::{Model, Var},
    types::{ObjSense, VarType},
};

use super::super::blended::{
    formulation::{BlendColumns, BlendSizes, FamilySize, FamilyTally, FormulationCancelled, Rows, formulate},
    input::BlendInput,
};
use crate::model::schedule::optimisation::StockpileId;

pub(crate) struct BlendFormulation {
    pub(crate) model: Model,
    pub(crate) columns: BlendColumns<Var>,
    pub(crate) sizes: BlendSizes,
    /// Sizes by family, largest first, when asked for; empty otherwise.
    pub(crate) families: Vec<(String, FamilySize)>,
}

struct ScipRows<'a> {
    model: Model,
    columns: BlendColumns<Var>,
    sizes: BlendSizes,
    cancel: Option<&'a AtomicBool>,
    checks: Option<&'a AtomicU64>,
    /// Each pile's capacity, which its mixing rows are divided by.
    capacities: BTreeMap<StockpileId, f64>,
    families: Option<FamilyTally>,
}

impl Rows for ScipRows<'_> {
    type Var = Var;

    fn columns(&mut self) -> &mut BlendColumns<Var> {
        &mut self.columns
    }

    fn cancelled(&self) -> bool {
        if let Some(checks) = self.checks {
            checks.fetch_add(1, Ordering::Relaxed);
        }
        self.cancel.is_some_and(|flag| flag.load(Ordering::Acquire))
    }

    /// A column's objective coefficient is fixed at creation
    /// (`SCIPcreateVarBasic`), so a column that earns value is created
    /// knowing it.
    fn valued(&mut self, upper: f64, value: f64, name: &str) -> Var {
        self.sizes.variables += 1;
        if let Some(families) = &mut self.families {
            families.column(name, false);
        }
        let upper = if upper.is_finite() { upper } else { 1e20 };
        self.model
            .add_var(name, 0.0, upper, value, VarType::Continuous)
            .expect("adding a column in the problem stage")
    }

    fn binary(&mut self, name: &str) -> Var {
        self.sizes.variables += 1;
        self.sizes.binaries += 1;
        if let Some(families) = &mut self.families {
            families.column(name, true);
        }
        self.model.add_var(name, 0.0, 1.0, 0.0, VarType::Binary).expect("adding a column in the problem stage")
    }

    fn linear(&mut self, terms: Vec<(Var, f64)>, lhs: f64, rhs: f64, name: &str) {
        if terms.is_empty() {
            return;
        }
        self.count_row(&terms, name);
        let (vars, coefs): (Vec<Var>, Vec<f64>) = terms.into_iter().unzip();
        self.model.add_linear(name, &vars, &coefs, lhs, rhs).expect("adding a row in the problem stage");
    }

    /// SCIP's own indicator constraint rather than a big-M row: the linear
    /// part is then checked at its own scale, so the implication cannot be
    /// bent by `M x feastol`.
    fn implies(&mut self, flag: Var, terms: Vec<(Var, f64)>, rhs: f64, _big_m: f64, name: &str) {
        if terms.is_empty() {
            return;
        }
        self.count_row(&terms, name);
        let (vars, coefs): (Vec<Var>, Vec<f64>) = terms.into_iter().unzip();
        self.model.add_indicator(name, flag, &vars, &coefs, rhs).expect("adding a row in the problem stage");
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
    fn mix(&mut self, pile: StockpileId, _interval: usize, _grade: usize, recl_q: Var, open_t: Var, recl_t: Var, open_q: Var, name: &str) {
        self.sizes.nonlinear_constraints += 1;
        if let Some(families) = &mut self.families {
            families.row(name, 4);
        }
        let scale = self.capacities.get(&pile).copied().filter(|capacity| *capacity > 1.0).map_or(1.0, f64::recip);
        self.model
            .add_quadratic(name, &[], &[], &[recl_q, recl_t], &[open_t, open_q], &[scale, -scale], 0.0, 0.0)
            .expect("adding a row in the problem stage");
    }
}

impl ScipRows<'_> {
    fn count_row(&mut self, terms: &[(Var, f64)], name: &str) {
        let entries = terms.iter().filter(|(_, coefficient)| *coefficient != 0.0).count();
        self.sizes.linear_constraints += 1;
        self.sizes.linear_coefficient_entries += entries;
        if let Some(families) = &mut self.families {
            families.row(name, entries);
        }
    }
}

/// Build the SCIP model of `input`, tallying its sizes by family as it goes
/// when `count_families` is set.
pub(crate) fn formulate_scip_with_cancel(
    input: &BlendInput,
    cancel: Option<&AtomicBool>,
    checks: Option<&AtomicU64>,
    count_families: bool,
) -> Result<BlendFormulation, FormulationCancelled> {
    let mut model = Model::new("blended_stockpile").expect("creating a SCIP instance");
    if super::adapter::dumping() {
        model.record_calls();
    }
    model.set_int_param("display/verblevel", 0).expect("a core parameter");
    model.set_objsense(ObjSense::Maximize).expect("setting the sense in the problem stage");
    let mut rows = ScipRows {
        model,
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        cancel,
        checks,
        capacities: input.piles.iter().map(|pile| (pile.id, pile.capacity_t)).collect(),
        families: count_families.then(FamilyTally::default),
    };
    formulate(&mut rows, input)?;
    Ok(BlendFormulation {
        model: rows.model,
        columns: rows.columns,
        sizes: rows.sizes,
        families: rows.families.map(FamilyTally::sorted).unwrap_or_default(),
    })
}
