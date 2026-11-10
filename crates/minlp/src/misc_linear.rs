//! Port of `misc_linear.c`: the side and coefficient getters that work on
//! every linear-type constraint (linear, setppc, logicor, knapsack,
//! varbound; SOS1 and SOS2 are not ported).

use crate::{
    cons_knapsack, cons_linear,
    cons_setppc::{self, SetppcType},
    cons_varbound,
    retcode::ScipResult,
    scip::Scip,
    types::ConsId,
};

/// `SCIPconsGetRhs`: returns the rhs of an arbitrary SCIP constraint that
/// can be represented as a single linear constraint; returns `(rhs,
/// success)`.
pub fn cons_get_rhs(scip: &Scip, cons: ConsId) -> (f64, bool) {
    let conshdlrname = scip.conshdlrs[scip.conss[cons].conshdlr].name.as_str();
    let mut success = true;
    let mut rhs = crate::def::INVALID;

    match conshdlrname {
        "linear" => rhs = cons_linear::get_rhs_linear(scip, cons),
        "setppc" => {
            rhs = match cons_setppc::get_type_setppc(scip, cons) {
                // Fall through intended.
                SetppcType::Partitioning | SetppcType::Packing => 1.0,
                SetppcType::Covering => scip.set.infinity(),
            }
        }
        "logicor" => rhs = scip.set.infinity(),
        "knapsack" => rhs = cons_knapsack::get_capacity_knapsack(scip, cons) as f64,
        "varbound" => rhs = cons_varbound::get_rhs_varbound(scip, cons),
        _ => {
            log::warn!("Cannot return rhs for constraint of type <{conshdlrname}>");
            success = false;
        }
    }

    (rhs, success)
}

/// `SCIPconsGetLhs`: returns the lhs of an arbitrary SCIP constraint that
/// can be represented as a single linear constraint; returns `(lhs,
/// success)`.
pub fn cons_get_lhs(scip: &Scip, cons: ConsId) -> (f64, bool) {
    let conshdlrname = scip.conshdlrs[scip.conss[cons].conshdlr].name.as_str();
    let mut success = true;
    let mut lhs = crate::def::INVALID;

    match conshdlrname {
        "linear" => lhs = cons_linear::get_lhs_linear(scip, cons),
        "setppc" => {
            lhs = match cons_setppc::get_type_setppc(scip, cons) {
                // Fall through intended.
                SetppcType::Partitioning | SetppcType::Covering => 1.0,
                SetppcType::Packing => -scip.set.infinity(),
            }
        }
        "logicor" => lhs = 1.0,
        "knapsack" => lhs = -scip.set.infinity(),
        "varbound" => lhs = cons_varbound::get_lhs_varbound(scip, cons),
        _ => {
            log::warn!("Cannot return lhs for constraint of type <{conshdlrname}>");
            success = false;
        }
    }

    (lhs, success)
}

/// `SCIPgetConsVals`: returns the value array of an arbitrary SCIP
/// constraint that can be represented as a single linear constraint;
/// returns `(vals, success)`.
pub fn get_cons_vals(scip: &mut Scip, cons: ConsId) -> ScipResult<(Vec<f64>, bool)> {
    let conshdlrname = scip.conshdlrs[scip.conss[cons].conshdlr].name.clone();

    let (nvars, success) = crate::cons::cons_get_n_vars(scip, cons)?;

    if !success {
        log::warn!("Cannot return value array for constraint of type <{conshdlrname}>");
        return Ok((Vec::new(), false));
    }

    let nvars = nvars as usize;
    let vals = match conshdlrname.as_str() {
        "linear" => cons_linear::get_vals_linear(scip, cons)[..nvars].to_vec(),
        "setppc" | "logicor" => vec![1.0; nvars],
        "knapsack" => cons_knapsack::get_weights_knapsack(scip, cons)[..nvars].iter().map(|&w| w as f64).collect(),
        "varbound" => vec![1.0, cons_varbound::get_vbdcoef_varbound(scip, cons)],
        "SOS1" | "SOS2" => unimplemented!("SCIPgetConsVals for SOS constraints"),
        _ => {
            log::warn!("Cannot return value array for constraint of type <{conshdlrname}>");
            return Ok((Vec::new(), false));
        }
    };

    Ok((vals, true))
}
