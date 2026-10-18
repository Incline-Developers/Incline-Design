//! HiGHS's `HighsSeparation`: rounds of cut separation on the LP
//! relaxation (implied bounds, cliques, the tableau, path and mod-k
//! separators, then the cut pool), resolving the LP after each.

use super::{
    cut_pool::CutSet,
    data::MipData,
    domain::{Domain, DomainEnv},
    lp_relaxation::{LpDomain, LpRelaxation, LpStatus},
    separators::{LpAggregator, ModkSeparator, PathSeparator, TableauSeparator},
    transformed_lp::TransformedLp,
};

const INF: f64 = f64::INFINITY;

/// Run `f` on the domain `propdomain` names, with what domain operations
/// read.
pub(crate) fn on_prop_domain<R>(propdomain: &mut LpDomain, data: &mut MipData, f: impl FnOnce(&mut Domain, &mut DomainEnv) -> R) -> R {
    let mut parts = data.parts();
    match propdomain {
        LpDomain::Global => parts.on_domain(f),
        LpDomain::Local(local) => {
            let mut env = DomainEnv {
                cx: &mut parts.cx,
                cliquetable: parts.cliquetable,
                implications: parts.implications,
            };
            f(local, &mut env)
        }
        LpDomain::None => unreachable!("a propagation domain"),
    }
}

/// The domain `propdomain` names.
pub(crate) fn prop_domain<'d>(propdomain: &'d LpDomain, data: &'d MipData) -> &'d Domain {
    match propdomain {
        LpDomain::Global => &data.domain,
        LpDomain::Local(local) => local,
        LpDomain::None => unreachable!("a propagation domain"),
    }
}

fn prop_domain_mut<'d>(propdomain: &'d mut LpDomain, data: &'d mut MipData) -> &'d mut Domain {
    match propdomain {
        LpDomain::Global => &mut data.domain,
        LpDomain::Local(local) => local,
        LpDomain::None => unreachable!("a propagation domain"),
    }
}

impl MipData {
    /// `redcostfixing.addRootRedcost` with the LP's reduced costs.
    pub fn add_root_redcost(&mut self, lp: &mut LpRelaxation) {
        let lpobjective = lp.objective();
        let mut redcostfixing = std::mem::take(&mut self.redcostfixing);
        redcostfixing.add_root_redcost(lp, self, lpobjective);
        self.redcostfixing = redcostfixing;
    }

    /// `redcostfixing.propagateRootRedcost`.
    pub fn propagate_root_redcost(&mut self) {
        let mut redcostfixing = std::mem::take(&mut self.redcostfixing);
        redcostfixing.propagate_root_redcost(self);
        self.redcostfixing = redcostfixing;
    }
}

pub(crate) struct Separation {
    tableau: TableauSeparator,
    path: PathSeparator,
    modk: ModkSeparator,
    cutset: CutSet,
}

impl Separation {
    pub fn new(data: &MipData) -> Self {
        Self {
            tableau: TableauSeparator::new(),
            path: PathSeparator::new(data),
            modk: ModkSeparator::new(),
            cutset: CutSet::default(),
        }
    }

