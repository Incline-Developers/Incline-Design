//! A small owned front end over the port for applications: one problem with
//! SCIP's default plugins, built from continuous, binary and integer
//! columns and linear, indicator and quadratic rows, solved once and read
//! back. It calls the same `SCIPcreate...` and `SCIPadd...` functions a C
//! program built on SCIP's basic interface would, in the order the caller
//! makes them, so a model built here solves as the reference SCIP solves
//! the same calls.
//!
//! Anything not wrapped here is reachable through [`Model::scip`] and
//! [`Model::scip_mut`].
//!
//! With [`Model::record_calls`] the model keeps every call made through it
//! as one line of text ([`Model::recorded_calls`]), from which the same
//! model can be built again, here or through SCIP's own C interface.

use std::{any::Any, fmt::Write as _};

use crate::{
    api,
    event::{Event, EventType},
    paramset,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_solve, scip_var,
    types::{EventhdlrId, ImplintType, LpSolStat, ObjSense, Status, VarId, VarType},
    var::NONE,
};

/// A column of the original problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Var(pub VarId);

/// One problem and the SCIP instance that solves it.
pub struct Model {
    scip: Box<Scip>,
    /// The calls made since [`Model::record_calls`], as spec lines.
    calls: Option<String>,
}

/// A number as the spec writes it: the shortest text that reads back as
/// the same double, and SCIP's infinity by name.
fn spec_num(scip: &Scip, value: f64) -> String {
    if value == scip.set.infinity() {
        "inf".into()
    } else if value == -scip.set.infinity() {
        "-inf".into()
    } else {
        format!("{value:?}")
    }
}

impl Model {
    /// `SCIPcreate`, `SCIPincludeDefaultPlugins` and `SCIPcreateProbBasic`.
    pub fn new(name: &str) -> ScipResult<Self> {
        let mut scip = Scip::create()?;
        crate::plugins::include_default_plugins(&mut scip)?;
        scip_solve::create_prob(&mut scip, name)?;
        Ok(Self { scip, calls: None })
    }

    /// Starts keeping every following call as a spec line (see the module
    /// documentation). Names must not contain whitespace.
    pub fn record_calls(&mut self) {
        self.calls.get_or_insert_with(String::new);
    }

    /// The calls kept since [`Model::record_calls`].
    pub fn recorded_calls(&self) -> Option<&str> {
        self.calls.as_deref()
    }

    /// Appends one spec line, built by `line`, while recording.
    fn record(&mut self, line: impl FnOnce(&Scip, &mut String)) {
        if let Some(calls) = self.calls.as_mut() {
            line(&self.scip, calls);
            calls.push('\n');
        }
    }

    /// Appends `<n> (<var> <coef>)*`.
    fn record_terms(scip: &Scip, out: &mut String, vars: &[Var], coefs: &[f64]) {
        let _ = write!(out, " {}", vars.len());
        for (var, &coef) in vars.iter().zip(coefs) {
            let _ = write!(out, " {} {}", scip.vars[var.0].name, spec_num(scip, coef));
        }
    }

    pub fn scip(&self) -> &Scip {
        &self.scip
    }

    pub fn scip_mut(&mut self) -> &mut Scip {
        &mut self.scip
    }

    /// `SCIPsetObjsense`.
    pub fn set_objsense(&mut self, sense: ObjSense) -> ScipResult {
        self.record(|_, out| out.push_str(if sense == ObjSense::Maximize { "sense max" } else { "sense min" }));
        scip_solve::set_objsense(&mut self.scip, sense)
    }

    /// SCIP's infinity (`SCIPinfinity`).
    pub fn infinity(&self) -> f64 {
        self.scip.set.infinity()
    }

    /// `SCIPisInfinity`.
    pub fn is_infinity(&self, value: f64) -> bool {
        self.scip.set.is_infinity(value)
    }

