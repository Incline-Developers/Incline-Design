//! Port of `cons_indicator.c`: constraint handler for indicator constraints
//! `z = 1 -> s = 0`, where `s` is the slack variable of a linear
//! constraint `a^T x - s <= b`.
//!
//! Not ported (off by default, `unimplemented!` where reached): the
//! alternative LP (`sepaalternativelp`, `enforcecuts`, `useotherconss`,
//! `useobjectivecut`, `trysolfromcover`, IIS cuts), `generatebilinear`,
//! `usesameslackvar`, the symmetry graphs, printing.

use std::any::Any;

use crate::{
    api,
    cons::*,
    cons_linear,
    event::*,
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    scip_cons, scip_lp, scip_var,
    types::{BoundType, ConfType, ConsId, ConshdlrId, EventhdlrId, HeurId, LockType, ScipResultCode, SolId, VarId, VarStatus, VarType},
    var::NONE,
};

const CONSHDLR_NAME: &str = "indicator";
const CONSHDLR_DESC: &str = "indicator constraint handler";
/// Priority of the constraint handler for separation.
const CONSHDLR_SEPAPRIORITY: i32 = 10;
/// Priority of the constraint handler for constraint enforcing.
const CONSHDLR_ENFOPRIORITY: i32 = -100;
/// Priority of the constraint handler for checking feasibility.
const CONSHDLR_CHECKPRIORITY: i32 = -6000000;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const CONSHDLR_SEPAFREQ: i32 = 10;
/// Frequency for propagating domains; zero means only preprocessing
/// propagation.
const CONSHDLR_PROPFREQ: i32 = 1;
/// Frequency for using all instead of only the useful constraints in
/// separation, propagation and enforcement, -1 for no eager evaluations, 0
/// for first only.
const CONSHDLR_EAGERFREQ: i32 = 100;
/// Maximal number of presolving rounds the constraint handler participates
/// in (-1: no limit).
const CONSHDLR_MAXPREROUNDS: i32 = -1;
/// Should separation method be delayed, if other separators found cuts?
const CONSHDLR_DELAYSEPA: bool = false;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const CONSHDLR_DELAYPROP: bool = false;
/// Should the constraint handler be skipped, if no constraints are
/// available?
const CONSHDLR_NEEDSCONS: bool = true;

const CONSHDLR_PRESOLTIMING: PresolTiming = PRESOLTIMING_FAST;
const CONSHDLR_PROP_TIMING: PropTiming = PROPTIMING_BEFORELP;

/* event handler properties */
const EVENTHDLR_BOUND_NAME: &str = "indicatorbound";
const EVENTHDLR_BOUND_DESC: &str = "bound change event handler for indicator constraints";

const EVENTHDLR_LINCONSBOUND_NAME: &str = "indicatorlinconsbound";
const EVENTHDLR_LINCONSBOUND_DESC: &str = "bound change event handler for lincons of indicator constraints";

const EVENTHDLR_RESTART_NAME: &str = "indicatorrestart";
const EVENTHDLR_RESTART_DESC: &str = "force restart if absolute gap is 1 or enough binary variables have been fixed";

/* conflict handler properties */
const CONFLICTHDLR_NAME: &str = "indicatorconflict";
const CONFLICTHDLR_DESC: &str = "replace slack variables and generate logicor constraints";
const CONFLICTHDLR_PRIORITY: i32 = 200000;

/* upgrade properties */
/// Priority of the constraint handler for upgrading of linear constraints.
const LINCONSUPGD_PRIORITY: i32 = 100000;

/* default values for parameters */
/// Branch on indicator constraints in enforcing?
const DEFAULT_BRANCHINDICATORS: bool = false;
/// Generate logicor constraints instead of cuts?
const DEFAULT_GENLOGICOR: bool = false;
/// Add coupling constraints or rows if big-M is small enough?
const DEFAULT_ADDCOUPLING: bool = true;
/// Maximum coefficient for binary variable in coupling constraint.
const DEFAULT_MAXCOUPLINGVALUE: f64 = 1e4;
/// Add initial variable upper bound constraints, if 'addcoupling' is true?
const DEFAULT_ADDCOUPLINGCONS: bool = false;
/// Should the coupling inequalities be separated dynamically?
const DEFAULT_SEPACOUPLINGCUTS: bool = true;
/// Allow to use local bounds in order to separate coupling inequalities?
const DEFAULT_SEPACOUPLINGLOCAL: bool = false;
/// Maximum coefficient for binary variable in separated coupling
/// constraint.
const DEFAULT_SEPACOUPLINGVALUE: f64 = 1e4;
/// Separate using the alternative LP?
const DEFAULT_SEPAALTERNATIVELP: bool = false;
/// Separate cuts based on perspective formulation?
const DEFAULT_SEPAPERSPECTIVE: bool = false;
/// Allow to use local bounds in order to separate perspectice cuts?
const DEFAULT_SEPAPERSPLOCAL: bool = true;
/// Maximal number of separated non violated IISs, before separation is
/// stopped.
const DEFAULT_MAXSEPANONVIOLATED: i32 = 3;
/// Try to construct a feasible solution from a cover?
const DEFAULT_TRYSOLFROMCOVER: bool = false;
/// Try to upgrade linear constraints to indicator constraints?
const DEFAULT_UPGRADELINEAR: bool = false;
/// Collect other constraints to alternative LP?
const DEFAULT_USEOTHERCONSS: bool = false;
/// Use objective cut with current best solution to alternative LP?
const DEFAULT_USEOBJECTIVECUT: bool = false;
/// Update bounds of original variables for separation?
const DEFAULT_UPDATEBOUNDS: bool = false;
/// Max. estimated condition of the solution basis matrix of the alt. LP to
/// be trustworthy (0.0 to disable check).
const DEFAULT_MAXCONDITIONALTLP: f64 = 0.0;
/// Maximal number of cuts separated per separation round.
const DEFAULT_MAXSEPACUTS: i32 = 100;
/// Maximal number of cuts separated per separation round in the root node.
const DEFAULT_MAXSEPACUTSROOT: i32 = 2000;
/// Remove indicator constraint if corresponding variable bound constraint
/// has been added?
const DEFAULT_REMOVEINDICATORS: bool = false;
/// Do not generate indicator constraint, but a bilinear constraint instead?
const DEFAULT_GENERATEBILINEAR: bool = false;
/// Scale slack variable coefficient at construction time?
const DEFAULT_SCALESLACKVAR: bool = false;
/// Decompose problem (do not generate linear constraint if all variables are
/// continuous)?
const DEFAULT_NOLINCONSCONT: bool = false;
/// Try to make solutions feasible by setting indicator variables?
const DEFAULT_TRYSOLUTIONS: bool = true;
/// In enforcing try to generate cuts (only if sepaalternativelp is true)?
const DEFAULT_ENFORCECUTS: bool = false;
/// Should dual reduction steps be performed?
const DEFAULT_DUALREDUCTIONS: bool = true;
/// Add opposite inequality in nodes in which the binary variable has been
/// fixed to 0?
const DEFAULT_ADDOPPOSITE: bool = false;
/// Try to upgrade bounddisjunction conflicts by replacing slack variables?
const DEFAULT_CONFLICTSUPGRADE: bool = false;
/// Force restart if absolute gap is 1 or enough binary variables have been
/// fixed?
const DEFAULT_FORCERESTART: bool = false;
/// Fraction of binary variables that need to be fixed before restart occurs
/// (in forcerestart).
const DEFAULT_RESTARTFRAC: f64 = 0.9;
/// Use same slack variable for indicator constraints with common binary
/// variable?
const DEFAULT_USESAMESLACKVAR: bool = false;

/* other values */
/// Only separate IIS cuts if the number of separated coupling cuts is less
/// than this value.
const SEPAALTTHRESHOLD: i32 = 10;
/// Maximal number of rounds that produced cuts in separation.
const MAXROUNDINGROUNDS: i32 = 1;

/// `SCIP_ConsData`: constraint data for indicator constraints.
#[derive(Clone)]
pub struct ConsData {
    /// Binary variable for indicator constraint.
    pub binvar: VarId,
    /// Slack variable of inequality of indicator constraint.
    pub slackvar: VarId,
    /// Linear constraint corresponding to indicator constraint.
    pub lincons: ConsId,
    /// Linear constraint variables with bound change events.
    pub varswithevents: Vec<VarId>,
    /// Eventtypes of linear constraint variables with bound change events.
    pub eventtypes: Vec<EventType>,
    /// Number of bound change events of linear constraint variables.
    pub nevents: i32,
    /// Whether the constraint is active on 1 or 0 (only used at creation
    /// time).
    pub activeone: bool,
    /// Whether the original linear constraint is less-than-rhs or
    /// greater-than-rhs (only used at creation time).
    pub lessthanineq: bool,
    /// Number of variables among binvar and slackvar fixed to be nonzero.
    pub nfixednonzero: i32,
    /// Column index in alternative LP.
    pub colindex: i32,
    /// Whether linear constraint and slack variable are active.
    pub linconsactive: bool,
    /// Whether corresponding implication has been added.
    pub implicationadded: bool,
    /// Whether it has been checked to convert the slack variable to be
    /// implicit integer.
    pub slacktypechecked: bool,
}

/// `SCIP_ConshdlrData`: indicator constraint handler data (the alternative
/// LP's fields are left out).
pub struct ConshdlrData {
    /// Event handler for bound change events.
    pub eventhdlrbound: EventhdlrId,
    /// Event handler for bound change events on linear constraint.
    pub eventhdlrlinconsbound: EventhdlrId,
    /// Event handler for performing restarts.
    pub eventhdlrrestart: EventhdlrId,
    /// Whether a bound of a binvar/slackvar of some indicator constraint has
    /// changed.
    pub boundhaschanged: bool,
    /// Whether bound change events are added to variables of linear
    /// constraints.
    pub linconsevents: bool,
    /// Whether bounds of variables of linear constraints changed.
    pub linconsboundschanged: bool,
    /// Whether the separated cuts should be removable.
    pub removable: bool,
    /// If first row of alt. LP has been scaled.
    pub scaled: bool,
    /// Whether the objective is nonzero only for indicator variables.
    pub objindicatoronly: bool,
    /// Whether the objective is nonzero only for non-indicator variables.
    pub objothervarsonly: bool,
    /// Minimum absolute nonzero objective of indicator variables.
    pub minabsobj: f64,
    /// # rows in the alt. LP corr. to original variables in linear
    /// constraints and slacks.
    pub nrows: i32,
    /// # lower bounds of original variables.
    pub nlbbounds: i32,
    /// # upper bounds of original variables.
    pub nubbounds: i32,
    /// Hash map from binary indicator variable to indicator constraint.
    pub binvarhash: Option<std::collections::HashMap<VarId, ConsId>>,
    /// # slack variables.
    pub nslackvars: i32,
    /// Number of IIS-cuts generated.
    pub niiscutsgen: i32,
    /// Number of cuts based on perspective formulation generated.
    pub nperspcutsgen: i32,
    /// Index of objective cut in alternative LP (-1 if not added).
    pub objcutindex: i32,
    /// Best upper bound on objective known.
    pub objupperbound: f64,
    /// Upper objective bound stored in alternative LP (infinity if not
    /// added).
    pub objaltlpbound: f64,
    /// Maximal number of rounds that produced cuts in separation.
    pub maxroundingrounds: i32,
    /// Minimal value for rounding in separation.
    pub roundingminthres: f64,
    /// Maximal value for rounding in separation.
    pub roundingmaxthres: f64,
    /// Offset for rounding in separation.
    pub roundingoffset: f64,
    /// Branch on indicator constraints in enforcing?
    pub branchindicators: bool,
    /// Generate logicor constraints instead of cuts?
    pub genlogicor: bool,
    /// Whether the coupling inequalities should be added at the beginning.
    pub addcoupling: bool,
    /// Add initial variable upper bound constraints, if 'addcoupling' is
    /// true?
    pub addcouplingcons: bool,
    /// Should the coupling inequalities be separated dynamically?
    pub sepacouplingcuts: bool,
    /// Allow to use local bounds in order to separate coupling inequalities?
    pub sepacouplinglocal: bool,
    /// Separate cuts based on perspective formulation?
    pub sepaperspective: bool,
    /// Allow to use local bounds in order to separate perspectice cuts?
    pub sepapersplocal: bool,
    /// Remove indicator constraint if corresponding variable bound constraint
    /// has been added?
    pub removeindicators: bool,
    /// Whether the bounds of the original variables should be changed for
    /// separation.
    pub updatebounds: bool,
    /// Try to make solutions feasible by setting indicator variables?
    pub trysolutions: bool,
    /// In enforcing try to generate cuts (only if sepaalternativelp is true).
    pub enforcecuts: bool,
    /// Should dual reduction steps be performed?
    pub dualreductions: bool,
    /// Add opposite inequality in nodes in which the binary variable has been
    /// fixed to 0?
    pub addopposite: bool,
    /// Do not generate indicator constraint, but a bilinear constraint
    /// instead?
    pub generatebilinear: bool,
    /// Scale slack variable coefficient at construction time?
    pub scaleslackvar: bool,
    /// Try to upgrade bounddisjunction conflicts by replacing slack
    /// variables?
    pub conflictsupgrade: bool,
    /// Whether a restart has been performed already.
    pub performedrestart: bool,
    /// Maximal number of cuts separated per separation round.
    pub maxsepacuts: i32,
    /// Maximal number of cuts separated per separation round in root node.
    pub maxsepacutsroot: i32,
    /// Maximal number of separated non violated IISs, before separation is
    /// stopped.
    pub maxsepanonviolated: i32,
    /// Binary variables globally fixed to zero.
    pub nbinvarszero: i32,
    /// Initial number of indicator constraints (needed in event handlers).
    pub ninitconss: i32,
    /// Maximum coefficient for binary variable in initial coupling
    /// constraint.
    pub maxcouplingvalue: f64,
    /// Maximum coefficient for binary variable in separated coupling
    /// constraint.
    pub sepacouplingvalue: f64,
    /// Maximum estimated condition number of the alternative LP to trust its
    /// solution.
    pub maxconditionaltlp: f64,
    /// Fraction of binary variables that need to be fixed before restart
    /// occurs (in forcerestart).
    pub restartfrac: f64,
    /// Trysol heuristic.
    pub heurtrysol: HeurId,
    /// Whether the coupling constraints have been added already.
    pub addedcouplingcons: bool,
    /// Additional linear constraints that should be added to the alternative
    /// LP.
    pub addlincons: Vec<ConsId>,
    /// Collect other constraints to alternative LP?
    pub useotherconss: bool,
    /// Use objective cut with current best solution to alternative LP?
    pub useobjectivecut: bool,
    /// Try to construct a feasible solution from a cover?
    pub trysolfromcover: bool,
    /// Try to upgrade linear constraints to indicator constraints?
    pub upgradelinear: bool,
    /// Norm type for cut computation.
    pub normtype: u8,
    /// Use same slack variable for indicator constraints with common binary
    /// variable?
    pub usesameslackvar: bool,
    /* parameters that should not be changed after problem stage: */
    /// Separate using the alternative LP?
    pub sepaalternativelp: bool,
    /// Used to store the sepaalternativelp parameter.
    pub sepaalternativelp_: bool,
    /// Decompose problem - do not generate linear constraint if all variables
    /// are continuous.
    pub nolinconscont: bool,
    /// Used to store the nolinconscont parameter.
    pub nolinconscont_: bool,
    /// Force restart if absolute gap is 1 or enough binary variables have
    /// been fixed?
    pub forcerestart: bool,
    /// Used to store the forcerestart parameter.
    pub forcerestart_: bool,
}

/// `SCIP_ENFOSEPATYPE`: type of enforcing/separation call.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnfoSepaType {
    /// Enforce LP.
    EnfoLp,
    /// Enforce pseudo solution.
    EnfoPs,
    /// Enforce relaxation solution.
    EnfoRelax,
    /// Separate LP.
    SepaLp,
    /// Separate relaxation solution.
    SepaRelax,
    /// Separate relaxation solution.
    SepaSol,
}

/// The constraint data of an indicator constraint.
#[inline]
pub fn cd(scip: &Scip, cons: ConsId) -> &ConsData {
    scip.conss[cons].consdata.as_ref().unwrap().downcast_ref::<ConsData>().unwrap()
}

/// The constraint data of an indicator constraint, mutably.
#[inline]
pub fn cdm(scip: &mut Scip, cons: ConsId) -> &mut ConsData {
    scip.conss[cons].consdata.as_mut().unwrap().downcast_mut::<ConsData>().unwrap()
}

/// The constraint handler data of the indicator constraint handler.
#[inline]
fn hd(scip: &Scip, h: ConshdlrId) -> &ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_ref().unwrap().downcast_ref::<ConshdlrData>().unwrap()
}

/// The constraint handler data of the indicator constraint handler,
/// mutably.
#[inline]
fn hdm(scip: &mut Scip, h: ConshdlrId) -> &mut ConshdlrData {
    scip.conshdlrs[h].conshdlrdata.as_mut().unwrap().downcast_mut::<ConshdlrData>().unwrap()
}

/// The indicator constraint handler (event data of the handler-wide event
/// handlers is the constraint handler, which stands for C's
/// `SCIP_CONSHDLRDATA*`).
fn conshdlr_of(scip: &Scip) -> ConshdlrId {
    crate::set::set_find_conshdlr(scip, CONSHDLR_NAME)
}

/* ---------------- Callback methods of event handlers ---------------- */

/// `eventExecIndicatorBound`: execute the event handler for getting
/// variable bound changes.
///
/// We update the number of variables fixed to be nonzero.
fn event_exec_indicator_bound(scip: &mut Scip, _eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let cons: ConsId = eventdata;
    let conshdlr = scip.conss[cons].conshdlr;

    let oldbound = event.data.oldbound;
    let newbound = event.data.newbound;

    let eventtype = event.eventtype;
    match eventtype {
        EVENTTYPE_LBTIGHTENED => {
            // If variable is now fixed to be positive.
            if !scip.set.is_feas_positive(oldbound) && scip.set.is_feas_positive(newbound) {
                cdm(scip, cons).nfixednonzero += 1;
            }
        }
        EVENTTYPE_UBTIGHTENED => {
            // If variable is now fixed to be negative.
            if !scip.set.is_feas_negative(oldbound) && scip.set.is_feas_negative(newbound) {
                cdm(scip, cons).nfixednonzero += 1;
            }
        }
        EVENTTYPE_LBRELAXED => {
            // If variable is not fixed to be positive anymore.
            if scip.set.is_feas_positive(oldbound) && !scip.set.is_feas_positive(newbound) {
                cdm(scip, cons).nfixednonzero -= 1;
            }
        }
        EVENTTYPE_UBRELAXED => {
            // If variable is not fixed to be negative anymore.
            if scip.set.is_feas_negative(oldbound) && !scip.set.is_feas_negative(newbound) {
                cdm(scip, cons).nfixednonzero -= 1;
            }
        }
        _ => {
            log::error!("Invalid event type.");
            return Err(Retcode::InvalidData);
        }
    }

    // Mark that some variable has changed.
    hdm(scip, conshdlr).boundhaschanged = true;

    Ok(())
}

