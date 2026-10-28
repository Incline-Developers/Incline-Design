//! HiGHS's `HighsRedcostFixing`: bounds the reduced costs imply under the
//! cutoff, at a node and, as "lurking" bounds waiting for a better cutoff,
//! from the root LP.

use super::{
    BoundType, DomainChange,
    data::MipData,
    domain::{Domain, DomainEnv, Reason},
    lp_relaxation::LpRelaxation,
};
use crate::util::CDouble;

const INF: f64 = f64::INFINITY;

/// HiGHS's `std::multimap<double, HighsInt>`: entries ordered by cutoff,
/// equal cutoffs in insertion order unless placed by a hint.
type Lurking = Vec<(f64, i64)>;

fn lower_bound(map: &Lurking, key: f64) -> usize {
    map.partition_point(|&(k, _)| k < key)
}

fn upper_bound(map: &Lurking, key: f64) -> usize {
    map.partition_point(|&(k, _)| k <= key)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RedcostFixing {
    lurking_col_upper: Vec<Lurking>,
    lurking_col_lower: Vec<Lurking>,
}

impl RedcostFixing {
    /// HiGHS's `getLurkingBounds`: `(cutoff, bound change)` for each lurking
    /// bound tighter than the global domain.
    pub fn lurking_bounds(&self, data: &MipData) -> Vec<(f64, DomainChange)> {
        let mut domchgs = Vec::new();
        if self.lurking_col_lower.is_empty() {
            return domchgs;
        }
        let globaldom = &data.domain;
        for &col in &data.integral_cols {
            for &(cutoff, bound) in &self.lurking_col_lower[col] {
                if bound as f64 > globaldom.col_lower[col] {
                    domchgs.push((cutoff, DomainChange::new(bound as f64, col, BoundType::Lower)));
                }
            }
            for &(cutoff, bound) in &self.lurking_col_upper[col] {
                if (bound as f64) < globaldom.col_upper[col] {
                    domchgs.push((cutoff, DomainChange::new(bound as f64, col, BoundType::Upper)));
                }
            }
        }
        domchgs
    }

    /// HiGHS's `propagateRootRedcost`: apply the lurking bounds the current
    /// cutoff reaches.
    pub fn propagate_root_redcost(&mut self, data: &mut MipData) {
        if self.lurking_col_lower.is_empty() {
            return;
        }
        let lower_bound_value = data.lower_bound;
        let upper_limit = data.upper_limit;
        let integral_cols = data.integral_cols.clone();
        let mut parts = data.parts();
        for col in integral_cols {
            let end = upper_bound(&self.lurking_col_lower[col], lower_bound_value);
            self.lurking_col_lower[col].drain(..end);
            let end = upper_bound(&self.lurking_col_upper[col], lower_bound_value);
            self.lurking_col_upper[col].drain(..end);
            for col_at in &self.lurking_col_lower[col][lower_bound(&self.lurking_col_lower[col], upper_limit)..self.lurking_col_lower[col].len()] {
                let bound = col_at.1 as f64;
                if bound > parts.domain.col_lower[col] {
                    parts.on_domain(|dom, env| dom.change_bound(DomainChange::new(bound, col, BoundType::Lower), Reason::unspecified(), env));
                    if parts.domain.infeasible() {
                        return;
                    }
                }
            }
            for col_at in &self.lurking_col_upper[col][lower_bound(&self.lurking_col_upper[col], upper_limit)..self.lurking_col_upper[col].len()] {
                let bound = col_at.1 as f64;
                if bound < parts.domain.col_upper[col] {
                    parts.on_domain(|dom, env| dom.change_bound(DomainChange::new(bound, col, BoundType::Upper), Reason::unspecified(), env));
                    if parts.domain.infeasible() {
                        return;
                    }
                }
            }
        }
        parts.on_domain(|dom, env| dom.propagate(env));
    }

    /// HiGHS's `propagateRedCost`: tighten `localdomain` by the LP's
    /// reduced costs under `upper_limit`.
    pub fn propagate_red_cost(localdomain: &mut Domain, lp: &LpRelaxation, data: &mut MipData, upper_limit: f64) {
        let lpredcost = lp.lp_solver().col_dual().expect("reduced costs").to_vec();
        let lpobjective = lp.objective();
        let gap = CDouble::new(upper_limit) - lpobjective;
        let feastol = data.feastol;
        let tolerance = (10.0 * feastol).max(data.epsilon * gap.value());
        let mut bound_changes: Vec<DomainChange> = Vec::with_capacity(data.integral_cols.len());
        let integral_cols = data.integral_cols.clone();
        for &col in &integral_cols {
            if localdomain.col_upper[col] == localdomain.col_lower[col] {
                continue;
            }
            if lpredcost[col].abs() <= tolerance {
                continue;
            }
            let max_increase = lpredcost[col] * (localdomain.col_upper[col] - localdomain.col_lower[col]);
            if max_increase > gap.value() {
                let newub = (gap / lpredcost[col] + localdomain.col_lower[col] + feastol).floor().value();
                if newub >= localdomain.col_upper[col] {
                    continue;
                }
                if data.domain.is_binary(&data.model, col) {
                    bound_changes.push(DomainChange::new(newub, col, BoundType::Upper));
                } else {
                    let mut parts = data.parts();
                    change_local(localdomain, &mut parts, DomainChange::new(newub, col, BoundType::Upper));
                    if localdomain.infeasible() {
                        return;
                    }
                }
            } else if max_increase < (-gap).value() {
                let newlb = (gap / lpredcost[col] + localdomain.col_upper[col] - feastol).ceil().value();
                if newlb <= localdomain.col_lower[col] {
                    continue;
                }
                if data.domain.is_binary(&data.model, col) {
                    bound_changes.push(DomainChange::new(newlb, col, BoundType::Lower));
                } else {
                    let mut parts = data.parts();
                    change_local(localdomain, &mut parts, DomainChange::new(newlb, col, BoundType::Lower));
                    if localdomain.infeasible() {
                        return;
                    }
                }
            }
        }
        if bound_changes.is_empty() {
            return;
        }
        let proof = if bound_changes.len() <= 100 {
            lp.compute_dual_proof(upper_limit, false, data)
        } else {
            None
        };
        let mut parts = data.parts();
        if let Some((inds, vals, rhs)) = proof {
            let old_num_conflicts = parts.cx.conflictpool.num_conflicts();
            for domchg in &bound_changes {
                if localdomain.is_active(domchg) {
                    continue;
                }
                let mut env = DomainEnv {
                    cx: &mut parts.cx,
                    cliquetable: parts.cliquetable,
                    implications: parts.implications,
                };
                localdomain.conflict_analyze_reconvergence(*domchg, &inds, &vals, rhs, parts.domain, parts.pseudocost, &mut env);
            }
            let added_constraints = parts.cx.conflictpool.num_conflicts() != old_num_conflicts;
            if added_constraints {
                propagate_local(localdomain, &mut parts);
                if localdomain.infeasible() {
                    return;
                }
                bound_changes.retain(|domchg| !localdomain.is_active(domchg));
            }
            if !bound_changes.is_empty() {
                for domchg in &bound_changes {
                    change_local(localdomain, &mut parts, *domchg);
                    if localdomain.infeasible() {
                        break;
                    }
                }
                if !localdomain.infeasible() {
                    propagate_local(localdomain, &mut parts);
                }
            }
        } else {
            for domchg in &bound_changes {
                change_local(localdomain, &mut parts, *domchg);
                if localdomain.infeasible() {
                    break;
                }
            }
            if !localdomain.infeasible() {
                propagate_local(localdomain, &mut parts);
            }
        }
    }

    /// HiGHS's `addRootRedcost`: record, for each integral column, the
    /// bounds its root reduced cost would imply at each cutoff.
    pub fn add_root_redcost(&mut self, lp: &mut LpRelaxation, data: &mut MipData, lpobjective: f64) {
        let num_col = data.model.num_col;
        self.lurking_col_lower.resize(num_col, Vec::new());
        self.lurking_col_upper.resize(num_col, Vec::new());
        lp.compute_basic_degenerate_duals(data.feastol, None, data, false);
        // HiGHS reads the LP solver's reduced costs through a reference, so
        // it sees what `computeBasicDegenerateDuals` made of them.
        let lpredcost = lp.lp_solver().col_dual().expect("reduced costs").to_vec();
        let lpredcost = &lpredcost[..];

        let feastol = data.feastol;
        let mut num_redcost_large_domain_cols = 0i64;
        for &col in &data.integral_cols {
            if data.domain.col_upper[col] - data.domain.col_lower[col] >= 512.0 && lpredcost[col].abs() > feastol {
                num_redcost_large_domain_cols += 1;
            }
        }
        let mut max_num_steps_exp: i64 = 10;
        let mut expshift = crate::util::frexp_exp((num_redcost_large_domain_cols / 10) as f64);
        if expshift > 5 {
            expshift = expshift.min(max_num_steps_exp as i32);
            max_num_steps_exp = max_num_steps_exp - expshift as i64 + 5;
        }
        let max_num_steps: i64 = 1i64 << max_num_steps_exp;
        let lower_bound_value = data.lower_bound;

        let find_lurking_bounds =
            |direction: i64, bound: i64, other_bound: i64, is_other_bound_finite: bool, red_cost: f64, lurking_bounds: &mut Lurking, other_lurking_bounds: &mut Lurking| {
                if direction as f64 * red_cost == INF {
                    lurking_bounds.clear();
                    other_lurking_bounds.clear();
                    lurking_bounds.push((-INF, bound));
                    return;
                }
                let last_bound = if !is_other_bound_finite {
                    bound + direction * max_num_steps
                } else {
                    other_bound - direction
                };
                let mut step: i64 = 1;
                let range = direction * (last_bound - bound);
                if range > max_num_steps {
                    step = (range + max_num_steps - 1) >> max_num_steps_exp;
                }
                let shift = direction as f64 * (1.0 - 10.0 * feastol);
                step *= direction;
                let mut lurking_bound = bound;
                while direction * lurking_bound <= direction * last_bound {
                    let frac_bound = (lurking_bound - bound) as f64 + shift;
                    let required_cutoff_bound = frac_bound * red_cost + lpobjective;
                    if required_cutoff_bound < lower_bound_value + feastol {
                        lurking_bound += step;
                        continue;
                    }
                    let pos = lower_bound(lurking_bounds, required_cutoff_bound);
                    let useful = lurking_bounds[pos..].iter().all(|&(_, b)| direction * b >= direction * (lurking_bound + step));
                    if !useful {
                        lurking_bound += step;
                        continue;
                    }
                    lurking_bounds.insert(pos, (required_cutoff_bound, lurking_bound));
                    let mut it = pos;
                    let mut i = 0;
                    while i != it {
                        if direction * lurking_bounds[i].1 >= direction * lurking_bound {
                            lurking_bounds.remove(i);
                            it -= 1;
                        } else {
                            i += 1;
                        }
                    }
                    lurking_bound += step;
                }
            };

        for &col in &data.integral_cols {
            let lower = data.domain.col_lower[col];
            let upper = data.domain.col_upper[col];
            // Bounds beyond HiGHS's integer infinity are left alone (#3241).
            let iinf = i32::MAX as f64;
            if lpredcost[col] > feastol && lower > -iinf {
                let (lurking_upper, lurking_lower) = (&mut self.lurking_col_upper[col], &mut self.lurking_col_lower[col]);
                find_lurking_bounds(1, lower as i32 as i64, upper as i32 as i64, upper < iinf, lpredcost[col], lurking_upper, lurking_lower);
            } else if lpredcost[col] < -feastol && upper < iinf {
                let (lurking_lower, lurking_upper) = (&mut self.lurking_col_lower[col], &mut self.lurking_col_upper[col]);
                find_lurking_bounds(-1, upper as i32 as i64, lower as i32 as i64, lower > -iinf, lpredcost[col], lurking_lower, lurking_upper);
            }
        }
    }
}

fn change_local(localdomain: &mut Domain, parts: &mut super::data::Parts, domchg: DomainChange) {
    let mut env = DomainEnv {
        cx: &mut parts.cx,
        cliquetable: parts.cliquetable,
        implications: parts.implications,
    };
    localdomain.change_bound(domchg, Reason::unspecified(), &mut env);
}

fn propagate_local(localdomain: &mut Domain, parts: &mut super::data::Parts) {
    let mut env = DomainEnv {
        cx: &mut parts.cx,
        cliquetable: parts.cliquetable,
        implications: parts.implications,
    };
    localdomain.propagate(&mut env);
}