    /// `separationRound`'s `propagateAndResolve`: `None` for HiGHS's -1.
    fn propagate_and_resolve(lp: &mut LpRelaxation, propdomain: &mut LpDomain, status: &mut LpStatus, data: &mut MipData) -> Option<usize> {
        let is_global = matches!(propdomain, LpDomain::Global);
        if prop_domain(propdomain, data).infeasible() || data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            prop_domain_mut(propdomain, data).clear_changed_cols();
            return None;
        }
        on_prop_domain(propdomain, data, |dom, env| dom.propagate(env));
        if prop_domain(propdomain, data).infeasible() {
            *status = LpStatus::Infeasible;
            prop_domain_mut(propdomain, data).clear_changed_cols();
            return None;
        }
        if is_global {
            let mut parts = data.parts();
            parts.on_cliques(|cliquetable, domain, cc| cliquetable.cleanup_fixed(domain, cc));
        }
        if data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            prop_domain_mut(propdomain, data).clear_changed_cols();
            return None;
        }
        let num_bound_chgs = prop_domain(propdomain, data).changed_cols().len();
        while !prop_domain(propdomain, data).changed_cols().is_empty() {
            lp.set_objective_limit(data.upper_limit, data);
            let domain = match propdomain {
                LpDomain::Global => LpDomain::Global,
                LpDomain::Local(local) => LpDomain::Local(local),
                LpDomain::None => LpDomain::None,
            };
            *status = lp.resolve_lp(domain, data);
            if !status.scaled_optimal() {
                return None;
            }
            if is_global && status.unscaled_dual_feasible() {
                data.add_root_redcost(lp);
                if data.upper_limit != INF {
                    data.propagate_root_redcost();
                }
            }
        }
        Some(num_bound_chgs)
    }

    /// HiGHS's `separationRound`: the number of cuts and bound changes it
    /// found.
    pub fn separation_round(&mut self, lp: &mut LpRelaxation, propdomain: &mut LpDomain, status: &mut LpStatus, data: &mut MipData) -> usize {
        let is_global = matches!(propdomain, LpDomain::Global);
        {
            let sol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
            let feastol = data.feastol;
            let entries = data.num_clique_entries_after_first_presolve;
            let mut parts = data.parts();
            parts.implications.separate_implied_bounds(
                lp.fractional_integers(),
                &sol,
                feastol,
                parts.domain,
                parts.cliquetable,
                &mut parts.cx,
                parts.pseudocost,
                entries,
            );
        }
        let mut ncuts = 0;
        match Self::propagate_and_resolve(lp, propdomain, status, data) {
            None => return 0,
            Some(numboundchgs) => ncuts += numboundchgs,
        }

        {
            let sol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
            let cuts = data.cliquetable.separate_cliques(
                &sol,
                &data.integral_cols,
                &data.domain,
                data.feastol,
                data.model.num_nonzero(),
                data.total_lp_iterations,
                None,
                None,
            );
            for (mut inds, mut vals, rhs) in cuts {
                data.add_cut(&mut inds, &mut vals, rhs, true, false, false, false);
            }
        }
        match Self::propagate_and_resolve(lp, propdomain, status, data) {
            None => return 0,
            Some(numboundchgs) => ncuts += numboundchgs,
        }

        if !is_global {
            let feastol = data.feastol;
            let LpDomain::Local(local) = propdomain else { unreachable!("a local domain") };
            lp.compute_basic_degenerate_duals(feastol, Some(local), data, true);
        }

        let mut trans_lp = TransformedLp::new(lp, data);
        if data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            return 0;
        }
        let mut aggregator = LpAggregator::new(lp);
        self.tableau.run(lp, &mut aggregator, &mut trans_lp, data);
        if data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            return 0;
        }
        self.path.run(lp, &mut aggregator, &mut trans_lp, data);
        if data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            return 0;
        }
        self.modk.run(lp, &mut aggregator, &mut trans_lp, data);
        if data.domain.infeasible() {
            *status = LpStatus::Infeasible;
            return 0;
        }

        match Self::propagate_and_resolve(lp, propdomain, status, data) {
            None => return 0,
            Some(numboundchgs) => ncuts += numboundchgs,
        }

        {
            let sol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
            let feastol = data.feastol;
            let dom = prop_domain(propdomain, data);
            let (col_lower, col_upper) = (dom.col_lower.clone(), dom.col_upper.clone());
            data.cutpool.separate(&sol, &col_lower, &col_upper, &mut self.cutset, feastol);
        }
        if self.cutset.num_cuts() > 0 {
            ncuts += self.cutset.num_cuts();
            lp.add_cuts(&mut self.cutset);
            let domain = match propdomain {
                LpDomain::Global => LpDomain::Global,
                LpDomain::Local(local) => LpDomain::Local(local),
                LpDomain::None => LpDomain::None,
            };
            *status = lp.resolve_lp(domain, data);
            lp.perform_aging(true, data);
            if is_global && status.unscaled_dual_feasible() {
                data.add_root_redcost(lp);
                if data.upper_limit != INF {
                    data.propagate_root_redcost();
                }
            }
        }
        ncuts
    }

    /// HiGHS's `separate`: separation rounds while the objective improves
    /// enough, or just aging when the LP has no fractional solution.
    pub fn separate(&mut self, lp: &mut LpRelaxation, propdomain: &mut LpDomain, data: &mut MipData) {
        let mut status = lp.status();
        if status.scaled_optimal() && !lp.fractional_integers().is_empty() {
            let firstobj = data.rootlpsolobj;
            while lp.objective() < data.optimality_limit {
                let lastobj = lp.objective();
                let mut nlpiters = -lp.num_lp_iterations();
                let ncuts = self.separation_round(lp, propdomain, &mut status, data);
                nlpiters += lp.num_lp_iterations();
                data.sepa_lp_iterations += nlpiters;
                data.total_lp_iterations += nlpiters;
                if ncuts == 0 || !status.scaled_optimal() || lp.fractional_integers().is_empty() {
                    break;
                }
                if lp.objective() - firstobj <= (lastobj - firstobj).max(data.feastol) * 1.01 {
                    break;
                }
            }
        } else {
            lp.perform_aging(true, data);
            data.cutpool.perform_aging();
        }
    }
}
