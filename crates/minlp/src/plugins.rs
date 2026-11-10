//! `SCIPincludeDefaultPlugins` (`scipdefplugins.c`), restricted to the
//! plugins ported so far, in the reference's inclusion order.

use crate::{retcode::ScipResult, scip::Scip};

/// `SCIPincludeDefaultPlugins`: includes the ported default plugins.
pub fn include_default_plugins(scip: &mut Scip) -> ScipResult {
    // Nonlinear constraint handler must be before linear due to constraint
    // upgrading.
    crate::cons_nonlinear::include_conshdlr_nonlinear(scip)?;
    crate::nlhdlr_default::include_nlhdlr_default(scip)?;
    crate::nlhdlr_convex::include_nlhdlr_convex(scip)?;
    crate::nlhdlr_convex::include_nlhdlr_concave(scip)?;
    crate::nlhdlr_bilinear::include_nlhdlr_bilinear(scip)?;
    crate::nlhdlr_perspective::include_nlhdlr_perspective(scip)?;
    crate::nlhdlr_quadratic::include_nlhdlr_quadratic(scip)?;
    crate::nlhdlr_quotient::include_nlhdlr_quotient(scip)?;
    crate::nlhdlr_signomial::include_nlhdlr_signomial(scip)?;
    crate::nlhdlr_soc::include_nlhdlr_soc(scip)?;
    // Linear must be before its specializations due to constraint upgrading.
    crate::cons_linear::include_conshdlr_linear(scip)?;
    crate::cons_and::include_conshdlr_and(scip)?;
    crate::cons_bounddisjunction::include_conshdlr_bounddisjunction(scip)?;
    crate::cons_indicator::include_conshdlr_indicator(scip)?;
    crate::cons_integral::include_conshdlr_integral(scip)?;
    crate::cons_knapsack::include_conshdlr_knapsack(scip)?;
    crate::cons_logicor::include_conshdlr_logicor(scip)?;
    crate::cons_setppc::include_conshdlr_setppc(scip)?;
    crate::cons_varbound::include_conshdlr_varbound(scip)?;
    crate::cons_components::include_conshdlr_components(scip)?;
    crate::presol_trivial::include_presol_trivial(scip)?;
    crate::presol_inttobinary::include_presol_inttobinary(scip)?;
    crate::presol_gateextraction::include_presol_gateextraction(scip)?;
    crate::presol_dualcomp::include_presol_dualcomp(scip)?;
    crate::presol_domcol::include_presol_domcol(scip)?;
    crate::presol_implics::include_presol_implics(scip)?;
    crate::presol_sparsify::include_presol_sparsify(scip)?;
    crate::presol_dualsparsify::include_presol_dualsparsify(scip)?;
    crate::nodesel_estimate::include_nodesel_estimate(scip)?;
    crate::nodesel_restartdfs::include_nodesel_restartdfs(scip)?;
    crate::branch_relpscost::include_branchrule_relpscost(scip)?;
    crate::branch_pscost::include_branchrule_pscost(scip)?;
    crate::branch_inference::include_branchrule_inference(scip)?;
    crate::branch_mostinf::include_branchrule_mostinf(scip)?;
    crate::event_estim::include_event_hdlr_estim(scip)?;
    crate::heur_trivial::include_heur_trivial(scip)?;
    crate::heur_clique::include_heur_clique(scip)?;
    crate::heur_locks::include_heur_locks(scip)?;
    crate::heur_vbounds::include_heur_vbounds(scip)?;
    crate::heur_shiftandpropagate::include_heur_shiftandpropagate(scip)?;
    crate::heur_simplerounding::include_heur_simplerounding(scip)?;
    crate::heur_randrounding::include_heur_randrounding(scip)?;
    crate::heur_zirounding::include_heur_zirounding(scip)?;
    crate::heur_rounding::include_heur_rounding(scip)?;
    crate::heur_shifting::include_heur_shifting(scip)?;
    crate::heur_intshifting::include_heur_intshifting(scip)?;
    crate::heur_oneopt::include_heur_oneopt(scip)?;
    crate::heur_indicator::include_heur_indicator(scip)?;
    crate::heur_adaptivediving::include_heur_adaptivediving(scip)?;
    crate::heur_indicatordiving::include_heur_indicatordiving(scip)?;
    crate::heur_farkasdiving::include_heur_farkasdiving(scip)?;
    crate::heur_feaspump::include_heur_feaspump(scip)?;
    crate::heur_conflictdiving::include_heur_conflictdiving(scip)?;
    crate::heur_coefdiving::include_heur_coefdiving(scip)?;
    crate::heur_pscostdiving::include_heur_pscostdiving(scip)?;
    crate::heur_fracdiving::include_heur_fracdiving(scip)?;
    crate::heur_veclendiving::include_heur_veclendiving(scip)?;
    crate::heur_distributiondiving::include_heur_distributiondiving(scip)?;
    crate::heur_actconsdiving::include_heur_actconsdiving(scip)?;
    crate::heur_objpscostdiving::include_heur_objpscostdiving(scip)?;
    crate::heur_rootsoldiving::include_heur_rootsoldiving(scip)?;
    crate::heur_linesearchdiving::include_heur_linesearchdiving(scip)?;
    crate::heur_guideddiving::include_heur_guideddiving(scip)?;
    crate::heur_rens::include_heur_rens(scip)?;
    crate::heur_alns::include_heur_alns(scip)?;
    crate::heur_rins::include_heur_rins(scip)?;
    crate::heur_gins::include_heur_gins(scip)?;
    crate::heur_crossover::include_heur_crossover(scip)?;
    crate::heur_lpface::include_heur_lpface(scip)?;
    crate::heur_undercover::include_heur_undercover(scip)?;
    crate::heur_trysol::include_heur_trysol(scip)?;
    crate::prop_rootredcost::include_prop_rootredcost(scip)?;
    crate::prop_dualfix::include_prop_dualfix(scip)?;
    crate::prop_genvbounds::include_prop_genvbounds(scip)?;
    crate::prop_vbounds::include_prop_vbounds(scip)?;
    crate::prop_pseudoobj::include_prop_pseudoobj(scip)?;
    crate::prop_redcost::include_prop_redcost(scip)?;
    crate::prop_probing::include_prop_probing(scip)?;
    crate::prop_obbt::include_prop_obbt(scip)?;
    crate::sepa_flower::include_sepa_flower(scip)?;
    crate::sepa_rlt::include_sepa_rlt(scip)?;
    crate::sepa_disjunctive::include_sepa_disjunctive(scip)?;
    crate::sepa_minor::include_sepa_minor(scip)?;
    crate::sepa_mixing::include_sepa_mixing(scip)?;
    crate::sepa_impliedbounds::include_sepa_impliedbounds(scip)?;
    crate::sepa_gomory::include_sepa_gomory(scip)?;
    crate::sepa_aggregation::include_sepa_aggregation(scip)?;
    crate::sepa_clique::include_sepa_clique(scip)?;
    crate::sepa_zerohalf::include_sepa_zerohalf(scip)?;
    crate::sepa_mcf::include_sepa_mcf(scip)?;
    crate::cutsel_hybrid::include_cutsel_hybrid(scip)?;
    crate::expr_pow::include_exprhdlr_signpower(scip)?;
    crate::expr_pow::include_exprhdlr_pow(scip)?;
    crate::expr_product::include_exprhdlr_product(scip)?;
    crate::expr_sum::include_exprhdlr_sum(scip)?;
    crate::expr_value::include_exprhdlr_value(scip)?;
    crate::expr_var::include_exprhdlr_var(scip)?;
    crate::relax_benders::include_relax_benders(scip)?;

    Ok(())
}