/// `eventExecIndicatorLinconsBound`: execute the event handler for getting
/// variable bound changes on variables of linear constraint.
///
/// Used for propagation of max activity of lincons to upper bound of
/// slackvar; only important bounds for this purpose are catched.
fn event_exec_indicator_lincons_bound(scip: &mut Scip, _eventhdlr: EventhdlrId, _event: &Event, eventdata: usize) -> ScipResult {
    // Mark that some variable has changed.
    hdm(scip, eventdata).linconsboundschanged = true;
    Ok(())
}

/// `eventExecIndicatorRestart`: exec the event handler for forcing a
/// restart.
///
/// There are two cases in which we perform a (user) restart:
/// - If we have a max FS instance, i.e., the objective is 1 for indicator
///   variables and 0 otherwise, we can force a restart if the gap is 1. In
///   this case, the remaining work consists of proving infeasibility of the
///   non-fixed indicators.
/// - If a large fraction of the binary indicator variables have been
///   globally fixed, it makes sense to force a restart.
fn event_exec_indicator_restart(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let conshdlr: ConshdlrId = eventdata;

    let eventtype = event.eventtype;
    match eventtype {
        EVENTTYPE_GUBCHANGED | EVENTTYPE_GLBCHANGED => {
            // Do not treat this case if we have performed a restart already.
            if hd(scip, conshdlr).performedrestart {
                return Ok(());
            }

            // Variable is now fixed.
            hdm(scip, conshdlr).nbinvarszero += 1;

            if scip.set.stage != Stage::Solving {
                return Ok(());
            }

            // If enough variables have been fixed.
            let d = hd(scip, conshdlr);
            if d.nbinvarszero > (d.ninitconss as f64 * d.restartfrac) as i32 {
                log::info!("Forcing restart, since {} binary variables among {} have been fixed.", d.nbinvarszero, d.ninitconss);
                crate::scip_solve::restart_solve(scip);

                // Drop event.
                if hd(scip, conshdlr).objindicatoronly {
                    api::drop_event(scip, EVENTTYPE_BESTSOLFOUND, eventhdlr, conshdlr, -1)?;
                }
                hdm(scip, conshdlr).performedrestart = true;
            }
        }
        EVENTTYPE_BESTSOLFOUND => {
            if scip.set.stage != Stage::Solving {
                return Ok(());
            }

            if !hd(scip, conshdlr).objindicatoronly {
                return Ok(());
            }

            // If the absolute gap is equal to minabsobj.
            let minabsobj = hd(scip, conshdlr).minabsobj;
            if scip.set.is_eq((api::get_primalbound(scip) - api::get_dualbound(scip)).abs(), minabsobj) {
                log::info!("Forcing restart, since the absolute gap is {minabsobj}.");
                crate::scip_solve::restart_solve(scip);

                // Use inference branching, since the objective is not meaningful.
                if crate::set::set_find_branchrule(scip, "inference") != NONE && !crate::paramset::paramset_is_fixed(scip, "branching/inference/priority")? {
                    crate::paramset::paramset_set_int(scip, "branching/inference/priority", i32::MAX / 4)?;
                }

                // Drop event.
                api::drop_event(scip, EVENTTYPE_BESTSOLFOUND, eventhdlr, conshdlr, -1)?;
                hdm(scip, conshdlr).performedrestart = true;
            }
        }
        _ => {
            log::error!("invalid event type.");
            return Err(Retcode::InvalidData);
        }
    }

    Ok(())
}

/* ------------------------ conflict handler ---------------------------------*/

/// `SCIP_ConflicthdlrData`: indicator conflict handler data.
struct ConflicthdlrData {
    /// Indicator constraint handler.
    conshdlr: ConshdlrId,
}

/// `conflictFreeIndicator`: destructor of conflict handler to free conflict
/// handler data (called when SCIP is exiting).
fn conflict_free_indicator(scip: &mut Scip, conflicthdlr: crate::types::ConflicthdlrId) -> ScipResult {
    scip.conflicthdlrs[conflicthdlr].conflicthdlrdata = None;
    Ok(())
}

