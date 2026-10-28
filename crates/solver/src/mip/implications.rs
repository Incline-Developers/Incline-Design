//! HiGHS's `HighsImplications`: what fixing a binary implies (by probing),
//! variable bounds `x <= a y + b` and `x >= a y + b` on binaries `y`, and
//! the substitutions probing finds.

use super::{
    BoundType, DomainChange, MipModel,
    clique_table::{CliqueCtx, CliqueTable, CliqueVar, NO_ORIGIN},
    cpp_partition,
    domain::{Domain, DomainEnv, MipCtx, Reason},
    node_queue::NodeQueue,
    pseudocost::Pseudocost,
};
use crate::{hash_tree::HashTree, pdqsort::pdqsort, util::CDouble};

const INF: f64 = f64::INFINITY;

/// HiGHS's `kHighsTiny`.
const TINY: f64 = super::TINY;

/// HiGHS's `VarBound`: `coef * y + constant`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct VarBound {
    pub coef: f64,
    pub constant: f64,
}

impl VarBound {
    pub fn min_value(&self) -> f64 {
        (CDouble::new(self.constant) + self.coef.min(0.0)).value()
    }

    pub fn max_value(&self) -> f64 {
        (CDouble::new(self.constant) + self.coef.max(0.0)).value()
    }
}

/// HiGHS's `HighsSubstitution`: `substcol = offset + scale * staycol`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Substitution {
    pub substcol: usize,
    pub staycol: usize,
    pub offset: f64,
    pub scale: f64,
}

/// What `cleanupVlb`/`cleanupVub` found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cleanup {
    Kept,
    Redundant,
    /// The variable bound forces this bound on its column.
    Bound(f64),
}

#[derive(Clone, Debug, Default)]
struct Implics {
    implics: Vec<DomainChange>,
    computed: bool,
}

/// A row made redundant by a fixing, which presolve may lift:
/// `(row, col, val, value)`.
pub(crate) type LiftingOpportunity = (usize, usize, i32, f64);

#[derive(Clone, Debug, Default)]
pub(crate) struct Implications {
    next_cleanup_call: i64,
    implications: Vec<Implics>,
    num_implications: i64,
    num_var_bounds: i64,
    max_var_bounds: i64,
    vubs: Vec<HashTree<usize, VarBound>>,
    vlbs: Vec<HashTree<usize, VarBound>>,
    pub substitutions: Vec<Substitution>,
    pub colsubstituted: Vec<bool>,
    /// HiGHS's `storeLiftingOpportunity`: collects them while set.
    pub lifting_opportunities: Option<Vec<LiftingOpportunity>>,
}

impl Implications {
    pub fn new(numcol: usize, num_nonzero: usize) -> Self {
        Self {
            next_cleanup_call: num_nonzero as i64,
            implications: vec![Implics::default(); 2 * numcol],
            num_implications: 0,
            num_var_bounds: 0,
            max_var_bounds: Self::calc_max_var_bounds(numcol),
            vubs: vec![HashTree::new(); numcol],
            vlbs: vec![HashTree::new(); numcol],
            substitutions: Vec::new(),
            colsubstituted: vec![false; numcol],
            lifting_opportunities: None,
        }
    }

    pub fn reset(&mut self, numcol: usize, num_nonzero: usize) {
        let lifting_opportunities = self.lifting_opportunities.take();
        let substitutions = std::mem::take(&mut self.substitutions);
        *self = Self::new(numcol, num_nonzero);
        self.substitutions = substitutions;
        self.lifting_opportunities = lifting_opportunities;
    }

    pub const fn calc_max_var_bounds(numcol: usize) -> i64 {
        5_000_000 + 10 * numcol as i64
    }

    pub fn num_implications(&self) -> i64 {
        self.num_implications
    }

    pub fn too_many_var_bounds(&self) -> bool {
        self.num_var_bounds >= self.max_var_bounds
    }

    pub fn implications_cached(&self, col: usize, val: bool) -> bool {
        self.implications[2 * col + usize::from(val)].computed
    }

    /// HiGHS's `getImplications`: what fixing `col` to `val` implies, and
    /// whether it is infeasible.
    pub fn get_implications(
        &mut self,
        col: usize,
        val: bool,
        globaldom: &mut Domain,
        cliquetable: &mut CliqueTable,
        cx: &mut MipCtx,
        pseudocost: &mut Pseudocost,
    ) -> (&[DomainChange], bool) {
        let loc = 2 * col + usize::from(val);
        let infeasible = if !self.implications[loc].computed {
            self.compute_implications(col, val, globaldom, cliquetable, cx, pseudocost)
        } else {
            false
        };
        (&self.implications[loc].implics, infeasible)
    }