    /// `SCIPcreateVarBasic` and `SCIPaddVar`: an initial, not removable
    /// column.
    pub fn add_var(&mut self, name: &str, lb: f64, ub: f64, obj: f64, vartype: VarType) -> ScipResult<Var> {
        self.record(|scip, out| {
            let ty = match vartype {
                VarType::Binary => 'b',
                VarType::Integer => 'i',
                _ => 'c',
            };
            let _ = write!(out, "var {name} {ty} {} {} {}", spec_num(scip, lb), spec_num(scip, ub), spec_num(scip, obj));
        });
        let var = scip_var::create_var_impl(&mut self.scip, name, lb, ub, obj, vartype, ImplintType::None, true, false)?;
        scip_var::add_var(&mut self.scip, var)?;
        Ok(Var(var))
    }

    /// `SCIPcreateConsBasicLinear` and `SCIPaddCons`: `lhs <= coefs x <= rhs`.
    pub fn add_linear(&mut self, name: &str, vars: &[Var], coefs: &[f64], lhs: f64, rhs: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "lin {name} {} {}", spec_num(scip, lhs), spec_num(scip, rhs));
            Self::record_terms(scip, out, vars, coefs);
        });
        let vars = ids(vars);
        let cons = crate::cons_linear::create_cons_basic_linear(&mut self.scip, name, &vars, coefs, lhs, rhs)?;
        self.add_and_release(cons)
    }

    /// `SCIPcreateConsBasicIndicator` and `SCIPaddCons`: `binvar = 1`
    /// implies `coefs x <= rhs`.
    pub fn add_indicator(&mut self, name: &str, binvar: Var, vars: &[Var], coefs: &[f64], rhs: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "ind {name} {} {}", scip.vars[binvar.0].name, spec_num(scip, rhs));
            Self::record_terms(scip, out, vars, coefs);
        });
        let vars = ids(vars);
        let cons = crate::cons_indicator::create_cons_basic_indicator(&mut self.scip, name, binvar.0, &vars, coefs, rhs)?;
        self.add_and_release(cons)
    }

    /// `SCIPcreateConsBasicQuadraticNonlinear` and `SCIPaddCons`:
    /// `lhs <= lincoefs x + sum quadcoefs[i] quadvars1[i] quadvars2[i] <= rhs`.
    pub fn add_quadratic(&mut self, name: &str, linvars: &[Var], lincoefs: &[f64], quadvars1: &[Var], quadvars2: &[Var], quadcoefs: &[f64], lhs: f64, rhs: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "quad {name} {} {}", spec_num(scip, lhs), spec_num(scip, rhs));
            Self::record_terms(scip, out, linvars, lincoefs);
            let _ = write!(out, " {}", quadcoefs.len());
            for i in 0..quadcoefs.len() {
                let _ = write!(
                    out,
                    " {} {} {}",
                    scip.vars[quadvars1[i].0].name,
                    scip.vars[quadvars2[i].0].name,
                    spec_num(scip, quadcoefs[i])
                );
            }
        });
        let (linvars, quadvars1, quadvars2) = (ids(linvars), ids(quadvars1), ids(quadvars2));
        let cons = crate::cons_nonlinear::create_cons_basic_quadratic_nonlinear(&mut self.scip, name, &linvars, lincoefs, &quadvars1, &quadvars2, quadcoefs, lhs, rhs)?;
        self.add_and_release(cons)
    }

    fn add_and_release(&mut self, mut cons: crate::types::ConsId) -> ScipResult {
        scip_cons::add_cons(&mut self.scip, cons)?;
        scip_cons::release_cons(&mut self.scip, &mut cons)
    }

    /// `SCIPchgVarLb`.
    pub fn chg_var_lb(&mut self, var: Var, bound: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "lb {} {}", scip.vars[var.0].name, spec_num(scip, bound));
        });
        scip_var::chg_var_lb(&mut self.scip, var.0, bound)
    }

    /// `SCIPchgVarUb`.
    pub fn chg_var_ub(&mut self, var: Var, bound: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "ub {} {}", scip.vars[var.0].name, spec_num(scip, bound));
        });
        scip_var::chg_var_ub(&mut self.scip, var.0, bound)
    }

    /// The original problem's columns, in the order they were added
    /// (`SCIPgetOrigVars`).
    pub fn orig_vars(&self) -> Vec<Var> {
        let prob = &self.scip.probs[crate::prob::ORIGPROB];
        prob.vars.0[..prob.nvars as usize].iter().map(|&v| Var(v)).collect()
    }

    /// `SCIPfindVar`: the column of this name, also one SCIP created.
    pub fn find_var(&self, name: &str) -> Option<Var> {
        let var = api::find_var(&self.scip, name);
        (var != NONE).then_some(Var(var))
    }

    pub fn var_name(&self, var: Var) -> &str {
        &self.scip.vars[var.0].name
    }

    /// `SCIPvarGetLbLocal`.
    pub fn var_lb(&self, var: Var) -> f64 {
        self.scip.vars[var.0].locdom.lb
    }

    /// `SCIPvarGetUbLocal`.
    pub fn var_ub(&self, var: Var) -> f64 {
        self.scip.vars[var.0].locdom.ub
    }

    pub fn var_type(&self, var: Var) -> VarType {
        self.scip.vars[var.0].vartype
    }

    /// Whether a parameter of this name exists (`SCIPgetParam` is not NULL).
    pub fn has_param(&self, name: &str) -> bool {
        paramset::paramset_get_param(&self.scip, name).is_some()
    }

    pub fn set_bool_param(&mut self, name: &str, value: bool) -> ScipResult {
        self.record(|_, out| {
            let _ = write!(out, "bool {name} {}", if value { "TRUE" } else { "FALSE" });
        });
        paramset::paramset_set_bool(&mut self.scip, name, value)
    }

    pub fn set_int_param(&mut self, name: &str, value: i32) -> ScipResult {
        self.record(|_, out| {
            let _ = write!(out, "int {name} {value}");
        });
        paramset::paramset_set_int(&mut self.scip, name, value)
    }

    pub fn set_longint_param(&mut self, name: &str, value: i64) -> ScipResult {
        self.record(|_, out| {
            let _ = write!(out, "long {name} {value}");
        });
        paramset::paramset_set_longint(&mut self.scip, name, value)
    }

    pub fn set_real_param(&mut self, name: &str, value: f64) -> ScipResult {
        self.record(|scip, out| {
            let _ = write!(out, "real {name} {}", spec_num(scip, value));
        });
        paramset::paramset_set_real(&mut self.scip, name, value)
    }

    pub fn set_char_param(&mut self, name: &str, value: u8) -> ScipResult {
        self.record(|_, out| {
            let _ = write!(out, "char {name} {}", value as char);
        });
        paramset::paramset_set_char(&mut self.scip, name, value)
    }

    pub fn set_str_param(&mut self, name: &str, value: &str) -> ScipResult {
        self.record(|_, out| {
            let _ = write!(out, "str {name} {value}");
        });
        paramset::paramset_set_string(&mut self.scip, name, value)
    }

    /// Offers a solution of the original problem before solving, as
    /// russcip's `add_sol` does: `SCIPcreateOrigSol`, `SCIPsetSolVal` for
    /// each value, `SCIPcheckSolOrig`, and `SCIPaddSolFree` if it is
    /// feasible (`SCIPfreeSol` if not). Columns left out are zero. Returns
    /// whether it was stored.
    pub fn add_orig_sol(&mut self, values: &[(Var, f64)]) -> ScipResult<bool> {
        if self.scip.set.stage != Stage::Problem {
            return Err(Retcode::InvalidCall);
        }
        self.record(|scip, out| {
            let _ = write!(out, "sol {}", values.len());
            for &(var, value) in values {
                let _ = write!(out, " {} {}", scip.vars[var.0].name, spec_num(scip, value));
            }
        });
        let sol = api::create_sol(&mut self.scip, NONE)?;
        for &(var, value) in values {
            api::set_sol_val(&mut self.scip, sol, var.0, value)?;
        }
        if api::check_sol_orig(&mut self.scip, sol, false, true)? {
            api::add_sol_free(&mut self.scip, sol)
        } else {
            api::free_sol(&mut self.scip, sol)?;
            Ok(false)
        }
    }

    /// Calls `callback` with the solve and the event's type on each event
    /// in `mask` (`SCIPincludeEventhdlrBasic`, caught in its init callback,
    /// as russcip's handlers are; the global event filter goes with the
    /// transformed problem).
    pub fn include_event_callback(&mut self, name: &str, desc: &str, mask: EventType, callback: impl FnMut(&mut Solving, EventType) + 'static) -> ScipResult {
        let data: Box<dyn Any> = Box::new(EventCallback {
            mask,
            callback: Box::new(callback),
        });
        let handler = api::include_eventhdlr_basic(&mut self.scip, name, desc, event_exec, Some(data))?;
        let h = &mut self.scip.eventhdlrs[handler];
        h.eventinit = Some(event_init);
        Ok(())
    }

    /// `SCIPwriteOrigProblem` in the `cip` format, as text.
    pub fn write_orig_problem(&self) -> ScipResult<String> {
        crate::reader_cip::write_orig_problem(&self.scip)
    }

    /// `SCIPwriteParams` with `onlychanged`, as text.
    pub fn write_changed_params(&self) -> String {
        paramset::paramset_write(&self.scip, true)
    }

    /// `SCIPsolve`.
    pub fn solve(&mut self) -> ScipResult {
        scip_solve::solve(&mut self.scip)
    }

    pub fn stage(&self) -> Stage {
        self.scip.set.stage
    }

    /// `SCIPgetStatus`.
    pub fn status(&self) -> Status {
        api::get_status(&self.scip)
    }

    /// `SCIPgetNSols`.
    pub fn n_sols(&self) -> usize {
        api::get_n_sols(&self.scip).max(0) as usize
    }

    /// The best solution's value of each column, or `None` without one
    /// (`SCIPgetBestSol`, `SCIPgetSolVal`).
    pub fn best_sol_values(&self, vars: &[Var]) -> Option<Vec<f64>> {
        let best = api::get_best_sol(&self.scip);
        (best != NONE).then(|| vars.iter().map(|var| api::get_sol_val(&self.scip, best, var.0)).collect())
    }

    /// The best solution's value of `var`, or `None` without one.
    pub fn best_sol_value(&self, var: Var) -> Option<f64> {
        let best = api::get_best_sol(&self.scip);
        (best != NONE).then(|| api::get_sol_val(&self.scip, best, var.0))
    }

    /// The best solution's original objective (`SCIPgetSolOrigObj`), or
    /// `None` without one.
    pub fn best_objective(&mut self) -> Option<f64> {
        let best = api::get_best_sol(&self.scip);
        (best != NONE).then(|| api::get_sol_orig_obj(&mut self.scip, best))
    }

    /// `SCIPgetPrimalbound`.
    pub fn primal_bound(&self) -> f64 {
        api::get_primalbound(&self.scip)
    }

    /// `SCIPgetDualbound`.
    pub fn dual_bound(&self) -> f64 {
        api::get_dualbound(&self.scip)
    }

    /// `SCIPgetGap`.
    pub fn gap(&self) -> f64 {
        api::get_gap(&self.scip)
    }

    /// `SCIPgetNNodes`.
    pub fn n_nodes(&self) -> i64 {
        api::get_n_nodes(&self.scip)
    }

    /// `SCIPgetSolvingTime`.
    pub fn solving_time(&self) -> f64 {
        api::get_solving_time(&self.scip)
    }

    /// `SCIPgetPresolvingTime`.
    pub fn presolving_time(&self) -> f64 {
        self.scip.stat.presolvingtime.get_time()
    }

    /// `SCIPgetNLPIterations`.
    pub fn n_lp_iterations(&self) -> i64 {
        api::get_n_lp_iterations(&self.scip)
    }

    /// `SCIPgetNRootLPIterations`.
    pub fn n_root_lp_iterations(&self) -> i64 {
        api::get_n_root_lp_iterations(&self.scip)
    }

    /// `SCIPgetNVars` of the stage's problem.
    pub fn n_vars(&self) -> usize {
        api::get_n_vars(&self.scip).max(0) as usize
    }

    /// `SCIPgetNConss` of the stage's problem.
    pub fn n_conss(&self) -> usize {
        api::get_n_conss(&self.scip).max(0) as usize
    }

    /// `SCIPgetNNZs`.
    pub fn n_nonzeros(&self) -> u64 {
        self.scip.stat.nnz.max(0) as u64
    }
}