/// `conflictExecIndicator`: conflict processing method of conflict handler
/// (called when conflict was found).
///
/// In this conflict handler we try to replace slack variables by binary
/// indicator variables and generate a logicor constraint if possible.
fn conflict_exec_indicator(scip: &mut Scip, conflicthdlr: crate::types::ConflicthdlrId, args: &crate::conflict::ConflictExecArgs, result: &mut ScipResultCode) -> ScipResult {
    // Don't process already resolved conflicts.
    if args.resolved {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    let conshdlr = scip.conflicthdlrs[conflicthdlr]
        .conflicthdlrdata
        .as_ref()
        .unwrap()
        .downcast_ref::<ConflicthdlrData>()
        .unwrap()
        .conshdlr;

    // Possibly skip conflict handler.
    if !hd(scip, conshdlr).conflictsupgrade {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Check whether there seems to be one slack variable and all other
    // variables are binary.
    let nbdchginfos = args.bdchginfos.len();
    let mut haveslack = false;
    let mut i = 0;
    while i < nbdchginfos {
        let info = crate::conflict::bdchginfo(scip, args.bdchginfos[i]);
        let var = info.var;

        // Check string for slack variable.
        if scip.vars[var].name.contains("indslack") {
            // Make sure that the slack variable occurs with its lower bound.
            if info.boundtype.opposite() != BoundType::Lower {
                break;
            }

            // Make sure that the lower bound is 0.
            if !scip.set.is_feas_zero(info.newbound) {
                break;
            }

            haveslack = true;
            i += 1;
            continue;
        }

        // We only treat binary variables (other than slack variables).
        if !crate::var::var_is_binary(scip, var) {
            break;
        }
        i += 1;
    }

    // If we have found at least one slack variable and all other variables are
    // binary.
    if haveslack && i == nbdchginfos {
        let conss = conshdlr_get_conss(scip, conshdlr);
        let nconss = conss.len();

        // Create array of variables in conflict constraint.
        let mut vars = vec![NONE; nbdchginfos];
        i = 0;
        while i < nbdchginfos {
            let info = crate::conflict::bdchginfo(scip, args.bdchginfos[i]);
            let mut var = info.var;
            let newbound = info.newbound;

            // Check string for slack variable.
            if scip.vars[var].name.contains("indslack") {
                // Search for slack variable.
                let mut j = 0;
                while j < nconss {
                    let slackvar = get_slack_var_indicator(scip, conss[j]);

                    // Check whether we found the variable.
                    if slackvar == var {
                        // Replace slack variable by binary variable.
                        var = get_binary_var_indicator(scip, conss[j]);
                        break;
                    }
                    j += 1;
                }

                // Check whether we found the slack variable.
                if j >= nconss {
                    break;
                }
            } else {
                // If the variable is fixed to one in the conflict set, we have to use
                // its negation.
                if newbound > 0.5 {
                    var = scip_var::get_negated_var(scip, var)?;
                }
            }

            vars[i] = var;
            i += 1;
        }

        // Whether all slack variables have been found.
        if i == nbdchginfos {
            // Create a logicor constraint out of the conflict set.
            let napplied = scip.conflict.as_ref().map_or(0, |c| c.nappliedglbconss + c.nappliedlocconss);
            let consname = format!("cf{}_{}", api::get_n_runs(scip), napplied);
            let flags = ConsFlags {
                initial: false,
                separate: args.separate,
                enforce: false,
                check: false,
                propagate: true,
                local: args.local,
                modifiable: false,
                dynamic: args.dynamic,
                removable: args.removable,
                stickingatnode: false,
            };
            let mut cons = crate::cons_logicor::create_cons_logicor(scip, &consname, &vars, &flags)?;

            // Add constraint to SCIP.
            scip_cons::add_conflict(scip, args.node, &mut cons, args.validnode, args.conftype, args.cutoffinvolved)?;
            *result = ScipResultCode::ConsAdded;
        }
    }

    Ok(())
}

/* ------------------------ parameter handling ---------------------------------*/

/// `checkTransferBoolParam` for one of the three parameters: whether we
/// transfer a changed parameter to the given value. The parameter's own
/// storage is the `_` field; the value used is copied while in the problem
/// stage and reset otherwise.
fn check_transfer_bool_param(scip: &mut Scip, paramname: &str, name: &str, get: fn(&mut ConshdlrData) -> (&mut bool, &mut bool)) -> ScipResult {
    let conshdlr = conshdlr_of(scip);
    let (newvalue, value) = {
        let (stored, value) = get(hdm(scip, conshdlr));
        (*stored, *value)
    };

    if value == newvalue {
        return Ok(());
    }

    // Check whether the change parameter corresponds to our name to check.
    if paramname == name {
        // Check stage and possibly ignore parameter change.
        if scip.set.stage > Stage::Problem {
            log::warn!(
                "Cannot change parameter <{name}> stage {:?} - reset to old value {}.",
                scip.set.stage,
                if value { "true" } else { "false" }
            );
            // Note that the following command will create a recursive call, but
            // then *value == newvalue above.
            crate::paramset::paramset_set_bool(scip, name, value)?;
        } else {
            // Otherwise copy value.
            *get(hdm(scip, conshdlr)).1 = newvalue;
        }
    }

    Ok(())
}

/// `paramChangedIndicator`: called after a parameter has been changed.
fn param_changed_indicator(scip: &mut Scip, param: usize) -> ScipResult {
    let paramname = scip.set.paramset.params[param].name.clone();
    check_transfer_bool_param(scip, &paramname, "constraints/indicator/sepaalternativelp", |d| {
        (&mut d.sepaalternativelp_, &mut d.sepaalternativelp)
    })?;
    check_transfer_bool_param(scip, &paramname, "constraints/indicator/forcerestart", |d| (&mut d.forcerestart_, &mut d.forcerestart))?;
    check_transfer_bool_param(scip, &paramname, "constraints/indicator/nolinconscont", |d| (&mut d.nolinconscont_, &mut d.nolinconscont))?;
    Ok(())
}

/* ------------------------ operations on the alternative LP -------------------*/

/// `varGetObjDelta`: returns the objective delta of fixing a variable to 1.
///
/// The objective of a negated binary variable is the negative of the
/// objective of the negated variable; for aggregated variables the
/// aggregation scalar times the objective of the aggregation variable.
/// `SCIPvarGetObj()` would return 0 in these cases.
fn var_get_obj_delta(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if crate::var::var_is_binary(scip, var) && crate::var::var_is_negated(scip, var) {
        -scip.vars[v.negatedvar].obj
    } else if v.varstatus == VarStatus::Aggregated {
        v.data.aggregate.scalar * scip.vars[v.data.aggregate.var].obj
    } else {
        v.obj
    }
}

/* ---------------------------- constraint handler local methods ----------------------*/

/// `consdataCreate`: creates and initializes consdata.
#[allow(clippy::too_many_arguments)]
fn consdata_create(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    eventhdlrrestart: EventhdlrId,
    binvar: VarId,
    activeone: bool,
    lessthanineq: bool,
    slackvar: VarId,
    lincons: ConsId,
    linconsactive: bool,
) -> ScipResult<ConsData> {
    // If active on 0, a provided binary variable is negated.
    let binvarinternal = if activeone || binvar == NONE {
        binvar
    } else {
        scip_var::get_negated_var(scip, binvar)?
    };

    // Create constraint data.
    let mut consdata = ConsData {
        nfixednonzero: 0,
        colindex: -1,
        linconsactive,
        binvar: binvarinternal,
        slackvar,
        activeone,
        lessthanineq,
        lincons,
        implicationadded: false,
        slacktypechecked: false,
        varswithevents: Vec::new(),
        eventtypes: Vec::new(),
        nevents: 0,
    };

    // If we are transformed, obtain transformed variables and catch events.
    if api::is_transformed(scip) {
        // Handle binary variable.
        if binvarinternal != NONE {
            let var = api::get_transformed_var(scip, binvarinternal)?;
            consdata.binvar = var;

            // Check type.
            if !crate::var::var_is_binary(scip, var) || crate::var::var_is_implied_integral(scip, var) {
                log::error!("Indicator variable <{}> is not binary {:?}.", scip.vars[var].name, scip.vars[var].vartype);
                return Err(Retcode::Error);
            }

            // The indicator variable must not be multi-aggregated because the
            // constraint handler propagation tries to tighten its bounds, which
            // is not allowed for multi-aggregated variables.
            scip_var::mark_do_not_multaggr_var(scip, var)?;

            // Catch global bound change events on binary variable.
            if hd(scip, conshdlr).forcerestart {
                api::catch_var_event(scip, var, EVENTTYPE_GBDCHANGED, eventhdlrrestart, conshdlr, None)?;
            }

            // If binary variable is fixed to be nonzero.
            if scip.vars[var].locdom.lb > 0.5 {
                consdata.nfixednonzero += 1;
            }
        }

        // Handle slack variable.
        let var = api::get_transformed_var(scip, slackvar)?;
        consdata.slackvar = var;

        // Catch bound change events on slack variable and adjust nfixednonzero.
        if linconsactive {
            // If slack variable is fixed to be nonzero.
            if scip.set.is_feas_positive(scip.vars[var].locdom.lb) {
                consdata.nfixednonzero += 1;
            }
        }

        // Add corresponding column to alternative LP if the constraint is new.
        if hd(scip, conshdlr).sepaalternativelp && scip.set.stage >= Stage::InitSolve && lincons != NONE {
            unimplemented!("constraints/indicator/sepaalternativelp (addAltLPConstraint)");
        }
    }

    Ok(consdata)
}

/// `createVarUbs`: create variable upper bounds for constraints; returns
/// the number of successful operations.
fn create_var_ubs(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult<i32> {
    let mut ngen = 0;

    // Check each constraint.
    for (c, &cons) in conss.iter().enumerate() {
        let (slackvar, binvar) = (cd(scip, cons).slackvar, cd(scip, cons).binvar);
        let ub = scip.vars[slackvar].glbdom.ub;

        // Insert corresponding row if helpful and coefficient is not too large.
        let d = hd(scip, conshdlr);
        if ub <= d.maxcouplingvalue {
            let removeindicators = d.removeindicators;
            // The reference is built without NDEBUG: the name is set.
            let name = format!("couple{c}");

            // Add variable upper bound:
            // - check constraint if we remove the indicator constraint afterwards
            // - constraint is dynamic if we do not remove indicator constraints
            // - constraint is removable if we do not remove indicator constraints
            let flags = ConsFlags {
                initial: true,
                separate: true,
                enforce: true,
                check: removeindicators,
                propagate: true,
                local: false,
                modifiable: false,
                dynamic: !removeindicators,
                removable: !removeindicators,
                stickingatnode: false,
            };
            let inf = scip.set.infinity();
            let mut vcons = crate::cons_varbound::create_cons_varbound(scip, &name, slackvar, binvar, ub, -inf, ub, &flags)?;
            scip_cons::add_cons(scip, vcons)?;
            scip_cons::release_cons(scip, &mut vcons)?;

            // Remove indicator constraint if required.
            if removeindicators {
                scip_cons::del_cons(scip, cons)?;
            }

            ngen += 1;
        }
    }

    Ok(ngen)
}

/// `presolRoundIndicator`: perform one presolving round; returns `(cutoff,
/// success)`.
fn presol_round_indicator(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, dualreductions: bool, ndelconss: &mut i32, nfixedvars: &mut i32) -> ScipResult<(bool, bool)> {
    let (binvar, slackvar) = (cd(scip, cons).binvar, cd(scip, cons).slackvar);

    // If the binary variable is fixed to nonzero.
    if scip.vars[binvar].locdom.lb > 0.5 {
        // If slack variable is fixed to nonzero, we are infeasible.
        if scip.set.is_feas_positive(scip.vars[slackvar].locdom.lb) {
            return Ok((true, false));
        }

        // Otherwise fix slack variable to 0.
        let (_infeasible, fixed) = scip_var::fix_var(scip, slackvar, 0.0)?;
        if fixed {
            *nfixedvars += 1;
        }

        // Delete indicator constraint (leave linear constraint).
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok((false, true));
    }

    // If the binary variable is fixed to zero.
    if scip.vars[binvar].locdom.ub < 0.5 {
        // Delete indicator constraint.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok((false, true));
    }

    // If the slack variable is fixed to nonzero.
    if scip.set.is_feas_positive(scip.vars[slackvar].locdom.lb) {
        // If binary variable is fixed to nonzero, we are infeasible.
        if scip.vars[binvar].locdom.lb > 0.5 {
            return Ok((true, false));
        }

        // Otherwise fix binary variable to 0.
        let (_infeasible, fixed) = scip_var::fix_var(scip, binvar, 0.0)?;
        if fixed {
            *nfixedvars += 1;
        }

        // Delete constraint.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok((false, true));
    }

    // If the slack variable is fixed to zero.
    if scip.set.is_feas_zero(scip.vars[slackvar].locdom.ub) {
        // Perform dual reductions - if required.
        if dualreductions {
            // Check objective of binary variable.
            let mut obj = var_get_obj_delta(scip, binvar);

            // If obj = 0, we prefer fixing the binary variable to 1 (if possible).
            if obj <= 0.0 {
                // In this case we would like to fix the binary variable to 1, if it
                // is not locked up except by this indicator constraint. If more
                // than one indicator constraint is affected, we have to hope that
                // they are all fulfilled - in this case the last constraint will
                // fix the binary variable to 1.
                if crate::var::var_get_n_locks_up_type(scip, binvar, LockType::Model) <= 1 && scip.vars[binvar].glbdom.ub > 0.5 {
                    let (_infeasible, fixed) = scip_var::fix_var(scip, binvar, 1.0)?;
                    if fixed {
                        *nfixedvars += 1;
                    }
                    // Make sure that the other case does not occur.
                    obj = -1.0;
                }
            }

            if obj >= 0.0 {
                // In this case we would like to fix the binary variable to 0, if it
                // is not locked down (should also have been performed by other dual
                // reductions).
                if crate::var::var_get_n_locks_down_type(scip, binvar, LockType::Model) == 0 && scip.vars[binvar].glbdom.lb < 0.5 {
                    let (_infeasible, fixed) = scip_var::fix_var(scip, binvar, 0.0)?;
                    if fixed {
                        *nfixedvars += 1;
                    }
                }
            }
        }

        // Delete constraint.
        scip_cons::del_cons(scip, cons)?;
        *ndelconss += 1;
        return Ok((false, true));
    }

    // Check whether indicator variable is aggregated.
    if scip.vars[binvar].varstatus == VarStatus::Aggregated {
        let mut negated = false;

        // Possibly get representation of indicator variable by active variable.
        let mut var = binvar;
        crate::var::var_get_probvar_binary(scip, &mut var, &mut negated)?;

        // We can replace the binary variable by the active variable if it is
        // not negated.
        if var != binvar && !negated {
            let (eventhdlrbound, eventhdlrrestart, forcerestart) = {
                let d = hd(scip, conshdlr);
                (d.eventhdlrbound, d.eventhdlrrestart, d.forcerestart)
            };

            // We need to update the events and locks.
            api::drop_var_event(scip, binvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, -1)?;
            api::catch_var_event(scip, var, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;

            // We also need to update the events and locks if restart is forced,
            // since global bound change events on binary variables are also
            // caught in this case. If it would not be updated and forcerestart =
            // TRUE, then an event might be dropped on a wrong variable.
            if forcerestart {
                api::drop_var_event(scip, binvar, EVENTTYPE_GBDCHANGED, eventhdlrrestart, conshdlr, -1)?;
                api::catch_var_event(scip, var, EVENTTYPE_GBDCHANGED, eventhdlrrestart, conshdlr, None)?;
            }
            api::add_var_locks_type(scip, binvar, LockType::Model, 0, -1)?;
            api::add_var_locks_type(scip, var, LockType::Model, 0, 1)?;

            // Change binvary variable.
            cdm(scip, cons).binvar = var;
        }
    } else if scip.vars[binvar].varstatus == VarStatus::Negated {
        let var = scip.vars[binvar].negatedvar;

        // If the binary variable is the negated slack variable, we have 1 - s =
        // 1 -> s = 0, i.e., the constraint is redundant.
        if var == slackvar {
            // Delete constraint.
            scip_cons::del_cons(scip, cons)?;
            *ndelconss += 1;
            return Ok((false, true));
        }
    }

    // Check whether slack variable is aggregated.
    let binvar = cd(scip, cons).binvar;
    if scip.vars[slackvar].varstatus == VarStatus::Aggregated || scip.vars[slackvar].varstatus == VarStatus::Negated {
        let mut boundtype = BoundType::Lower;

        // Possibly get representation of slack variable by active variable.
        let mut var = slackvar;
        let mut bound = scip.vars[var].glbdom.lb;
        crate::var::var_get_probvar_bound(scip, &mut var, &mut bound, &mut boundtype)?;

        // We can replace the slack variable by the active variable if it is also
        // a >= variable.
        if var != binvar && boundtype == BoundType::Lower && scip.set.is_eq(bound, 0.0) {
            let eventhdlrbound = hd(scip, conshdlr).eventhdlrbound;

            // We need to update the events, locks, and captures.
            api::drop_var_event(scip, slackvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, -1)?;
            api::catch_var_event(scip, var, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;

            api::unlock_var_cons(scip, slackvar, cons, false, true)?;
            api::lock_var_cons(scip, var, cons, false, true)?;

            let mut old = slackvar;
            api::release_var(scip, &mut old)?;
            crate::var::var_capture(scip, var);

            // Change slack variable.
            cdm(scip, cons).slackvar = var;
        } else if var == binvar {
            // Check special case that aggregating variable is equal to the
            // indicator variable.

            // If the lower bound is transformed to an upper bound, we have "y = 1
            // -> 1 - y = 0", i.e., the constraint is redundant.
            if boundtype == BoundType::Upper {
                // Delete constraint.
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
                return Ok((false, true));
            } else {
                // If the lower bound is transformed to a lower bound, we have "y = 1
                // -> y = 0", i.e., we can fix the binary variable to 0.
                let (_infeasible, fixed) = scip_var::fix_var(scip, binvar, 0.0)?;
                if fixed {
                    *nfixedvars += 1;
                }
                scip_cons::del_cons(scip, cons)?;
                *ndelconss += 1;
                return Ok((false, true));
            }
        }
    }

    // Note that because of possible multi-aggregation we cannot simply remove
    // the indicator constraint if the linear constraint is not active or
    // disabled - see the note in @ref PREPROC.

    Ok((false, false))
}

/// `propIndicator`: propagate indicator constraint; returns `(cutoff,
/// nGen)`.
fn prop_indicator(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, dualreductions: bool, addopposite: bool) -> ScipResult<(bool, i32)> {
    let mut ngen = 0;

    // If the linear constraint has not been generated, we do nothing.
    if !cd(scip, cons).linconsactive {
        return Ok((false, 0));
    }

    let (binvar, slackvar, lincons, nfixednonzero) = {
        let c = cd(scip, cons);
        (c.binvar, c.slackvar, c.lincons, c.nfixednonzero)
    };

    // Increase age of constraint; age will be reset to zero, if a conflict or a
    // propagation was found.
    if !crate::tree::tree_in_repropagation(scip) {
        api::inc_cons_age(scip, cons)?;
    }

    // If both slackvar and binvar are fixed to be nonzero.
    if nfixednonzero > 1 {
        api::reset_cons_age(scip, cons);

        // Check if conflict analysis is turned on.
        if !api::is_conflict_analysis_applicable(scip) {
            return Ok((true, 0));
        }

        // Conflict analysis can only be applied in solving stage.

        // Perform conflict analysis.
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        api::add_conflict_binvar(scip, binvar)?;
        api::add_conflict_lb(scip, slackvar, None)?;

        api::analyze_conflict_cons(scip, cons)?;

        return Ok((true, 0));
    }

    // If exactly one of the variables is fixed to be nonzero.
    if nfixednonzero == 1 {
        // If binvar is fixed to be nonzero.
        if scip.vars[binvar].locdom.lb > 0.5 {
            // If slack variable is not already fixed to 0.
            if !scip.set.is_zero(scip.vars[slackvar].locdom.ub) {
                // Fix slack variable to 0.
                let (_infeasible, tightened) = scip_var::infer_var_ub_cons(scip, slackvar, 0.0, cons, 0, false)?;
                if tightened {
                    ngen += 1;
                }
            }
        }

        // If slackvar is fixed to be nonzero.
        if scip.set.is_feas_positive(scip.vars[slackvar].locdom.lb) {
            // If binary variable is not yet fixed to 0.
            if scip.vars[binvar].locdom.ub > 0.5 {
                // Fix binary variable to 0.
                let (_infeasible, tightened) = scip_var::infer_var_ub_cons(scip, binvar, 0.0, cons, 1, false)?;
                if tightened {
                    ngen += 1;
                }
            }
        }

        // Remove constraint if we are not in probing.
        if !api::in_probing(scip) {
            // Delete constraint locally.
            scip_cons::del_cons_local(scip, cons)?;
        }
    } else {
        // If the binary variable is fixed to zero.
        if scip.vars[binvar].locdom.ub < 0.5 {
            if addopposite && cd(scip, cons).linconsactive {
                // Determine lhs/rhs (first exchange lhs/rhs).
                let mut lhs = cons_linear::get_rhs_linear(scip, lincons);
                if scip.set.is_infinity(lhs) {
                    lhs = -scip.set.infinity();
                }
                let mut rhs = cons_linear::get_lhs_linear(scip, lincons);
                if scip.set.is_infinity(-rhs) {
                    rhs = scip.set.infinity();
                }

                // Consider only finite lhs/rhs.
                if !scip.set.is_infinity(-lhs) || !scip.set.is_infinity(rhs) {
                    // Ignore equations (cannot add opposite constraint).
                    if !scip.set.is_eq(lhs, rhs) {
                        let linvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
                        let linvals = cons_linear::get_vals_linear(scip, lincons).to_vec();
                        let mut allintegral = true;
                        let mut vars = Vec::with_capacity(linvars.len());
                        let mut vals = Vec::with_capacity(linvars.len());

                        // Copy data and check whether the linear constraint is integral.
                        for (j, &linvar) in linvars.iter().enumerate() {
                            if linvar != slackvar {
                                if !crate::var::var_is_integral(scip, linvar) || !scip.set.is_integral(linvals[j]) {
                                    allintegral = false;
                                }

                                vars.push(linvar);
                                vals.push(linvals[j]);
                            }
                        }

                        // Possibly adjust lhs/rhs.
                        if allintegral && !scip.set.is_infinity(lhs.abs()) {
                            lhs += 1.0;
                        }

                        if allintegral && !scip.set.is_infinity(rhs.abs()) {
                            rhs -= 1.0;
                        }

                        // Create reverse constraint.
                        let name = format!("reverse_{}", scip.conss[lincons].name);

                        // Constraint is initial, separated, not enforced, not checked,
                        // propagated, local, not modifiable, dynamic, removable.
                        let mut reversecons = cons_linear::create_cons_linear(scip, &name, &vars, &vals, lhs, rhs, true, true, false, false, true, true, false, true, true, false)?;

                        // Add constraint.
                        scip_cons::add_cons(scip, reversecons)?;
                        scip_cons::release_cons(scip, &mut reversecons)?;
                    }
                }
            }

            // Remove constraint if we are not in probing.
            if !api::in_probing(scip) {
                // Delete constraint locally.
                scip_cons::del_cons_local(scip, cons)?;
            }
        }
        // If the slack variable is fixed to zero.
        else if scip.set.is_feas_zero(scip.vars[slackvar].locdom.ub) {
            // Perform dual reduction - if required.
            if dualreductions {
                // Check objective of binary variable.
                let mut obj = var_get_obj_delta(scip, binvar);

                // If obj = 0, we prefer setting the binary variable to 1 (if
                // possible).
                if obj <= 0.0 {
                    // In this case we would like to fix the binary variable to 1, if it
                    // is not locked up except by this indicator constraint. If more
                    // than one indicator constraint is affected, we have to hope that
                    // they are all fulfilled - in this case the last constraint will
                    // fix the binary variable to 1.
                    if crate::var::var_get_n_locks_up_type(scip, binvar, LockType::Model) <= 1 && scip.vars[binvar].locdom.ub > 0.5 {
                        let (_infeasible, tightened) = scip_var::infer_var_lb_cons(scip, binvar, 1.0, cons, 2, false)?;
                        if tightened {
                            ngen += 1;
                        }
                        // Make sure that the other case does not occur, since we are
                        // not sure whether SCIPinferVarLbCons() directly changes the
                        // bounds.
                        obj = -1.0;
                    }
                }

                if obj >= 0.0 {
                    // In this case we would like to fix the binary variable to 0, if it
                    // is not locked down (should also have been performed by other
                    // dual reductions).
                    if crate::var::var_get_n_locks_down_type(scip, binvar, LockType::Model) == 0 && scip.vars[binvar].locdom.lb < 0.5 {
                        let (_infeasible, tightened) = scip_var::infer_var_ub_cons(scip, binvar, 0.0, cons, 2, false)?;
                        if tightened {
                            ngen += 1;
                        }
                    }
                }
            }

            // Delete constraint: remove constraint if we are not in probing.
            if !api::in_probing(scip) {
                scip_cons::del_cons_local(scip, cons)?;
            }
        }

        // Note that because of possible multi-aggregation we cannot simply
        // remove the indicator constraint if the linear constraint is not active
        // or disabled - see the note in @ref PREPROC and consPresolIndicator().
        // Moreover, it would drastically increase memory consumption, because the
        // linear constraints have to be stored in each node.
    }

    // Propagate maximal activity of linear constraint to upper bound of slack
    // variable.
    //
    // It is especially worth to tighten the upper bound if it is greater than
    // maxcouplingvalue or sepacouplingvalue. But do not tighten it if slackvar
    // is locked down by other constraints, or if it has a nonzero coefficient
    // in the objective function.
    //
    // ax - c * s <= rhs  ->  s <= (maxActivity(ax) - rhs) / c;
    let (maxcouplingvalue, sepacouplingvalue) = (hd(scip, conshdlr).maxcouplingvalue, hd(scip, conshdlr).sepacouplingvalue);
    let slackub = scip.vars[slackvar].locdom.ub;
    if (slackub > maxcouplingvalue || slackub > sepacouplingvalue)
        && crate::var::var_get_n_locks_down_type(scip, slackvar, LockType::Model) <= 1
        && scip.vars[slackvar].obj == 0.0
        && scip.conss[lincons].is_active()
    {
        let mut maxactivity = 0.0;
        let mut coeffslack = crate::def::INVALID; // -c

        let linconsvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
        let linconsvals = cons_linear::get_vals_linear(scip, lincons).to_vec();

        // Calculate maximal activity of linear constraint without slackvar.
        for (j, &var) in linconsvars.iter().enumerate() {
            let val = linconsvals[j];

            // Store slackvar coefficient.
            if var == slackvar {
                coeffslack = val;
                continue;
            }

            let bound = if val > 0.0 { scip.vars[var].locdom.ub } else { scip.vars[var].locdom.lb };

            if scip.set.is_infinity(bound.abs()) {
                maxactivity = scip.set.infinity();
                break;
            } else {
                maxactivity += val * bound;
            }
        }

        // Continue only if maxactivity is not infinity.
        if !scip.set.is_infinity(maxactivity) && coeffslack != crate::def::INVALID && coeffslack < 0.0 {
            let rhs = cons_linear::get_rhs_linear(scip, lincons);

            // Continue if rhs is not finite; happens, e.g., if variables are
            // multiaggregated; we would need the minimal activity in this case.
            if !scip.set.is_infinity(rhs) {
                // Divide by coeff of slackvar.
                let mut newub = (maxactivity - rhs) / (-1.0 * coeffslack);

                // Adjust bound if slackvar is (implicit) integer.
                newub = scip_var::adjusted_var_ub(scip, slackvar, newub);

                if scip.set.is_feas_lt(newub, scip.vars[slackvar].locdom.ub) && newub > scip.vars[slackvar].locdom.lb {
                    // Propagate bound.
                    let (_infeasible, tightened) = scip_var::infer_var_ub_cons(scip, slackvar, newub, cons, 3, false)?;
                    if tightened {
                        ngen += 1;
                    }
                }
            }
        }
    }

    // Reset constraint age counter.
    if ngen > 0 {
        api::reset_cons_age(scip, cons);
    }

    Ok((false, ngen))
}

/// `enforceIndicators`: enforcement method.
///
/// We check whether the current solution is feasible, i.e., if binvar = 1
/// implies that slackvar = 0. If not, we branch as follows:
///
/// In one branch we fix binvar = 1 and slackvar = 0. In the other branch we
/// fix binvar = 0 and leave slackvar unchanged.
fn enforce_indicators(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], sol: SolId, _enfosepatype: EnfoSepaType, _genlogicor: bool) -> ScipResult<ScipResultCode> {
    let mut branchcons = NONE;
    let mut maxslack = -1.0;
    let mut somelinconsnotactive = false;

    let mut result = ScipResultCode::Feasible;

    // Get constraint handler data.
    let dualreductions = hd(scip, conshdlr).dualreductions && scip_var::allow_strong_dual_reds(scip);

    // Check each constraint.
    for &cons in conss {
        // If the linear constraint has not been generated, we do nothing.
        if !cd(scip, cons).linconsactive {
            somelinconsnotactive = true;
            continue;
        }

        // First perform propagation (it might happen that standard propagation
        // is turned off).
        let addopposite = hd(scip, conshdlr).addopposite;
        let (cutoff, cnt) = prop_indicator(scip, conshdlr, cons, dualreductions, addopposite)?;
        if cutoff {
            return Ok(ScipResultCode::Cutoff);
        }
        if cnt > 0 {
            return Ok(ScipResultCode::ReducedDom);
        }

        // Check whether constraint is infeasible.
        let (binvar, slackvar) = (cd(scip, cons).binvar, cd(scip, cons).slackvar);
        let valslack = api::get_sol_val(scip, sol, slackvar);
        if !scip.set.is_feas_zero(api::get_sol_val(scip, sol, binvar)) && !scip.set.is_feas_zero(valslack) {
            // Binary variable is not fixed - otherwise we would not be infeasible.
            if valslack > maxslack {
                maxslack = valslack;
                branchcons = cons;
            }
        }
    }

    // If some constraint has a linear constraint that is not active, we need to
    // check feasibility via the alternative polyhedron.
    if (somelinconsnotactive || hd(scip, conshdlr).enforcecuts) && hd(scip, conshdlr).sepaalternativelp {
        unimplemented!("constraints/indicator/sepaalternativelp (enforceCuts)");
    }

    // If all constraints are feasible.
    if branchcons == NONE {
        return Ok(result);
    }

    // Skip branching if required.
    if !hd(scip, conshdlr).branchindicators {
        return Ok(ScipResultCode::Infeasible);
    }

    // Otherwise create branches.
    let (binvar, slackvar) = (cd(scip, branchcons).binvar, cd(scip, branchcons).slackvar);

    // node1: binvar = 1, slackvar = 0
    let estimate = api::calc_child_estimate(scip, binvar, 1.0);
    let node1 = api::create_child(scip, 0.0, estimate)?;

    if scip.vars[binvar].locdom.lb < 0.5 {
        scip_var::chg_var_lb_node(scip, node1, binvar, 1.0)?;
    }

    // If slack-variable is multi-aggregated.
    if !scip.set.is_feas_zero(scip.vars[slackvar].locdom.ub) {
        scip_var::chg_var_ub_node(scip, node1, slackvar, 0.0)?;
    }

    // node2: binvar = 0, no restriction on slackvar
    let estimate = api::calc_child_estimate(scip, binvar, 0.0);
    let node2 = api::create_child(scip, 0.0, estimate)?;

    if scip.vars[binvar].locdom.ub > 0.5 {
        scip_var::chg_var_ub_node(scip, node2, binvar, 0.0)?;
    }
    api::reset_cons_age(scip, branchcons);
    result = ScipResultCode::Branched;

    Ok(result)
}

/// `separatePerspective`: separate cuts based on perspective formulation.
///
/// Hijazi, Bonami, and Ouorou (2014) introduced the following cuts: We
/// consider an indicator constraint
/// \f[ y = 1 \rightarrow \alpha^T x \leq \beta \f]
/// and assume finite bounds \f$\ell \leq x \leq u\f$. Then for \f$I
/// \subseteq \{1, \dots, n\}\f$ define
/// \f[ \Sigma(I,x,y) = \sum_{i \notin I} \alpha_i x_i + y \Big(\sum_{i \in
///    I, \alpha_i < 0} \alpha_i u_i + \sum_{i \in I, \alpha_i > 0} \alpha_i
///    \ell_i + \sum_{i \notin I, \alpha_i < 0} \alpha_i \ell_i + \sum_{i
///    \notin I, \alpha_i > 0} \alpha_i u_i - \beta\Big). \f]
/// Then the cuts
/// \f[ \Sigma(I,x,y) \leq \sum_{i \notin I, \alpha_i < 0} \alpha_i \ell_i
///    + \sum_{i \notin I, \alpha_i > 0} \alpha_i u_i \f]
/// are valid for the disjunction
/// \f[ \{y = 0,\; \ell \leq x \leq u\} \cup \{y = 1,\; \ell \leq x \leq
///    u,\; \alpha^T x \leq \beta\}. \f]
/// These cuts can easily be separated for a given point \f$(x^*, y^*)\f$ by
/// checking for each \f$i \in \{1, \dots, n\}\f$ whether
/// \f[ y^*(\alpha_i\, u_i\, [\alpha_i < 0] + \alpha_i\, \ell_i\, [\alpha_i
///    > 0]) > \alpha_i x_i^* + y^* )\alpha_i \ell_i [\alpha_i < 0] +
/// > \alpha_i u_i [\alpha_i > 0]), \f]
/// > where \f$[C] = 1\f$ if condition \f$C\f$ is satisfied, otherwise it is 0.
/// > If the above inequality holds, \f$i\f$ is included in \f$I\f$, otherwise
/// > not.
fn separate_perspective(scip: &mut Scip, conshdlr: ConshdlrId, sol: SolId, conss: &[ConsId], maxsepacuts: i32, ngen: &mut i32) -> ScipResult {
    if *ngen >= maxsepacuts {
        return Ok(());
    }

    let nvars = api::get_n_vars(scip) as usize;
    let mut cutvars = vec![NONE; nvars + 1];
    let mut cutvals = vec![0.0; nvars + 1];

    // Loop through constraints.
    for &cons in conss {
        let (slackvar, lincons, binvar) = {
            let c = cd(scip, cons);
            (c.slackvar, c.lincons, c.binvar)
        };

        let binval = api::get_sol_val(scip, sol, binvar);

        if scip.conss[lincons].is_active() {
            let mut finitebound = true;
            let mut cutrhs = 0.0;
            let mut signfactor = 1.0;
            let mut islocal = false;
            let mut cnt = 0;

            let linvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
            let linvals = cons_linear::get_vals_linear(scip, lincons).to_vec();
            let mut linrhs = cons_linear::get_rhs_linear(scip, lincons);

            if scip.set.is_infinity(linrhs) {
                if !scip.set.is_infinity(cons_linear::get_lhs_linear(scip, lincons)) {
                    linrhs = -cons_linear::get_lhs_linear(scip, lincons);
                    signfactor = -1.0;
                } else {
                    continue;
                }
            }
            let mut ypart = -linrhs;
            let mut cutval = binval * ypart;

            let sepapersplocal = hd(scip, conshdlr).sepapersplocal;
            for (j, &linvar) in linvars.iter().enumerate() {
                let mut din = 0.0;
                let mut dout = 0.0;

                if linvar == slackvar {
                    continue;
                }

                let (lb, ub);
                if sepapersplocal {
                    let v = &scip.vars[linvar];
                    lb = v.locdom.lb;
                    ub = v.locdom.ub;
                    if lb > v.glbdom.lb {
                        islocal = true;
                    }
                    if ub < v.glbdom.ub {
                        islocal = true;
                    }
                } else {
                    lb = scip.vars[linvar].glbdom.lb;
                    ub = scip.vars[linvar].glbdom.ub;
                }

                // Skip cases with unbounded variables.
                if scip.set.is_infinity(-lb) || scip.set.is_infinity(ub) {
                    finitebound = false;
                    break;
                }

                // Compute rest parts for i in the set (din) or not in the set
                // (dout).
                let linval = signfactor * linvals[j];
                if scip.set.is_negative(linval) {
                    din += linval * ub;
                    dout += linval * lb;
                } else if scip.set.is_positive(linval) {
                    din += linval * lb;
                    dout += linval * ub;
                }

                let xval = api::get_sol_val(scip, sol, linvar);
                let xpart = linval * xval;

                // If din > dout, we want to include i in the set.
                if scip.set.is_gt(binval * din, binval * dout + xpart) {
                    ypart += din;
                    cutval += binval * din;
                } else {
                    // Otherwise i is not in the set.
                    ypart += dout;

                    cutrhs += dout;
                    cutval += binval * dout + xpart;

                    cutvars[cnt] = linvar;
                    cutvals[cnt] = linval;
                    cnt += 1;
                }
            }

            if !finitebound {
                continue;
            }

            if api::is_efficacious(scip, cutval - cutrhs) {
                // Add y-variable.
                cutvars[cnt] = binvar;
                cutvals[cnt] = ypart;
                cnt += 1;

                let name = format!("persp{}", hd(scip, conshdlr).nperspcutsgen + *ngen);
                let (inf, removable) = (scip.set.infinity(), hd(scip, conshdlr).removable);
                let mut row = scip_lp::create_empty_row_cons(scip, cons, &name, -inf, cutrhs, islocal, false, removable)?;
                scip_lp::add_vars_to_row(scip, row, &cutvars[..cnt], &cutvals[..cnt])?;
                scip_lp::add_row(scip, row, false)?;
                scip_lp::release_row(scip, &mut row)?;
                api::reset_cons_age(scip, cons);
                *ngen += 1;
            }
        }
        if *ngen >= maxsepacuts {
            break;
        }
    }

    Ok(())
}

/// `separateIndicators`: separation method.
///
/// We first check whether coupling inequalities can be separated (if
/// required). If not enough of these could be generated, we check whether IIS
/// inequalities can be separated.
fn separate_indicators(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, _enfosepatype: EnfoSepaType) -> ScipResult<ScipResultCode> {
    let mut result = ScipResultCode::DidNotRun;

    if conss.is_empty() {
        return Ok(result);
    }

    let mut ncuts = 0;

    // Get the maximal number of cuts allowed in a separation round.
    let maxsepacuts = if api::get_depth(scip) == 0 {
        hd(scip, conshdlr).maxsepacutsroot
    } else {
        hd(scip, conshdlr).maxsepacuts
    };

    // First separate coupling inequalities (if required).
    if hd(scip, conshdlr).sepacouplingcuts {
        result = ScipResultCode::DidNotFind;

        // Check each constraint.
        let mut c = 0;
        while c < nusefulconss as usize && ncuts < maxsepacuts {
            let cons = conss[c];
            let (slackvar, binvar) = (cd(scip, cons).slackvar, cd(scip, cons).binvar);

            // Get upper bound for slack variable in linear constraint.
            let mut islocal = false;
            let ub = if hd(scip, conshdlr).sepacouplinglocal {
                let ub = scip.vars[slackvar].locdom.ub;
                if ub < scip.vars[slackvar].glbdom.ub {
                    islocal = true;
                }
                ub
            } else {
                scip.vars[slackvar].glbdom.ub
            };

            // Only use coefficients that are not too large.
            if ub <= hd(scip, conshdlr).sepacouplingvalue {
                let activity = api::get_sol_val(scip, sol, slackvar) + ub * api::get_sol_val(scip, sol, binvar) - ub;
                if api::is_efficacious(scip, activity) {
                    // The reference is built without NDEBUG: the row is named.
                    let name = format!("couple{c}");
                    let (inf, removable) = (scip.set.infinity(), hd(scip, conshdlr).removable);
                    let mut row = scip_lp::create_empty_row_cons(scip, cons, &name, -inf, ub, islocal, false, removable)?;
                    scip_lp::cache_row_extensions(scip, row);
                    api::add_var_to_row(scip, row, slackvar, 1.0)?;
                    api::add_var_to_row(scip, row, binvar, ub)?;
                    scip_lp::flush_row_extensions(scip, row)?;
                    scip_lp::add_row(scip, row, false)?;
                    scip_lp::release_row(scip, &mut row)?;
                    api::reset_cons_age(scip, cons);

                    result = ScipResultCode::Separated;
                    ncuts += 1;
                }
            }
            c += 1;
        }
    }

    // Separate cuts from the alternative lp (if required).
    if hd(scip, conshdlr).sepaalternativelp && ncuts < SEPAALTTHRESHOLD {
        unimplemented!("constraints/indicator/sepaalternativelp (separateIISRounding)");
    }

    // Separate cuts based on perspective formulation.
    if hd(scip, conshdlr).sepaperspective && ncuts < SEPAALTTHRESHOLD {
        let noldcuts = ncuts;

        if result == ScipResultCode::DidNotRun {
            result = ScipResultCode::DidNotFind;
        }

        // Start separation.
        separate_perspective(scip, conshdlr, sol, conss, maxsepacuts, &mut ncuts)?;

        if ncuts > noldcuts {
            hdm(scip, conshdlr).nperspcutsgen += ncuts;

            // Possibly overwrite result from separation above.
            result = ScipResultCode::Separated;
        }
    }

    Ok(result)
}

/// `initConshdlrData`: initializes the constraint handler data.
fn init_conshdlr_data(scip: &mut Scip, conshdlr: ConshdlrId) {
    let inf = scip.set.infinity();
    let d = hdm(scip, conshdlr);
    d.linconsevents = false;
    d.linconsboundschanged = true;
    d.boundhaschanged = true;
    d.removable = true;
    d.scaled = false;
    d.nrows = 0;
    d.nlbbounds = 0;
    d.nubbounds = 0;
    d.nslackvars = 0;
    d.objcutindex = -1;
    d.objupperbound = inf;
    d.objaltlpbound = inf;
    d.roundingminthres = 0.1;
    d.roundingmaxthres = 0.6;
    d.maxroundingrounds = MAXROUNDINGROUNDS;
    d.roundingoffset = 0.1;
    d.addedcouplingcons = false;
    d.ninitconss = 0;
    d.nbinvarszero = 0;
    d.performedrestart = false;
    d.objindicatoronly = false;
    d.objothervarsonly = false;
    d.minabsobj = 0.0;
    d.normtype = b'e';
    d.niiscutsgen = 0;
    d.nperspcutsgen = 0;
}

/* ---------------------------- upgrading methods -----------------------------------*/

/// `linconsUpgdIndicator`: tries to upgrade a linear constraint into an
/// indicator constraint.
///
/// For some linear constraint of the form \f$a^T x + \alpha\, y \geq
/// \beta\f$ with \f$y \in \{0,1\}\f$, we can upgrade it to an indicator
/// constraint if for the residual value \f$a^T x \geq \gamma\f$, we have
/// \f$\alpha + \gamma \geq \beta\f$: in this case, the constraint is always
/// satisfied if \f$y = 1\f$.
///
/// Similarly, for a linear constraint in the form \f$a^T x + \alpha\, y
/// \leq \beta\f$ with \f$y \in \{0,1\}\f$, we can upgrade it to an indicator
/// constraint if for the residual value \f$a^T x \leq \gamma\f$, we have
/// \f$\alpha + \gamma \leq \beta\f$.
fn lincons_upgd_indicator(scip: &mut Scip, cons: ConsId, stats: &cons_linear::LinConsStats, upgdcons: &mut ConsId) -> ScipResult {
    let (vars, vals, lhs, rhs) = (&stats.vars, &stats.vals, stats.lhs, stats.rhs);
    let nvars = vars.len();
    let mut minactivity = 0.0;
    let mut maxactivity = 0.0;
    let mut maxabsval = -1.0;
    let mut secabsval = -1.0;
    let mut maxabsvalidx: i32 = -1;

    // Do not upgrade if there are at most 2 variables (2 variables should be
    // upgraded to a varbound constraint).
    if nvars <= 2 {
        return Ok(());
    }

    // Cannot currently ranged constraints, since we can only return one
    // constraint (and we would need one for each side each).
    if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(rhs) {
        return Ok(());
    }

    // Check whether upgrading is turned on.
    let conshdlr = conshdlr_of(scip);
    if !hd(scip, conshdlr).upgradelinear {
        return Ok(());
    }

    // Calculate activities.
    for j in 0..nvars {
        let val = vals[j];
        let var = vars[j];

        // Store maximal (and second to largest) value of coefficients.
        if scip.set.is_ge(val.abs(), maxabsval) {
            secabsval = maxabsval;
            maxabsval = val.abs();
            maxabsvalidx = j as i32;
        }

        if !crate::var::var_is_binary(scip, var) {
            let (lb, ub) = if val > 0.0 {
                (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub)
            } else {
                (scip.vars[var].glbdom.ub, scip.vars[var].glbdom.lb)
            };

            // Compute minimal activity.
            if scip.set.is_infinity(-lb) {
                minactivity = -scip.set.infinity();
            } else if !scip.set.is_infinity(-minactivity) {
                minactivity += val * lb;
            }

            // Compute maximal activity.
            if scip.set.is_infinity(ub) {
                maxactivity = scip.set.infinity();
            } else if !scip.set.is_infinity(maxactivity) {
                maxactivity += val * ub;
            }
        }
    }

    // Exit if largest coefficient does not belong to binary variable.
    if !crate::var::var_is_binary(scip, vars[maxabsvalidx as usize]) {
        return Ok(());
    }

    // Exit if the second largest coefficient is as large as largest.
    if scip.set.is_eq(secabsval, maxabsval) {
        return Ok(());
    }

    // Cannot upgrade if all activities are infinity.
    if scip.set.is_infinity(-minactivity) && scip.set.is_infinity(maxactivity) {
        return Ok(());
    }

    // Check each variable as indicator variable.
    for j in 0..nvars {
        let mut upgdlhs = false;
        let mut upgdrhs = false;
        let mut indneglhs = false;
        let mut indnegrhs = false;

        let indvar = vars[j];
        let indval = vals[j];

        if !crate::var::var_is_binary(scip, indvar) {
            continue;
        }

        // Check for upgrading of lhs.
        if !scip.set.is_infinity(-minactivity) && !scip.set.is_infinity(-lhs) {
            // Upgrading is possible with binary variable.
            if scip.set.is_ge(minactivity, lhs) {
                upgdlhs = true;
            }

            // Upgrading is possible with negated binary variable.
            if scip.set.is_ge(minactivity + indval, lhs) {
                upgdlhs = true;
                indneglhs = true;
            }
        }

        // Check for upgrading of rhs.
        if !scip.set.is_infinity(maxactivity) && !scip.set.is_infinity(rhs) {
            // Upgrading is possible with binary variable.
            if scip.set.is_le(maxactivity, rhs) {
                upgdrhs = true;
            }

            // Upgrading is possible with negated binary variable.
            if scip.set.is_le(maxactivity + indval, rhs) {
                upgdrhs = true;
                indnegrhs = true;
            }
        }

        // Upgrade constraint.
        if upgdlhs || upgdrhs {
            let mut indconsvars = Vec::with_capacity(nvars - 1);
            let mut indconsvals = Vec::with_capacity(nvars - 1);

            // Create constraint.
            for l in 0..nvars {
                if vars[l] == indvar {
                    continue;
                }
                indconsvars.push(vars[l]);
                if upgdlhs {
                    indconsvals.push(-vals[l]);
                } else {
                    indconsvals.push(vals[l]);
                }
            }

            let indvar2 = if indneglhs || indnegrhs { scip_var::get_negated_var(scip, indvar)? } else { indvar };

            let flags = cons_flags(scip, cons);
            let name = scip.conss[cons].name.clone();
            if upgdlhs {
                let mut bnd = -lhs;
                if !indneglhs {
                    bnd -= indval;
                }
                *upgdcons = create_cons_indicator(scip, &name, indvar2, &indconsvars, &indconsvals, bnd, &flags)?;
            } else {
                let mut bnd = rhs;
                if !indnegrhs {
                    bnd -= indval;
                }
                *upgdcons = create_cons_indicator(scip, &name, indvar2, &indconsvars, &indconsvals, bnd, &flags)?;
            }

            return Ok(());
        }
    }

    Ok(())
}

/// The flags of a constraint (`SCIPconsIsInitial(cons)`, ...).
fn cons_flags(scip: &Scip, cons: ConsId) -> ConsFlags {
    let c = &scip.conss[cons];
    ConsFlags {
        initial: c.initial,
        separate: c.separate,
        enforce: c.enforce,
        check: c.check,
        propagate: c.propagate,
        local: c.local,
        modifiable: c.modifiable,
        dynamic: c.dynamic,
        removable: c.removable,
        stickingatnode: c.stickingatnode,
    }
}

/* ---------------------------- constraint handler callback methods ----------------------*/

/// `conshdlrCopyIndicator`: copy method for constraint handler plugins
/// (called when SCIP copies plugins).
fn conshdlr_copy_indicator(scip: &mut Scip, _sourcescip: &Scip, _conshdlr: ConshdlrId, valid: &mut bool) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_conshdlr_indicator(scip)?;
    *valid = true;
    Ok(())
}

/// `consInitIndicator`: initialization method of constraint handler (called
/// after problem was transformed).
fn cons_init_indicator(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId]) -> ScipResult {
    init_conshdlr_data(scip, conshdlr);

    // Find trysol heuristic.
    if hd(scip, conshdlr).trysolutions && hd(scip, conshdlr).heurtrysol == NONE {
        hdm(scip, conshdlr).heurtrysol = crate::set::set_find_heur(scip, "trysol");
    }

    Ok(())
}

/// `consExitIndicator`: deinitialization method of constraint handler
/// (called before transformed problem is freed).
fn cons_exit_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    hdm(scip, conshdlr).binvarhash = None;

    // Drop bound change events on variables of linear constraints.
    let eventhdlrlinconsbound = hd(scip, conshdlr).eventhdlrlinconsbound;
    for &cons in conss {
        if !cd(scip, cons).varswithevents.is_empty() {
            for j in 0..cd(scip, cons).nevents as usize {
                let (var, eventtype) = (cd(scip, cons).varswithevents[j], cd(scip, cons).eventtypes[j]);
                api::drop_var_event(scip, var, eventtype, eventhdlrlinconsbound, conshdlr, -1)?;
            }
            let c = cdm(scip, cons);
            c.varswithevents = Vec::new();
            c.eventtypes = Vec::new();
            c.nevents = 0;
        }
    }

    let d = hdm(scip, conshdlr);
    d.addlincons = Vec::new();
    d.nrows = 0;

    Ok(())
}

/// `consFreeIndicator`: destructor of constraint handler to free constraint
/// handler data (called when SCIP is exiting).
fn cons_free_indicator(scip: &mut Scip, conshdlr: ConshdlrId) -> ScipResult {
    scip.conshdlrs[conshdlr].conshdlrdata = None;
    Ok(())
}

/// `consInitsolIndicator`: solving process initialization method of
/// constraint handler (called when branch and bound process is about to
/// begin).
fn cons_initsol_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    use crate::types::Status;
    let status = api::get_status(scip);
    if status == Status::Optimal || status == Status::Infeasible || status == Status::Unbounded || status == Status::InfOrUnbd {
        return Ok(());
    }

    // Make sure that we propagate at the beginning.
    hdm(scip, conshdlr).boundhaschanged = true;

    hdm(scip, conshdlr).normtype = crate::paramset::paramset_get_char(scip, "separating/efficacynorm")?;

    if hd(scip, conshdlr).sepaalternativelp {
        unimplemented!("constraints/indicator/sepaalternativelp (slack hash)");
    }

    // Check each constraint.
    hdm(scip, conshdlr).objothervarsonly = true;
    for &cons in conss {
        let (binvar, linconsactive, lincons) = (cd(scip, cons).binvar, cd(scip, cons).linconsactive, cd(scip, cons).lincons);

        // Presolving might replace a slack variable by an active variable. Thus,
        // the objective of a slack variables might be nonzero. However, we do not
        // need to check slack variables here.
        if !scip.set.is_zero(var_get_obj_delta(scip, binvar)) {
            hdm(scip, conshdlr).objothervarsonly = false;
        }

        // Deactivate.
        if !linconsactive {
            crate::cons::cons_disable(scip, lincons)?;
        } else {
            // Add constraint to alternative LP if not already done.
            if hd(scip, conshdlr).sepaalternativelp && cd(scip, cons).colindex < 0 {
                unimplemented!("constraints/indicator/sepaalternativelp (addAltLPConstraint)");
            }
        }

        // Add nlrow representation to NLP, if NLP had been constructed.
        //
        // Note, that we did not tell SCIP in exitpre that we have something to
        // add to the NLP, thus indicators are only available in the NLP for
        // MINLPs, but not for MIPs with indicators.
        if api::is_nlp_constructed(scip) && scip.conss[cons].check {
            // Create nonlinear row binary variable * slack variable = 0.
            let slackvar = cd(scip, cons).slackvar;
            let mut varexpr0 = crate::expr_var::create_expr_var(scip, binvar, None, 0)?;
            let mut varexpr1 = crate::expr_var::create_expr_var(scip, slackvar, None, 0)?;
            let mut quadexpr = crate::expr_product::create_expr_product(scip, &[varexpr0, varexpr1], 1.0, None, 0)?;

            let name = scip.conss[cons].name.clone();
            let mut nlrow = crate::nlp::create_nl_row(scip, &name, 0.0, &[], &[], quadexpr, 0.0, 0.0, crate::expr::ExprCurv::Unknown)?;

            crate::expr::expr_release(scip, &mut quadexpr)?;
            crate::expr::expr_release(scip, &mut varexpr1)?;
            crate::expr::expr_release(scip, &mut varexpr0)?;

            // Add row to NLP and forget about it.
            crate::nlp::add_nl_row(scip, nlrow)?;
            crate::nlp::release_nl_row(scip, &mut nlrow)?;
        }
    }

    // Initialize event handler if restart should be forced.
    if hd(scip, conshdlr).forcerestart {
        // Store number of initial constraints.
        let nactive = scip.conshdlrs[conshdlr].nactiveconss;
        hdm(scip, conshdlr).ninitconss = nactive;

        // Reset number of fixed binary variables.
        hdm(scip, conshdlr).nbinvarszero = 0;

        // Loop through variables.
        let vars = api::get_vars(scip).to_vec();
        let nvars = vars.len();

        hdm(scip, conshdlr).objindicatoronly = false;
        hdm(scip, conshdlr).minabsobj = crate::def::REAL_MAX;

        // Unmark all variables.
        let mut covered = vec![false; nvars];

        // Mark indicator variables.
        for &cons in conss {
            // Avoid non-active indicator constraints.
            if !scip.conss[cons].is_active() {
                continue;
            }

            let binvar = cd(scip, cons).binvar;
            let probindex = if crate::var::var_is_negated(scip, binvar) {
                scip.vars[scip.vars[binvar].negatedvar].probindex
            } else {
                scip.vars[binvar].probindex
            };

            // If presolving detected infeasibility it might be that the binary
            // variables are not active.
            if probindex < 0 {
                continue;
            }

            covered[probindex as usize] = true;
        }

        // Check all variables.
        let mut j = 0;
        while j < nvars {
            let obj = scip.vars[vars[j]].obj;
            if !scip.set.is_zero(obj) {
                if !covered[j] {
                    break;
                }

                if !scip.set.is_integral(obj) {
                    break;
                }

                if obj.abs() < hd(scip, conshdlr).minabsobj {
                    hdm(scip, conshdlr).minabsobj = obj.abs();
                }
            }
            j += 1;
        }

        // If all variables have integral objective and only indicator variables
        // have nonzero objective.
        if j >= nvars {
            // If there are variables with nonzero objective.
            if hd(scip, conshdlr).minabsobj < crate::def::REAL_MAX {
                hdm(scip, conshdlr).objindicatoronly = true;

                let eventhdlrrestart = hd(scip, conshdlr).eventhdlrrestart;
                api::catch_event(scip, EVENTTYPE_BESTSOLFOUND, eventhdlrrestart, conshdlr, None)?;
            }
        }
    }

    Ok(())
}