    fn cached_implications(&self, col: usize, val: bool) -> &[DomainChange] {
        &self.implications[2 * col + usize::from(val)].implics
    }

    pub fn vubs(&self, col: usize) -> &HashTree<usize, VarBound> {
        &self.vubs[col]
    }

    pub fn vlbs(&self, col: usize) -> &HashTree<usize, VarBound> {
        &self.vlbs[col]
    }

    /// HiGHS's `computeImplications`: probe `col = val`; returns whether
    /// that is infeasible (or the column already fixed).
    fn compute_implications(&mut self, col: usize, val: bool, globaldom: &mut Domain, cliquetable: &mut CliqueTable, cx: &mut MipCtx, pseudocost: &mut Pseudocost) -> bool {
        let model = cx.model;
        globaldom.propagate(&mut DomainEnv {
            cx,
            cliquetable,
            implications: self,
        });
        if globaldom.infeasible() || globaldom.is_fixed(col) {
            return true;
        }
        if self.lifting_opportunities.is_some() {
            globaldom.set_record_redundant_rows(true);
        }
        let changedend = globaldom.changed_cols().len();
        let stackimplicstart = globaldom.domain_change_stack().len() + 1;
        let mut num_implications = -(stackimplicstart as i64);
        let boundchg = if val {
            DomainChange::new(1.0, col, BoundType::Lower)
        } else {
            DomainChange::new(0.0, col, BoundType::Upper)
        };
        globaldom.change_bound(
            boundchg,
            Reason::branching(),
            &mut DomainEnv {
                cx,
                cliquetable,
                implications: self,
            },
        );

        let store_lifting_opportunities = |this: &mut Self, globaldom: &mut Domain| {
            if let Some(opportunities) = this.lifting_opportunities.as_mut() {
                for (&row, _) in globaldom.redundant_rows().iter() {
                    opportunities.push((row, col, i32::from(val), if val { -1.0 } else { 1.0 } * globaldom.redundant_row_value(row, model)));
                }
                globaldom.clear_redundant_rows();
                globaldom.set_record_redundant_rows(false);
            }
        };
        let do_backtrack = |this: &Self, globaldom: &mut Domain, cliquetable: &CliqueTable, cx: &mut MipCtx| {
            globaldom.backtrack(&mut DomainEnv {
                cx,
                cliquetable,
                implications: this,
            });
            globaldom.clear_changed_cols_from(changedend);
        };
        let is_infeasible = |this: &mut Self, globaldom: &mut Domain, cliquetable: &mut CliqueTable, cx: &mut MipCtx| {
            if !globaldom.infeasible() {
                return false;
            }
            store_lifting_opportunities(this, globaldom);
            do_backtrack(this, globaldom, cliquetable, cx);
            cliquetable.vertex_infeasible(globaldom, col, usize::from(val), &mut CliqueCtx { cx, implications: this });
            true
        };

        if is_infeasible(self, globaldom, cliquetable, cx) {
            return true;
        }
        globaldom.propagate(&mut DomainEnv {
            cx,
            cliquetable,
            implications: self,
        });
        if is_infeasible(self, globaldom, cliquetable, cx) {
            return true;
        }

        let stackimplicend = globaldom.domain_change_stack().len();
        num_implications += stackimplicend as i64;
        pseudocost.add_inference_observation(col, num_implications, val);

        let mut implics: Vec<DomainChange> = Vec::with_capacity(num_implications.max(0) as usize);
        let num_entries = cliquetable.num_entries();
        let max_entries = 100_000 + model.num_nonzero();
        for i in stackimplicstart..stackimplicend {
            let reason = globaldom.domain_change_reason()[i];
            if reason.kind == Reason::CLIQUE_TABLE && ((reason.index >> 1) == col || num_entries >= max_entries) {
                continue;
            }
            implics.push(globaldom.domain_change_stack()[i]);
        }

        store_lifting_opportunities(self, globaldom);
        do_backtrack(self, globaldom, cliquetable, cx);

        let binstart = cpp_partition(&mut implics, |a| !globaldom.is_binary(model, a.column));
        pdqsort(&mut implics[..binstart], |a, b| a < b);

        let colvar = CliqueVar::new(col, usize::from(val));
        for i in binstart..implics.len() {
            let other = if implics[i].boundtype == BoundType::Lower {
                CliqueVar::new(implics[i].column, 0)
            } else {
                CliqueVar::new(implics[i].column, 1)
            };
            let mut clique = [colvar, other];
            cliquetable.add_clique(model, globaldom, &mut clique, false, NO_ORIGIN, &mut CliqueCtx { cx, implications: self });
            if globaldom.infeasible() || globaldom.is_fixed(col) {
                return true;
            }
        }

        let feastol = cx.feastol;
        for i in 0..binstart {
            let implic = implics[i];
            let c = implic.column;
            if implic.boundtype == BoundType::Lower {
                if val {
                    if globaldom.col_lower[c] != -INF {
                        self.add_vlb(c, col, implic.boundval - globaldom.col_lower[c], globaldom.col_lower[c], globaldom, model, feastol);
                    }
                } else {
                    // An infinite lower bound still gives a finite offset.
                    self.add_vlb(c, col, globaldom.col_lower[c] - implic.boundval, implic.boundval, globaldom, model, feastol);
                }
            } else if val {
                if globaldom.col_upper[c] != INF {
                    self.add_vub(c, col, implic.boundval - globaldom.col_upper[c], globaldom.col_upper[c], globaldom, model, feastol);
                }
            } else {
                self.add_vub(c, col, globaldom.col_upper[c] - implic.boundval, implic.boundval, globaldom, model, feastol);
            }
        }

        let loc = 2 * col + usize::from(val);
        self.implications[loc].computed = true;
        implics.truncate(binstart);
        if !implics.is_empty() {
            self.num_implications += implics.len() as i64;
            self.implications[loc].implics = implics;
        }
        false
    }