fn ids(vars: &[Var]) -> Vec<VarId> {
    vars.iter().map(|var| var.0).collect()
}

/// The solve as an event callback sees it.
pub struct Solving<'a> {
    scip: &'a mut Scip,
}

impl Solving<'_> {
    pub fn scip(&self) -> &Scip {
        self.scip
    }

    /// `SCIPinterruptSolve`.
    pub fn interrupt(&mut self) {
        scip_solve::interrupt_solve(self.scip);
    }

    /// `SCIPgetSolvingTime`.
    pub fn solving_time(&self) -> f64 {
        api::get_solving_time(self.scip)
    }

    /// `SCIPgetDepth`.
    pub fn depth(&self) -> i32 {
        api::get_depth(self.scip)
    }

    /// `SCIPgetNVars`.
    pub fn n_vars(&self) -> usize {
        api::get_n_vars(self.scip).max(0) as usize
    }

    /// `SCIPgetNConss`.
    pub fn n_conss(&self) -> usize {
        api::get_n_conss(self.scip).max(0) as usize
    }

    /// `SCIPgetLPSolstat`.
    pub fn lp_solstat(&self) -> LpSolStat {
        api::get_lp_solstat(self.scip)
    }

    /// `SCIPgetNLPRows`.
    pub fn n_lp_rows(&self) -> usize {
        api::get_n_lp_rows(self.scip).max(0) as usize
    }

    /// `SCIPgetNLPCols`.
    pub fn n_lp_cols(&self) -> usize {
        if crate::tree::tree_is_focus_node_lp_constructed(self.scip) {
            self.scip.lp.as_ref().unwrap().ncols.max(0) as usize
        } else {
            0
        }
    }

    /// The nonzeros of the current LP's rows (`SCIProwGetNNonz` over
    /// `SCIPgetLPRows`).
    pub fn lp_nonzeros(&self) -> usize {
        api::get_lp_rows(self.scip).iter().map(|&row| self.scip.rows[row].len.max(0) as usize).sum()
    }
}

/// What an application's event callback is handed.
type EventFn = Box<dyn FnMut(&mut Solving, EventType)>;

struct EventCallback {
    mask: EventType,
    callback: EventFn,
}

fn event_init(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    let mask = callback_data(scip, eventhdlr).mask;
    api::catch_event(scip, mask, eventhdlr, 0, None)
}

fn event_exec(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, _eventdata: usize) -> ScipResult {
    // The callback is taken out of its handler while it runs, so it may
    // be handed the whole instance.
    let mut data = scip.eventhdlrs[eventhdlr].eventhdlrdata.take().expect("event callback data");
    let callback = data.downcast_mut::<EventCallback>().expect("event callback data");
    (callback.callback)(&mut Solving { scip }, event.eventtype);
    scip.eventhdlrs[eventhdlr].eventhdlrdata = Some(data);
    Ok(())
}

fn callback_data(scip: &Scip, eventhdlr: EventhdlrId) -> &EventCallback {
    scip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_ref()
        .and_then(|data| data.downcast_ref::<EventCallback>())
        .expect("event callback data")
}