/// `consExitsolIndicator`: solving process deinitialization method of
/// constraint handler (called before branch and bound process data is
/// freed).
fn cons_exitsol_indicator(scip: &mut Scip, conshdlr: ConshdlrId, _conss: &[ConsId], _restart: bool) -> ScipResult {
    if hd(scip, conshdlr).sepaalternativelp {
        unimplemented!("constraints/indicator/sepaalternativelp (free the alternative LP)");
    }
    Ok(())
}

/// `consDeleteIndicator`: frees specific constraint data.
fn cons_delete_indicator(scip: &mut Scip, conshdlr: ConshdlrId, cons: ConsId, consdata: &mut Option<Box<dyn Any>>) -> ScipResult {
    let mut data = *consdata.take().unwrap().downcast::<ConsData>().unwrap();

    // Make sure that the hash for binary variables is freed (binslackvarhash is
    // only used with usesameslackvar).

    // Drop events on transformed variables.
    if api::cons_is_transformed(scip, cons) {
        if hd(scip, conshdlr).sepaalternativelp {
            unimplemented!("constraints/indicator/sepaalternativelp (deleteAltLPConstraint)");
        }

        // Free events only in correct stages.
        if scip.set.stage >= Stage::Transforming && scip.set.stage <= Stage::ExitSolve {
            let (eventhdlrbound, eventhdlrrestart, eventhdlrlinconsbound, forcerestart, linconsevents) = {
                let d = hd(scip, conshdlr);
                (d.eventhdlrbound, d.eventhdlrrestart, d.eventhdlrlinconsbound, d.forcerestart, d.linconsevents)
            };
            if data.linconsactive {
                api::drop_var_event(scip, data.binvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, -1)?;
                api::drop_var_event(scip, data.slackvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, -1)?;
            }
            if forcerestart {
                api::drop_var_event(scip, data.binvar, EVENTTYPE_GBDCHANGED, eventhdlrrestart, conshdlr, -1)?;
            }

            // Drop bound change events on variables of linear constraints.
            if linconsevents && data.linconsactive && !data.varswithevents.is_empty() {
                for j in 0..data.nevents as usize {
                    api::drop_var_event(scip, data.varswithevents[j], data.eventtypes[j], eventhdlrlinconsbound, conshdlr, -1)?;
                }
                data.varswithevents = Vec::new();
                data.eventtypes = Vec::new();
                data.nevents = 0;
            }
        }
    }

    // Can there be cases where lincons is NULL, e.g., if presolve found the
    // problem infeasible?

    // Mark linear constraint to be upgrade-able.
    if scip.conss[data.lincons].is_active() {
        scip.conss[data.lincons].add_upgrade_locks(-1);
    }

    // Release linear constraint and slack variable.
    api::release_var(scip, &mut data.slackvar)?;
    scip_cons::release_cons(scip, &mut data.lincons)?;

    Ok(())
}