    /// HiGHS's `getBestVub`: the variable upper bound on `col` closest to
    /// the LP solution (column `None` when there is none).
    #[allow(clippy::too_many_arguments)]
    pub fn get_best_vub(
        &self,
        col: usize,
        col_value: &[f64],
        col_dual: &[f64],
        best_ub: &mut f64,
        globaldom: &Domain,
        nodequeue: &NodeQueue,
        feastol: f64,
    ) -> (Option<usize>, VarBound) {
        let mut best_vub: (Option<usize>, VarBound) = (None, VarBound { coef: 0.0, constant: INF });
        let mut minbest_ub = *best_ub;
        let mut best_ub_dist = INF;
        let mut bestvubnodes = 0i64;
        let scale = globaldom.col_upper[col] - globaldom.col_lower[col];
        let scale = if scale == INF { 1.0 } else { 1.0 / scale };
        self.vubs[col].for_each(|&vub_col, vub| {
            if vub.coef == INF || globaldom.is_fixed(vub_col) {
                return;
            }
            let vubval = col_value[vub_col] * vub.coef + vub.constant;
            let mut ub_dist = (vubval - col_value[col]).max(0.0);
            let y_dist = feastol + if vub.coef > 0.0 { 1.0 - col_value[vub_col] } else { col_value[vub_col] };
            let norm2 = 1.0 + vub.coef * vub.coef;
            if ub_dist * ub_dist > y_dist * y_dist * norm2 {
                return;
            }
            ub_dist *= scale;
            if ub_dist <= best_ub_dist + feastol {
                let minvubval = vub.min_value();
                let vubnodes = if vub.coef > 0.0 {
                    nodequeue.num_nodes_down(vub_col)
                } else {
                    nodequeue.num_nodes_up(vub_col)
                };
                let better = 'better: {
                    if ub_dist < best_ub_dist - feastol || vubnodes > bestvubnodes {
                        break 'better true;
                    }
                    if vubnodes < bestvubnodes {
                        break 'better false;
                    }
                    if minvubval < minbest_ub - feastol {
                        break 'better true;
                    }
                    if minvubval > minbest_ub + feastol {
                        break 'better false;
                    }
                    let best = best_vub.0.expect("a finite distance replaced the empty bound");
                    col_dual[vub_col] / vub.coef - col_dual[best] / best_vub.1.coef > feastol
                };
                if better {
                    *best_ub = vubval;
                    minbest_ub = minvubval;
                    best_vub = (Some(vub_col), *vub);
                    bestvubnodes = vubnodes;
                    best_ub_dist = ub_dist;
                }
            }
        });
        best_vub
    }

    /// HiGHS's `getBestVlb`.
    #[allow(clippy::too_many_arguments)]
    pub fn get_best_vlb(
        &self,
        col: usize,
        col_value: &[f64],
        col_dual: &[f64],
        best_lb: &mut f64,
        globaldom: &Domain,
        nodequeue: &NodeQueue,
        feastol: f64,
    ) -> (Option<usize>, VarBound) {
        let mut best_vlb: (Option<usize>, VarBound) = (None, VarBound { coef: 0.0, constant: -INF });
        let mut maxbestlb = *best_lb;
        let mut best_lb_dist = INF;
        let mut bestvlbnodes = 0i64;
        let scale = globaldom.col_upper[col] - globaldom.col_lower[col];
        let scale = if scale == INF { 1.0 } else { 1.0 / scale };
        self.vlbs[col].for_each(|&vlb_col, vlb| {
            if vlb.coef == -INF || globaldom.is_fixed(vlb_col) {
                return;
            }
            let vlbval = col_value[vlb_col] * vlb.coef + vlb.constant;
            let mut lb_dist = (col_value[col] - vlbval).max(0.0);
            let y_dist = feastol + if vlb.coef > 0.0 { col_value[vlb_col] } else { 1.0 - col_value[vlb_col] };
            let norm2 = 1.0 + vlb.coef * vlb.coef;
            if lb_dist * lb_dist > y_dist * y_dist * norm2 {
                return;
            }
            lb_dist *= scale;
            if lb_dist <= best_lb_dist + feastol {
                let maxvlbval = vlb.max_value();
                let vlbnodes = if vlb.coef > 0.0 {
                    nodequeue.num_nodes_up(vlb_col)
                } else {
                    nodequeue.num_nodes_down(vlb_col)
                };
                let better = 'better: {
                    if lb_dist < best_lb_dist - feastol || vlbnodes > bestvlbnodes {
                        break 'better true;
                    }
                    if vlbnodes < bestvlbnodes {
                        break 'better false;
                    }
                    if maxvlbval > maxbestlb + feastol {
                        break 'better true;
                    }
                    if maxvlbval < maxbestlb - feastol {
                        break 'better false;
                    }
                    let best = best_vlb.0.expect("a finite distance replaced the empty bound");
                    col_dual[vlb_col] / vlb.coef - col_dual[best] / best_vlb.1.coef < -feastol
                };
                if better {
                    *best_lb = vlbval;
                    maxbestlb = maxvlbval;
                    best_vlb = (Some(vlb_col), *vlb);
                    bestvlbnodes = vlbnodes;
                    best_lb_dist = lb_dist;
                }
            }
        });
        best_vlb
    }

    /// HiGHS's `runProbing`: probe both values of binary `col`; returns
    /// whether it probed.
    pub fn run_probing(
        &mut self,
        col: usize,
        num_reductions: &mut usize,
        globaldom: &mut Domain,
        cliquetable: &mut CliqueTable,
        cx: &mut MipCtx,
        pseudocost: &mut Pseudocost,
    ) -> bool {
        let model = cx.model;
        if !(globaldom.is_binary(model, col) && !self.implications_cached(col, true) && !self.implications_cached(col, false) && cliquetable.substitution(col).is_none()) {
            return false;
        }
        let infeasible = self.compute_implications(col, true, globaldom, cliquetable, cx, pseudocost);
        if globaldom.infeasible() || infeasible || cliquetable.substitution(col).is_some() {
            return true;
        }
        let infeasible = self.compute_implications(col, false, globaldom, cliquetable, cx, pseudocost);
        if globaldom.infeasible() || infeasible || cliquetable.substitution(col).is_some() {
            return true;
        }
        // Both are computed now, so `getImplications` only reads them.
        let implicsdown = self.cached_implications(col, false).to_vec();
        let implicsup = self.cached_implications(col, true).to_vec();
        let (nimplicsdown, nimplicsup) = (implicsdown.len(), implicsup.len());
        let (mut u, mut d) = (0, 0);
        while u < nimplicsup && d < nimplicsdown {
            if implicsup[u].column < implicsdown[d].column {
                u += 1;
            } else if implicsdown[d].column < implicsup[u].column {
                d += 1;
            } else {
                let implcol = implicsup[u].column;
                let mut lb_down = globaldom.col_lower[implcol];
                let mut ub_down = globaldom.col_upper[implcol];
                let mut lb_up = lb_down;
                let mut ub_up = ub_down;
                loop {
                    if implicsdown[d].boundtype == BoundType::Lower {
                        lb_down = lb_down.max(implicsdown[d].boundval);
                    } else {
                        ub_down = ub_down.min(implicsdown[d].boundval);
                    }
                    d += 1;
                    if !(d < nimplicsdown && implicsdown[d].column == implcol) {
                        break;
                    }
                }
                loop {
                    if implicsup[u].boundtype == BoundType::Lower {
                        lb_up = lb_up.max(implicsup[u].boundval);
                    } else {
                        ub_up = ub_up.min(implicsup[u].boundval);
                    }
                    u += 1;
                    if !(u < nimplicsup && implicsup[u].column == implcol) {
                        break;
                    }
                }
                if self.colsubstituted[implcol] || globaldom.is_fixed(implcol) {
                    continue;
                }
                if lb_down == ub_down && lb_up == ub_up && (lb_down - lb_up).abs() > cx.feastol {
                    self.substitutions.push(Substitution {
                        substcol: implcol,
                        staycol: col,
                        offset: lb_down,
                        scale: lb_up - lb_down,
                    });
                    self.colsubstituted[implcol] = true;
                    *num_reductions += 1;
                } else {
                    let lb = lb_down.min(lb_up);
                    let ub = ub_down.max(ub_up);
                    if lb > globaldom.col_lower[implcol] {
                        globaldom.change_bound(
                            DomainChange::new(lb, implcol, BoundType::Lower),
                            Reason::unspecified(),
                            &mut DomainEnv {
                                cx,
                                cliquetable,
                                implications: self,
                            },
                        );
                        *num_reductions += 1;
                    }
                    if ub < globaldom.col_upper[implcol] {
                        globaldom.change_bound(
                            DomainChange::new(ub, implcol, BoundType::Upper),
                            Reason::unspecified(),
                            &mut DomainEnv {
                                cx,
                                cliquetable,
                                implications: self,
                            },
                        );
                        *num_reductions += 1;
                    }
                }
            }
        }
        true
    }

    /// HiGHS's `strengthenVarBound`: an MIR on `x + a y <= b` for integral
    /// `x` (`multiplier` -1 for a lower bound).
    pub fn strengthen_var_bound(vbnd: &mut VarBound, multiplier: f64) {
        if vbnd.coef.abs() == INF || vbnd.constant.abs() == INF {
            return;
        }
        const F0MIN: f64 = 0.005;
        const F0MAX: f64 = 0.995;
        let downrhs = (multiplier * vbnd.constant).floor();
        let f0 = multiplier * vbnd.constant - downrhs;
        if !(F0MIN..=F0MAX).contains(&f0) {
            return;
        }
        let downaj = (-multiplier * vbnd.coef + TINY).floor();
        let fj = -multiplier * vbnd.coef - downaj;
        vbnd.constant = multiplier * downrhs;
        vbnd.coef = -multiplier * (downaj + (fj - f0).max(0.0) / (1.0 - f0));
    }

    /// HiGHS's 4-argument `addVUB`, against the global domain.
    #[allow(clippy::too_many_arguments)]
    pub fn add_vub(&mut self, col: usize, vubcol: usize, vubcoef: f64, vubconstant: f64, globaldom: &Domain, model: &MipModel, feastol: f64) {
        self.add_vub_with(col, vubcol, vubcoef, vubconstant, globaldom.col_upper[col], model.is_col_integral(col), feastol);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_vub_with(&mut self, col: usize, vubcol: usize, vubcoef: f64, vubconstant: f64, colupperbound: f64, colisintegral: bool, feastol: f64) {
        if self.too_many_var_bounds() {
            return;
        }
        let mut vub = VarBound {
            coef: vubcoef,
            constant: vubconstant,
        };
        if colisintegral {
            Self::strengthen_var_bound(&mut vub, 1.0);
            if vub.coef == 0.0 {
                return;
            }
        }
        let min_bound = vub.min_value();
        if min_bound >= colupperbound - feastol {
            return;
        }
        let (current, inserted) = self.vubs[col].insert_or_get(vubcol, vub);
        if !inserted {
            if min_bound < current.min_value() - feastol {
                *current = vub;
            }
        } else {
            self.num_var_bounds += 1;
        }
    }

    /// HiGHS's 4-argument `addVLB`, against the global domain.
    #[allow(clippy::too_many_arguments)]
    pub fn add_vlb(&mut self, col: usize, vlbcol: usize, vlbcoef: f64, vlbconstant: f64, globaldom: &Domain, model: &MipModel, feastol: f64) {
        self.add_vlb_with(col, vlbcol, vlbcoef, vlbconstant, globaldom.col_lower[col], model.is_col_integral(col), feastol);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_vlb_with(&mut self, col: usize, vlbcol: usize, vlbcoef: f64, vlbconstant: f64, collowerbound: f64, colisintegral: bool, feastol: f64) {
        if self.too_many_var_bounds() {
            return;
        }
        let mut vlb = VarBound {
            coef: vlbcoef,
            constant: vlbconstant,
        };
        if colisintegral {
            Self::strengthen_var_bound(&mut vlb, -1.0);
            if vlb.coef == 0.0 {
                return;
            }
        }
        let max_bound = vlb.max_value();
        if max_bound <= collowerbound + feastol {
            return;
        }
        let (current, inserted) = self.vlbs[col].insert_or_get(vlbcol, vlb);
        if !inserted {
            if max_bound > current.max_value() + feastol {
                *current = vlb;
            }
        } else {
            self.num_var_bounds += 1;
        }
    }

    /// HiGHS's `columnTransformed`: `col` became `(col - constant) / scale`.
    pub fn column_transformed(&mut self, col: usize, scale: f64, constant: f64) {
        if scale < 0.0 {
            let (vubs, vlbs) = (&mut self.vubs, &mut self.vlbs);
            std::mem::swap(&mut vubs[col], &mut vlbs[col]);
        }
        let transform = |_: &usize, vbd: &mut VarBound| {
            vbd.constant -= constant;
            vbd.constant /= scale;
            vbd.coef /= scale;
        };
        self.vlbs[col].for_each_mut(transform);
        self.vubs[col].for_each_mut(transform);
        for substitution in &mut self.substitutions {
            if substitution.substcol == col {
                substitution.offset -= constant;
                substitution.offset /= scale;
                substitution.scale /= scale;
            }
        }
    }

    /// HiGHS's `rebuild`: the variable bounds over the presolved columns.
    pub fn rebuild(
        &mut self,
        ncols: usize,
        orig2reducedcol: &[Option<usize>],
        is_col_linearly_transformable: &dyn Fn(usize) -> bool,
        globaldom: &Domain,
        model: &MipModel,
        feastol: f64,
    ) {
        let oldvubs = std::mem::take(&mut self.vubs);
        let oldvlbs = std::mem::take(&mut self.vlbs);
        self.colsubstituted = vec![false; ncols];
        self.implications = vec![Implics::default(); 2 * ncols];
        self.substitutions.clear();
        self.vubs = vec![HashTree::new(); ncols];
        self.vlbs = vec![HashTree::new(); ncols];
        self.num_implications = 0;
        self.num_var_bounds = 0;
        self.next_cleanup_call = model.num_nonzero() as i64;
        for i in 0..oldvubs.len() {
            let Some(newi) = orig2reducedcol[i] else { continue };
            if !is_col_linearly_transformable(newi) {
                continue;
            }
            let keep = |vbd_col: usize| orig2reducedcol[vbd_col].filter(|&new| globaldom.is_binary(model, new) && is_col_linearly_transformable(new));
            oldvubs[i].for_each(|&vub_col, vub| {
                if let Some(new_vub_col) = keep(vub_col) {
                    self.add_vub(newi, new_vub_col, vub.coef, vub.constant, globaldom, model, feastol);
                }
            });
            oldvlbs[i].for_each(|&vlb_col, vlb| {
                if let Some(new_vlb_col) = keep(vlb_col) {
                    self.add_vlb(newi, new_vlb_col, vlb.coef, vlb.constant, globaldom, model, feastol);
                }
            });
        }
    }

    /// HiGHS's `buildFrom`.
    pub fn build_from(&mut self, init: &Implications, globaldom: &Domain, model: &MipModel, feastol: f64) {
        for i in 0..model.num_col {
            init.vubs[i].for_each(|&vub_col, vub| {
                if globaldom.is_binary(model, vub_col) {
                    self.add_vub(i, vub_col, vub.coef, vub.constant, globaldom, model, feastol);
                }
            });
            init.vlbs[i].for_each(|&vlb_col, vlb| {
                if globaldom.is_binary(model, vlb_col) {
                    self.add_vlb(i, vlb_col, vlb.coef, vlb.constant, globaldom, model, feastol);
                }
            });
        }
    }

    /// HiGHS's `separateImpliedBounds`: probe the fractional binaries not
    /// yet probed, then add the implied bound cuts `sol` violates to the
    /// cut pool. `fractional` is the LP's fractional integers.
    #[allow(clippy::too_many_arguments)]
    pub fn separate_implied_bounds(
        &mut self,
        fractional: &[(usize, f64)],
        sol: &[f64],
        feastol: f64,
        globaldom: &mut Domain,
        cliquetable: &mut CliqueTable,
        cx: &mut MipCtx,
        pseudocost: &mut Pseudocost,
        num_clique_entries_after_first_presolve: usize,
    ) {
        let model = cx.model;
        let mut numboundchgs = 0;
        if !cliquetable.is_full() {
            let old_num_queries = cliquetable.num_neighbourhood_queries;
            let old_num_entries = cliquetable.num_entries();
            for &(col, _) in fractional {
                if globaldom.col_lower[col] != 0.0 || globaldom.col_upper[col] != 1.0 || (self.implications_cached(col, false) && self.implications_cached(col, true)) {
                    continue;
                }
                if self.run_probing(col, &mut numboundchgs, globaldom, cliquetable, cx, pseudocost) && globaldom.infeasible() {
                    return;
                }
                if cliquetable.is_full() {
                    break;
                }
            }
            let num_new_entries = cliquetable.num_entries() as i64 - old_num_entries as i64;
            self.next_cleanup_call -= num_new_entries.max(0);
            if self.next_cleanup_call < 0 {
                cliquetable.run_clique_merging(globaldom, &mut CliqueCtx { cx, implications: self });
                self.next_cleanup_call = num_clique_entries_after_first_presolve.min(cliquetable.num_entries()) as i64;
            }
            cliquetable.num_neighbourhood_queries = old_num_queries;
        }

        for &(col, _) in fractional {
            if globaldom.col_lower[col] != 0.0 || globaldom.col_upper[col] != 1.0 {
                continue;
            }
            for val in [true, false] {
                if !self.implications_cached(col, val) {
                    continue;
                }
                // Cached, so `getImplications` only reads (and reports no
                // infeasibility).
                if globaldom.infeasible() {
                    return;
                }
                let implics = self.cached_implications(col, val).to_vec();
                for implic in implics {
                    let c = implic.column;
                    let (inds, vals, rhs) = if implic.boundtype == BoundType::Upper {
                        if implic.boundval + feastol >= globaldom.col_upper[c] {
                            continue;
                        }
                        if val {
                            ([c, col], [1.0, globaldom.col_upper[c] - implic.boundval], globaldom.col_upper[c])
                        } else {
                            ([c, col], [1.0, implic.boundval - globaldom.col_upper[c]], implic.boundval)
                        }
                    } else {
                        if implic.boundval - feastol <= globaldom.col_lower[c] {
                            continue;
                        }
                        if val {
                            ([c, col], [-1.0, implic.boundval - globaldom.col_lower[c]], -globaldom.col_lower[c])
                        } else {
                            ([c, col], [-1.0, globaldom.col_lower[c] - implic.boundval], -implic.boundval)
                        }
                    };
                    let viol = sol[inds[0]] * vals[0] + sol[inds[1]] * vals[1] - rhs;
                    if viol > feastol {
                        let (mut inds, mut vals) = (inds, vals);
                        cx.cutpool
                            .add_cut(&mut inds, &mut vals, rhs, !model.is_col_continuous(c), false, false, model.num_nonzero(), model.num_row);
                    }
                }
            }
        }
    }

    /// HiGHS's `cleanupVarbounds`.
    pub fn cleanup_varbounds(&mut self, col: usize, globaldom: &mut Domain, cliquetable: &CliqueTable, cx: &mut MipCtx) {
        let ub = globaldom.col_upper[col];
        let lb = globaldom.col_lower[col];
        if ub == lb {
            let mut num = 0;
            self.vubs[col].for_each(|_, _| num += 1);
            self.vlbs[col].for_each(|_, _| num += 1);
            self.num_var_bounds -= num;
            self.vlbs[col] = HashTree::new();
            self.vubs[col] = HashTree::new();
            return;
        }
        let (feastol, epsilon) = (cx.feastol, cx.epsilon);
        let mut del_vbds = Vec::new();
        let mut vubs = std::mem::take(&mut self.vubs[col]);
        vubs.for_each_mut(|&vub_col, vub| match Self::cleanup_vub(Some(vub_col), vub, ub, feastol, epsilon) {
            Cleanup::Redundant => del_vbds.push(vub_col),
            Cleanup::Bound(bound) => globaldom.change_bound(
                DomainChange::new(bound, col, BoundType::Upper),
                Reason::unspecified(),
                &mut DomainEnv {
                    cx,
                    cliquetable,
                    implications: self,
                },
            ),
            Cleanup::Kept => {}
        });
        for vub_col in &del_vbds {
            vubs.erase(vub_col);
        }
        self.vubs[col] = vubs;
        self.num_var_bounds -= del_vbds.len() as i64;
        del_vbds.clear();
        let mut vlbs = std::mem::take(&mut self.vlbs[col]);
        vlbs.for_each_mut(|&vlb_col, vlb| match Self::cleanup_vlb(Some(vlb_col), vlb, lb, feastol, epsilon) {
            Cleanup::Redundant => del_vbds.push(vlb_col),
            Cleanup::Bound(bound) => globaldom.change_bound(
                DomainChange::new(bound, col, BoundType::Lower),
                Reason::unspecified(),
                &mut DomainEnv {
                    cx,
                    cliquetable,
                    implications: self,
                },
            ),
            Cleanup::Kept => {}
        });
        for vlb_col in &del_vbds {
            vlbs.erase(vlb_col);
        }
        self.vlbs[col] = vlbs;
        self.num_var_bounds -= del_vbds.len() as i64;
    }

    /// HiGHS's `cleanupVlb`: tighten `vlb` against lower bound `lb`; a
    /// [`Cleanup::Bound`] is the lower bound HiGHS then sets on the column
    /// when bound changes are allowed.
    pub fn cleanup_vlb(vlb_col: Option<usize>, vlb: &mut VarBound, lb: f64, feastol: f64, epsilon: f64) -> Cleanup {
        if vlb_col.is_none() {
            return Cleanup::Kept;
        }
        let maxlb = CDouble::new(vlb.max_value());
        let minlb = CDouble::new(vlb.min_value());
        if maxlb <= lb + feastol {
            return Cleanup::Redundant;
        }
        if minlb < lb - epsilon {
            let newcoef = (CDouble::new(lb) - maxlb).value();
            if vlb.coef < 0.0 {
                vlb.coef = newcoef;
            } else {
                vlb.constant = lb;
                vlb.coef = -newcoef;
            }
            Cleanup::Kept
        } else if minlb > lb + epsilon {
            Cleanup::Bound(minlb.value())
        } else {
            Cleanup::Kept
        }
    }

    /// HiGHS's `cleanupVub`, as [`Implications::cleanup_vlb`].
    pub fn cleanup_vub(vub_col: Option<usize>, vub: &mut VarBound, ub: f64, feastol: f64, epsilon: f64) -> Cleanup {
        if vub_col.is_none() {
            return Cleanup::Kept;
        }
        let maxub = CDouble::new(vub.max_value());
        let minub = CDouble::new(vub.min_value());
        if minub >= ub - feastol {
            return Cleanup::Redundant;
        }
        if maxub > ub + epsilon {
            let newcoef = (CDouble::new(ub) - minub).value();
            if vub.coef > 0.0 {
                vub.coef = newcoef;
            } else {
                vub.constant = ub;
                vub.coef = -newcoef;
            }
            Cleanup::Kept
        } else if maxub < ub - epsilon {
            Cleanup::Bound(maxub.value())
        } else {
            Cleanup::Kept
        }
    }

    /// HiGHS's `applyImplications`: once `col` is fixed to `val`, apply
    /// what probing found it implies.
    pub fn apply_implications(&self, domain: &mut Domain, col: usize, val: usize, env: &mut DomainEnv) {
        let loc = 2 * col + val;
        if !self.implications[loc].computed {
            return;
        }
        let feastol = env.feastol;
        for domchg in &self.implications[loc].implics {
            if domain.is_fixed(domchg.column) {
                continue;
            }
            let isint = !env.model.is_col_continuous(domchg.column);
            let apply = if domchg.boundtype == BoundType::Lower {
                (!isint && domchg.boundval > domain.col_upper[domchg.column] - feastol) || (isint && domchg.boundval > domain.col_lower[domchg.column] + feastol)
            } else {
                (!isint && domchg.boundval < domain.col_lower[domchg.column] + feastol) || (isint && domchg.boundval < domain.col_upper[domchg.column] - feastol)
            };
            if apply {
                domain.change_bound(*domchg, Reason::clique_table(col, val), env);
            }
            if domain.infeasible() {
                break;
            }
        }
    }
}