/// `consTransIndicator`: transforms constraint data into data belonging to
/// the transformed problem.
fn cons_trans_indicator(scip: &mut Scip, conshdlr: ConshdlrId, sourcecons: ConsId, targetcons: &mut ConsId) -> ScipResult {
    // Get data of original constraint.
    let sourcedata = cd(scip, sourcecons).clone();

    // Check for slackvar.
    if sourcedata.slackvar == NONE {
        log::error!("The indicator constraint <{}> needs a slack variable.", scip.conss[sourcecons].name);
        return Err(Retcode::InvalidData);
    }

    // Check for linear constraint.
    if sourcedata.lincons == NONE {
        log::error!("The indicator constraint <{}> needs a linear constraint.", scip.conss[sourcecons].name);
        return Err(Retcode::InvalidData);
    }

    // Create constraint data. Note that the constraint has activeone = TRUE,
    // since the binary variable has been negated already if needed.
    let eventhdlrrestart = hd(scip, conshdlr).eventhdlrrestart;
    let mut consdata = consdata_create(
        scip,
        conshdlr,
        eventhdlrrestart,
        sourcedata.binvar,
        true,
        sourcedata.lessthanineq,
        sourcedata.slackvar,
        sourcedata.lincons,
        sourcedata.linconsactive,
    )?;
    consdata.activeone = sourcedata.activeone;
    let (binvar, slackvar, lincons) = (consdata.binvar, consdata.slackvar, consdata.lincons);

    // Capture slack variable and linear constraint.
    crate::var::var_capture(scip, slackvar);
    scip_cons::capture_cons(scip, lincons);

    // Create transformed constraint with the same flags.
    let s = format!("t_{}", scip.conss[sourcecons].name);
    let flags = cons_flags(scip, sourcecons);
    *targetcons = scip_cons::create_cons(scip, &s, conshdlr, Some(Box::new(consdata)), &flags)?;

    // Catch local bound change events on binary variable.
    if sourcedata.linconsactive {
        let eventhdlrbound = hd(scip, conshdlr).eventhdlrbound;
        api::catch_var_event(scip, binvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, *targetcons, None)?;
        api::catch_var_event(scip, slackvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, *targetcons, None)?;
    }

    // Make sure that binary variable hash exists.
    if hd(scip, conshdlr).sepaalternativelp {
        let target = *targetcons;
        let d = hdm(scip, conshdlr);
        d.binvarhash.get_or_insert_with(Default::default).entry(binvar).or_insert(target);
    }

    Ok(())
}

/// `SCIPgetTransformedCons`: the transformed constraint of a constraint
/// (itself if it is transformed), or `NONE`.
fn get_transformed_cons(scip: &Scip, cons: ConsId) -> ConsId {
    if api::cons_is_transformed(scip, cons) { cons } else { scip.conss[cons].transorigcons }
}

/// `consInitpreIndicator`: presolving initialization method of constraint
/// handler (called when presolving is about to begin).
fn cons_initpre_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId]) -> ScipResult {
    if api::get_status(scip) != crate::types::Status::Unknown {
        return Ok(());
    }

    // Check each constraint and get transformed linear constraint.
    for &cons in conss {
        // If not happened already, get transformed linear constraint; in a
        // restart the linear constraint might already be transformed.
        let lincons = cd(scip, cons).lincons;
        if !api::cons_is_transformed(scip, lincons) {
            let translincons = get_transformed_cons(scip, lincons);
            let mut old = lincons;
            scip_cons::release_cons(scip, &mut old)?;
            scip_cons::capture_cons(scip, translincons);
            cdm(scip, cons).lincons = translincons;
        }
    }

    // Reset flag, in case presolve was called for some problem before.
    hdm(scip, conshdlr).addedcouplingcons = false;

    Ok(())
}

/// `consPresolIndicator`: presolving method of constraint handler.
///
/// For an indicator constraint with binary variable \f$y\f$ and slack
/// variable \f$s\f$ the coupling inequality \f$s \le M (1-y)\f$
/// (equivalently: \f$s + M y \le M\f$) is inserted, where \f$M\f$ is an
/// upper bound on the value of \f$s\f$. If \f$M\f$ is too large the
/// inequality is not inserted. Depending on the parameter @a
/// addcouplingcons we add a variable upper bound or a row (in
/// consInitlpIndicator()).
///
/// @warning We can never delete linear constraints, because we need them to
/// get the right values for the slack variables!
#[allow(clippy::too_many_arguments)]
fn cons_presol_indicator(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    nrounds: i32,
    _presoltiming: PresolTiming,
    newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only run if success is possible.
    if nrounds == 0 || newcounts.nfixedvars > 0 || newcounts.nchgbds > 0 || newcounts.naggrvars > 0 {
        *result = ScipResultCode::DidNotFind;

        // Check each constraint.
        for &cons in conss {
            // Do nothing if the linear constraint is not active.
            if !cd(scip, cons).linconsactive {
                continue;
            }

            // Add implications if not yet done.
            if !cd(scip, cons).implicationadded {
                let (binvar, slackvar) = (cd(scip, cons).binvar, cd(scip, cons).slackvar);
                let (cutoff, nbnds) = scip_var::add_var_implication(scip, binvar, true, slackvar, BoundType::Upper, 0.0)?;
                counts.nchgbds += nbnds;

                // Cutoff/infeasible might be true if preprocessing was truncated.
                // Note: nbdchgs == 0 is not necessarily true, because preprocessing
                // might be truncated.
                cdm(scip, cons).implicationadded = true;
                if cutoff {
                    *result = ScipResultCode::Cutoff;
                    return Ok(());
                }
            }

            // Determine integrality of slack variable if not yet done.
            if !cd(scip, cons).slacktypechecked {
                cdm(scip, cons).slacktypechecked = true;
                let slackvar = cd(scip, cons).slackvar;
                if !crate::var::var_is_implied_integral(scip, slackvar) {
                    let lincons = cd(scip, cons).lincons;
                    let vars = cons_linear::get_vars_linear(scip, lincons).to_vec();
                    let vals = cons_linear::get_vals_linear(scip, lincons).to_vec();
                    let nvars = vars.len();
                    let mut foundslackvar = false;

                    let mut j = 0;
                    while j < nvars {
                        if vars[j] == slackvar {
                            foundslackvar = true;
                        } else if !crate::var::var_is_integral(scip, vars[j]) || !scip.set.is_integral(vals[j]) {
                            break;
                        }
                        j += 1;
                    }

                    // Something is strange if the slack variable does not appear in
                    // the linear constraint (possibly because it is an artificial
                    // constraint).
                    if j == nvars && foundslackvar {
                        let lb = scip.vars[slackvar].glbdom.lb;
                        let ub = scip.vars[slackvar].glbdom.ub;
                        if (scip.set.is_infinity(-lb) || scip.set.is_integral(lb)) && (scip.set.is_infinity(ub) || scip.set.is_integral(ub)) {
                            // Don't assert feasibility here because the presolver should
                            // detect infeasibility.
                            scip_var::chg_var_impl_type(scip, slackvar, crate::types::ImplintType::Weak)?;
                        } else {
                            // It can happen that the bounds of the slack variable have
                            // been changed to be non-integral in previous presolving
                            // steps. We then might get a problem with tightening the
                            // bounds. In this case, we just leave the slack variable to be
                            // continuous.
                        }
                    }
                }
            }

            // Perform one presolving round.
            let dualreductions = hd(scip, conshdlr).dualreductions && scip_var::allow_strong_dual_reds(scip);
            let (cutoff, success) = presol_round_indicator(scip, conshdlr, cons, dualreductions, &mut counts.ndelconss, &mut counts.nfixedvars)?;

            if cutoff {
                *result = ScipResultCode::Cutoff;
                return Ok(());
            }
            if success {
                *result = ScipResultCode::Success;
            }
        }
    }

    // Determine whether other methods have found reductions.
    let noreductions = newcounts.nfixedvars == 0
        && newcounts.naggrvars == 0
        && newcounts.nchgvartypes == 0
        && newcounts.nchgbds == 0
        && newcounts.ndelconss == 0
        && newcounts.nchgcoefs == 0
        && newcounts.nchgsides == 0;

    // Add variable upper bounds after bounds are likely to be strengthened.
    if noreductions && *result != ScipResultCode::Success && hd(scip, conshdlr).addcouplingcons && !hd(scip, conshdlr).addedcouplingcons {
        // Create variable upper bounds, possibly removing indicator constraints.
        let ngen = create_var_ubs(scip, conshdlr, conss)?;

        if ngen > 0 {
            *result = ScipResultCode::Success;
            counts.nupgdconss += ngen;
            if hd(scip, conshdlr).removeindicators {
                counts.ndelconss += ngen;
            }
        }
        hdm(scip, conshdlr).addedcouplingcons = true;
    }

    Ok(())
}

/// `consInitlpIndicator`: LP initialization method of constraint handler
/// (called before the initial LP relaxation at a node is solved).
///
/// For an indicator constraint with binary variable \f$y\f$ and slack
/// variable \f$s\f$ the coupling inequality \f$s \le M (1-y)\f$
/// (equivalently: \f$s + M y \le M\f$) is inserted, where \f$M\f$ is an
/// upper bound on the value of \f$s\f$. If \f$M\f$ is too large the
/// inequality is not inserted.
fn cons_initlp_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    // Check whether coupling constraints should be added.
    if !hd(scip, conshdlr).addcoupling {
        return Ok(());
    }

    // Check whether coupling constraints have been added already.
    if hd(scip, conshdlr).addcouplingcons && hd(scip, conshdlr).addedcouplingcons {
        return Ok(());
    }

    // Check each constraint.
    let mut c = 0;
    while c < conss.len() && !*infeasible {
        let cons = conss[c];

        // Do not add inequalities if there are no linear constraints (no slack
        // variable available).
        if !cd(scip, cons).linconsactive {
            c += 1;
            continue;
        }

        // Get upper bound for slack variable in linear constraint.
        let (slackvar, binvar) = (cd(scip, cons).slackvar, cd(scip, cons).binvar);
        let ub = scip.vars[slackvar].glbdom.ub;

        // Insert corresponding row if helpful and coefficient is not too large.
        if ub <= hd(scip, conshdlr).maxcouplingvalue {
            // The reference is built without NDEBUG: the name is set.
            let name = format!("couple{c}");
            let inf = scip.set.infinity();

            // Add variable upper bound if required.
            if hd(scip, conshdlr).addcouplingcons {
                let flags = ConsFlags {
                    initial: true,
                    separate: true,
                    enforce: true,
                    check: false,
                    propagate: true,
                    local: false,
                    modifiable: false,
                    dynamic: true,
                    removable: true,
                    stickingatnode: false,
                };
                let mut vcons = crate::cons_varbound::create_cons_varbound(scip, &name, slackvar, binvar, ub, -inf, ub, &flags)?;
                scip_cons::add_cons(scip, vcons)?;
                scip_cons::release_cons(scip, &mut vcons)?;
            } else {
                let mut row = scip_lp::create_empty_row_cons(scip, cons, &name, -inf, ub, false, false, false)?;
                scip_lp::cache_row_extensions(scip, row);
                api::add_var_to_row(scip, row, slackvar, 1.0)?;
                api::add_var_to_row(scip, row, binvar, ub)?;
                scip_lp::flush_row_extensions(scip, row)?;
                *infeasible = scip_lp::add_row(scip, row, false)?;
                scip_lp::release_row(scip, &mut row)?;
            }
        }
        c += 1;
    }

    Ok(())
}

/// `consSepalpIndicator`: separation method of constraint handler for LP
/// solutions.
fn cons_sepalp_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, result: &mut ScipResultCode) -> ScipResult {
    // Perform separation.
    *result = separate_indicators(scip, conshdlr, conss, nusefulconss, NONE, EnfoSepaType::SepaLp)?;
    Ok(())
}

/// `consSepasolIndicator`: separation method of constraint handler for
/// arbitrary primal solutions.
fn cons_sepasol_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], nusefulconss: i32, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    // Perform separation.
    *result = separate_indicators(scip, conshdlr, conss, nusefulconss, sol, EnfoSepaType::SepaSol)?;
    Ok(())
}

/// `consEnfolpIndicator`: constraint enforcing method of constraint handler
/// for LP solutions.
fn cons_enfolp_indicator(scip: &mut Scip, conshdlr: ConshdlrId, conss: &[ConsId], _nusefulconss: i32, solinfeasible: bool, result: &mut ScipResultCode) -> ScipResult {
    if solinfeasible {
        *result = ScipResultCode::Feasible;
        return Ok(());
    }

    let genlogicor = hd(scip, conshdlr).genlogicor;
    *result = enforce_indicators(scip, conshdlr, conss, NONE, EnfoSepaType::EnfoLp, genlogicor)?;
    Ok(())
}

/// `consEnforelaxIndicator`: constraint enforcing method of constraint
/// handler for relaxation solutions.
fn cons_enforelax_indicator(
    scip: &mut Scip,
    sol: SolId,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    solinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    if solinfeasible {
        *result = ScipResultCode::Feasible;
        return Ok(());
    }

    let genlogicor = hd(scip, conshdlr).genlogicor;
    *result = enforce_indicators(scip, conshdlr, conss, sol, EnfoSepaType::EnfoRelax, genlogicor)?;
    Ok(())
}

/// `consEnfopsIndicator`: constraint enforcing method of constraint handler
/// for pseudo solutions.
fn cons_enfops_indicator(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    solinfeasible: bool,
    objinfeasible: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    if solinfeasible {
        *result = ScipResultCode::Feasible;
        return Ok(());
    }

    if objinfeasible {
        *result = ScipResultCode::DidNotRun;
        return Ok(());
    }

    *result = enforce_indicators(scip, conshdlr, conss, NONE, EnfoSepaType::EnfoPs, true)?;
    Ok(())
}

/// `consCheckIndicator`: feasibility check method of constraint handler for
/// integral solutions.
#[allow(clippy::too_many_arguments)]
fn cons_check_indicator(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    sol: SolId,
    _checkintegrality: bool,
    _checklprows: bool,
    printreason: bool,
    completely: bool,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut trysol = NONE;

    // Try to repair solution below, if it makes sense (will send solution to
    // trysol heuristic in any case (see below).
    let (trysolutions, heurtrysol) = (hd(scip, conshdlr).trysolutions, hd(scip, conshdlr).heurtrysol);
    if scip.set.stage > Stage::Problem && scip.set.stage < Stage::Solved && trysolutions && heurtrysol != NONE {
        trysol = api::create_sol_copy(scip, sol)?;
    }

    // Check each constraint.
    *result = ScipResultCode::Feasible;
    let mut changedsol = false;
    let mut somelinconsnotactive = false;
    for &cons in conss {
        // If the linear constraint has not been generated, we do nothing.
        if !cd(scip, cons).linconsactive {
            somelinconsnotactive = true;
            continue;
        }

        // If printreason is true it can happen that non-integral solutions are
        // checked.

        // If constraint is infeasible.
        let (binvar, slackvar) = (cd(scip, cons).binvar, cd(scip, cons).slackvar);
        if !scip.set.is_feas_zero(api::get_sol_val(scip, sol, binvar)) && !scip.set.is_feas_zero(api::get_sol_val(scip, sol, slackvar)) {
            let absviol = api::get_sol_val(scip, sol, slackvar).abs();
            let relviol = crate::misc::rel_diff(absviol, 0.0);
            if sol != NONE {
                api::update_sol_cons_violation(scip, sol, absviol, relviol);
            }

            api::reset_cons_age(scip, cons);
            *result = ScipResultCode::Infeasible;

            if printreason {
                log::info!(
                    "violation:  <{}> = {} and <{}> = {:.15}",
                    scip.vars[binvar].name,
                    api::get_sol_val(scip, sol, binvar),
                    scip.vars[slackvar].name,
                    api::get_sol_val(scip, sol, slackvar)
                );
            }

            // Try to make solution feasible if it makes sense - otherwise exit.
            if trysol != NONE {
                let changed = make_indicator_feasible(scip, cons, trysol)?;
                changedsol = changedsol || changed;
            } else if !completely {
                return Ok(());
            }
        } else if trysol != NONE {
            let changed = make_indicator_feasible(scip, cons, trysol)?;
            changedsol = changedsol || changed;
        }
    }

    // If some linear constraints are not active, we need to check feasibility
    // via the alternative polyhedron.
    if somelinconsnotactive {
        // The check maybe called before we have build the alternative polyhedron
        // -> return SCIP_INFEASIBLE (the alternative LP is not ported, so it never
        // exists).
        *result = ScipResultCode::Infeasible;
    } else {
        // Tell heur_trysol about solution - it will pass it to SCIP.
        if trysol != NONE && changedsol {
            crate::heur_trysol::heur_pass_sol_try_sol(scip, heurtrysol, trysol)?;
        }
    }

    if trysol != NONE {
        api::free_sol(scip, trysol)?;
    }

    // At this point we are feasible or infeasible.
    Ok(())
}

/// `consPropIndicator`: domain propagation method of constraint handler.
fn cons_prop_indicator(
    scip: &mut Scip,
    conshdlr: ConshdlrId,
    conss: &[ConsId],
    _nusefulconss: i32,
    _nmarkedconss: i32,
    _proptiming: PropTiming,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut ngen = 0;

    *result = ScipResultCode::DidNotRun;

    // Avoid propagation if no bound has changed.
    if !hd(scip, conshdlr).boundhaschanged && !crate::tree::tree_in_repropagation(scip) && !hd(scip, conshdlr).linconsboundschanged {
        *result = ScipResultCode::DidNotFind;
        return Ok(());
    }

    // If first time called, add events on variables of linear constraint.
    if !hd(scip, conshdlr).linconsevents {
        let (maxcouplingvalue, eventhdlrlinconsbound) = (hd(scip, conshdlr).maxcouplingvalue, hd(scip, conshdlr).eventhdlrlinconsbound);
        for &cons in conss {
            // If the linear constraint is not present, we continue.
            if !cd(scip, cons).linconsactive {
                continue;
            }

            // Do not add events if upper bound of slackvar is already small.
            let (slackvar, lincons) = (cd(scip, cons).slackvar, cd(scip, cons).lincons);
            if scip.vars[slackvar].locdom.ub <= maxcouplingvalue {
                continue;
            }

            // Do not add events if it is not a <= inequality; we do not propagate
            // in this case.
            if scip.set.is_infinity(cons_linear::get_rhs_linear(scip, lincons)) {
                continue;
            }

            let vars = cons_linear::get_vars_linear(scip, lincons).to_vec();
            let vals = cons_linear::get_vals_linear(scip, lincons).to_vec();

            // Alloc memory to store events on variables of linear constraint;
            // otherwise we can not drop the events when the cons is deleted.
            for (j, &var) in vars.iter().enumerate() {
                if var == slackvar {
                    continue;
                }

                // Catch only bound changes which are important for propagation of
                // max activity to upper bound of slackvar.
                let eventtype = if vals[j] > 0.0 { EVENTTYPE_UBTIGHTENED } else { EVENTTYPE_LBTIGHTENED };
                api::catch_var_event(scip, var, eventtype, eventhdlrlinconsbound, conshdlr, None)?;
                let c = cdm(scip, cons);
                c.varswithevents.push(var);
                c.eventtypes.push(eventtype);
                c.nevents += 1;
            }
        }
        hdm(scip, conshdlr).linconsevents = true;
    }

    // Already mark that no bound has changed.
    hdm(scip, conshdlr).boundhaschanged = false;
    hdm(scip, conshdlr).linconsboundschanged = false;

    let dualreductions = hd(scip, conshdlr).dualreductions && scip_var::allow_strong_dual_reds(scip);
    let addopposite = hd(scip, conshdlr).addopposite;

    // Check each constraint.
    for &cons in conss {
        *result = ScipResultCode::DidNotFind;

        let (cutoff, cnt) = prop_indicator(scip, conshdlr, cons, dualreductions, addopposite)?;

        if cutoff {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
        ngen += cnt;
    }

    if ngen > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/// `consRespropIndicator`: propagation conflict resolving method of
/// constraint handler.
///
/// We check which bound changes were the reason for infeasibility. We use
/// that @a inferinfo is 0 if the binary variable has bounds that fix it to
/// be nonzero (these bounds are the reason). Likewise @a inferinfo is 1 if
/// the slack variable has bounds that fix it to be nonzero.
#[allow(clippy::too_many_arguments)]
fn cons_resprop_indicator(
    scip: &mut Scip,
    _conshdlr: ConshdlrId,
    cons: ConsId,
    _infervar: VarId,
    inferinfo: i32,
    _boundtype: BoundType,
    bdchgidx: Option<crate::var::BdChgIdx>,
    _relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    let (binvar, slackvar, lincons) = (cd(scip, cons).binvar, cd(scip, cons).slackvar, cd(scip, cons).lincons);

    // If the binary variable was the reason.
    if inferinfo == 0 {
        api::add_conflict_lb(scip, binvar, bdchgidx.as_ref())?;
    } else if inferinfo == 1 {
        // If the slack variable fixed to a positive value was the reason. Use a
        // weaker comparison to SCIPgetVarLbAtIndex here (i.e., SCIPisPositive
        // instead of SCIPisFeasPositive), because SCIPgetVarLbAtIndex might
        // differ from the local bound at time bdchgidx by epsilon.
        api::add_conflict_lb(scip, slackvar, bdchgidx.as_ref())?;
    } else if inferinfo == 2 {
        api::add_conflict_ub(scip, slackvar, bdchgidx.as_ref())?;
    } else {
        // Mark variables in linear constraint.
        let linconsvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
        let linconsvals = cons_linear::get_vals_linear(scip, lincons).to_vec();

        for (j, &var) in linconsvars.iter().enumerate() {
            if linconsvals[j] > 0.0 {
                api::add_conflict_ub(scip, var, bdchgidx.as_ref())?;
            } else {
                api::add_conflict_lb(scip, var, bdchgidx.as_ref())?;
            }
        }
    }

    *result = ScipResultCode::Success;

    Ok(())
}

/// `consLockIndicator`: variable rounding lock method of constraint handler.
///
/// The up-rounding of the binary and slack variable may violate the
/// constraint. If the linear constraint is not active, we lock all
/// variables in the depending constraint - otherwise they will be fixed by
/// dual presolving methods.
fn cons_lock_indicator(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, locktype: LockType, nlockspos: i32, nlocksneg: i32) -> ScipResult {
    let (binvar, slackvar, lincons, linconsactive) = {
        let c = cd(scip, cons);
        (c.binvar, c.slackvar, c.lincons, c.linconsactive)
    };

    api::add_var_locks_type(scip, binvar, locktype, nlocksneg, nlockspos)?;

    if linconsactive {
        api::add_var_locks_type(scip, slackvar, locktype, nlocksneg, nlockspos)?;
    } else {
        let linvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
        let linvals = cons_linear::get_vals_linear(scip, lincons).to_vec();
        let haslhs = !scip.set.is_infinity(cons_linear::get_lhs_linear(scip, lincons).abs());
        let hasrhs = !scip.set.is_infinity(cons_linear::get_rhs_linear(scip, lincons).abs());

        for (j, &var) in linvars.iter().enumerate() {
            if scip.set.is_positive(linvals[j]) {
                if haslhs {
                    api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
                }
                if hasrhs {
                    api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
                }
            } else {
                if haslhs {
                    api::add_var_locks_type(scip, var, locktype, nlocksneg, nlockspos)?;
                }
                if hasrhs {
                    api::add_var_locks_type(scip, var, locktype, nlockspos, nlocksneg)?;
                }
            }
        }
    }

    Ok(())
}

/// `consCopyIndicator`: constraint copying method of constraint handler.
#[allow(clippy::too_many_arguments)]
fn cons_copy_indicator(
    scip: &mut Scip,
    cons: &mut ConsId,
    name: Option<&str>,
    sourcescip: &mut Scip,
    _sourceconshdlr: ConshdlrId,
    sourcecons: ConsId,
    maps: &mut crate::copy::CopyMaps,
    flags: &ConsFlags,
    global: bool,
    valid: &mut bool,
) -> ScipResult {
    let mut targetlincons;
    let mut targetbinvar = NONE;
    let mut targetslackvar = NONE;

    *valid = true;

    let consname = name.map_or_else(|| sourcescip.conss[sourcecons].name.clone(), str::to_owned);

    if flags.modifiable {
        log::warn!(
            "cannot create modifiable indicator constraint when trying to copy constraint <{}>,",
            sourcescip.conss[sourcecons].name
        );
        *valid = false;
        return Ok(());
    }

    // Get linear constraint.
    let mut sourcelincons = cd(sourcescip, sourcecons).lincons;

    // If the constraint has been deleted -> create empty constraint
    // (multi-aggregation might still contain slack variable, so indicator is
    // valid).
    if sourcescip.conss[sourcelincons].deleted {
        let inf = scip.set.infinity();
        targetlincons = cons_linear::create_cons_linear(scip, "dummy", &[], &[], 0.0, inf, false, false, false, false, false, false, false, false, false, false)?;
        scip_cons::add_cons(scip, targetlincons)?;
    } else {
        // Get copied version of linear constraint.
        let conshdlrlinear = crate::set::set_find_conshdlr(sourcescip, "linear");

        // If copying scip after transforming the original instance before
        // presolving, we need to correct the linear constraint pointer.
        if api::cons_is_transformed(sourcescip, sourcecons) && !api::cons_is_transformed(sourcescip, sourcelincons) {
            // Adjust the linear constraint in the original constraint (no need to
            // release translincons).
            let translincons = get_transformed_cons(sourcescip, sourcelincons);
            let mut old = sourcelincons;
            scip_cons::release_cons(sourcescip, &mut old)?;
            scip_cons::capture_cons(sourcescip, translincons);
            cdm(sourcescip, sourcecons).lincons = translincons;
            sourcelincons = translincons;
        }

        let linname = sourcescip.conss[sourcelincons].name.clone();
        let linflags = cons_flags(sourcescip, sourcelincons);
        (targetlincons, *valid) = crate::copy::get_cons_copy(sourcescip, scip, sourcelincons, conshdlrlinear, maps, Some(&linname), &linflags, global)?;
    }

    // Find copied variable corresponding to binvar.
    if *valid {
        let sourcebinvar = cd(sourcescip, sourcecons).binvar;
        (targetbinvar, *valid) = crate::copy::get_var_copy(sourcescip, scip, sourcebinvar, Some(maps), global)?;
    }

    // Find copied variable corresponding to slackvar.
    if *valid {
        let sourceslackvar = cd(sourcescip, sourcecons).slackvar;
        (targetslackvar, *valid) = crate::copy::get_var_copy(sourcescip, scip, sourceslackvar, Some(maps), global)?;
    }

    // Create indicator constraint.
    if *valid {
        // Creates indicator constraint (and captures the linear constraint). Note
        // that the copied constraint has activeone = TRUE, since the target
        // binary variable already was negated if needed.
        *cons = create_cons_indicator_generic_lin_cons(scip, &consname, targetbinvar, targetlincons, targetslackvar, true, flags)?;
    } else {
        log::info!("could not copy linear constraint <{}>", sourcescip.conss[sourcelincons].name);
    }

    // Release copied linear constraint.
    if targetlincons != NONE {
        scip_cons::release_cons(scip, &mut targetlincons)?;
    }

    Ok(())
}

/// `consEnableIndicator`: constraint enabling notification method of
/// constraint handler (the alternative LP never exists).
fn cons_enable_indicator(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId) -> ScipResult {
    Ok(())
}

/// `consDisableIndicator`: constraint disabling notification method of
/// constraint handler (the alternative LP never exists).
fn cons_disable_indicator(_scip: &mut Scip, _conshdlr: ConshdlrId, _cons: ConsId) -> ScipResult {
    Ok(())
}

/// `consGetVarsIndicator`: constraint method of constraint handler which
/// returns the variables (if possible).
fn cons_get_vars_indicator(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, vars: &mut Vec<VarId>, success: &mut bool) -> ScipResult {
    *success = true;

    // If indicator constraint is already deleted.
    if scip.conss[cons].deleted {
        return Ok(());
    }

    let (binvar, slackvar, lincons) = (cd(scip, cons).binvar, cd(scip, cons).slackvar, cd(scip, cons).lincons);

    if binvar != NONE {
        vars.push(binvar);
    }
    if slackvar != NONE {
        vars.push(slackvar);
    }

    // If linear constraint of indicator is already deleted.
    if scip.conss[lincons].deleted {
        return Ok(());
    }

    let (linvars, linsuccess) = crate::cons::cons_get_vars(scip, lincons)?;
    vars.extend(linvars);
    *success = linsuccess;

    Ok(())
}

/// `consGetNVarsIndicator`: constraint method of constraint handler which
/// returns the number of variables (if possible).
fn cons_get_n_vars_indicator(scip: &mut Scip, _conshdlr: ConshdlrId, cons: ConsId, nvars: &mut i32, success: &mut bool) -> ScipResult {
    *success = true;
    *nvars = 0;

    // If indicator constraint is already deleted.
    if scip.conss[cons].deleted {
        return Ok(());
    }

    let (binvar, slackvar, lincons) = (cd(scip, cons).binvar, cd(scip, cons).slackvar, cd(scip, cons).lincons);

    if binvar != NONE {
        *nvars += 1;
    }
    if slackvar != NONE {
        *nvars += 1;
    }

    // If linear constraint of indicator is already deleted.
    if scip.conss[lincons].deleted {
        return Ok(());
    }

    let (nlinvars, linsuccess) = crate::cons::cons_get_n_vars(scip, lincons)?;
    *success = linsuccess;

    if *success {
        *nvars += nlinvars;
    }

    Ok(())
}

/// `consGetDiveBdChgsIndicator`: constraint handler method to suggest dive
/// bound changes during the generic diving algorithm.
fn cons_get_dive_bd_chgs_indicator(scip: &mut Scip, conshdlr: ConshdlrId, diveset: usize, sol: SolId, success: &mut bool, infeasible: &mut bool) -> ScipResult {
    let mut bestvar = NONE;
    let mut bestvarroundup = false;
    let mut bestscore = crate::def::REAL_MIN;

    *success = false;
    *infeasible = false;

    let indconss = conshdlr_get_conss(scip, conshdlr);

    // Loop over indicator constraints and score indicator variables with
    // already integral solution value.
    for &cons in &indconss {
        // Check whether constraint is violated.
        if is_violated_indicator(scip, cons, sol) {
            let binvar = get_binary_var_indicator(scip, cons);
            let solval = api::get_sol_val(scip, sol, binvar);

            // We only treat indicator variables with integral solution values that
            // are not yet fixed.
            if scip.set.is_feas_integral(solval) && scip.vars[binvar].locdom.lb < scip.vars[binvar].locdom.ub - 0.5 {
                let (score, roundup) = crate::heur::diveset_get_score(scip, diveset, crate::heur::DIVETYPE_INTEGRALITY, binvar, solval, 0.0)?;

                // Best candidate maximizes the score.
                if score > bestscore {
                    bestscore = score;
                    *success = true;
                    bestvar = binvar;
                    bestvarroundup = roundup;
                }
            }
        }
    }

    if *success {
        // If the diving score voted for fixing the best variable to 1.0, we add
        // this as the preferred bound change.
        api::add_dive_bound_change(scip, bestvar, crate::types::BranchDir::Upwards, 1.0, bestvarroundup);
        api::add_dive_bound_change(scip, bestvar, crate::types::BranchDir::Downwards, 0.0, !bestvarroundup);
    }

    Ok(())
}

/// `SCIPconshdlrGetConss` (all constraints of the handler, active ones
/// first).
fn conshdlr_get_conss(scip: &Scip, conshdlr: ConshdlrId) -> Vec<ConsId> {
    let h = &scip.conshdlrs[conshdlr];
    h.conss.0[..h.nconss as usize].to_vec()
}

/* ---------------- Constraint specific interface methods ---------------- */

/// `SCIPincludeConshdlrIndicator`: creates the handler for indicator
/// constraints and includes it in SCIP.
pub fn include_conshdlr_indicator(scip: &mut Scip) -> ScipResult {
    // Create event handler for bound change events.
    let eventhdlrbound = api::include_eventhdlr_basic(scip, EVENTHDLR_BOUND_NAME, EVENTHDLR_BOUND_DESC, event_exec_indicator_bound, None)?;

    // Create event handler for bound change events on linear constraint.
    let eventhdlrlinconsbound = api::include_eventhdlr_basic(scip, EVENTHDLR_LINCONSBOUND_NAME, EVENTHDLR_LINCONSBOUND_DESC, event_exec_indicator_lincons_bound, None)?;

    // Create event handler for restart events.
    let eventhdlrrestart = api::include_eventhdlr_basic(scip, EVENTHDLR_RESTART_NAME, EVENTHDLR_RESTART_DESC, event_exec_indicator_restart, None)?;

    // Create constraint handler data (the parameters' storage is filled when
    // they are added below).
    let conshdlrdata = ConshdlrData {
        eventhdlrbound,
        eventhdlrlinconsbound,
        eventhdlrrestart,
        boundhaschanged: false,
        linconsevents: false,
        linconsboundschanged: false,
        removable: false,
        scaled: false,
        objindicatoronly: false,
        objothervarsonly: false,
        minabsobj: 0.0,
        nrows: 0,
        nlbbounds: 0,
        nubbounds: 0,
        binvarhash: None,
        nslackvars: 0,
        niiscutsgen: 0,
        nperspcutsgen: 0,
        objcutindex: 0,
        objupperbound: 0.0,
        objaltlpbound: 0.0,
        maxroundingrounds: 0,
        roundingminthres: 0.0,
        roundingmaxthres: 0.0,
        roundingoffset: 0.0,
        branchindicators: false,
        genlogicor: false,
        addcoupling: false,
        addcouplingcons: false,
        sepacouplingcuts: false,
        sepacouplinglocal: false,
        sepaperspective: false,
        sepapersplocal: false,
        removeindicators: false,
        updatebounds: false,
        trysolutions: false,
        enforcecuts: false,
        dualreductions: false,
        addopposite: false,
        generatebilinear: false,
        scaleslackvar: false,
        conflictsupgrade: false,
        performedrestart: false,
        maxsepacuts: 0,
        maxsepacutsroot: 0,
        maxsepanonviolated: 0,
        nbinvarszero: 0,
        ninitconss: 0,
        maxcouplingvalue: 0.0,
        sepacouplingvalue: 0.0,
        maxconditionaltlp: 0.0,
        restartfrac: 0.0,
        heurtrysol: NONE,
        addedcouplingcons: false,
        addlincons: Vec::new(),
        useotherconss: false,
        useobjectivecut: false,
        trysolfromcover: false,
        upgradelinear: false,
        normtype: 0,
        usesameslackvar: false,
        sepaalternativelp: DEFAULT_SEPAALTERNATIVELP,
        sepaalternativelp_: false,
        nolinconscont: DEFAULT_NOLINCONSCONT,
        nolinconscont_: false,
        forcerestart: DEFAULT_FORCERESTART,
        forcerestart_: false,
    };

    // Include constraint handler.
    let conshdlr = include_conshdlr_basic(
        scip,
        CONSHDLR_NAME,
        CONSHDLR_DESC,
        CONSHDLR_ENFOPRIORITY,
        CONSHDLR_CHECKPRIORITY,
        CONSHDLR_EAGERFREQ,
        CONSHDLR_NEEDSCONS,
        Some(cons_enfolp_indicator),
        Some(cons_enfops_indicator),
        Some(cons_check_indicator),
        Some(cons_lock_indicator),
        Some(Box::new(conshdlrdata)),
    )?;

    // Initialize constraint handler data.
    init_conshdlr_data(scip, conshdlr);

    // Set non-fundamental callbacks via specific setter functions.
    let h = &mut scip.conshdlrs[conshdlr];
    h.conshdlrcopy = Some(conshdlr_copy_indicator);
    h.conscopy = Some(cons_copy_indicator);
    h.consdelete = Some(cons_delete_indicator);
    h.consdisable = Some(cons_disable_indicator);
    h.consenable = Some(cons_enable_indicator);
    h.consgetdivebdchgs = Some(cons_get_dive_bd_chgs_indicator);
    h.consexit = Some(cons_exit_indicator);
    h.consexitsol = Some(cons_exitsol_indicator);
    h.consfree = Some(cons_free_indicator);
    h.consgetvars = Some(cons_get_vars_indicator);
    h.consgetnvars = Some(cons_get_n_vars_indicator);
    h.consinit = Some(cons_init_indicator);
    h.consinitpre = Some(cons_initpre_indicator);
    h.consinitsol = Some(cons_initsol_indicator);
    h.consinitlp = Some(cons_initlp_indicator);
    set_conshdlr_presol(scip, conshdlr, Some(cons_presol_indicator), CONSHDLR_MAXPREROUNDS, CONSHDLR_PRESOLTIMING)?;
    set_conshdlr_prop(scip, conshdlr, Some(cons_prop_indicator), CONSHDLR_PROPFREQ, CONSHDLR_DELAYPROP, CONSHDLR_PROP_TIMING)?;
    scip.conshdlrs[conshdlr].consresprop = Some(cons_resprop_indicator);
    set_conshdlr_sepa(
        scip,
        conshdlr,
        Some(cons_sepalp_indicator),
        Some(cons_sepasol_indicator),
        CONSHDLR_SEPAFREQ,
        CONSHDLR_SEPAPRIORITY,
        CONSHDLR_DELAYSEPA,
    )?;
    scip.conshdlrs[conshdlr].constrans = Some(cons_trans_indicator);
    scip.conshdlrs[conshdlr].consenforelax = Some(cons_enforelax_indicator);

    // Add upgrading method.
    if crate::set::set_find_conshdlr(scip, "linear") != NONE {
        // Include the linear constraint upgrade in the linear constraint handler.
        cons_linear::include_lincons_upgrade(scip, lincons_upgd_indicator, LINCONSUPGD_PRIORITY, CONSHDLR_NAME)?;
    }

    // Create conflict handler for indicator constraints.
    let conflicthdlrdata = ConflicthdlrData { conshdlr };
    let conflicthdlr = api::include_conflicthdlr_basic(
        scip,
        CONFLICTHDLR_NAME,
        CONFLICTHDLR_DESC,
        CONFLICTHDLR_PRIORITY,
        conflict_exec_indicator,
        Some(Box::new(conflicthdlrdata)),
    )?;
    scip.conflicthdlrs[conflicthdlr].conflictfree = Some(conflict_free_indicator);

    // Add indicator constraint handler parameters.
    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, conshdlrs, conshdlr, conshdlrdata, ConshdlrData, $f)
        };
    }
    use crate::paramset::{paramset_add_bool, paramset_add_int, paramset_add_real};
    paramset_add_bool(
        scip,
        "constraints/indicator/branchindicators",
        "Branch on indicator constraints in enforcing?",
        t!(bool, branchindicators),
        true,
        DEFAULT_BRANCHINDICATORS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/genlogicor",
        "Generate logicor constraints instead of cuts?",
        t!(bool, genlogicor),
        true,
        DEFAULT_GENLOGICOR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/addcoupling",
        "Add coupling constraints or rows if big-M is small enough?",
        t!(bool, addcoupling),
        true,
        DEFAULT_ADDCOUPLING,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/indicator/maxcouplingvalue",
        "maximum coefficient for binary variable in coupling constraint",
        t!(f64, maxcouplingvalue),
        true,
        DEFAULT_MAXCOUPLINGVALUE,
        0.0,
        1e9,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/addcouplingcons",
        "Add initial variable upper bound constraints, if 'addcoupling' is true?",
        t!(bool, addcouplingcons),
        true,
        DEFAULT_ADDCOUPLINGCONS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/sepacouplingcuts",
        "Should the coupling inequalities be separated dynamically?",
        t!(bool, sepacouplingcuts),
        true,
        DEFAULT_SEPACOUPLINGCUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/sepacouplinglocal",
        "Allow to use local bounds in order to separate coupling inequalities?",
        t!(bool, sepacouplinglocal),
        true,
        DEFAULT_SEPACOUPLINGLOCAL,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/indicator/sepacouplingvalue",
        "maximum coefficient for binary variable in separated coupling constraint",
        t!(f64, sepacouplingvalue),
        true,
        DEFAULT_SEPACOUPLINGVALUE,
        0.0,
        1e9,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/sepaperspective",
        "Separate cuts based on perspective formulation?",
        t!(bool, sepaperspective),
        true,
        DEFAULT_SEPAPERSPECTIVE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/sepapersplocal",
        "Allow to use local bounds in order to separate perspective cuts?",
        t!(bool, sepapersplocal),
        true,
        DEFAULT_SEPAPERSPLOCAL,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/indicator/maxsepanonviolated",
        "maximal number of separated non violated IISs, before separation is stopped",
        t!(i32, maxsepanonviolated),
        false,
        DEFAULT_MAXSEPANONVIOLATED,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/updatebounds",
        "Update bounds of original variables for separation?",
        t!(bool, updatebounds),
        true,
        DEFAULT_UPDATEBOUNDS,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/indicator/maxconditionaltlp",
        "maximum estimated condition of the solution basis matrix of the alternative LP to be trustworthy (0.0 to disable check)",
        t!(f64, maxconditionaltlp),
        true,
        DEFAULT_MAXCONDITIONALTLP,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/indicator/maxsepacuts",
        "maximal number of cuts separated per separation round",
        t!(i32, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/indicator/maxsepacutsroot",
        "maximal number of cuts separated per separation round in the root node",
        t!(i32, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/removeindicators",
        "Remove indicator constraint if corresponding variable bound constraint has been added?",
        t!(bool, removeindicators),
        true,
        DEFAULT_REMOVEINDICATORS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/generatebilinear",
        "Do not generate indicator constraint, but a bilinear constraint instead?",
        t!(bool, generatebilinear),
        true,
        DEFAULT_GENERATEBILINEAR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/scaleslackvar",
        "Scale slack variable coefficient at construction time?",
        t!(bool, scaleslackvar),
        true,
        DEFAULT_SCALESLACKVAR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/trysolutions",
        "Try to make solutions feasible by setting indicator variables?",
        t!(bool, trysolutions),
        true,
        DEFAULT_TRYSOLUTIONS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/enforcecuts",
        "In enforcing try to generate cuts (only if sepaalternativelp is true)?",
        t!(bool, enforcecuts),
        true,
        DEFAULT_ENFORCECUTS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/dualreductions",
        "Should dual reduction steps be performed?",
        t!(bool, dualreductions),
        true,
        DEFAULT_DUALREDUCTIONS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/addopposite",
        "Add opposite inequality in nodes in which the binary variable has been fixed to 0?",
        t!(bool, addopposite),
        true,
        DEFAULT_ADDOPPOSITE,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/conflictsupgrade",
        "Try to upgrade bounddisjunction conflicts by replacing slack variables?",
        t!(bool, conflictsupgrade),
        true,
        DEFAULT_CONFLICTSUPGRADE,
        None,
    )?;
    paramset_add_real(
        scip,
        "constraints/indicator/restartfrac",
        "fraction of binary variables that need to be fixed before restart occurs (in forcerestart)",
        t!(f64, restartfrac),
        true,
        DEFAULT_RESTARTFRAC,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/useotherconss",
        "Collect other constraints to alternative LP?",
        t!(bool, useotherconss),
        true,
        DEFAULT_USEOTHERCONSS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/useobjectivecut",
        "Use objective cut with current best solution to alternative LP?",
        t!(bool, useobjectivecut),
        true,
        DEFAULT_USEOBJECTIVECUT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/trysolfromcover",
        "Try to construct a feasible solution from a cover?",
        t!(bool, trysolfromcover),
        true,
        DEFAULT_TRYSOLFROMCOVER,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/upgradelinear",
        "Try to upgrade linear constraints to indicator constraints?",
        t!(bool, upgradelinear),
        true,
        DEFAULT_UPGRADELINEAR,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/usesameslackvar",
        "Use same slack variable for indicator constraints with common binary variable?",
        t!(bool, usesameslackvar),
        true,
        DEFAULT_USESAMESLACKVAR,
        None,
    )?;

    // Parameters that should not be changed after problem stage.
    paramset_add_bool(
        scip,
        "constraints/indicator/sepaalternativelp",
        "Separate using the alternative LP?",
        t!(bool, sepaalternativelp_),
        true,
        DEFAULT_SEPAALTERNATIVELP,
        Some(param_changed_indicator),
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/forcerestart",
        "Force restart if absolute gap is 1 or enough binary variables have been fixed?",
        t!(bool, forcerestart_),
        true,
        DEFAULT_FORCERESTART,
        Some(param_changed_indicator),
    )?;
    paramset_add_bool(
        scip,
        "constraints/indicator/nolinconscont",
        "Decompose problem (do not generate linear constraint if all variables are continuous)?",
        t!(bool, nolinconscont_),
        true,
        DEFAULT_NOLINCONSCONT,
        Some(param_changed_indicator),
    )?;

    Ok(())
}

/// `SCIPcreateConsIndicator`: creates and captures an indicator constraint.
///
/// @note @a binvar is checked to be binary only later. This enables a change
/// of the type in procedures reading an instance.
pub fn create_cons_indicator(scip: &mut Scip, name: &str, binvar: VarId, vars: &[VarId], vals: &[f64], rhs: f64, flags: &ConsFlags) -> ScipResult<ConsId> {
    create_cons_indicator_generic(scip, name, binvar, vars, vals, rhs, true, true, flags)
}

/// `SCIPcreateConsIndicatorGeneric`: creates and captures a indicator
/// constraint in a more generic version.
///
/// The key difference from SCIPcreateConsIndicator() is the activeone and
/// lessthanineq Booleans. If \f$z = o\f$, with \f$o\f$ the activeone flag,
/// then: if lessthanineq then \f$a^T x \leq b\f$ holds, else the passed
/// vectors are assumed to be of the form \f$a^T x \geq b\f$. The underlying
/// linear constraint is always created as a less-than inequality. The
/// constraint is never modifiable (`flags.modifiable` is ignored).
#[allow(clippy::too_many_arguments)]
pub fn create_cons_indicator_generic(
    scip: &mut Scip,
    name: &str,
    binvar: VarId,
    vars: &[VarId],
    vals: &[f64],
    mut rhs: f64,
    activeone: bool,
    lessthanineq: bool,
    flags: &ConsFlags,
) -> ScipResult<ConsId> {
    let nvars = vars.len();
    let modifiable = false;
    let mut integral = true;
    let mut absvalsum = 0.0;

    // Find the indicator constraint handler.
    let conshdlr = conshdlr_of(scip);
    if conshdlr == NONE {
        log::error!("<{CONSHDLR_NAME}> constraint handler not found");
        return Err(Retcode::PluginNotFound);
    }

    if hd(scip, conshdlr).nolinconscont && !hd(scip, conshdlr).sepaalternativelp {
        log::error!("constraint handler <{CONSHDLR_NAME}>: need parameter <sepaalternativelp> to be true if parameter <nolinconscont> is true.");
        return Err(Retcode::InvalidData);
    }

    if hd(scip, conshdlr).nolinconscont && hd(scip, conshdlr).generatebilinear {
        log::error!("constraint handler <{CONSHDLR_NAME}>: parameters <nolinconscont> and <generatebilinear> cannot both be true.");
        return Err(Retcode::InvalidData);
    }

    let valscopy: Vec<f64> = if lessthanineq {
        vals.to_vec()
    } else {
        // Flip coefficients and RHS of indicator.
        rhs = -rhs;
        vals.iter().map(|&v| -v).collect()
    };

    // Determine integrality of slack variable and whether problem is
    // decomposed if no variables are integral.
    let scaleslackvar = hd(scip, conshdlr).scaleslackvar;
    let mut linconsactive = !hd(scip, conshdlr).nolinconscont;
    for v in 0..nvars {
        if scaleslackvar {
            absvalsum += valscopy[v].abs();
        }
        if crate::var::var_is_integral(scip, vars[v]) {
            linconsactive = true;
            if !scaleslackvar && !integral {
                break;
            }
            if !scip.set.is_integral(valscopy[v]) {
                integral = false;
                if !scaleslackvar {
                    break;
                }
            }
        } else {
            integral = false;
            if !scaleslackvar && linconsactive {
                break;
            }
        }
    }

    // If active on 0, a provided binary variable is negated.
    let binvarinternal = if activeone || binvar == NONE {
        binvar
    } else {
        scip_var::get_negated_var(scip, binvar)?
    };

    // Check whether the same slack variable should be use for constraints with
    // a common binary variable. This can reduce the size of the problem,
    // because only one coupling constraint is needed. However, it is less
    // tight.
    if binvarinternal != NONE && hd(scip, conshdlr).usesameslackvar {
        unimplemented!("constraints/indicator/usesameslackvar");
    }

    // Create slack variable.
    let s = format!("indslack_{name}");
    let inf = scip.set.infinity();
    let slackvar = scip_var::create_var_impl(
        scip,
        &s,
        0.0,
        inf,
        0.0,
        VarType::Continuous,
        if integral { crate::types::ImplintType::Weak } else { crate::types::ImplintType::None },
        true,
        false,
    )?;
    scip_var::add_var(scip, slackvar)?;

    // Mark slack variable not to be multi-aggregated.
    scip_var::mark_do_not_multaggr_var(scip, slackvar)?;

    // Create linear constraint.
    let s = format!("indlin_{name}");

    // If the linear constraint should be activated (lincons is captured).
    let lincons = if linconsactive {
        // The constraint is initial if initial is true, enforced, separated, and
        // checked.
        cons_linear::create_cons_linear(
            scip,
            &s,
            vars,
            &valscopy,
            -inf,
            rhs,
            flags.initial,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
        )?
    } else {
        // Create non-active linear constraint, which is neither initial, nor
        // enforced, nor separated, nor checked.
        cons_linear::create_cons_linear(scip, &s, vars, &valscopy, -inf, rhs, false, false, false, false, false, false, false, false, false, false)?
    };

    // Mark linear constraint not to be upgraded - otherwise we loose control
    // over it.
    scip.conss[lincons].add_upgrade_locks(1);

    // Add slack variable.
    if scaleslackvar && nvars > 0 {
        absvalsum /= nvars as f64;
        if integral {
            absvalsum = scip.set.ceil(absvalsum);
        }
        if scip.set.is_zero(absvalsum) {
            absvalsum = 1.0;
        }
        cons_linear::add_coef_linear(scip, lincons, slackvar, -absvalsum)?;
    } else {
        cons_linear::add_coef_linear(scip, lincons, slackvar, -1.0)?;
    }
    scip_cons::add_cons(scip, lincons)?;

    // Check whether we should generate a bilinear constraint instead of an
    // indicator constraint.
    if hd(scip, conshdlr).generatebilinear {
        unimplemented!("constraints/indicator/generatebilinear (SCIPcreateConsQuadraticNonlinear)");
    }

    // Create constraint data.
    let eventhdlrrestart = hd(scip, conshdlr).eventhdlrrestart;
    let consdata = consdata_create(scip, conshdlr, eventhdlrrestart, binvar, activeone, lessthanineq, slackvar, lincons, linconsactive)?;
    let (cbinvar, cslackvar) = (consdata.binvar, consdata.slackvar);

    // Do not need to capture slack variable and linear constraint here.

    // Create constraint.
    let cflags = ConsFlags { modifiable, ..*flags };
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), &cflags)?;

    if api::is_transformed(scip) {
        // Catch local bound change events on binary variable.
        if linconsactive {
            let eventhdlrbound = hd(scip, conshdlr).eventhdlrbound;
            api::catch_var_event(scip, cbinvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;
            api::catch_var_event(scip, cslackvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;
        }

        // Make sure that binary variable hash exists.
        if hd(scip, conshdlr).sepaalternativelp {
            let d = hdm(scip, conshdlr);
            d.binvarhash.get_or_insert_with(Default::default).entry(binvarinternal).or_insert(cons);
        }
    }

    Ok(cons)
}

/// `SCIPcreateConsBasicIndicator`: creates and captures an indicator
/// constraint in its most basic version, i. e., all constraint flags are set
/// to their basic value as explained for the method
/// SCIPcreateConsIndicator().
pub fn create_cons_basic_indicator(scip: &mut Scip, name: &str, binvar: VarId, vars: &[VarId], vals: &[f64], rhs: f64) -> ScipResult<ConsId> {
    create_cons_indicator(scip, name, binvar, vars, vals, rhs, &ConsFlags::default())
}

/// `SCIPcreateConsIndicatorGenericLinCons`: creates and captures an
/// indicator constraint with given linear constraint and slack variable in a
/// generic version, i.e., with a flag activeone indicating whether the
/// constraint is active on value 1 or 0 of the binary variable.
///
/// @note we assume that @a slackvar actually appears in @a lincons and we
/// also assume that it takes the role of a slack variable!
pub fn create_cons_indicator_generic_lin_cons(
    scip: &mut Scip,
    name: &str,
    binvar: VarId,
    lincons: ConsId,
    slackvar: VarId,
    activeone: bool,
    flags: &ConsFlags,
) -> ScipResult<ConsId> {
    let modifiable = false;

    // Check whether lincons is really a linear constraint.
    let linhdlr = scip.conss[lincons].conshdlr;
    if scip.conshdlrs[linhdlr].name != "linear" {
        log::error!("Lincons constraint is not linear.");
        return Err(Retcode::InvalidData);
    }

    // Find the indicator constraint handler.
    let conshdlr = conshdlr_of(scip);
    if conshdlr == NONE {
        log::error!("<{CONSHDLR_NAME}> constraint handler not found.");
        return Err(Retcode::PluginNotFound);
    }

    if hd(scip, conshdlr).nolinconscont && !hd(scip, conshdlr).sepaalternativelp {
        log::error!("constraint handler <{CONSHDLR_NAME}>: need parameter <sepaalternativelp> to be true if parameter <nolinconscont> is true.");
        return Err(Retcode::InvalidData);
    }

    // Determine whether the linear constraint needs to be active.
    let vars = cons_linear::get_vars_linear(scip, lincons).to_vec();
    let mut linconsactive = !hd(scip, conshdlr).nolinconscont;
    let mut v = 0;
    while v < vars.len() && !linconsactive {
        if vars[v] != slackvar && crate::var::var_is_integral(scip, vars[v]) {
            linconsactive = true;
        }
        v += 1;
    }

    // Note that we do not change the type of the slackvariable here, because it
    // might appear in other constraints and the locks are note yet set up if we
    // are copying.

    // Mark slack variable not to be multi-aggregated.
    scip_var::mark_do_not_multaggr_var(scip, slackvar)?;

    // Mark linear constraint not to be upgraded - otherwise we loose control
    // over it.
    scip.conss[lincons].add_upgrade_locks(1);

    // Check whether we should generate a bilinear constraint instead of an
    // indicator constraint.
    if hd(scip, conshdlr).generatebilinear {
        unimplemented!("constraints/indicator/generatebilinear (SCIPcreateConsQuadraticNonlinear)");
    }

    // Create constraint data.
    let eventhdlrrestart = hd(scip, conshdlr).eventhdlrrestart;
    let consdata = consdata_create(scip, conshdlr, eventhdlrrestart, binvar, activeone, true, slackvar, lincons, linconsactive)?;
    let (cbinvar, cslackvar, clinconsactive) = (consdata.binvar, consdata.slackvar, consdata.linconsactive);

    // Create constraint.
    let cflags = ConsFlags { modifiable, ..*flags };
    let cons = scip_cons::create_cons(scip, name, conshdlr, Some(Box::new(consdata)), &cflags)?;

    // Catch local bound change events on binary variable.
    if clinconsactive && api::is_transformed(scip) {
        let eventhdlrbound = hd(scip, conshdlr).eventhdlrbound;
        api::catch_var_event(scip, cbinvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;
        api::catch_var_event(scip, cslackvar, EVENTTYPE_BOUNDCHANGED, eventhdlrbound, cons, None)?;
    }

    // Capture slack variable and linear constraint.
    crate::var::var_capture(scip, slackvar);
    scip_cons::capture_cons(scip, lincons);

    Ok(cons)
}

/// `SCIPcreateConsIndicatorLinCons`: creates and captures an indicator
/// constraint with given linear constraint and slack variable.
pub fn create_cons_indicator_lin_cons(scip: &mut Scip, name: &str, binvar: VarId, lincons: ConsId, slackvar: VarId, flags: &ConsFlags) -> ScipResult<ConsId> {
    create_cons_indicator_generic_lin_cons(scip, name, binvar, lincons, slackvar, true, flags)
}

/// `SCIPcreateConsBasicIndicatorLinCons`: creates and captures an indicator
/// constraint with given linear constraint and slack variable in its most
/// basic version.
pub fn create_cons_basic_indicator_lin_cons(scip: &mut Scip, name: &str, binvar: VarId, lincons: ConsId, slackvar: VarId) -> ScipResult<ConsId> {
    create_cons_indicator_lin_cons(scip, name, binvar, lincons, slackvar, &ConsFlags::default())
}

/// `SCIPaddVarIndicator`: adds variable to the inequality of the indicator
/// constraint.
pub fn add_var_indicator(scip: &mut Scip, cons: ConsId, var: VarId, mut val: f64) -> ScipResult {
    let (lessthanineq, lincons, slackvar) = (cd(scip, cons).lessthanineq, cd(scip, cons).lincons, cd(scip, cons).slackvar);

    // If linear inequality is flipped, variable is added with negative
    // coefficient.
    if !lessthanineq {
        val = -val;
    }

    cons_linear::add_coef_linear(scip, lincons, var, val)?;

    // Make sure that the type of the slack is as general as necessary.
    if (!crate::var::var_is_integral(scip, var) || !scip.set.is_integral(val)) && crate::var::var_is_integral(scip, slackvar) {
        scip_var::chg_var_type(scip, slackvar, VarType::Continuous)?;
        scip_var::chg_var_impl_type(scip, slackvar, crate::types::ImplintType::None)?;
    }

    Ok(())
}

/// `SCIPgetLinearConsIndicator`: gets the linear constraint corresponding to
/// the indicator constraint (may be NULL).
pub fn get_linear_cons_indicator(scip: &Scip, cons: ConsId) -> ConsId {
    cd(scip, cons).lincons
}

/// `SCIPgetActiveOnIndicator`: gets activation value of an indicator
/// constraint, TRUE for active on 1, FALSE for active on 0.
pub fn get_active_on_indicator(scip: &Scip, cons: ConsId) -> bool {
    cd(scip, cons).activeone
}

/// `SCIPgetBinaryVarIndicator`: gets binary variable corresponding to
/// indicator constraint.
pub fn get_binary_var_indicator(scip: &Scip, cons: ConsId) -> VarId {
    cd(scip, cons).binvar
}

/// `SCIPgetBinaryVarIndicatorGeneric`: similar to
/// SCIPgetBinaryVarIndicator but returns the original binary variable passed
/// by the user.
pub fn get_binary_var_indicator_generic(scip: &Scip, cons: ConsId) -> VarId {
    let c = cd(scip, cons);
    let mut binvar = c.binvar;
    if !c.activeone {
        binvar = scip.vars[binvar].negatedvar;
    }
    binvar
}

/// `SCIPgetSlackVarIndicator`: gets slack variable corresponding to
/// indicator constraint.
pub fn get_slack_var_indicator(scip: &Scip, cons: ConsId) -> VarId {
    cd(scip, cons).slackvar
}

/// `SCIPsetSlackVarUb`: sets upper bound for slack variable corresponding to
/// indicator constraint.
///
/// Use with care if you know that the maximal violation of the corresponding
/// constraint is at most @p ub. This bound might be improved automatically
/// during the solution process.
pub fn set_slack_var_ub(scip: &mut Scip, cons: ConsId, ub: f64) -> ScipResult {
    if scip.set.stage > Stage::Problem {
        return Ok(());
    }

    let slackvar = cd(scip, cons).slackvar;
    scip_var::chg_var_ub(scip, slackvar, ub)
}

/// `SCIPisViolatedIndicator`: checks whether indicator constraint is
/// violated w.r.t. sol.
pub fn is_violated_indicator(scip: &Scip, cons: ConsId, sol: SolId) -> bool {
    // Deleted constraints should always be satisfied.
    if scip.conss[cons].deleted {
        return false;
    }

    let c = cd(scip, cons);
    if c.linconsactive {
        return scip.set.is_feas_positive(api::get_sol_val(scip, sol, c.slackvar)) && scip.set.is_feas_positive(api::get_sol_val(scip, sol, c.binvar));
    }

    // @todo: check how this can be decided for linconsactive == FALSE.
    true
}

/// `SCIPmakeIndicatorFeasible`: based on values of other variables, computes
/// slack and binary variable to turn constraint feasible; returns whether
/// the solution has been changed.
///
/// It will also clean up the solution, i.e., shift slack variable, as
/// follows:
///
/// If the inequality is \f$a^T x + \gamma\, s \leq \beta\f$, the value of
/// the slack variable \f$s\f$ to achieve equality is
/// \f[ s^* = \frac{\beta - a^T x^*}{\gamma}, \f]
/// where \f$x^*\f$ is the given solution. In case of \f$a^T x + \gamma\, s
/// \geq \alpha\f$, we arrive at
/// \f[ s^* = \frac{\alpha - a^T x^*}{\gamma}. \f]
/// The typical values of \f$\gamma\f$ in the creation are -1 (if
/// scaleslackvar = FALSE) or something positive (if scaleslackvar = TRUE).
///
/// Let \f$\sigma\f$ be the direction (`sigma`) into which the slack variable
/// would make the constraint infeasible. If \f$\sigma\, s^* \leq 0\f$, we can
/// set the slack variable to 0. Otherwise, the constraint is violated
/// without the slack variable, and we need to set the slack variable to
/// \f$s^*\f$ and the binary variable to 0.
pub fn make_indicator_feasible(scip: &mut Scip, cons: ConsId, sol: SolId) -> ScipResult<bool> {
    let mut changed = false;

    // Avoid deleted indicator constraints, e.g., due to preprocessing.
    if !scip.conss[cons].is_active() && scip.set.stage >= Stage::InitPresolve {
        return Ok(false);
    }

    // If the linear constraint is not present, we cannot do anything.
    if !cd(scip, cons).linconsactive {
        return Ok(false);
    }

    let lincons = cd(scip, cons).lincons;

    // Avoid non-active linear constraints, e.g., due to preprocessing.
    if scip.conss[lincons].is_active() || scip.set.stage < Stage::InitPresolve {
        let (slackvar, binvar) = (cd(scip, cons).slackvar, cd(scip, cons).binvar);

        let linvars = cons_linear::get_vars_linear(scip, lincons).to_vec();
        let linvals = cons_linear::get_vals_linear(scip, lincons).to_vec();

        // Compute value of regular variables.
        let mut sum = 0.0;
        let mut slackcoef = 0.0;
        for (v, &var) in linvars.iter().enumerate() {
            if var != slackvar {
                sum += linvals[v] * api::get_sol_val(scip, sol, var);
            } else {
                slackcoef = linvals[v];
            }
        }

        // Do nothing if slack variable does not appear.
        if scip.set.is_feas_zero(slackcoef) {
            return Ok(false);
        }

        let mut val = cons_linear::get_rhs_linear(scip, lincons);
        let mut sigma = 1;
        if scip.set.is_infinity(val) {
            val = cons_linear::get_lhs_linear(scip, lincons);
            sigma = -1;
        }
        // Compute value of slack that would achieve equality.
        val = (val - sum) / slackcoef;

        // Compute direction into which slack variable would be infeasible.
        if slackcoef < 0.0 {
            sigma *= -1;
        }

        // Filter out cases in which no sensible change is possible.
        if sigma > 0 && scip.set.is_feas_negative(val) {
            return Ok(false);
        }

        let binvar_not_fixed = |scip: &Scip| {
            let b = &scip.vars[binvar];
            b.varstatus != VarStatus::Fixed && (b.varstatus != VarStatus::Negated || scip.vars[b.negatedvar].varstatus != VarStatus::Fixed)
        };

        // Check if linear constraint w/o slack variable is violated.
        if sigma < 0 && scip.set.is_feas_positive(val) {
            // The original constraint is violated.
            if !scip.set.is_feas_eq(api::get_sol_val(scip, sol, slackvar), val) {
                api::set_sol_val(scip, sol, slackvar, val)?;
                changed = true;
            }
            // Check whether binary variable is fixed or its negated variable is
            // fixed.
            if binvar_not_fixed(scip) && !scip.set.is_feas_eq(api::get_sol_val(scip, sol, binvar), 0.0) {
                api::set_sol_val(scip, sol, binvar, 0.0)?;
                changed = true;
            }
        } else {
            // The original constraint is satisfied - we can set the slack variable
            // to 0 (slackvar should only occur in this indicator constraint).
            if !scip.set.is_feas_eq(api::get_sol_val(scip, sol, slackvar), 0.0) && scip.set.is_feas_positive(scip.vars[slackvar].locdom.lb) {
                api::set_sol_val(scip, sol, slackvar, 0.0)?;
                changed = true;
            }

            // Check whether binary variable is fixed or its negated variable is
            // fixed.
            if binvar_not_fixed(scip) {
                let mut obj = var_get_obj_delta(scip, binvar);

                // Check objective for possibly setting binary variable.
                if obj <= 0.0 {
                    // Setting variable to 1 does not increase objective - check whether
                    // we can set it to 1.
                    if !scip.set.is_feas_eq(api::get_sol_val(scip, sol, binvar), 1.0) {
                        // Check whether variable only occurs in the current constraint.
                        if crate::var::var_get_n_locks_up_type(scip, binvar, LockType::Model) <= 1 {
                            api::set_sol_val(scip, sol, binvar, 1.0)?;
                            changed = true;
                            // Make sure that the other case does not occur if obj = 0:
                            // prefer variables set to 1.
                            obj = -1.0;
                        }
                    } else {
                        // Make sure that the other case does not occur if obj = 0: prefer
                        // variables set to 1.
                        obj = -1.0;
                    }
                }
                if obj >= 0.0 {
                    // Setting variable to 0 does not increase objective -> check whether
                    // variable only occurs in the current constraint. Note: binary
                    // variables are only locked up.
                    if crate::var::var_get_n_locks_down_type(scip, binvar, LockType::Model) <= 0 && !scip.set.is_feas_eq(api::get_sol_val(scip, sol, binvar), 0.0) {
                        api::set_sol_val(scip, sol, binvar, 0.0)?;
                        changed = true;
                    }
                }
            }
        }
    }

    Ok(changed)
}

/// `SCIPmakeIndicatorsFeasible`: based on values of other variables,
/// computes slack and binary variable to turn all constraints feasible;
/// returns whether the solution has been changed.
pub fn make_indicators_feasible(scip: &mut Scip, conshdlr: ConshdlrId, sol: SolId) -> ScipResult<bool> {
    let mut changed = false;

    // Only run after or in presolving.
    if scip.set.stage < Stage::InitPresolve {
        return Ok(false);
    }

    let conss = conshdlr_get_conss(scip, conshdlr);

    for &cons in &conss {
        // If the linear constraint is not present, we stop.
        if !cd(scip, cons).linconsactive {
            break;
        }

        let chg = make_indicator_feasible(scip, cons, sol)?;
        changed = changed || chg;
    }

    Ok(changed)
}
