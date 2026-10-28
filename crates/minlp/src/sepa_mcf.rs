//! Port of `sepa_mcf.c`: multi-commodity-flow network cut separator.
//!
//! The network is extracted from the LP rows by score heuristics, shrunk
//! into clusters and cuts between cluster subsets are separated with c-MIR
//! (and flow cover / knapsack cover) on aggregations of the capacity rows.

use std::any::Any;

use crate::{
    api,
    def::{INVALID, REAL_MAX, max, min},
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ColId, RowId, ScipResultCode, SepaId, SolId, VarType},
    var::NONE,
};

const SEPA_NAME: &str = "mcf";
const SEPA_DESC: &str = "multi-commodity-flow network cut separator";
const SEPA_PRIORITY: i32 = -10000;
const SEPA_FREQ: i32 = 0;
const SEPA_MAXBOUNDDIST: f64 = 0.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Number of clusters to generate in the shrunken network.
const DEFAULT_NCLUSTERS: i32 = 5;
/// Maximal valid range max(|weights|)/min(|weights|) of row weights for CMIR.
const DEFAULT_MAXWEIGHTRANGE: f64 = 1e+06;
/// Maximal number of different deltas to try (-1: unlimited) for CMIR.
const DEFAULT_MAXTESTDELTA: i32 = 20;
/// Should negative values also be tested in scaling? for CMIR
const DEFAULT_TRYNEGSCALING: bool = false;
/// Should an additional variable be complemented if f0 = 0? for CMIR
const DEFAULT_FIXINTEGRALRHS: bool = true;
/// Should generated cuts be removed from the LP if they are no longer tight?
const DEFAULT_DYNAMICCUTS: bool = true;
/// Model type of network (0: auto, 1:directed, 2:undirected).
const DEFAULT_MODELTYPE: i32 = 0;
/// Maximal number of cuts separated per separation round (-1: unlimited).
const DEFAULT_MAXSEPACUTS: i32 = 100;
/// Maximal number of cuts separated per separation round in root node (-1:
/// unlimited).
const DEFAULT_MAXSEPACUTSROOT: i32 = 200;
/// Maximum inconsistency ratio (inconsistencies/(arcs*commodities)) at all.
const DEFAULT_MAXINCONSISTENCYRATIO: f64 = 0.02;
/// Maximum inconsistency ratio for arcs not to be deleted.
const DEFAULT_MAXARCINCONSISTENCYRATIO: f64 = 0.5;
/// Should we only separate if the cuts shores are connected.
const DEFAULT_CHECKCUTSHORECONNECTIVITY: bool = true;
/// Should we separate inequalities based on single node cuts.
const DEFAULT_SEPARATESINGLENODECUTS: bool = true;
/// Should we separate flowcutset inequalities.
const DEFAULT_SEPARATEFLOWCUTSET: bool = true;
/// Should we separate knapsack cover inequalities.
const DEFAULT_SEPARATEKNAPSACK: bool = true;

const BOUNDSWITCH: f64 = 0.5;
const POSTPROCESS: bool = true;
/// We allow variable bound substitution for variables with continuous
/// vartype only.
const VARTYPEUSEVBDS: i32 = 2;
const MINFRAC: f64 = 0.05;
const MAXFRAC: f64 = 0.999;

/// Maximum number of columns.
const MAXCOLS: i32 = 2000000;
/// Maximal length of base inequality.
fn maxaggrlen(nvars: i32) -> f64 {
    0.1 * nvars as f64 + 1000.0
}
/// Minimum ratio cols/rows for the separator to be switched on.
const MINCOLROWRATIO: f64 = 0.01;
/// Maximum ratio cols/rows for the separator to be switched on.
const MAXCOLROWRATIO: f64 = 100.0;
/// Maximum ratio flowvars/flowrows for the separator to be switched on.
const MAXFLOWVARFLOWROWRATIO: f64 = 100.0;
/// Maximum ratio arcs/nodes for the separator to be switched on.
const MAXARCNODERATIO: f64 = 100.0;
/// Maximum number of networks to consider.
const MAXNETWORKS: i32 = 4;
/// Maximum density of rows to be accepted as flow candidates.
const MAXFLOWCANDDENSITY: f64 = 0.1;
/// Minimal size of commodity relative to largest commodity to keep it in the
/// network.
const MINCOMNODESFRACTION: f64 = 0.5;
/// Minimal number of nodes in network to keep it for separation.
const MINNODES: i32 = 3;
/// Minimal number of arcs in network to keep it for separation.
const MINARCS: i32 = 3;
/// Maximal slack of weighted capacity constraints to use in aggregation.
const MAXCAPACITYSLACK: f64 = 0.1;
/// Threshold for the percentage of commodities an uncapacitated arc should
/// appear in.
const UNCAPACITATEDARCSTRESHOLD: f64 = 0.8;
// HASHSIZE_NODEPAIRS (minimal size of hash table for nodepairs) only sizes
// the C hash table; the port uses a map.

/// Model type of the network (`SCIP_MCFMODELTYPE`).
const MCFMODELTYPE_AUTO: i32 = 0;
const MCFMODELTYPE_DIRECTED: i32 = 1;
const MCFMODELTYPE_UNDIRECTED: i32 = 2;

/// Effort level for separation (`MCFEFFORTLEVEL`).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum EffortLevel {
    #[default]
    Off,
    Default,
    Aggressive,
}

/// Extracted multi-commodity-flow network (`SCIP_MCFNETWORK`).
#[derive(Default)]
struct McfNetwork {
    /// `nodeflowrows[v][k]`: flow conservation constraint for node v and
    /// commodity k; NONE if this node does not exist in the commodity.
    nodeflowrows: Vec<Vec<RowId>>,
    /// Scaling factors to convert `nodeflowrows[v][k]` into a +/-1 <= row.
    nodeflowscales: Vec<Vec<f64>>,
    /// Does `nodeflowrows[v][k]` have to be inverted to fit the network
    /// structure?
    nodeflowinverted: Vec<Vec<bool>>,
    /// `arccapacity[a]`: capacity constraint on arc a; NONE if uncapacitated.
    arccapacityrows: Vec<RowId>,
    /// Scaling factors to convert `arccapacity[a]` into a <= row with
    /// positive entries for the flow variables.
    arccapacityscales: Vec<f64>,
    arcsources: Vec<i32>,
    arctargets: Vec<i32>,
    /// Commodity number of each column, or -1.
    colcommodity: Vec<i32>,
    nnodes: i32,
    narcs: i32,
    nuncapacitatedarcs: i32,
    ncommodities: i32,
    modeltype: i32,
}

/// Separator data.
#[derive(Default)]
struct SepaData {
    mcfnetworks: Vec<McfNetwork>,
    /// Number of multi-commodity-flow networks (-1: extraction not yet done).
    nmcfnetworks: i32,
    nclusters: i32,
    maxweightrange: f64,
    maxtestdelta: i32,
    trynegscaling: bool,
    fixintegralrhs: bool,
    dynamiccuts: bool,
    modeltype: i32,
    maxsepacuts: i32,
    maxsepacutsroot: i32,
    maxinconsistencyratio: f64,
    maxarcinconsistencyratio: f64,
    checkcutshoreconnectivity: bool,
    separatesinglenodecuts: bool,
    separateflowcutset: bool,
    separateknapsack: bool,
    /// Did we find a cut in the last round?
    lastroundsuccess: bool,
    effortlevel: EffortLevel,
}

/// Internal MCF extraction data to pass to subroutines (`MCFDATA`).
#[derive(Default)]
struct McfData {
    flowrowsigns: Vec<u8>,
    flowrowscalars: Vec<f64>,
    flowrowscores: Vec<f64>,
    capacityrowsigns: Vec<u8>,
    capacityrowscores: Vec<f64>,
    flowcands: Vec<i32>,
    nflowcands: i32,
    capacitycands: Vec<i32>,
    ncapacitycands: i32,
    plusflow: Vec<bool>,
    minusflow: Vec<bool>,
    ncommodities: i32,
    nemptycommodities: i32,
    /// +1: regular, -1: all arcs have opposite direction; 0: undecided.
    commoditysigns: Vec<i32>,
    commoditysignssize: i32,
    colcommodity: Vec<i32>,
    rowcommodity: Vec<i32>,
    colarcid: Vec<i32>,
    rowarcid: Vec<i32>,
    rownodeid: Vec<i32>,
    arcarraysize: i32,
    arcsources: Vec<i32>,
    arctargets: Vec<i32>,
    /// Source node for flow columns, -1 for non existing, -2 for unknown.
    colsources: Vec<i32>,
    coltargets: Vec<i32>,
    firstoutarcs: Vec<i32>,
    firstinarcs: Vec<i32>,
    nextoutarcs: Vec<i32>,
    nextinarcs: Vec<i32>,
    newcols: Vec<i32>,
    nnewcols: i32,
    narcs: i32,
    nnodes: i32,
    ninconsistencies: f64,
    capacityrows: Vec<RowId>,
    capacityrowssize: i32,
    colisincident: Vec<bool>,
    zeroarcarray: Vec<i32>,
    modeltype: i32,
}

/// We may use the constraint as lhs <= a*x.
const LHSPOSSIBLE: u8 = 1;
/// We may use the constraint as a*x <= rhs.
const RHSPOSSIBLE: u8 = 2;
/// We have chosen to use the constraint as lhs <= a*x.
const LHSASSIGNED: u8 = 4;
/// We have chosen to use the constraint as a*x <= rhs.
const RHSASSIGNED: u8 = 8;
/// We need to invert the row.
const INVERTED: u8 = 16;
/// We have chosen to not use the constraint.
const DISCARDED: u8 = 32;
/// The capacity candidate has two flow variables for a commodity.
const UNDIRECTED: u8 = 64;

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// The LP rows (`SCIPgetLPRowsData`, `SCIPgetLPRows`).
fn lp_rows(scip: &Scip) -> Vec<RowId> {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    } else {
        Vec::new()
    }
}

/// `SCIPgetNLPCols`.
fn n_lp_cols(scip: &Scip) -> i32 {
    if scip.tree.as_ref().unwrap().focuslpconstructed {
        scip.lp.as_ref().unwrap().ncols
    } else {
        0
    }
}

/// `SCIPcolGetLPPos` of the i-th column of a row.
fn row_col_pos(scip: &Scip, row: RowId, i: usize) -> i32 {
    scip.cols[scip.rows[row].cols[i]].lppos
}

/*
 * Local methods
 */

/// `mcfnetworkFree`: releases the rows of an MCF network.
fn mcfnetwork_free(scip: &mut Scip, mcfnetwork: &mut McfNetwork) -> ScipResult {
    for v in 0..mcfnetwork.nnodes as usize {
        for k in 0..mcfnetwork.ncommodities as usize {
            if mcfnetwork.nodeflowrows[v][k] != NONE {
                crate::scip_lp::release_row(scip, &mut mcfnetwork.nodeflowrows[v][k])?;
            }
        }
    }
    for a in 0..mcfnetwork.narcs as usize {
        if mcfnetwork.arccapacityrows[a] != NONE {
            crate::scip_lp::release_row(scip, &mut mcfnetwork.arccapacityrows[a])?;
        }
    }
    *mcfnetwork = McfNetwork::default();
    Ok(())
}

/// `mcfnetworkFill`: fills the MCF network structure with the MCF data.
#[allow(clippy::too_many_arguments)]
fn mcfnetwork_fill(
    scip: &mut Scip,
    mcfnetwork: &mut McfNetwork,
    mcfdata: &McfData,
    compnodeid: &mut [i32],
    compnodes: &[i32],
    ncompnodes: i32,
    comparcs: &[i32],
    ncomparcs: i32,
) -> ScipResult {
    let flowrowsigns = &mcfdata.flowrowsigns;
    let flowrowscalars = &mcfdata.flowrowscalars;
    let capacityrowsigns = &mcfdata.capacityrowsigns;
    let flowcands = &mcfdata.flowcands;
    let nflowcands = mcfdata.nflowcands;
    let ncommodities = mcfdata.ncommodities;
    let commoditysigns = &mcfdata.commoditysigns;
    let colcommodity = &mcfdata.colcommodity;
    let rowcommodity = &mcfdata.rowcommodity;
    let rownodeid = &mcfdata.rownodeid;
    let capacityrows = &mcfdata.capacityrows;
    let modeltype = mcfdata.modeltype;

    debug_assert!(modeltype != MCFMODELTYPE_AUTO);
    debug_assert!(2 <= ncompnodes && ncompnodes <= mcfdata.nnodes);
    debug_assert!(1 <= ncomparcs && ncomparcs <= mcfdata.narcs);
    debug_assert!(ncommodities > 0);

    // Allocate temporary memory and initialize demand array.
    let mut comdemands = vec![0.0f64; ncommodities as usize];

    // Initialize k -> compk mapping.
    let mut compcommodity = vec![-1i32; ncommodities as usize];

    // Get LP rows and cols data.
    let rows = lp_rows(scip);
    let ncols = n_lp_cols(scip);

    // Generate v -> compv mapping.
    for i in 0..ncompnodes as usize {
        let v = compnodes[i];
        compnodeid[v as usize] = i as i32;
    }

    // Generate k -> compk mapping.
    let mut ncompcommodities = 0;
    for i in 0..nflowcands as usize {
        let r = flowcands[i] as usize;
        let rv = rownodeid[r];
        if rv >= 0 && compnodeid[rv as usize] >= 0 {
            let k = rowcommodity[r];
            debug_assert!(0 <= k && k < ncommodities);
            if compcommodity[k as usize] == -1 {
                compcommodity[k as usize] = ncompcommodities;
                ncompcommodities += 1;
            }
        }
    }

    // Record model and flow type.
    mcfnetwork.modeltype = modeltype;

    // Record network size.
    mcfnetwork.nnodes = ncompnodes;
    mcfnetwork.narcs = ncomparcs;
    mcfnetwork.nuncapacitatedarcs = 0;
    mcfnetwork.ncommodities = ncompcommodities;

    // Allocate memory for arrays and initialize with default values.
    let nn = mcfnetwork.nnodes as usize;
    let nk = mcfnetwork.ncommodities as usize;
    mcfnetwork.nodeflowrows = vec![vec![NONE; nk]; nn];
    mcfnetwork.nodeflowscales = vec![vec![0.0; nk]; nn];
    mcfnetwork.nodeflowinverted = vec![vec![false; nk]; nn];

    let na = mcfnetwork.narcs as usize;
    mcfnetwork.arccapacityrows = vec![NONE; na];
    mcfnetwork.arccapacityscales = vec![0.0; na];
    mcfnetwork.arcsources = vec![-1; na];
    mcfnetwork.arctargets = vec![-1; na];
    mcfnetwork.colcommodity = vec![0; ncols as usize];

    // Fill in existing node data.
    for i in 0..nflowcands as usize {
        let r = flowcands[i] as usize;
        let rv = rownodeid[r];
        if rv >= 0 && compnodeid[rv as usize] >= 0 {
            let v = compnodeid[rv as usize] as usize;
            let rk = rowcommodity[r];
            debug_assert!((flowrowsigns[r] & (LHSASSIGNED | RHSASSIGNED)) != 0);

            let k = compcommodity[rk as usize] as usize;

            // Fill in node -> row assignment.
            crate::lp::row_capture(scip, rows[r]);
            mcfnetwork.nodeflowrows[v][k] = rows[r];
            let mut scale = flowrowscalars[r];
            if (flowrowsigns[r] & LHSASSIGNED) != 0 {
                scale *= -1.0;
            }
            if commoditysigns[rk as usize] == -1 {
                scale *= -1.0;
            }
            mcfnetwork.nodeflowscales[v][k] = scale;
            mcfnetwork.nodeflowinverted[v][k] = (flowrowsigns[r] & INVERTED) != 0;
        }
    }

    // Fill in existing arc data.
    for a in 0..na {
        let globala = comparcs[a] as usize;
        let capacityrow = capacityrows[globala];

        mcfnetwork.arccapacityscales[a] = 1.0;

        // If arc is capacitated.
        if capacityrow != NONE {
            let r = scip.rows[capacityrow].lppos as usize;
            debug_assert!((capacityrowsigns[r] & (LHSASSIGNED | RHSASSIGNED)) != 0);
            debug_assert!((capacityrowsigns[r] & INVERTED) == 0);
            debug_assert!(mcfdata.rowarcid[r] == globala as i32);

            crate::lp::row_capture(scip, capacityrow);
            mcfnetwork.arccapacityrows[a] = capacityrow;

            // Scale constraint such that the coefficients for the flow
            // variables are normalized in such a way that coefficients in
            // multiple capacity constraints that belong to the same
            // commodity are (hopefully) equal.
            let rowlen = scip.rows[capacityrow].nlpcols as usize;
            for j in 0..rowlen {
                let c = row_col_pos(scip, capacityrow, j) as usize;
                let k = colcommodity[c];
                if k >= 0 && comdemands[k as usize] != 0.0 {
                    // Update the scaling factor.
                    mcfnetwork.arccapacityscales[a] = comdemands[k as usize] / scip.rows[capacityrow].vals[j];
                    break;
                }
            }

            // Use negative scaling if we use the left hand side, use
            // positive scaling if we use the right hand side.
            mcfnetwork.arccapacityscales[a] = mcfnetwork.arccapacityscales[a].abs();
            if (capacityrowsigns[r] & LHSASSIGNED) != 0 {
                mcfnetwork.arccapacityscales[a] *= -1.0;
            }

            // Record the commodity demands.
            for j in 0..rowlen {
                let c = row_col_pos(scip, capacityrow, j) as usize;
                let k = colcommodity[c];
                if k >= 0 && comdemands[k as usize] == 0.0 {
                    comdemands[k as usize] = mcfnetwork.arccapacityscales[a] * scip.rows[capacityrow].vals[j];
                }
            }
        } else {
            // Arc is uncapacitated.
            mcfnetwork.arccapacityrows[a] = NONE;
            mcfnetwork.nuncapacitatedarcs += 1;
        }

        // Copy the source/target node assignment.
        if mcfdata.arcsources[globala] >= 0 {
            mcfnetwork.arcsources[a] = compnodeid[mcfdata.arcsources[globala] as usize];
        }
        if mcfdata.arctargets[globala] >= 0 {
            mcfnetwork.arctargets[a] = compnodeid[mcfdata.arctargets[globala] as usize];
        }
    }

    // Translate colcommodity array.
    for c in 0..ncols as usize {
        let k = colcommodity[c];
        mcfnetwork.colcommodity[c] = if k >= 0 { compcommodity[k as usize] } else { -1 };
    }

    // Reset v -> compv mapping.
    for i in 0..ncompnodes as usize {
        debug_assert!(compnodeid[compnodes[i] as usize] == i as i32);
        compnodeid[compnodes[i] as usize] = -1;
    }

    Ok(())
}

/// `compCands`: comparator method for flow and capacity row candidates
/// (non-increasing score).
fn comp_cands(rowscores: &[f64], ind1: i32, ind2: i32) -> i32 {
    if rowscores[ind2 as usize] < rowscores[ind1 as usize] {
        -1
    } else if rowscores[ind2 as usize] > rowscores[ind1 as usize] {
        1
    } else {
        0
    }
}

/// `SCIProwGetDualsol` with the C code's SCIP_INVALID check folded in.
fn row_dualsol(scip: &Scip, row: RowId) -> f64 {
    crate::scip_lp::row_get_dualsol(scip, row)
}

/// `extractFlowRows`: extracts flow conservation from the LP.
fn extract_flow_rows(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    let rows = lp_rows(scip);
    let nrows = rows.len();
    let ncols = n_lp_cols(scip);

    // Allocate temporary memory for extraction data.
    mcfdata.flowrowsigns = vec![0; nrows];
    mcfdata.flowrowscalars = vec![0.0; nrows];
    mcfdata.flowrowscores = vec![0.0; nrows];
    mcfdata.flowcands = vec![0; nrows];

    debug_assert!(mcfdata.nflowcands == 0);

    let mut maxdualflow = 0.0;
    for r in 0..nrows {
        let row = rows[r];
        debug_assert!(scip.rows[row].lppos == r as i32);

        // Get dual solution, if available.
        let mut absdualsol = row_dualsol(scip, row);
        if absdualsol == INVALID {
            absdualsol = 0.0;
        }
        absdualsol = absdualsol.abs();

        mcfdata.flowrowsigns[r] = 0;
        mcfdata.flowrowscalars[r] = 0.0;
        mcfdata.flowrowscores[r] = 0.0;

        let rw = &scip.rows[row];

        // Ignore modifiable rows.
        if rw.modifiable {
            continue;
        }

        // Ignore empty rows.
        let rowlen = rw.nlpcols as usize;
        if rowlen == 0 {
            continue;
        }

        // No dense rows please.
        if rowlen as f64 > MAXFLOWCANDDENSITY * ncols as f64 {
            continue;
        }

        let rowlhs = rw.lhs - rw.constant;
        let rowrhs = rw.rhs - rw.constant;

        // Identify flow conservation constraints.
        let coef = rw.vals[0usize].abs();
        let mut hasposcoef = false;
        let mut hasnegcoef = false;
        let mut nbinvars = 0;
        let mut nintvars = 0;
        let mut nimplintvars = 0;
        let mut ncontvars = 0;
        let mut i = 0;
        while i < rowlen {
            let absval = rw.vals[i].abs();

            if !scip.set.is_eq(absval, coef) {
                break;
            }

            hasposcoef = hasposcoef || rw.vals[i] > 0.0;
            hasnegcoef = hasnegcoef || rw.vals[i] < 0.0;
            let var = scip.cols[rw.cols[i]].var;

            if crate::var::var_is_implied_integral(scip, var) {
                nimplintvars += 1;
            } else {
                match scip.vars[var].vartype {
                    VarType::Binary => nbinvars += 1,
                    VarType::Integer => nintvars += 1,
                    VarType::Continuous => ncontvars += 1,
                    #[allow(unreachable_patterns)]
                    _ => return Err(Retcode::InvalidData),
                }
            }
            i += 1;
        }

        if i == rowlen {
            // Flow conservation constraints should always be a*x <= -d. If
            // lhs and rhs are finite, both sides are still valid candidates.
            if !scip.set.is_infinity(-rowlhs) {
                mcfdata.flowrowsigns[r] |= LHSPOSSIBLE;
            }
            if !scip.set.is_infinity(rowrhs) {
                mcfdata.flowrowsigns[r] |= RHSPOSSIBLE;
            }
            mcfdata.flowrowscalars[r] = 1.0 / coef;
            mcfdata.flowcands[mcfdata.nflowcands as usize] = r as i32;
            mcfdata.nflowcands += 1;
        }

        // Calculate flow row score.
        if (mcfdata.flowrowsigns[r] & (LHSPOSSIBLE | RHSPOSSIBLE)) != 0 {
            // Row does not need to be scaled: score +1000.
            if scip.set.is_eq(mcfdata.flowrowscalars[r], 1.0) {
                mcfdata.flowrowscores[r] += 1000.0;
            }

            // Row has positive and negative coefficients: score +500.
            if hasposcoef && hasnegcoef {
                mcfdata.flowrowscores[r] += 500.0;
            }

            // All variables are of the same type: continuous +1000,
            // integer +500, binary +100.
            if ncontvars == rowlen || nimplintvars == rowlen {
                mcfdata.flowrowscores[r] += 1000.0;
            } else if nintvars + nimplintvars == rowlen {
                mcfdata.flowrowscores[r] += 500.0;
            } else if nbinvars == rowlen {
                mcfdata.flowrowscores[r] += 100.0;
            }

            // The longer the row, the earlier we want to process it: score
            // +10*len/(len+10).
            mcfdata.flowrowscores[r] += 10.0 * rowlen as f64 / (rowlen as f64 + 10.0);

            // Row is an equation: score +50, tie-breaking.
            if (mcfdata.flowrowsigns[r] & (LHSPOSSIBLE | RHSPOSSIBLE)) == (LHSPOSSIBLE | RHSPOSSIBLE) {
                mcfdata.flowrowscores[r] += 50.0;
            }

            debug_assert!(mcfdata.flowrowscores[r] > 0.0);

            // Update maximum dual solution value for additional score tie
            // breaking.
            maxdualflow = max(maxdualflow, absdualsol);
        }
    }

    // Apply additional score tie breaking using the dual solutions.
    if scip.set.is_positive(maxdualflow) {
        for i in 0..mcfdata.nflowcands as usize {
            let r = mcfdata.flowcands[i] as usize;
            let mut dualsol = row_dualsol(scip, rows[r]);
            if dualsol == INVALID {
                dualsol = 0.0;
            } else if mcfdata.flowrowsigns[r] == (LHSPOSSIBLE | RHSPOSSIBLE) {
                dualsol = dualsol.abs();
            } else if mcfdata.flowrowsigns[r] == RHSPOSSIBLE {
                dualsol = -dualsol;
            }
            mcfdata.flowrowscores[r] += dualsol / maxdualflow + 1.0;
            debug_assert!(mcfdata.flowrowscores[r] > 0.0);
        }
    }

    // Sort candidates by score.
    let n = mcfdata.nflowcands as usize;
    let scores = &mcfdata.flowrowscores;
    crate::sort::sort(&mut mcfdata.flowcands[..n], (), |&a, &b| comp_cands(scores, a, b));

    Ok(())
}

/// `extractCapacityRows`: extracts capacity rows from the LP.
fn extract_capacity_rows(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    let ncommodities = mcfdata.ncommodities;
    let nactivecommodities = mcfdata.ncommodities - mcfdata.nemptycommodities;
    let mut modeltype = mcfdata.modeltype;

    let rows = lp_rows(scip);
    let nrows = rows.len();

    // Allocate temporary memory for extraction data.
    mcfdata.capacityrowsigns = vec![0; nrows];
    mcfdata.capacityrowscores = vec![0.0; nrows];
    mcfdata.capacitycands = vec![0; nrows];

    debug_assert!(mcfdata.ncapacitycands == 0);

    // Allocate temporary memory for model type identification.
    let mut ncolspercommodity = vec![0i32; ncommodities as usize];
    let mut maxcolspercommodity = vec![0i32; nrows];

    // Identify model type and set the maximal number of flow variables per
    // capacity constraint and commodity.
    let maxcolspercommoditylimit = match modeltype {
        MCFMODELTYPE_AUTO => 2, // will be set to 1 later if we detect that the network is directed
        MCFMODELTYPE_DIRECTED => 1,
        MCFMODELTYPE_UNDIRECTED => 2,
        _ => return Err(Retcode::InvalidData),
    };

    let mut maxdualcapacity = 0.0;
    let mut directedcandsscore = 0.0;
    let mut undirectedcandsscore = 0.0;
    for r in 0..nrows {
        let row = rows[r];
        debug_assert!(scip.rows[row].lppos == r as i32);

        mcfdata.capacityrowsigns[r] = 0;
        mcfdata.capacityrowscores[r] = 0.0;

        let rw = &scip.rows[row];

        // Ignore modifiable rows.
        if rw.modifiable {
            continue;
        }

        // Ignore empty rows.
        let rowlen = rw.nlpcols as usize;
        if rowlen == 0 {
            continue;
        }

        // Ignore rows that have already been used as flow conservation
        // constraints.
        if (mcfdata.flowrowsigns[r] & (LHSASSIGNED | RHSASSIGNED)) != 0 {
            continue;
        }

        // Get dual solution, if available.
        let mut absdualsol = row_dualsol(scip, row);
        if absdualsol == INVALID {
            absdualsol = 0.0;
        }
        absdualsol = absdualsol.abs();

        let rowlhs = rw.lhs - rw.constant;
        let rowrhs = rw.rhs - rw.constant;

        // Reset commodity counting array.
        ncolspercommodity.fill(0);
        maxcolspercommodity[r] = 0;

        // Identify capacity constraints.
        let mut nposflowcoefs = 0i32;
        let mut nnegflowcoefs = 0i32;
        let mut nposcapacitycoefs = 0i32;
        let mut nnegcapacitycoefs = 0i32;
        let mut nbadcoefs = 0i32;
        let mut ncoveredcommodities = 0i32;
        let mut sameflowcoef = 0.0;
        let mut sameabsflowcoef = 0.0;
        let mut maxabscapacitycoef = 0.0;

        let mut rowsign = 0u8;
        if !scip.set.is_infinity(-rowlhs) {
            rowsign |= LHSPOSSIBLE;
        }
        if !scip.set.is_infinity(rowrhs) {
            rowsign |= RHSPOSSIBLE;
        }
        for i in 0..rowlen {
            let c = scip.cols[rw.cols[i]].lppos as usize;
            let val = rw.vals[i];

            // Check if this is a flow variable.
            let k = mcfdata.colcommodity[c];
            debug_assert!(-1 <= k && k < ncommodities);
            if k >= 0 {
                let abscoef = val.abs();
                if sameflowcoef == 0.0 {
                    sameflowcoef = val;
                } else if !scip.set.is_eq(sameflowcoef, val) {
                    sameflowcoef = REAL_MAX;
                }
                if sameabsflowcoef == 0.0 {
                    sameabsflowcoef = abscoef;
                } else if !scip.set.is_eq(sameabsflowcoef, abscoef) {
                    sameabsflowcoef = REAL_MAX;
                }

                if val > 0.0 {
                    nposflowcoefs += 1;
                } else {
                    nnegflowcoefs += 1;
                }

                // Count number of covered commodities in capacity candidate.
                let k = k as usize;
                if ncolspercommodity[k] == 0 {
                    ncoveredcommodities += 1;
                }
                ncolspercommodity[k] += 1;
                maxcolspercommodity[r] = max(maxcolspercommodity[r], ncolspercommodity[k]);

                if ncolspercommodity[k] >= 2 {
                    mcfdata.capacityrowsigns[r] |= UNDIRECTED;
                }
            } else {
                // Save maximal capacity coef.
                let abscoef = val.abs();
                if abscoef > maxabscapacitycoef {
                    maxabscapacitycoef = abscoef;
                }

                // A variable which is not a flow variable can be used as
                // capacity variable.
                if val > 0.0 {
                    nposcapacitycoefs += 1;
                } else {
                    nnegcapacitycoefs += 1;
                }

                // A continuous variable is considered to be not so nice.
                if !crate::var::var_is_integral(scip, scip.cols[rw.cols[i]].var) {
                    nbadcoefs += 1;
                }
            }
        }
        let _ = nbadcoefs;

        // Check if this is a valid capacity constraint: it has at least one
        // flow variable.
        if rowsign != 0 && nposflowcoefs + nnegflowcoefs > 0 {
            mcfdata.capacityrowsigns[r] |= rowsign;
            mcfdata.capacitycands[mcfdata.ncapacitycands as usize] = r as i32;
            mcfdata.ncapacitycands += 1;

            // Calculate capacity row score.
            let s = &mut mcfdata.capacityrowscores[r];
            *s = 1.0;

            // Calculate mean commodity excess: in the (un)directed case
            // there should be exactly one (two) flow variable per commodity.
            debug_assert!(ncoveredcommodities > 0);
            let commodityexcessratio = ((nposflowcoefs + nnegflowcoefs) as f64 / ncoveredcommodities as f64 - maxcolspercommoditylimit as f64).abs();

            *s += 1000.0 * max(0.0, 2.0 - commodityexcessratio);

            // Row is of type f - c*x <= b: score +1000.
            if (mcfdata.capacityrowsigns[r] & RHSPOSSIBLE) != 0 && nnegflowcoefs == 0 && nposcapacitycoefs == 0 && nnegcapacitycoefs > 0 {
                mcfdata.capacityrowscores[r] += 1000.0;
            }
            if (mcfdata.capacityrowsigns[r] & LHSPOSSIBLE) != 0 && nposflowcoefs == 0 && nposcapacitycoefs > 0 && nnegcapacitycoefs == 0 {
                mcfdata.capacityrowscores[r] += 1000.0;
            }

            // Almost all commodities are covered: score
            // +2000*ncoveredcommodities/(nactivecommodities+3).
            debug_assert!(nactivecommodities + 3 > 0);
            let s = &mut mcfdata.capacityrowscores[r];
            *s += 2000.0 * ncoveredcommodities as f64 / (nactivecommodities + 3) as f64;

            // All coefficients of flow variables are +1 or all are -1:
            // score +500.
            if scip.set.is_eq(sameflowcoef.abs(), 1.0) {
                *s += 500.0;
            }

            // All coefficients of flow variables are equal: score +250.
            if sameflowcoef != 0.0 && sameflowcoef != REAL_MAX {
                *s += 250.0;
            }

            // All coefficients of flow variables are +1 or -1: score +100.
            if scip.set.is_eq(sameabsflowcoef, 1.0) {
                *s += 100.0;
            }

            // There is at least one capacity variable with coefficient not
            // equal to +/-1: score +100.
            if maxabscapacitycoef > 0.0 && !scip.set.is_eq(maxabscapacitycoef, 1.0) {
                *s += 100.0;
            }

            // Flow coefficients are mostly of the same sign: score
            // +20*max(npos,nneg)/(npos+nneg).
            *s += 20.0 * max(nposflowcoefs, nnegflowcoefs) as f64 / max(1.0, (nposflowcoefs + nnegflowcoefs) as f64);

            // Capacity coefficients are mostly of the same sign: score
            // +10*max(npos,nneg)/(npos+nneg+1).
            *s += 10.0 * max(nposcapacitycoefs, nnegcapacitycoefs) as f64 / ((nposcapacitycoefs + nnegcapacitycoefs) as f64 + 1.0);

            // Row is a <= row with non-negative right hand side: score +10.
            if (mcfdata.capacityrowsigns[r] & RHSPOSSIBLE) != 0 && !scip.set.is_negative(rowrhs) {
                mcfdata.capacityrowscores[r] += 10.0;
            }

            // Row is an inequality: score +10.
            if scip.set.is_infinity(-rowlhs) != scip.set.is_infinity(rowrhs) {
                mcfdata.capacityrowscores[r] += 10.0;
            }

            debug_assert!(mcfdata.capacityrowscores[r] > 0.0);

            // Update maximum dual solution value for additional score tie
            // breaking.
            maxdualcapacity = max(maxdualcapacity, absdualsol);

            // If the model type should be detected automatically, count the
            // number of directed and undirected capacity candidates.
            if modeltype == MCFMODELTYPE_AUTO {
                debug_assert!(maxcolspercommoditylimit == 2);
                if (mcfdata.capacityrowsigns[r] & UNDIRECTED) != 0 {
                    undirectedcandsscore += mcfdata.capacityrowscores[r];
                } else {
                    directedcandsscore += mcfdata.capacityrowscores[r];
                }
            }
        }
    }

    // If the model type should be detected automatically, decide it by a
    // majority vote.
    if modeltype == MCFMODELTYPE_AUTO {
        modeltype = if directedcandsscore > undirectedcandsscore {
            MCFMODELTYPE_DIRECTED
        } else {
            MCFMODELTYPE_UNDIRECTED
        };

        if modeltype == MCFMODELTYPE_DIRECTED {
            // Discard all undirected arcs.
            for i in 0..mcfdata.ncapacitycands as usize {
                let r = mcfdata.capacitycands[i] as usize;
                if (mcfdata.capacityrowsigns[r] & UNDIRECTED) != 0 {
                    // Reduce the score of the undirected row in the directed
                    // model.
                    if maxcolspercommodity[r] <= maxcolspercommoditylimit {
                        mcfdata.capacityrowscores[r] -= 1000.0;
                    }
                }
            }
        }

        // Record the detected model type.
        mcfdata.modeltype = modeltype;
    }

    // Apply additional score tie breaking using the dual solutions.
    if scip.set.is_positive(maxdualcapacity) {
        for i in 0..mcfdata.ncapacitycands as usize {
            let r = mcfdata.capacitycands[i] as usize;
            let mut dualsol = row_dualsol(scip, rows[r]);
            if dualsol == INVALID {
                dualsol = 0.0;
            } else if mcfdata.capacityrowsigns[r] == (LHSPOSSIBLE | RHSPOSSIBLE) {
                dualsol = dualsol.abs();
            } else if mcfdata.capacityrowsigns[r] == RHSPOSSIBLE {
                dualsol = -dualsol;
            }
            mcfdata.capacityrowscores[r] += max(dualsol, 0.0) / maxdualcapacity;
            debug_assert!(mcfdata.capacityrowscores[r] > 0.0);
        }
    }

    // Sort candidates by score.
    let n = mcfdata.ncapacitycands as usize;
    let scores = &mcfdata.capacityrowscores;
    crate::sort::sort(&mut mcfdata.capacitycands[..n], (), |&a, &b| comp_cands(scores, a, b));

    Ok(())
}

/// `createNewCommodity`: creates a new commodity.
fn create_new_commodity(mcfdata: &mut McfData) {
    // Get memory for commoditysigns array.
    if mcfdata.ncommodities == mcfdata.commoditysignssize {
        mcfdata.commoditysignssize = max(2 * mcfdata.commoditysignssize, mcfdata.ncommodities + 1);
        mcfdata.commoditysigns.resize(mcfdata.commoditysignssize as usize, 0);
    }

    // Create commodity.
    mcfdata.commoditysigns[mcfdata.ncommodities as usize] = 0;
    mcfdata.ncommodities += 1;
}

/// `createNewArc`: creates a new arc and returns its id.
fn create_new_arc(mcfdata: &mut McfData, source: i32, target: i32) -> i32 {
    let newarcid = mcfdata.narcs;

    // Get memory for arrays indexed by arcs.
    if mcfdata.narcs == mcfdata.arcarraysize {
        mcfdata.arcarraysize = max(2 * mcfdata.arcarraysize, mcfdata.narcs + 1);
        let n = mcfdata.arcarraysize as usize;
        mcfdata.arcsources.resize(n, 0);
        mcfdata.arctargets.resize(n, 0);
        mcfdata.nextinarcs.resize(n, 0);
        mcfdata.nextoutarcs.resize(n, 0);
    }

    // capacityrows is a special case since it is used earlier.
    if mcfdata.capacityrowssize < mcfdata.arcarraysize {
        mcfdata.capacityrowssize = mcfdata.arcarraysize;
        mcfdata.capacityrows.resize(mcfdata.capacityrowssize as usize, NONE);
    }

    // Create new arc.
    let a = newarcid as usize;
    mcfdata.arcsources[a] = source;
    mcfdata.arctargets[a] = target;
    mcfdata.nextoutarcs[a] = mcfdata.firstoutarcs[source as usize];
    mcfdata.firstoutarcs[source as usize] = newarcid;
    mcfdata.nextinarcs[a] = mcfdata.firstinarcs[target as usize];
    mcfdata.firstinarcs[target as usize] = newarcid;
    mcfdata.capacityrows[a] = NONE;

    mcfdata.narcs += 1;

    newarcid
}

/// `addFlowrowToCommodity`: adds the given flow row and all involved columns
/// to the current commodity.
fn add_flowrow_to_commodity(scip: &Scip, mcfdata: &mut McfData, row: RowId, mut rowsign: u8, comcolids: &mut [i32], ncomcolids: &mut i32) {
    let k = mcfdata.ncommodities - 1;
    debug_assert!(k >= 0);

    let r = scip.rows[row].lppos as usize;

    // Check if row has to be inverted.
    let invertrow = (rowsign & INVERTED) != 0;
    rowsign &= !INVERTED;

    debug_assert!(mcfdata.rowcommodity[r] == -1);
    debug_assert!((mcfdata.flowrowsigns[r] | rowsign) == mcfdata.flowrowsigns[r]);
    debug_assert!((rowsign & (LHSPOSSIBLE | RHSPOSSIBLE)) == rowsign);
    debug_assert!(rowsign != 0);

    // If the row is only usable as flow row in one direction, we cannot
    // change the sign of the whole commodity anymore.
    if (mcfdata.flowrowsigns[r] & (LHSPOSSIBLE | RHSPOSSIBLE)) != (LHSPOSSIBLE | RHSPOSSIBLE) {
        mcfdata.commoditysigns[k as usize] = 1; // we cannot switch directions
    }

    // Decide the sign (direction) of the row.
    if rowsign == LHSPOSSIBLE {
        rowsign = LHSASSIGNED;
    } else if rowsign == RHSPOSSIBLE {
        rowsign = RHSASSIGNED;
    } else {
        let dualsol = row_dualsol(scip, row);

        debug_assert!(rowsign == (LHSPOSSIBLE | RHSPOSSIBLE));

        // If we have a valid non-zero dual solution, choose the side which
        // is tight.
        if !scip.set.is_zero(dualsol) && dualsol != INVALID {
            rowsign = if dualsol > 0.0 { LHSASSIGNED } else { RHSASSIGNED };
        } else {
            let rw = &scip.rows[row];
            let rowlhs = rw.lhs - rw.constant;
            let rowrhs = rw.rhs - rw.constant;

            // Choose row sign such that we get a*x <= -d with d
            // non-negative.
            rowsign = if rowrhs < 0.0 {
                RHSASSIGNED
            } else if rowlhs > 0.0 {
                LHSASSIGNED
            } else {
                RHSASSIGNED // if we are still undecided, choose rhs
            };
        }
    }
    let mut rowscale: i32 = if rowsign == RHSASSIGNED { 1 } else { -1 };

    // Reintroduce inverted flag.
    if invertrow {
        rowsign |= INVERTED;
        rowscale *= -1;
    }
    mcfdata.flowrowsigns[r] |= rowsign;

    // Add row to commodity.
    mcfdata.rowcommodity[r] = k;
    let rw = &scip.rows[row];
    let rowlen = rw.nlpcols as usize;
    for i in 0..rowlen {
        let c = scip.cols[rw.cols[i]].lppos as usize;

        // Assign column to commodity.
        if mcfdata.colcommodity[c] == -1 {
            mcfdata.colcommodity[c] = k;
            mcfdata.newcols[mcfdata.nnewcols as usize] = c as i32;
            mcfdata.nnewcols += 1;
            comcolids[*ncomcolids as usize] = c as i32;
            *ncomcolids += 1;
        }

        // Update plusflow/minusflow.
        let val = rowscale as f64 * rw.vals[i];
        if val > 0.0 {
            debug_assert!(!mcfdata.plusflow[c]);
            mcfdata.plusflow[c] = true;
        } else {
            debug_assert!(!mcfdata.minusflow[c]);
            mcfdata.minusflow[c] = true;
        }
    }
}

/// `invertCommodity`: inverts the lhs/rhs assignment of all rows in the
/// given commodity.
fn invert_commodity(scip: &Scip, mcfdata: &mut McfData, comrows: &[RowId], ncomrows: i32, comcolids: &[i32], ncomcolids: i32) {
    // Switch assignments of rows.
    for i in 0..ncomrows as usize {
        let r = scip.rows[comrows[i]].lppos as usize;
        let rowsign = mcfdata.flowrowsigns[r];

        mcfdata.flowrowsigns[r] &= !(LHSASSIGNED | RHSASSIGNED);
        if (rowsign & LHSASSIGNED) != 0 {
            mcfdata.flowrowsigns[r] |= RHSASSIGNED;
        } else {
            mcfdata.flowrowsigns[r] |= LHSASSIGNED;
        }
    }

    // Switch plus/minusflow of columns of the given commodity.
    for i in 0..ncomcolids as usize {
        let c = comcolids[i] as usize;
        std::mem::swap(&mut mcfdata.plusflow[c], &mut mcfdata.minusflow[c]);
    }
}

/// `deleteCommodity`: deletes a commodity and removes the flow rows again
/// from the system; returns the numbers of flow rows and flow vars in the
/// deleted commodity.
fn delete_commodity(scip: &Scip, mcfdata: &mut McfData, k: i32, comrows: &[RowId], nrows: i32) -> (i32, i32) {
    let ncommodities = mcfdata.ncommodities;
    let mut ndelflowrows = 0;
    let mut ndelflowvars = 0;

    for n in 0..nrows as usize {
        let row = comrows[n];
        let r = scip.rows[row].lppos as usize;

        // Remove the lhs/rhs assignment and the inverted flag.
        mcfdata.flowrowsigns[r] &= !(LHSASSIGNED | RHSASSIGNED | INVERTED);

        // Remove row from commodity.
        mcfdata.rowcommodity[r] = -1;
        let rowlen = scip.rows[row].nlpcols as usize;
        for i in 0..rowlen {
            let c = row_col_pos(scip, row, i) as usize;

            // Remove column from commodity.
            if mcfdata.colcommodity[c] == k {
                ndelflowvars += 1;
            }
            mcfdata.colcommodity[c] = -1;

            // Reset plusflow/minusflow.
            mcfdata.plusflow[c] = false;
            mcfdata.minusflow[c] = false;
        }

        ndelflowrows += 1;
    }

    // Get rid of commodity if it is the last one; otherwise, just leave it
    // as an empty commodity which will be discarded later.
    if k == ncommodities - 1 {
        mcfdata.ncommodities -= 1;
    } else {
        mcfdata.nemptycommodities += 1;
    }

    (ndelflowrows, ndelflowvars)
}

/// `getFlowrowFit`: checks whether the given row fits into the given
/// commodity and returns the possible flow row signs and whether the
/// commodity has to be inverted to accommodate the row.
fn get_flowrow_fit(scip: &Scip, mcfdata: &mut McfData, row: RowId, k: i32) -> (u8, bool) {
    let r = scip.rows[row].lppos as usize;

    // Ignore rows that are already used.
    if mcfdata.rowcommodity[r] != -1 {
        return (0, false);
    }

    // Check if row is an available flow row.
    let mut flowrowsign = mcfdata.flowrowsigns[r];
    if (flowrowsign & DISCARDED) != 0 {
        return (0, false);
    }
    if (flowrowsign & (LHSPOSSIBLE | RHSPOSSIBLE)) == 0 {
        return (0, false);
    }
    let mut invflowrowsign = flowrowsign;

    // Check whether the row fits w.r.t. the signs of the coefficients.
    let rw = &scip.rows[row];
    let rowlen = rw.nlpcols as usize;
    let mut j = 0;
    while j < rowlen && (flowrowsign != 0 || invflowrowsign != 0) {
        let rowc = scip.cols[rw.cols[j]].lppos as usize;

        // Check if column already belongs to the same commodity.
        if mcfdata.colcommodity[rowc] == k {
            // Column only fits if it is not yet present with the same sign.
            if mcfdata.plusflow[rowc] {
                // Column must not be included with positive sign.
                if rw.vals[j] > 0.0 {
                    flowrowsign &= !RHSPOSSIBLE;
                    invflowrowsign &= !LHSPOSSIBLE;
                } else {
                    flowrowsign &= !LHSPOSSIBLE;
                    invflowrowsign &= !RHSPOSSIBLE;
                }
            }
            if mcfdata.minusflow[rowc] {
                // Column must not be included with negative sign.
                if rw.vals[j] > 0.0 {
                    flowrowsign &= !LHSPOSSIBLE;
                    invflowrowsign &= !RHSPOSSIBLE;
                } else {
                    flowrowsign &= !RHSPOSSIBLE;
                    invflowrowsign &= !LHSPOSSIBLE;
                }
            }
        } else if mcfdata.colcommodity[rowc] != -1 {
            // Column does not fit if it already belongs to a different
            // commodity.
            flowrowsign = 0;
            invflowrowsign = 0;
        }
        j += 1;
    }

    if flowrowsign != 0 {
        // Flow row fits without inverting anything.
        (flowrowsign, false)
    } else if invflowrowsign != 0 {
        // This must be an inequality.
        debug_assert!((mcfdata.flowrowsigns[r] & (LHSPOSSIBLE | RHSPOSSIBLE)) != (LHSPOSSIBLE | RHSPOSSIBLE));

        // Flow row fits only if row or commodity is inverted.
        if mcfdata.commoditysigns.is_empty() || mcfdata.commoditysigns[k as usize] == 0 {
            // Commodity can be inverted.
            (invflowrowsign, true)
        } else {
            // Row has to be inverted.
            (invflowrowsign | INVERTED, false)
        }
    } else {
        // We can discard the row, since it can also not be member of a
        // different commodity.
        mcfdata.flowrowsigns[r] |= DISCARDED;
        (0, false)
    }
}

/// `getNextFlowrow`: returns a flow conservation row that fits into the
/// current commodity (NONE if there is none), its possible signs and whether
/// the current commodity has to be inverted to accommodate it.
fn get_next_flowrow(scip: &Scip, mcfdata: &mut McfData) -> (RowId, u8, bool) {
    let k = mcfdata.ncommodities - 1;

    let cols = api::get_lp_cols(scip);

    // Check if there are any columns left in the commodity that have not yet
    // been inspected for incident flow rows.
    while mcfdata.nnewcols > 0 {
        // Pop next new column from stack.
        let c = mcfdata.newcols[mcfdata.nnewcols as usize - 1] as usize;
        mcfdata.nnewcols -= 1;

        // Check if this columns already as both signs.
        if mcfdata.plusflow[c] && mcfdata.minusflow[c] {
            continue;
        }

        // Check whether column is incident to a valid flow row that fits
        // into the current commodity.
        let mut bestrow = NONE;
        let mut bestrowsign = 0u8;
        let mut bestinvertcommodity = false;
        let mut bestscore = 0.0;
        let col = cols[c];
        let collen = scip.cols[col].nlprows as usize;
        for i in 0..collen {
            let row = scip.cols[col].rows[i];

            // Check if row fits into the current commodity.
            let (flowrowsign, invertcommodity) = get_flowrow_fit(scip, mcfdata, row, k);

            // Do we have a winner?
            if flowrowsign != 0 {
                let r = scip.rows[row].lppos as usize;
                let mut score = mcfdata.flowrowscores[r];

                // If we have to invert the row, this will lead to a negative
                // slack variable in the MIR cut, which needs to be
                // substituted in the end. We like to avoid this and therefore
                // reduce the score.
                if (flowrowsign & INVERTED) != 0 {
                    score *= 0.75;
                }

                if score > bestscore {
                    bestrow = row;
                    bestrowsign = flowrowsign;
                    bestinvertcommodity = invertcommodity;
                    bestscore = score;
                }
            }
        }

        // If there was a valid row for this column, pick the best one.
        if bestrow != NONE {
            return (bestrow, bestrowsign, bestinvertcommodity);
        }
    }
    (NONE, 0, false)
}

/// `extractFlow`: extracts flow conservation rows and puts them into
/// commodities; returns whether the flowvar/flowrow ratio was exceeded.
fn extract_flow(scip: &mut Scip, mcfdata: &mut McfData, maxflowvarflowrowratio: f64, failed: &mut bool) -> ScipResult {
    // Get LP data.
    let rows = lp_rows(scip);
    let nrows = rows.len();
    let ncols = n_lp_cols(scip) as usize;

    // Allocate memory.
    mcfdata.plusflow = vec![false; ncols];
    mcfdata.minusflow = vec![false; ncols];
    mcfdata.colcommodity = vec![-1; ncols];
    mcfdata.rowcommodity = vec![-1; nrows];
    mcfdata.newcols = vec![0; ncols];

    // Allocate temporary memory.
    let mut comrows = vec![NONE; nrows];
    let mut ncomnodes = vec![0i32; nrows];
    let mut comcolids = vec![0i32; ncols];

    // (b) As long as there are flow conservation candidates left:
    //  (i) Create new commodity and use first flow conservation constraint
    //      as new row.
    // (ii) Add new row to commodity, update pluscom/minuscom accordingly.
    // (iii) For the newly added columns search for an incident flow
    //      conservation constraint. Pick the one of highest ranking.
    // (iv) If found, set new row to this row and goto (ii).
    let mut maxnnodes = 0;
    let mut nflowrows = 0;
    let mut nflowvars = 0;
    for i in 0..mcfdata.nflowcands as usize {
        let r = mcfdata.flowcands[i] as usize;
        let mut newrow = rows[r];

        // Check if row fits into a new commodity.
        let (mut newrowsign, mut newinvertcommodity) = get_flowrow_fit(scip, mcfdata, newrow, mcfdata.ncommodities);
        if newrowsign == 0 {
            continue;
        }
        debug_assert!(!newinvertcommodity);
        debug_assert!((newrowsign & INVERTED) == 0);

        // Start new commodity.
        create_new_commodity(mcfdata);
        let mut nnodes = 0;
        let mut ncomcolids = 0;

        // Fill commodity with flow conservation constraints.
        loop {
            // If next flow row demands an inverting of the commodity, do it
            // now.
            if newinvertcommodity {
                invert_commodity(scip, mcfdata, &comrows, nnodes, &comcolids, ncomcolids);
            }

            // Add new row to commodity.
            add_flowrow_to_commodity(scip, mcfdata, newrow, newrowsign, &mut comcolids, &mut ncomcolids);
            comrows[nnodes as usize] = newrow;
            nnodes += 1;
            nflowrows += 1;

            // Get next row to add.
            (newrow, newrowsign, newinvertcommodity) = get_next_flowrow(scip, mcfdata);
            if newrow == NONE {
                break;
            }
        }

        ncomnodes[mcfdata.ncommodities as usize - 1] = nnodes;
        maxnnodes = max(maxnnodes, nnodes);
        nflowvars += ncomcolids;

        // If the commodity has too few nodes, or if it has much fewer nodes
        // than the largest commodity, discard it.
        if nnodes < MINNODES || (nnodes as f64) < MINCOMNODESFRACTION * maxnnodes as f64 {
            let (ndelflowrows, ndelflowvars) = delete_commodity(scip, mcfdata, mcfdata.ncommodities - 1, &comrows, nnodes);
            nflowrows -= ndelflowrows;
            nflowvars -= ndelflowvars;
        }
    }

    // Final cleanup of small commodities.
    let mut k = 0;
    while k < mcfdata.ncommodities {
        // If the commodity has much fewer nodes than the largest commodity,
        // discard it.
        if (ncomnodes[k as usize] as f64) < MINCOMNODESFRACTION * maxnnodes as f64 {
            let mut nnodes = 0;
            for i in 0..mcfdata.nflowcands as usize {
                let r = mcfdata.flowcands[i] as usize;
                if mcfdata.rowcommodity[r] == k {
                    comrows[nnodes as usize] = rows[r];
                    nnodes += 1;
                    if nnodes == ncomnodes[k as usize] {
                        break;
                    }
                }
            }
            let (ndelflowrows, ndelflowvars) = delete_commodity(scip, mcfdata, k, &comrows, nnodes);
            nflowrows -= ndelflowrows;
            nflowvars -= ndelflowvars;
        }
        k += 1;
    }

    // Do not allow flow system exceeding the flowvarflowrowratio (average
    // node degree).
    if nflowrows == 0 || nflowvars as f64 / nflowrows as f64 > maxflowvarflowrowratio {
        *failed = true;
    }

    Ok(())
}

/// `extractCapacities`: Arc-Detection -- identifies capacity constraints for
/// the arcs and assigns arc ids to columns and capacity constraints.
fn extract_capacities(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    // Get LP data.
    let rows = lp_rows(scip);
    let nrows = rows.len();
    let ncols = n_lp_cols(scip) as usize;

    // Allocate temporary memory for extraction data and initialize arcid
    // arrays.
    mcfdata.colarcid = vec![-1; ncols];
    mcfdata.rowarcid = vec![-1; nrows];

    // Loop through the list of capacity cands in non-increasing score order.
    for i in 0..mcfdata.ncapacitycands as usize {
        let r = mcfdata.capacitycands[i] as usize;
        let capacityrow = rows[r];

        debug_assert!((mcfdata.capacityrowsigns[r] & (LHSPOSSIBLE | RHSPOSSIBLE)) != 0);
        debug_assert!((mcfdata.capacityrowsigns[r] & (LHSASSIGNED | RHSASSIGNED)) == 0);
        debug_assert!(mcfdata.rowarcid[r] == -1);
        debug_assert!(mcfdata.rowcommodity[r] == -1);

        // Count the number of already assigned and not yet assigned flow
        // variables.
        let rowlen = scip.rows[capacityrow].nlpcols as usize;
        let mut nassignedflowvars = 0;
        let mut nunassignedflowvars = 0;
        for k in 0..rowlen {
            let c = row_col_pos(scip, capacityrow, k) as usize;

            // Ignore columns that are not flow variables.
            if mcfdata.colcommodity[c] == -1 {
                continue;
            }

            // Check if column is already assigned to an arc.
            if mcfdata.colarcid[c] >= 0 {
                nassignedflowvars += 1;
            } else {
                nunassignedflowvars += 1;
            }
        }

        // Ignore row if all of its flow variables have already been assigned
        // to some other arc. Only accept the row as capacity constraint if
        // at least 1/3 of its flow vars are not yet assigned to some other
        // arc.
        if nunassignedflowvars == 0 || nassignedflowvars >= nunassignedflowvars * 2 {
            mcfdata.capacityrowsigns[r] |= DISCARDED;
            continue;
        }

        // Create new arc -- store capacity row.
        if mcfdata.narcs == mcfdata.capacityrowssize {
            mcfdata.capacityrowssize = max(2 * mcfdata.capacityrowssize, mcfdata.narcs + 1);
            mcfdata.capacityrows.resize(mcfdata.capacityrowssize as usize, NONE);
        }

        mcfdata.capacityrows[mcfdata.narcs as usize] = capacityrow;

        // Assign the capacity row to a new arc id.
        mcfdata.rowarcid[r] = mcfdata.narcs;

        // Decide which sign to use.
        if (mcfdata.capacityrowsigns[r] & RHSPOSSIBLE) != 0 {
            mcfdata.capacityrowsigns[r] |= RHSASSIGNED;
        } else {
            debug_assert!((mcfdata.capacityrowsigns[r] & LHSPOSSIBLE) != 0);
            mcfdata.capacityrowsigns[r] |= LHSASSIGNED;
        }

        // Assign all involved flow variables to the new arc id.
        for k in 0..rowlen {
            let rowc = row_col_pos(scip, capacityrow, k) as usize;

            // Due to aggregations in preprocessing it may happen that a flow
            // variable appears in multiple capacity constraints; in this
            // case, assign it to the first that has been found.
            if mcfdata.colcommodity[rowc] >= 0 && mcfdata.colarcid[rowc] == -1 {
                mcfdata.colarcid[rowc] = mcfdata.narcs;
            }
        }

        // Increase number of arcs.
        mcfdata.narcs += 1;
    }
    Ok(())
}

/// `collectIncidentFlowCols`: collects all flow columns of all commodities
/// (except the one of the base row) that are incident to the node described
/// by the given flow row.
fn collect_incident_flow_cols(scip: &Scip, mcfdata: &mut McfData, flowrow: RowId, basecommodity: i32) {
    // Loop through all flow columns in the flow conservation constraint.
    let rowlen = scip.rows[flowrow].nlpcols as usize;
    mcfdata.nnewcols = 0;

    for i in 0..rowlen {
        let c = row_col_pos(scip, flowrow, i) as usize;

        // Get arc id of the column in the flow conservation constraint.
        let arcid = mcfdata.colarcid[c];
        if arcid == -1 {
            continue;
        }

        // Collect flow variables in the capacity constraint of this arc.
        let caprow = mcfdata.capacityrows[arcid as usize];
        let capacityrowlen = scip.rows[caprow].nlpcols as usize;

        for j in 0..capacityrowlen {
            let caprowc = row_col_pos(scip, caprow, j) as usize;

            // Ignore columns that do not belong to a commodity, i.e., are
            // not flow variables.
            if mcfdata.colcommodity[caprowc] == -1 {
                debug_assert!(!mcfdata.colisincident[caprowc]);
                continue;
            }

            // Ignore columns in the same commodity as the base row.
            if mcfdata.colcommodity[caprowc] == basecommodity {
                continue;
            }

            // If not already done, collect the column.
            if !mcfdata.colisincident[caprowc] {
                mcfdata.colisincident[caprowc] = true;
                mcfdata.newcols[mcfdata.nnewcols as usize] = caprowc as i32;
                mcfdata.nnewcols += 1;
            }
        }
    }
}

/// `getNodeSimilarityScore`: compares given row against a base node flow row
/// and calculates a similarity score (0.0 if the rows are incompatible);
/// also returns whether the arcs in the commodity of the row have to be
/// inverted for the row to be compatible to the base row.
#[allow(clippy::too_many_arguments, unused_assignments)]
fn get_node_similarity_score(scip: &Scip, mcfdata: &mut McfData, baserowlen: i32, basearcpattern: &[i32], basenposuncap: i32, basenneguncap: i32, row: RowId) -> (f64, bool) {
    let modeltype = mcfdata.modeltype;
    let narcs = mcfdata.narcs as usize;

    let mut score = 0.0;
    let mut invertcommodity = false;

    // Allocate temporary memory.
    let mut overlappingarcs = vec![0i32; narcs];

    let r = scip.rows[row].lppos as usize;
    let rowcom = mcfdata.rowcommodity[r];
    debug_assert!(0 <= rowcom && rowcom < mcfdata.ncommodities);
    let mut rowcomsign = mcfdata.commoditysigns[rowcom as usize];

    let rw = &scip.rows[row];
    let rowlen = rw.nlpcols;
    let mut incompatible = false;
    let mut noverlappingarcs = 0usize;
    let mut nposuncap = 0i32;
    let mut nneguncap = 0i32;
    let ncols = n_lp_cols(scip);
    let arcpattern = &mut mcfdata.zeroarcarray;
    for i in 0..rowlen as usize {
        let c = scip.cols[rw.cols[i]].lppos as usize;

        // Get the sign of the coefficient in the flow conservation
        // constraint.
        let mut valsign: i32 = if rw.vals[i] > 0.0 { 1 } else { -1 };
        if (mcfdata.flowrowsigns[r] & LHSASSIGNED) != 0 {
            valsign *= -1;
        }
        if (mcfdata.flowrowsigns[r] & INVERTED) != 0 {
            valsign *= -1;
        }

        let arcid = mcfdata.colarcid[c];
        if arcid == -1 {
            if valsign > 0 {
                nposuncap += 1;
            } else {
                nneguncap += 1;
            }
            continue;
        }
        let arcid = arcid as usize;

        // Check if this arc is also member of the base row.
        if basearcpattern[arcid] != 0 {
            // Check if the sign of the arc matches in the directed case.
            if modeltype == MCFMODELTYPE_DIRECTED {
                let validcomsign = if valsign * basearcpattern[arcid] > 0 { 1 } else { -1 };

                if rowcomsign == 0 {
                    // The first entry decides whether we have to invert the
                    // commodity.
                    rowcomsign = validcomsign;
                } else if rowcomsign != validcomsign {
                    // The signs do not fit: this is incompatible.
                    incompatible = true;
                    break;
                }
            } else {
                // In the undirected case, we ignore the sign of the
                // coefficient.
                valsign = 1;
            }

            // Store overlapping arc pattern.
            if arcpattern[arcid] == 0 {
                overlappingarcs[noverlappingarcs] = arcid as i32;
                noverlappingarcs += 1;
            }
            arcpattern[arcid] += valsign;
        }
    }

    // Calculate the weighted overlap and reset the zeroarcarray.
    let mut overlap = 0.0;
    for &arcid in &overlappingarcs[..noverlappingarcs] {
        let arcid = arcid as usize;
        debug_assert!(basearcpattern[arcid] != 0);
        debug_assert!(arcpattern[arcid] != 0);

        let basenum = basearcpattern[arcid].abs() as f64;
        let arcnum = arcpattern[arcid].abs() as f64;

        if basenum > arcnum {
            overlap += arcnum / basenum;
        } else {
            overlap += basenum / arcnum;
        }

        arcpattern[arcid] = 0;
    }

    // Calculate the score: maximize overlap and use minimal number of
    // non-overlapping entries as tie breaker.
    if !incompatible && overlap > 0.0 {
        // Flow variables with arc-id.
        let rowarcs = rowlen - nposuncap - nneguncap;
        let baserowarcs = baserowlen - basenposuncap - basenneguncap;

        debug_assert!(overlap <= rowlen as f64);
        debug_assert!(overlap <= baserowlen as f64);
        debug_assert!(noverlappingarcs >= 1);

        invertcommodity = rowcomsign == -1;

        // Only one overlapping arc is very dangerous, since this can also be
        // the other end node of the arc.
        if noverlappingarcs >= 2 {
            score += 1000.0;
        }

        // In the ideal undirected case there are two flow variables with the
        // same arc-id.
        if modeltype == MCFMODELTYPE_DIRECTED {
            score = overlap - ((rowarcs + baserowarcs) as f64 - 2.0 * overlap) / (2.0 * ncols as f64 + 1.0);
        } else {
            score = overlap - ((rowarcs + baserowarcs) as f64 - 4.0 * overlap) / (2.0 * ncols as f64 + 1.0);
        }

        // Also use number of uncapacitated flowvars (variables without
        // arcid) as tie-breaker.
        if invertcommodity {
            score += 1.0 - ((nneguncap - basenposuncap).abs() + (nposuncap - basenneguncap).abs()) as f64 / (2.0 * ncols as f64 + 1.0);
        } else {
            score += 1.0 - ((nposuncap - basenposuncap).abs() + (nneguncap - basenneguncap).abs()) as f64 / (2.0 * ncols as f64 + 1.0);
        }

        score = max(score, 1e-6); // score may get negative due to many columns having the same arcid
    }

    (score, invertcommodity)
}

/// `extractNodes`: assigns node ids to flow conservation constraints.
fn extract_nodes(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    let ncommodities = mcfdata.ncommodities as usize;
    let narcs = mcfdata.narcs as usize;
    let modeltype = mcfdata.modeltype;

    // Get LP data.
    let rows = lp_rows(scip);
    let cols = api::get_lp_cols(scip);
    let nrows = rows.len();
    let ncols = cols.len();

    // Allocate temporary memory.
    mcfdata.rownodeid = vec![-1; nrows];
    mcfdata.colisincident = vec![false; ncols];
    mcfdata.zeroarcarray = vec![0; narcs];

    // Allocate temporary local memory.
    let mut arcpattern = vec![0i32; narcs];
    let mut bestflowrows = vec![NONE; ncommodities];
    let mut bestscores = vec![0.0f64; ncommodities];
    let mut bestinverted = vec![false; ncommodities];
    let mut rowprocessed = vec![false; nrows];

    // Process all flow conservation constraints that have been used.
    for n in 0..mcfdata.nflowcands as usize {
        let r = mcfdata.flowcands[n] as usize;

        // Ignore rows that are not used as flow conservation constraint.
        let basecommodity = mcfdata.rowcommodity[r];
        if basecommodity == -1 {
            continue;
        }

        // Skip rows that are already assigned to a node.
        if mcfdata.rownodeid[r] >= 0 {
            continue;
        }

        // Assign row to new node id.
        mcfdata.rownodeid[r] = mcfdata.nnodes;

        // Increase number of nodes.
        mcfdata.nnodes += 1;

        // For single commodity models we are done -- no matching flow rows
        // need to be found.
        if ncommodities == 1 {
            continue;
        }

        // Get the arc pattern of the flow row.
        arcpattern.fill(0);
        let mut nposuncap = 0;
        let mut nneguncap = 0;

        let rw = &scip.rows[rows[r]];
        let rowlen = rw.nlpcols;

        let mut rowscale: i32 = if (mcfdata.flowrowsigns[r] & RHSASSIGNED) != 0 { 1 } else { -1 };
        if (mcfdata.flowrowsigns[r] & INVERTED) != 0 {
            rowscale *= -1;
        }
        if mcfdata.commoditysigns[basecommodity as usize] == -1 {
            rowscale *= -1;
        }

        for i in 0..rowlen as usize {
            let c = scip.cols[rw.cols[i]].lppos as usize;
            let arcid = mcfdata.colarcid[c];
            if arcid >= 0 {
                // Due to presolving we may have multiple flow variables of
                // the same arc in the row.
                if modeltype == MCFMODELTYPE_UNDIRECTED || rowscale as f64 * rw.vals[i] > 0.0 {
                    arcpattern[arcid as usize] += 1;
                } else {
                    arcpattern[arcid as usize] -= 1;
                }
            }
            // We also count variables that have no arc -- these have no
            // capacity constraint --> uncapacitated.
            else if modeltype == MCFMODELTYPE_UNDIRECTED || rowscale as f64 * rw.vals[i] > 0.0 {
                nposuncap += 1;
            } else {
                nneguncap += 1;
            }
        }

        // Initialize arrays to store best flow rows.
        for i in 0..ncommodities {
            bestflowrows[i] = NONE;
            bestscores[i] = 0.0;
            bestinverted[i] = false;
        }

        // Collect columns that are member of incident arc capacity
        // constraints.
        collect_incident_flow_cols(scip, mcfdata, rows[r], basecommodity);

        // Initialize rowprocessed array.
        rowprocessed.fill(false);

        // Identify flow conservation constraints in other commodities that
        // match this node; search for flow rows in the column vectors of the
        // incident columns.
        for i in 0..mcfdata.nnewcols as usize {
            let c = mcfdata.newcols[i] as usize;
            debug_assert!(mcfdata.colisincident[c]);

            // Clean up the marker array.
            mcfdata.colisincident[c] = false;

            // Scan column vector for flow conservation constraints.
            let collen = scip.cols[cols[c]].nlprows as usize;

            for j in 0..collen {
                let colrow = scip.cols[cols[c]].rows[j];
                let colr = scip.rows[colrow].lppos as usize;

                // Ignore rows that have already been processed.
                if rowprocessed[colr] {
                    continue;
                }
                rowprocessed[colr] = true;

                // Ignore rows that are not flow conservation constraints in
                // the network.
                let rowcom = mcfdata.rowcommodity[colr];
                if rowcom == -1 {
                    continue;
                }

                // Ignore rows that are already assigned to a node.
                if mcfdata.rownodeid[colr] >= 0 {
                    continue;
                }

                // Compare row against arc pattern and calculate score.
                let (score, invertcommodity) = get_node_similarity_score(scip, mcfdata, rowlen, &arcpattern, nposuncap, nneguncap, colrow);

                let rowcom = rowcom as usize;
                if score > bestscores[rowcom] {
                    bestflowrows[rowcom] = colrow;
                    bestscores[rowcom] = score;
                    bestinverted[rowcom] = invertcommodity;
                }
            }
        }

        // For each commodity, pick the best flow conservation constraint to
        // define this node.
        for i in 0..ncommodities {
            if bestflowrows[i] == NONE {
                continue;
            }

            let comr = scip.rows[bestflowrows[i]].lppos as usize;

            // Assign flow row to current node.
            mcfdata.rownodeid[comr] = mcfdata.nnodes - 1;

            // Fix the direction of the arcs of the commodity.
            mcfdata.commoditysigns[i] = if bestinverted[i] { -1 } else { 1 };
        }
    }

    Ok(())
}

/// `fixCommoditySigns`: if there are still undecided commodity signs, fix
/// them to +1.
fn fix_commodity_signs(mcfdata: &mut McfData) {
    for k in 0..mcfdata.ncommodities as usize {
        if mcfdata.commoditysigns[k] == 0 {
            mcfdata.commoditysigns[k] = 1;
        }
    }
}

/// `getIncidentNodes`: identifies the (at most) two nodes which contain the
/// given flow variable; returns (sourcenode, targetnode).
fn get_incident_nodes(scip: &Scip, mcfdata: &mut McfData, col: ColId) -> (i32, i32) {
    let c = scip.cols[col].lppos as usize;

    // Check if we have this column already in cache.
    if mcfdata.colsources[c] >= -1 {
        return (mcfdata.colsources[c], mcfdata.coltargets[c]);
    }

    let mut sourcenode = -1;
    let mut targetnode = -1;

    // Search for flow conservation rows in the column vector.
    let cl = &scip.cols[col];
    let collen = cl.nlprows as usize;
    for i in 0..collen {
        let r = scip.rows[cl.rows[i]].lppos as usize;

        if mcfdata.rownodeid[r] >= 0 {
            let v = mcfdata.rownodeid[r];
            let k = mcfdata.rowcommodity[r];

            // Check whether the flow row is inverted.
            let mut scale: i32 = 1;
            if (mcfdata.flowrowsigns[r] & LHSASSIGNED) != 0 {
                scale *= -1;
            }
            if (mcfdata.flowrowsigns[r] & INVERTED) != 0 {
                scale *= -1;
            }
            if mcfdata.commoditysigns[k as usize] == -1 {
                scale *= -1;
            }

            // Decide whether this node is source or target.
            if scale as f64 * cl.vals[i] > 0.0 {
                sourcenode = v;
                if targetnode >= 0 {
                    break;
                }
            } else {
                targetnode = v;
                if sourcenode >= 0 {
                    break;
                }
            }
        }
    }

    // Cache result for future use.
    mcfdata.colsources[c] = sourcenode;
    mcfdata.coltargets[c] = targetnode;
    (sourcenode, targetnode)
}

/// `findUncapacitatedArcs`: find uncapacitated arcs for flow columns that
/// have no associated arc yet.
fn find_uncapacitated_arcs(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    let nflowcands = mcfdata.nflowcands as usize;
    let nnodes = mcfdata.nnodes as usize;
    let ncommodities = mcfdata.ncommodities;
    let modeltype = mcfdata.modeltype;

    // There should have been a cleanup already.
    debug_assert!(mcfdata.nemptycommodities == 0);

    // Avoid trivial cases.
    if ncommodities == 0 || nflowcands == 0 || nnodes == 0 {
        return Ok(());
    }

    // Get LP data.
    let rows = lp_rows(scip);
    let cols = api::get_lp_cols(scip);
    let ncols = cols.len();

    // Allocate temporary memory.
    let mut sortedflowcands = vec![0i32; nflowcands];
    let mut sortedflowcandnodeid = vec![0i32; nflowcands];
    let mut sourcecount = vec![0i32; nnodes];
    let mut targetcount = vec![0i32; nnodes];
    let mut adjnodes = vec![0i32; nnodes];
    let mut inccols = vec![0i32; ncols];

    // Copy flowcands and initialize sortedflowcandnodeid arrays.
    for n in 0..nflowcands {
        sortedflowcands[n] = mcfdata.flowcands[n];
        sortedflowcandnodeid[n] = mcfdata.rownodeid[mcfdata.flowcands[n] as usize];
    }

    // Sort flow candidates by node id.
    crate::sort::sort(&mut sortedflowcandnodeid[..], (&mut sortedflowcands[..],), crate::sort::cmp_int);

    let mut nadjnodes = 0usize;
    let mut ninccols = 0i32;

    // We only accept an arc if at least this many flow variables give rise
    // to this arc.
    let mut arcsthreshold = scip.set.ceil(ncommodities as f64 * UNCAPACITATEDARCSTRESHOLD) as i32;

    // In the undirected case, there are two variables per commodity in each
    // capacity row.
    if modeltype == MCFMODELTYPE_UNDIRECTED {
        arcsthreshold *= 2;
    }

    // Skip unused flow candidates.
    let mut n = 0usize;
    while n < nflowcands {
        if sortedflowcandnodeid[n] >= 0 {
            break;
        }
        debug_assert!(mcfdata.rowcommodity[sortedflowcands[n] as usize] == -1);
        n += 1;
    }

    // For each node s, count for each other node t the number of flow
    // variables that are not yet assigned to an arc and that give rise to an
    // (s,t) arc or an (t,s) arc.
    let mut v: i32 = 0;
    while n < nflowcands {
        // Process all flow rows that belong to node v.
        while n < nflowcands && sortedflowcandnodeid[n] == v {
            let r = sortedflowcands[n] as usize;
            let row = rows[r];

            // Update sourcecount and targetcount for all flow columns in the
            // row that are not yet assigned to an arc.
            let rowlen = scip.rows[row].nlpcols as usize;
            for i in 0..rowlen {
                let col = scip.rows[row].cols[i];
                let c = scip.cols[col].lppos as usize;
                let arcid = mcfdata.colarcid[c];

                if arcid == -2 {
                    // This is the second time we see this column, and we were
                    // unable to assign an arc to this column at the first
                    // time. So, this time we can ignore it. Just reset the
                    // temporary arcid -2 to -1.
                    mcfdata.colarcid[c] = -1;
                } else if arcid == -1 {
                    // Identify the (at most) two nodes which contain this
                    // flow variable.
                    let (mut s, mut t) = get_incident_nodes(scip, mcfdata, col);

                    // In the undirected case, always use s as other node.
                    if modeltype == MCFMODELTYPE_UNDIRECTED && s == v {
                        s = t;
                        t = v;
                    }

                    // If there is no other node than v, ignore column.
                    if s < 0 || t < 0 {
                        continue;
                    }

                    // Remember column in incidence list.
                    inccols[ninccols as usize] = c as i32;
                    ninccols += 1;

                    // Update source or target count.
                    let u = if s != v {
                        sourcecount[s as usize] += 1;
                        s
                    } else {
                        targetcount[t as usize] += 1;
                        t
                    } as usize;

                    // If other node has been seen the first time, store it in
                    // adjlist for sparse access of count arrays.
                    if sourcecount[u] + targetcount[u] == 1 {
                        adjnodes[nadjnodes] = u as i32;
                        nadjnodes += 1;
                    }
                }
            }
            n += 1;
        }

        // Check if we want to add uncapacitated arcs s -> v or v -> t.
        for l in 0..nadjnodes {
            let u = adjnodes[l];

            // Add arcs u -> v.
            if sourcecount[u as usize] >= arcsthreshold {
                // Create new arc.
                let arcid = create_new_arc(mcfdata, u, v);

                // Assign arcid to all involved columns.
                let mut m: i32 = 0;
                while m < ninccols {
                    let c = inccols[m as usize] as usize;

                    let (s, t) = get_incident_nodes(scip, mcfdata, cols[c]);

                    if s == u || (modeltype == MCFMODELTYPE_UNDIRECTED && t == u) {
                        mcfdata.colarcid[c] = arcid;

                        // Remove column from incidence array.
                        inccols[m as usize] = inccols[ninccols as usize - 1];
                        ninccols -= 1;
                        m -= 1;
                    }
                    m += 1;
                }
            }

            // Add arcs v -> u.
            if targetcount[u as usize] >= arcsthreshold {
                // Create new arc.
                let arcid = create_new_arc(mcfdata, v, u);

                // Assign arcid to all involved columns.
                let mut m: i32 = 0;
                while m < ninccols {
                    let c = inccols[m as usize] as usize;

                    let (_s, t) = get_incident_nodes(scip, mcfdata, cols[c]);

                    if t == u {
                        mcfdata.colarcid[c] = arcid;

                        // Remove column from incidence array.
                        inccols[m as usize] = inccols[ninccols as usize - 1];
                        ninccols -= 1;
                        m -= 1;
                    }
                    m += 1;
                }
            }
        }

        // Reset sourcecount and targetcount arrays. (C quirk: it clears the
        // first nadjnodes entries, not the adjacent nodes.)
        for l in 0..nadjnodes {
            sourcecount[l] = 0;
            targetcount[l] = 0;
        }
        nadjnodes = 0;

        // Mark the incident columns that could not be assigned to a new arc
        // such that we do not inspect them again.
        for l in 0..ninccols as usize {
            debug_assert!(mcfdata.colarcid[inccols[l] as usize] == -1);
            mcfdata.colarcid[inccols[l] as usize] = -2;
        }
        ninccols = 0;
        v += 1;
    }

    Ok(())
}

/// `cleanupNetwork`: cleans up the network: gets rid of commodities without
/// arcs or with at most one node.
fn cleanup_network(scip: &mut Scip, mcfdata: &mut McfData) -> ScipResult {
    let nflowcands = mcfdata.nflowcands as usize;
    let mut ncommodities = mcfdata.ncommodities as usize;
    let mut narcs = mcfdata.narcs as usize;
    let nnodes = mcfdata.nnodes as usize;

    // Get LP data.
    let rows = lp_rows(scip);
    let ncols = n_lp_cols(scip) as usize;

    // Allocate temporary memory.
    let permsize = max(max(ncommodities, narcs), nnodes);
    let mut nnodespercom = vec![0i32; ncommodities];
    let mut narcspercom = vec![0i32; ncommodities];
    let mut arcisincom = vec![false; ncommodities];
    let mut perm = vec![0i32; permsize];

    // Count the number of nodes in each commodity.
    for i in 0..nflowcands {
        let r = mcfdata.flowcands[i] as usize;
        if mcfdata.rowcommodity[r] >= 0 {
            debug_assert!(mcfdata.rownodeid[r] >= 0);
            nnodespercom[mcfdata.rowcommodity[r] as usize] += 1;
        }
    }

    // Count the number of arcs in each commodity.
    for a in 0..narcs {
        let r = scip.rows[mcfdata.capacityrows[a]].lppos as usize;
        debug_assert!(mcfdata.rowarcid[r] == a as i32);

        // Identify commodities which are touched by this arc capacity
        // constraint.
        arcisincom.fill(false);
        let row = rows[r];
        let rowlen = scip.rows[row].nlpcols as usize;
        for j in 0..rowlen {
            let c = row_col_pos(scip, row, j) as usize;
            if mcfdata.colcommodity[c] >= 0 && mcfdata.colarcid[c] == a as i32 {
                arcisincom[mcfdata.colcommodity[c] as usize] = true;
            }
        }

        // Increase arc counters of touched commodities.
        for k in 0..ncommodities {
            if arcisincom[k] {
                narcspercom[k] += 1;
            }
        }
    }

    // Calculate maximal number of nodes per commodity.
    let mut maxnnodes = 0;
    for k in 0..ncommodities {
        maxnnodes = max(maxnnodes, nnodespercom[k]);
    }

    // We want to keep only commodities that have at least a certain size
    // relative to the largest commodity.
    let mut nnodesthreshold = (MINCOMNODESFRACTION * maxnnodes as f64) as i32;
    nnodesthreshold = max(nnodesthreshold, MINNODES);

    // Discard trivial commodities.
    let mut newncommodities = 0;
    for k in 0..ncommodities {
        // Only keep commodities of a certain size that have at least one
        // arc.
        if nnodespercom[k] >= nnodesthreshold && narcspercom[k] >= 1 {
            debug_assert!(newncommodities <= k);
            perm[k] = newncommodities as i32;
            mcfdata.commoditysigns[newncommodities] = mcfdata.commoditysigns[k];
            newncommodities += 1;
        } else {
            perm[k] = -1;
        }
    }

    if newncommodities < ncommodities {
        let mut arcisused = vec![false; narcs];
        let mut nodeisused = vec![false; nnodes];

        // Update data structures to new commodity ids.
        for c in 0..ncols {
            if mcfdata.colcommodity[c] >= 0 {
                mcfdata.colcommodity[c] = perm[mcfdata.colcommodity[c] as usize];
                debug_assert!(mcfdata.colcommodity[c] < newncommodities as i32);
                if mcfdata.colcommodity[c] == -1 {
                    // We are lazy and do not update plusflow and minusflow.
                    mcfdata.colarcid[c] = -1;
                } else if mcfdata.colarcid[c] >= 0 {
                    arcisused[mcfdata.colarcid[c] as usize] = true;
                }
            }
        }
        for i in 0..nflowcands {
            let r = mcfdata.flowcands[i] as usize;
            if mcfdata.rowcommodity[r] >= 0 {
                mcfdata.rowcommodity[r] = perm[mcfdata.rowcommodity[r] as usize];
                debug_assert!(mcfdata.rowcommodity[r] < newncommodities as i32);
                if mcfdata.rowcommodity[r] == -1 {
                    // We are lazy and do not update flowrowsigns.
                    mcfdata.rownodeid[r] = -1;
                } else {
                    nodeisused[mcfdata.rownodeid[r] as usize] = true;
                }
            }
        }

        mcfdata.ncommodities = newncommodities as i32;
        ncommodities = newncommodities;
        let _ = ncommodities;

        // Discard unused arcs.
        let mut newnarcs = 0usize;
        for a in 0..narcs {
            if arcisused[a] {
                debug_assert!(newnarcs <= a);
                perm[a] = newnarcs as i32;
                mcfdata.capacityrows[newnarcs] = mcfdata.capacityrows[a];
                newnarcs += 1;
            } else {
                // We are lazy and do not update capacityrowsigns.
                perm[a] = -1;
            }
            let r = scip.rows[mcfdata.capacityrows[a]].lppos as usize;
            mcfdata.rowarcid[r] = perm[a];
        }

        // Update remaining data structures to new arc ids.
        if newnarcs < narcs {
            for c in 0..ncols {
                if mcfdata.colarcid[c] >= 0 {
                    mcfdata.colarcid[c] = perm[mcfdata.colarcid[c] as usize];
                    debug_assert!(mcfdata.colarcid[c] >= 0); // otherwise colarcid[c] was set to -1 in the colcommodity update
                }
            }
            mcfdata.narcs = newnarcs as i32;
            narcs = newnarcs;
        }
        let _ = narcs;

        // Discard unused nodes.
        let mut newnnodes = 0;
        for v in 0..nnodes {
            if nodeisused[v] {
                perm[v] = newnnodes;
                newnnodes += 1;
            } else {
                perm[v] = -1;
            }
        }

        // Update data structures to new node ids.
        if (newnnodes as usize) < nnodes {
            for i in 0..nflowcands {
                let r = mcfdata.flowcands[i] as usize;
                if mcfdata.rowcommodity[r] >= 0 {
                    debug_assert!(mcfdata.rownodeid[r] >= 0);
                    mcfdata.rownodeid[r] = perm[mcfdata.rownodeid[r] as usize];
                    debug_assert!(mcfdata.rownodeid[r] >= 0);
                }
            }
            mcfdata.nnodes = newnnodes;
        }
    }

    // Empty commodities have been removed here.
    mcfdata.nemptycommodities = 0;

    Ok(())
}

/// `identifySourcesTargets`: for each arc identifies a source and target
/// node; returns the effort level of separation.
fn identify_sources_targets(scip: &mut Scip, mcfdata: &mut McfData, maxinconsistencyratio: f64, maxarcinconsistencyratio: f64) -> EffortLevel {
    let narcs = mcfdata.narcs as usize;
    let nnodes = mcfdata.nnodes as usize;
    let ncommodities = mcfdata.ncommodities as usize;
    let modeltype = mcfdata.modeltype;

    let ncols = n_lp_cols(scip) as usize;

    // Allocate memory in mcfdata; initialize colsources and coltargets and
    // the adjacency lists.
    mcfdata.arcsources = vec![0; narcs];
    mcfdata.arctargets = vec![0; narcs];
    mcfdata.colsources = vec![-2; ncols];
    mcfdata.coltargets = vec![-2; ncols];
    mcfdata.firstoutarcs = vec![-1; nnodes];
    mcfdata.firstinarcs = vec![-1; nnodes];
    mcfdata.nextoutarcs = vec![-1; narcs];
    mcfdata.nextinarcs = vec![-1; narcs];

    mcfdata.arcarraysize = narcs as i32;

    // Allocate temporary memory for source and target node identification.
    let mut sourcenodecnt = vec![0.0f64; nnodes];
    let mut targetnodecnt = vec![0.0f64; nnodes];
    let mut flowvarspercom = vec![0i32; ncommodities];
    let mut comtouched = vec![false; ncommodities];
    let mut touchednodes = vec![0i32; nnodes];

    mcfdata.ninconsistencies = 0.0;
    let maxninconsistencies = maxinconsistencyratio * narcs as f64;

    // Search for source and target nodes.
    for a in 0..narcs {
        let caprow = mcfdata.capacityrows[a];
        let rowlen = scip.rows[caprow].nlpcols as usize;

        // Count number of flow variables per commodity.
        flowvarspercom.fill(0);
        comtouched.fill(false);
        let mut ntouchedcoms = 0;
        for i in 0..rowlen {
            let c = row_col_pos(scip, caprow, i) as usize;
            if mcfdata.colarcid[c] >= 0 {
                let k = mcfdata.colcommodity[c] as usize;
                flowvarspercom[k] += 1;
                if !comtouched[k] {
                    ntouchedcoms += 1;
                    comtouched[k] = true;
                }
            }
        }

        // If the row does not have any flow variable, it is not a capacity
        // constraint.
        if ntouchedcoms == 0 {
            mcfdata.capacityrows[a] = NONE;
            mcfdata.arcsources[a] = -1;
            mcfdata.arctargets[a] = -1;
            continue;
        }

        // Check the flow variables of the capacity row for flow conservation
        // constraints.
        let mut ntouchednodes = 0usize;
        let mut totalsourcecnt = 0.0;
        let mut totaltargetcnt = 0.0;
        let mut totalnodecnt = 0.0;
        for i in 0..rowlen {
            let col = scip.rows[caprow].cols[i];
            let c = scip.cols[col].lppos as usize;
            if mcfdata.colarcid[c] >= 0 {
                let k = mcfdata.colcommodity[c] as usize;

                // Identify the (at most) two nodes which contain this flow
                // variable.
                let (sourcev, targetv) = get_incident_nodes(scip, mcfdata, col);

                // Count the nodes.
                let weight = 1.0 / flowvarspercom[k] as f64;
                if sourcev >= 0 {
                    let sv = sourcev as usize;
                    if sourcenodecnt[sv] == 0.0 && targetnodecnt[sv] == 0.0 {
                        touchednodes[ntouchednodes] = sourcev;
                        ntouchednodes += 1;
                    }
                    sourcenodecnt[sv] += weight;
                    totalsourcecnt += weight;
                }
                if targetv >= 0 {
                    let tv = targetv as usize;
                    if sourcenodecnt[tv] == 0.0 && targetnodecnt[tv] == 0.0 {
                        touchednodes[ntouchednodes] = targetv;
                        ntouchednodes += 1;
                    }
                    targetnodecnt[tv] += weight;
                    totaltargetcnt += weight;
                }
                if sourcev >= 0 || targetv >= 0 {
                    totalnodecnt += weight;
                }
            }
        }

        // Perform a majority vote on source and target node.
        let mut bestsourcev = -1;
        let mut besttargetv = -1;
        let mut bestsourcecnt = 0.0;
        let mut besttargetcnt = 0.0;
        for i in 0..ntouchednodes {
            let v = touchednodes[i];
            let vu = v as usize;

            if modeltype == MCFMODELTYPE_DIRECTED {
                // In the directed model, we distinguish between source and
                // target.
                if sourcenodecnt[vu] >= targetnodecnt[vu] {
                    if sourcenodecnt[vu] > bestsourcecnt {
                        bestsourcev = v;
                        bestsourcecnt = sourcenodecnt[vu];
                    }
                } else if targetnodecnt[vu] > besttargetcnt {
                    besttargetv = v;
                    besttargetcnt = targetnodecnt[vu];
                }
            } else {
                let nodecnt = sourcenodecnt[vu] + targetnodecnt[vu];

                // In the undirected model, we use source for the maximum and
                // target for the second largest number of total hits.
                if nodecnt > bestsourcecnt {
                    besttargetv = bestsourcev;
                    besttargetcnt = bestsourcecnt;
                    bestsourcev = v;
                    bestsourcecnt = nodecnt;
                } else if nodecnt > besttargetcnt {
                    besttargetv = v;
                    besttargetcnt = nodecnt;
                }
            }

            // Clear the nodecnt arrays.
            sourcenodecnt[vu] = 0.0;
            targetnodecnt[vu] = 0.0;
        }

        // Check inconsistency of arcs.
        if modeltype == MCFMODELTYPE_UNDIRECTED {
            totalsourcecnt = totalnodecnt;
            totaltargetcnt = totalnodecnt;
        }
        let nsourceinconsistencies = (totalsourcecnt - bestsourcecnt) / ntouchedcoms as f64;
        let ntargetinconsistencies = (totaltargetcnt - besttargetcnt) / ntouchedcoms as f64;

        // Delete arcs that have to large inconsistency.
        if nsourceinconsistencies > maxarcinconsistencyratio {
            // Delete source assignment.
            bestsourcev = -1;
        }

        if ntargetinconsistencies > maxarcinconsistencyratio {
            // Delete target assignment.
            besttargetv = -1;
        }

        // Assign the incident nodes.
        mcfdata.arcsources[a] = bestsourcev;
        mcfdata.arctargets[a] = besttargetv;

        // Update adjacency lists.
        if bestsourcev != -1 {
            mcfdata.nextoutarcs[a] = mcfdata.firstoutarcs[bestsourcev as usize];
            mcfdata.firstoutarcs[bestsourcev as usize] = a as i32;
        }
        if besttargetv != -1 {
            mcfdata.nextinarcs[a] = mcfdata.firstinarcs[besttargetv as usize];
            mcfdata.firstinarcs[besttargetv as usize] = a as i32;
        }

        // Update the number of inconsistencies.
        mcfdata.ninconsistencies += 0.5 * (nsourceinconsistencies + ntargetinconsistencies);

        if mcfdata.ninconsistencies > maxninconsistencies {
            break;
        }
    }

    if mcfdata.ninconsistencies <= maxninconsistencies && narcs > 0 && ncommodities > 0 {
        EffortLevel::Default
    } else {
        EffortLevel::Off
    }
}

/// Node has not yet been seen.
const UNKNOWN: i32 = 0;
/// Node is currently on the processing stack.
const ONSTACK: i32 = 1;
/// Node has been visited and assigned to some component.
const VISITED: i32 = 2;

/// `identifyComponent`: returns lists of nodes and arcs in the connected
/// component of the given startv.
fn identify_component(mcfdata: &McfData, nodevisited: &mut [i32], startv: i32, compnodes: &mut [i32], ncompnodes: &mut i32, comparcs: &mut [i32], ncomparcs: &mut i32) {
    *ncompnodes = 0;
    *ncomparcs = 0;

    // Allocate temporary memory for node stack.
    let mut stacknodes = vec![0i32; mcfdata.nnodes as usize];

    // Put startv on stack.
    stacknodes[0] = startv;
    let mut nstacknodes = 1usize;
    nodevisited[startv as usize] = ONSTACK;

    // Perform depth-first search.
    while nstacknodes > 0 {
        // Pop first element from stack.
        let v = stacknodes[nstacknodes - 1];
        nstacknodes -= 1;
        nodevisited[v as usize] = VISITED;

        // Put node into component.
        compnodes[*ncompnodes as usize] = v;
        *ncompnodes += 1;

        // Go through the list of outgoing arcs.
        let mut a = mcfdata.firstoutarcs[v as usize];
        while a != -1 {
            let targetv = mcfdata.arctargets[a as usize];

            // Check if we have already visited the target node.
            if !(targetv != -1 && nodevisited[targetv as usize] == VISITED) {
                // Put arc to component.
                comparcs[*ncomparcs as usize] = a;
                *ncomparcs += 1;

                // Push target node to stack.
                if targetv != -1 && nodevisited[targetv as usize] == UNKNOWN {
                    stacknodes[nstacknodes] = targetv;
                    nstacknodes += 1;
                    nodevisited[targetv as usize] = ONSTACK;
                }
            }
            a = mcfdata.nextoutarcs[a as usize];
        }

        // Go through the list of ingoing arcs.
        let mut a = mcfdata.firstinarcs[v as usize];
        while a != -1 {
            let sourcev = mcfdata.arcsources[a as usize];

            // Check if we have already seen the source node.
            if !(sourcev != -1 && nodevisited[sourcev as usize] == VISITED) {
                // Put arc to component.
                comparcs[*ncomparcs as usize] = a;
                *ncomparcs += 1;

                // Push source node to stack.
                if sourcev != -1 && nodevisited[sourcev as usize] == UNKNOWN {
                    stacknodes[nstacknodes] = sourcev;
                    nstacknodes += 1;
                    nodevisited[sourcev as usize] = ONSTACK;
                }
            }
            a = mcfdata.nextinarcs[a as usize];
        }
    }
}

/// `mcfnetworkExtract`: extracts MCF network structures from the current
/// LP; returns the networks (sorted by non-increasing number of nodes) and
/// the effort level of separation.
fn mcfnetwork_extract(scip: &mut Scip, sepa: SepaId) -> ScipResult<(Vec<McfNetwork>, EffortLevel)> {
    let (modeltype, maxinconsistencyratio, maxarcinconsistencyratio) = {
        let d = data(scip, sepa);
        (d.modeltype, d.maxinconsistencyratio, d.maxarcinconsistencyratio)
    };

    let mut failed = false;
    let mut effortlevel = EffortLevel::Off;
    let mut mcfnetworks: Vec<McfNetwork> = Vec::new();

    // Initialize local extraction data.
    let mut mcfdata = McfData { modeltype, ..Default::default() };

    // 1. identify candidate rows for flow conservation constraints in the LP
    // 2. Sort flow conservation candidates by a ranking on how sure we are
    //    that it is indeed a constraint of the desired type
    extract_flow_rows(scip, &mut mcfdata)?;

    if mcfdata.nflowcands == 0 {
        failed = true;
    }

    if !failed {
        // 3. extract network structure of flow conservation constraints.
        extract_flow(scip, &mut mcfdata, MAXFLOWVARFLOWROWRATIO, &mut failed)?;
    }

    if !failed {
        // 4. identify candidate rows for capacity constraints in the LP
        // 5. sort capacity constraint candidates by a ranking on how sure we
        //    are that it is indeed a constraint of the desired type
        extract_capacity_rows(scip, &mut mcfdata)?;

        if mcfdata.ncapacitycands == 0 {
            failed = true;
        }
    }

    if !failed {
        // 6. arc-detection -- identify capacity constraints for the arcs and
        //    assign arc ids to columns and capacity constraints
        extract_capacities(scip, &mut mcfdata)?;

        // 7. node-detection -- assign node ids to flow conservation
        //    constraints
        extract_nodes(scip, &mut mcfdata)?;

        // 8. postprocessing
        // 8.a if there are still undecided commodity signs, fix them to +1
        fix_commodity_signs(&mut mcfdata);

        // 8.b clean up the network: get rid of commodities without arcs or
        //     with at most one node
        cleanup_network(scip, &mut mcfdata)?;

        // 8.c construct incidence function -- assign source and target nodes
        //     to capacitated arcs
        effortlevel = identify_sources_targets(scip, &mut mcfdata, maxinconsistencyratio, maxarcinconsistencyratio);
    }

    if !failed && effortlevel != EffortLevel::Off {
        // 8.d find uncapacitated arcs
        find_uncapacitated_arcs(scip, &mut mcfdata)?;

        let mut minnodes = MINNODES;

        // Allocate temporary memory for component finding.
        let nnodes = mcfdata.nnodes as usize;
        let mut nodevisited = vec![UNKNOWN; nnodes];
        let mut compnodes = vec![0i32; nnodes];
        let mut comparcs = vec![0i32; mcfdata.narcs as usize];

        // Allocate temporary memory for v -> compv mapping.
        let mut compnodeid = vec![-1i32; nnodes];

        // Search components and create a network structure for each of
        // them.
        for v in 0..nnodes {
            // Ignore nodes that have been already assigned to a component.
            if nodevisited[v] == VISITED {
                continue;
            }

            // Identify nodes and arcs of this component.
            let mut ncompnodes = 0;
            let mut ncomparcs = 0;
            identify_component(&mcfdata, &mut nodevisited, v as i32, &mut compnodes, &mut ncompnodes, &mut comparcs, &mut ncomparcs);

            // Ignore network component if it is trivial.
            if ncompnodes >= minnodes && ncomparcs >= MINARCS {
                // Create network data structure and fill sparse network
                // structure.
                let mut mcfnetwork = McfNetwork::default();
                mcfnetwork_fill(scip, &mut mcfnetwork, &mcfdata, &mut compnodeid, &compnodes, ncompnodes, &comparcs, ncomparcs)?;

                // Insert in sorted network list.
                let mut i = mcfnetworks.len();
                while i > 0 && mcfnetwork.nnodes > mcfnetworks[i - 1].nnodes {
                    i -= 1;
                }
                mcfnetworks.insert(i, mcfnetwork);

                // If we reached the maximal number of networks, update
                // minnodes.
                if mcfnetworks.len() as i32 >= MAXNETWORKS {
                    minnodes = max(minnodes, mcfnetworks.last().unwrap().nnodes);
                }

                // If we exceeded the maximal number of networks, delete the
                // last one.
                if mcfnetworks.len() as i32 > MAXNETWORKS {
                    let mut last = mcfnetworks.pop().unwrap();
                    mcfnetwork_free(scip, &mut last)?;
                }
            }
        }
    }

    Ok((mcfnetworks, effortlevel))
}

/*
 * Union find methods: used for generating partitions of node sets and for
 * checking connectivity of cut shores.
 */

/// `unionfindInitSets`: initializes a union find data structure by putting
/// each element into its own set.
fn unionfind_init_sets(representatives: &mut [i32], nelems: usize) {
    // We start with each element being in its own set.
    for v in 0..nelems {
        representatives[v] = v as i32;
    }
}

/// `unionfindGetRepresentative`: applies a union find algorithm to get the
/// representative of v.
fn unionfind_get_representative(representatives: &mut [i32], mut v: i32) -> i32 {
    while v != representatives[v as usize] {
        representatives[v as usize] = representatives[representatives[v as usize] as usize];
        v = representatives[v as usize];
    }
    v
}

/// `unionfindJoinSets`: joins two sets in the union find framework.
fn unionfind_join_sets(representatives: &mut [i32], rep1: i32, rep2: i32) {
    debug_assert!(rep1 != rep2);
    debug_assert!(representatives[rep1 as usize] == rep1);
    debug_assert!(representatives[rep2 as usize] == rep2);

    // Make sure that the smaller representative survives -> element 0 is
    // always a representative.
    if rep1 < rep2 {
        representatives[rep2 as usize] = rep1;
    } else {
        representatives[rep1 as usize] = rep2;
    }
}

/*
 * Node pair methods: used for shrinking the network based on nodepair
 * weights -> creating partition.
 */

/// Data structure to put a nodepair on the heap -- it holds node1 <= node2.
#[derive(Clone, Copy)]
struct NodePairEntry {
    node1: i32,
    node2: i32,
    /// Weight of the arc in the separation problem.
    weight: f64,
}

/// `compNodepairs`: comparison method for weighted nodepairs (larger weight
/// first).
fn comp_nodepairs(nodepair1: &NodePairEntry, nodepair2: &NodePairEntry) -> i32 {
    if nodepair1.weight > nodepair2.weight {
        -1
    } else if nodepair1.weight < nodepair2.weight {
        1
    } else {
        0
    }
}

/// Nodepair priority queue (`NODEPAIRQUEUE`): the heap holds indices into
/// `nodepairs`.
struct NodePairQueue {
    pqueue: crate::pqueue::PQueue<usize>,
    nodepairs: Vec<NodePairEntry>,
}

/// `nodepairqueueCreate`: creates a priority queue and fills it with the
/// given nodepair entries.
fn nodepairqueue_create(scip: &mut Scip, mcfnetwork: &McfNetwork) -> NodePairQueue {
    // For every nodepair that is used in the network (at least one arc
    // exists having this nodepair as endnodes) we calculate a weight: the
    // weight w_st of a nodepair (s,t) is the minimum of the weights of all
    // s-t and t-s arcs. The weight w_a of an arc a is w_a := s_a + pi_a,
    // where s_a>=0 is the slack of the capacity constraint and pi_a<=0 its
    // dual. The weight of uncapacitated arcs is infinite.
    // (BETTERWEIGHTFORDEMANDNODES is defined.)
    let ncommodities = mcfnetwork.ncommodities as usize;

    // Create a hash table for all used node pairs; it is only needed to have
    // unique nodepairs (identify arcs using the same nodepair), so a map
    // keyed by the node pair is equivalent.
    let mut hashtable: std::collections::HashMap<(i32, i32), usize> = std::collections::HashMap::new();

    // nodepairs will contain all constructed nodepairs and is used to fill
    // the priority queue.
    let mut nodepairs: Vec<NodePairEntry> = Vec::with_capacity(mcfnetwork.narcs as usize);

    // Initialize hash table of all used node pairs and fill nodepairs.
    for a in 0..mcfnetwork.narcs as usize {
        let capacityrow = mcfnetwork.arccapacityrows[a];

        // Construct fresh nodepair: smaller node gets node1 in nodeentry.
        let mut nodepair = if mcfnetwork.arcsources[a] <= mcfnetwork.arctargets[a] {
            NodePairEntry {
                node1: mcfnetwork.arcsources[a],
                node2: mcfnetwork.arctargets[a],
                weight: 0.0,
            }
        } else {
            NodePairEntry {
                node2: mcfnetwork.arcsources[a],
                node1: mcfnetwork.arctargets[a],
                weight: 0.0,
            }
        };

        if nodepair.node1 == -1 || nodepair.node2 == -1 {
            continue;
        }

        // Construct arc weight of a.
        if capacityrow != NONE {
            let mut slack = if crate::tree::tree_has_current_node_lp(scip) {
                crate::lp::row_get_lp_feasibility(scip, capacityrow)
            } else {
                crate::lp::row_get_pseudo_feasibility(scip, capacityrow)
            };
            slack = max(slack, 0.0); // can only be negative due to numerics
            let dualsol = row_dualsol(scip, capacityrow);
            let maxval = crate::lp::row_get_maxval(scip, capacityrow);
            let scale = mcfnetwork.arccapacityscales[a].abs() / maxval; // divide by maxval to normalize rows
            debug_assert!(scale > 0.0);

            // Put the arc weight into a fresh nodepair.
            nodepair.weight = scale * slack - dualsol.abs() / scale;
        } else {
            // Uncapacitated arc has infinite slack.
            nodepair.weight = scip.set.infinity();
        }

        // Check if nodepair already exists in hash-table.
        if let Some(&pos) = hashtable.get(&(nodepair.node1, nodepair.node2)) {
            // If nodepair already exists update its weight.
            nodepairs[pos].weight = min(nodepair.weight, nodepairs[pos].weight);
        } else {
            // No such nodepair in current hash table: insert into array and
            // hashtable.
            hashtable.insert((nodepair.node1, nodepair.node2), nodepairs.len());
            nodepairs.push(nodepair);
        }
    }
    let nnodepairs = nodepairs.len();

    // Initial weights have been calculated; we modify them now depending on
    // the demand emanating at the endnodes of nodepairs.

    // Calculate max and min weight.
    let mut maxweight = 1.0; // we want maxweight to be positive
    let mut minweight = -1.0; // we want minweight to be negative
    for n in 0..nnodepairs {
        // maxweight should not be infinity (uncap arcs have infinity
        // weight).
        if !scip.set.is_infinity(nodepairs[n].weight) {
            maxweight = max(maxweight, nodepairs[n].weight);
        }

        minweight = min(minweight, nodepairs[n].weight);
    }

    // Initialize priority queue.
    let mut pqueue = crate::pqueue::PQueue::new(nnodepairs);

    // Fill priority queue using array nodepairs.
    for n in 0..nnodepairs {
        let node1 = nodepairs[n].node1 as usize;
        let node2 = nodepairs[n].node2 as usize;

        let mut hasdemand1 = false;
        let mut hasdemand2 = false;

        // Check both nodes for their demand value in all commodities; the
        // demand value can be read from the rhs of the flowrows.
        for (node, hasdemand) in [(node1, &mut hasdemand1), (node2, &mut hasdemand2)] {
            for k in 0..ncommodities {
                let row = mcfnetwork.nodeflowrows[node][k];
                if row == NONE {
                    continue;
                }

                let rw = &scip.rows[row];
                let rhs = if mcfnetwork.nodeflowscales[node][k] > 0.0 {
                    rw.rhs - rw.constant
                } else {
                    rw.lhs - rw.constant
                };

                if !scip.set.is_zero(rhs) {
                    *hasdemand = true;
                    break;
                }
            }
        }

        // If one of the nodes has no demand increase the score (slack arcs
        // are still shrunk first).
        if scip.set.is_positive(nodepairs[n].weight) {
            if !hasdemand1 || !hasdemand2 {
                nodepairs[n].weight += maxweight;
            }
        } else if hasdemand1 && hasdemand2 {
            nodepairs[n].weight += minweight;
        }

        // Fill priority queue.
        pqueue.insert(n, |&x, &y| comp_nodepairs(&nodepairs[x], &nodepairs[y]), |_, _, _| {});
    }

    NodePairQueue { pqueue, nodepairs }
}

/// `nodepairqueueRemove`: removes the top element from the nodepair
/// priority queue.
fn nodepairqueue_remove(q: &mut NodePairQueue) -> Option<NodePairEntry> {
    let nodepairs = &q.nodepairs;
    q.pqueue.remove(|&x, &y| comp_nodepairs(&nodepairs[x], &nodepairs[y]), |_, _, _| {}).map(|n| nodepairs[n])
}

/*
 * Node partition methods: used for creating partitions of nodes; use union
 * find algorithm.
 */

/// Partitioning of the nodes into clusters (`NODEPARTITION`).
struct NodePartition {
    /// Mapping of node ids to their representatives within their cluster.
    representatives: Vec<i32>,
    /// Cluster for each node id.
    nodeclusters: Vec<i32>,
    /// Node ids sorted by cluster.
    clusternodes: Vec<i32>,
    /// First entry in clusternodes for each cluster (size: nclusters+1).
    clusterbegin: Vec<i32>,
    nclusters: i32,
}

/// `nodepartitionCreate`: partitions nodes into a small number of clusters.
fn nodepartition_create(scip: &mut Scip, mcfnetwork: &McfNetwork, nclusters: i32) -> NodePartition {
    let nnodes = mcfnetwork.nnodes as usize;

    // Allocate and initialize memory.
    let mut np = NodePartition {
        representatives: vec![0; nnodes],
        nodeclusters: vec![0; nnodes],
        clusternodes: vec![0; nnodes],
        clusterbegin: vec![0; nclusters as usize + 1],
        nclusters: 0,
    };

    // We start with each node being in its own cluster.
    unionfind_init_sets(&mut np.representatives, nnodes);

    // Create priority queue for nodepairs.
    let mut nodepairqueue = nodepairqueue_create(scip, mcfnetwork);

    // Loop over nodepairs in order of their weights.
    let mut nclustersleft = mcfnetwork.nnodes;
    while nodepairqueue.pqueue.first().is_some() && nclustersleft > nclusters {
        // Get the next nodepair.
        let nodepair = nodepairqueue_remove(&mut nodepairqueue).unwrap();
        let node1 = nodepair.node1;
        let node2 = nodepair.node2;

        // Identify the representatives of the two nodes.
        let node1rep = unionfind_get_representative(&mut np.representatives, node1);
        let node2rep = unionfind_get_representative(&mut np.representatives, node2);

        // There is nothing to do if the two nodes are already in the same
        // cluster.
        if node1rep == node2rep {
            continue;
        }

        // Shrink nodepair by joining the two clusters.
        unionfind_join_sets(&mut np.representatives, node1rep, node2rep);
        nclustersleft -= 1;
    }

    // Node 0 must be a representative due to our join procedure.
    debug_assert!(np.representatives[0] == 0);

    // If there have been too few arcs to shrink the graph to the required
    // number of clusters, join clusters with first cluster to create a
    // larger disconnected cluster.
    if nclustersleft > nclusters {
        let mut v = 1;
        while v < mcfnetwork.nnodes && nclustersleft > nclusters {
            let rep = unionfind_get_representative(&mut np.representatives, v);
            if rep != 0 {
                unionfind_join_sets(&mut np.representatives, 0, rep);
                nclustersleft -= 1;
            }
            v += 1;
        }
    }
    debug_assert!(nclustersleft <= nclusters);

    // Extract the clusters.
    let mut clustersize = vec![0i32; nclusters as usize];
    for v in 0..nnodes {
        // Get cluster of node.
        let rep = unionfind_get_representative(&mut np.representatives, v as i32);
        let c = if rep == v as i32 {
            // Node is its own representative: this is a new cluster.
            let c = np.nclusters;
            np.nclusters += 1;
            c
        } else {
            np.nodeclusters[rep as usize]
        };

        // Assign node to cluster.
        np.nodeclusters[v] = c;
        clustersize[c as usize] += 1;
    }

    // Fill the clusterbegin array.
    let mut pos = 0;
    for c in 0..np.nclusters as usize {
        np.clusterbegin[c] = pos;
        pos += clustersize[c];
    }
    np.clusterbegin[np.nclusters as usize] = mcfnetwork.nnodes;

    // Fill the clusternodes array.
    clustersize[..np.nclusters as usize].fill(0);
    for v in 0..nnodes {
        let c = np.nodeclusters[v] as usize;
        let pos = np.clusterbegin[c] + clustersize[c];
        np.clusternodes[pos as usize] = v as i32;
        clustersize[c] += 1;
    }

    np
}

/// `nodeInPartition`: returns whether given node v is in a cluster that
/// belongs to the partition S (`nodepartition` NONE: `partition` is the node
/// number in a single-node partition).
fn node_in_partition(nodepartition: Option<&NodePartition>, partition: u32, inverted: bool, v: i32) -> bool {
    // If the node does not exist, it is not in the partition (and also not
    // in the inverted partition).
    if v < 0 {
        return false;
    }

    match nodepartition {
        None => (v == partition as i32) == !inverted,
        Some(np) => {
            let cluster = np.nodeclusters[v as usize];
            let clusterbit = 1u32.wrapping_shl(cluster as u32);
            ((partition & clusterbit) != 0) == !inverted
        }
    }
}

/// `nodepartitionIsConnected`: returns whether each of the sets S and T
/// defined by the nodepartition is connected.
fn nodepartition_is_connected(mcfnetwork: &McfNetwork, nodepartition: &NodePartition, partition: u32) -> bool {
    let nodeclusters = &nodepartition.nodeclusters;
    let nclusters = nodepartition.nclusters as usize;

    let mut rep = vec![0i32; nclusters];

    // Start with each cluster being isolated.
    unionfind_init_sets(&mut rep, nclusters);
    let mut ncomponents = nclusters as i32;
    debug_assert!(ncomponents >= 2);

    // For each arc within S or within T join the connected clusters.
    for a in 0..mcfnetwork.narcs as usize {
        let s = mcfnetwork.arcsources[a];
        let t = mcfnetwork.arctargets[a];

        // Ignore arcs that connect the pseudo node -1.
        if s == -1 || t == -1 {
            continue;
        }

        // Check if arc is within one of the components.
        if node_in_partition(Some(nodepartition), partition, false, s) == node_in_partition(Some(nodepartition), partition, false, t) {
            // Get clusters of the two nodes.
            let cs = nodeclusters[s as usize];
            let ct = nodeclusters[t as usize];

            // Nothing to do if we are already in the same cluster.
            if cs == ct {
                continue;
            }

            // Get representatives of clusters in the union structure.
            let repcs = unionfind_get_representative(&mut rep, cs);
            let repct = unionfind_get_representative(&mut rep, ct);
            if repcs == repct {
                continue;
            }

            // The arc connects two previously unconnected components of S or
            // T; check if we already reached two distinct components.
            ncomponents -= 1;
            if ncomponents <= 2 {
                break;
            }

            // Join the two cluster sets and continue.
            unionfind_join_sets(&mut rep, repcs, repct);
        }
    }

    // Each of the two shores S and T are connected if we were able to shrink
    // the graph into two components.
    ncomponents == 2
}

/// `addCut`: adds given cut to LP if violated.
#[allow(clippy::too_many_arguments)]
fn add_cut(
    scip: &mut Scip,
    sepa: SepaId,
    sol: SolId,
    cutcoefs: &[f64],
    cutrhs: f64,
    cutinds: &[i32],
    cutnnz: i32,
    cutislocal: bool,
    cutrank: i32,
    ncuts: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    let (dynamiccuts, separateknapsack) = {
        let d = data(scip, sepa);
        (d.dynamiccuts, d.separateknapsack)
    };

    // Get active problem variables.
    let vars = api::get_vars(scip).to_vec();

    *cutoff = false;

    let cutnnz = cutnnz as usize;
    let cutvars: Vec<_> = (0..cutnnz).map(|i| vars[cutinds[i] as usize]).collect();

    // Create the cut.
    let cutname = format!("mcf{}_{}", scip.stat.nlps, *ncuts);
    let inf = scip.set.infinity();
    let mut cut = crate::scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, cutrhs, cutislocal, false, dynamiccuts)?;

    crate::scip_lp::add_vars_to_row(scip, cut, &cutvars, &cutcoefs[..cutnnz])?;

    // Set cut rank.
    scip.rows[cut].rank = cutrank;

    if !cutislocal {
        api::add_pool_cut(scip, cut)?;
    } else {
        *cutoff = crate::scip_lp::add_row(scip, cut, false)?;
    }
    *ncuts += 1;

    // Release the row.
    crate::scip_lp::release_row(scip, &mut cut)?;

    if !*cutoff && separateknapsack {
        // Relax cut to knapsack row and separate lifted cover cuts.
        crate::cons_knapsack::separate_relaxed_knapsack(scip, NONE, sepa, &cutvars, &cutcoefs[..cutnnz], 1.0, cutrhs, sol, cutoff, ncuts)?;
    }

    Ok(())
}

/// `generateClusterCuts`: enumerates cuts between subsets of the clusters;
/// generates single-node cuts if `nodepartition` is `None`, otherwise
/// generates cluster cuts.
#[allow(clippy::too_many_arguments)]
fn generate_cluster_cuts(
    scip: &mut Scip,
    sepa: SepaId,
    sol: SolId,
    allowlocal: bool,
    depth: i32,
    mcfnetwork: &McfNetwork,
    nodepartition: Option<&NodePartition>,
    ncuts: &mut i32,
    cutoff: &mut bool,
) -> ScipResult {
    let nodeflowrows = &mcfnetwork.nodeflowrows;
    let nodeflowscales = &mcfnetwork.nodeflowscales;
    let nodeflowinverted = &mcfnetwork.nodeflowinverted;
    let arccapacityrows = &mcfnetwork.arccapacityrows;
    let arccapacityscales = &mcfnetwork.arccapacityscales;
    let colcommodity = &mcfnetwork.colcommodity;
    let arcsources = &mcfnetwork.arcsources;
    let arctargets = &mcfnetwork.arctargets;
    let nnodes = mcfnetwork.nnodes;
    let narcs = mcfnetwork.narcs as usize;
    let ncommodities = mcfnetwork.ncommodities as usize;
    let modeltype = mcfnetwork.modeltype;

    let (effortlevel, maxsepacutsroot, maxsepacuts0, maxtestdelta0, separatesinglenodecuts, separateflowcutset, checkcutshoreconnectivity, fixintegralrhs) = {
        let d = data(scip, sepa);
        (
            d.effortlevel,
            d.maxsepacutsroot,
            d.maxsepacuts,
            d.maxtestdelta,
            d.separatesinglenodecuts,
            d.separateflowcutset,
            d.checkcutshoreconnectivity,
            d.fixintegralrhs,
        )
    };

    let mut oldncuts; // to check success of separation for one nodeset
    *cutoff = false;

    let nrows = lp_rows(scip).len();
    let nvars = api::get_n_vars(scip);

    // Get the maximal number of cuts allowed in a separation round.
    let mut maxsepacuts = if depth == 0 { maxsepacutsroot } else { maxsepacuts0 };
    if maxsepacuts < 0 {
        maxsepacuts = i32::MAX;
    } else if effortlevel == EffortLevel::Aggressive {
        maxsepacuts *= 2;
    }

    // Get the maximal number of deltas to use for cmir separation.
    let mut maxtestdelta = maxtestdelta0;
    if maxtestdelta <= 0 {
        maxtestdelta = i32::MAX;
    } else if effortlevel == EffortLevel::Aggressive {
        maxtestdelta *= 2;
    }

    // Our system has the following form:
    //  (1) \sum_{a \in \delta^+(v)} f_a^k - \sum_{a \in \delta^-(v)} f_a^k <= -d_v^k for all v \in V and k \in K
    //  (2) \sum_{k \in K} f_a^k - c_a x_a <= 0 for all a \in A
    //
    // The partitioning yields two clusters S and T with arcs A^+ from S to
    // T and A^- from T to S. Now we look at all commodities in which we
    // have to route flow from T to S: K^+ = {k : d^k_S := sum_{v \in S}
    // d_v^k > 0}. Then, the base constraint of the c-MIR cut is the sum of
    // those flow conservation constraints and the capacity constraints for
    // arcs A^-.

    let mut deltas: Vec<f64> = Vec::with_capacity(16);
    let mut rowweights = vec![0.0f64; nrows];
    let mut comcutdemands = vec![0.0f64; ncommodities];
    let mut comdemands = vec![0.0f64; ncommodities];
    let mut cutcoefs = vec![0.0f64; nvars as usize];
    let mut cutinds = vec![0i32; nvars as usize];

    // Create empty aggregation row.
    let mut aggrrow = crate::cuts::aggr_row_create(scip);

    let (startpartition, allpartitions): (u32, u32) = match nodepartition {
        // Loop over all nodes and generate single-node cuts.
        None => (0, nnodes as u32),
        // Loop over all possible partitions of the clusters; cluster i is in
        // S iff bit i of 'partition' is 1. We fix the last cluster to belong
        // to partition T. In the undirected case, this is sufficient,
        // because S-T is equivalent to T-S. In the directed case, the loop
        // below is conducted two times, one with regular S-T and one with
        // inverted partitions.
        Some(np) => (1, 1u32.wrapping_shl((np.nclusters - 1) as u32)),
    };
    let useinverted = mcfnetwork.modeltype == MCFMODELTYPE_DIRECTED;

    let mut partition = startpartition;
    while partition <= allpartitions.wrapping_sub(1) && !crate::solve::solve_is_stopped(scip, false) && *ncuts < maxsepacuts && !*cutoff {
        let mut bestdelta = 1.0;

        if checkcutshoreconnectivity {
            if let Some(np) = nodepartition {
                if !nodepartition_is_connected(mcfnetwork, np, partition) {
                    // If S or T are not connected, it is very likely that
                    // there is a cut in our cluster partition that gives
                    // dominating inequalities.
                    partition += 1;
                    continue;
                }
            }
        }

        let mut inverted = false;
        while inverted <= useinverted && !*cutoff {
            // Clear memory.
            rowweights.fill(0.0);
            comcutdemands.fill(0.0);
            comdemands.fill(0.0);

            let mut baserhs = 0.0;
            deltas.clear();

            // Identify commodities with positive T -> S demand.
            let mut nnodesins = 0;
            for v in 0..nnodes {
                // Check if node belongs to S.
                if !node_in_partition(nodepartition, partition, inverted, v) {
                    // Node does not belong to S.
                    continue;
                }
                nnodesins += 1;
                let v = v as usize;
                // Update commodity demand.
                for k in 0..ncommodities {
                    let row = nodeflowrows[v][k];
                    if row == NONE {
                        continue;
                    }

                    let rw = &scip.rows[row];
                    let mut rhs = if nodeflowscales[v][k] > 0.0 { rw.rhs - rw.constant } else { rw.lhs - rw.constant };
                    if nodeflowinverted[v][k] {
                        rhs *= -1.0;
                    }

                    comcutdemands[k] += rhs * nodeflowscales[v][k];
                }
            }
            debug_assert!(1 <= nnodesins && nnodesins <= nnodes - 1);

            // Ignore cuts with only a single node in S or in T, since these
            // have already been tried as single node cuts.
            if separatesinglenodecuts && nodepartition.is_some() && (nnodesins == 1 || nnodesins == nnodes - 1) {
                break;
            }

            // Check if there is at least one useful commodity.
            let mut k = 0;
            if modeltype == MCFMODELTYPE_DIRECTED {
                while k < ncommodities {
                    // In the directed case, use commodities with positive
                    // demand (negative -d_k).
                    if scip.set.is_negative(comcutdemands[k]) {
                        break;
                    }
                    k += 1;
                }
            } else {
                while k < ncommodities {
                    // In the undirected case, use commodities with non-zero
                    // demand.
                    if !scip.set.is_zero(comcutdemands[k]) {
                        break;
                    }
                    k += 1;
                }
            }
            if k == ncommodities {
                inverted = !inverted && useinverted;
                if !inverted {
                    break;
                }
                continue;
            }

            // Set weights of capacity rows that go from T to S, i.e., a \in
            // A^-.
            for a in 0..narcs {
                // Check if this is an arc of our cut.
                if modeltype == MCFMODELTYPE_DIRECTED {
                    // In the directed case, check if arc goes from T to S.
                    if node_in_partition(nodepartition, partition, inverted, arcsources[a]) || !node_in_partition(nodepartition, partition, inverted, arctargets[a]) {
                        // Arc has source in S or target in T.
                        continue;
                    }
                } else {
                    // In the undirected case, check if the arc has endpoints
                    // in S and T.
                    if node_in_partition(nodepartition, partition, inverted, arcsources[a]) && node_in_partition(nodepartition, partition, inverted, arctargets[a]) {
                        // Both endpoints are in S.
                        continue;
                    }
                    if !node_in_partition(nodepartition, partition, inverted, arcsources[a]) && !node_in_partition(nodepartition, partition, inverted, arctargets[a]) {
                        // Both endpoints are in T.
                        continue;
                    }
                }

                // Arc might be uncapacitated.
                let caprow = arccapacityrows[a];
                if caprow == NONE {
                    continue;
                }

                // Use capacity row in c-MIR cut.
                let r = scip.rows[caprow].lppos;
                if r == -1 {
                    // Row might have been removed from LP in the meantime.
                    continue;
                }
                let r = r as usize;

                // If one of the arc nodes is unknown, we only use the
                // capacity row if it does not have slack, otherwise, we
                // discard it if the slack is too large.
                let feasibility = crate::scip_lp::get_row_sol_feasibility(scip, caprow, sol);
                if arcsources[a] == -1 || arctargets[a] == -1 {
                    if scip.set.is_feas_positive(feasibility) {
                        continue;
                    }
                } else {
                    let maxcoef = crate::lp::row_get_maxval(scip, caprow);
                    debug_assert!(scip.set.is_positive(maxcoef));
                    if scip.set.is_feas_gt(feasibility / maxcoef, MAXCAPACITYSLACK) {
                        continue;
                    }
                }

                rowweights[r] = arccapacityscales[a];

                let rw = &scip.rows[caprow];
                if separateflowcutset {
                    if rowweights[r] > 0.0 {
                        baserhs += rowweights[r] * (rw.rhs - rw.constant);
                    } else {
                        baserhs += rowweights[r] * (rw.lhs - rw.constant);
                    }
                }

                // Extract useful deltas for c-MIR scaling and update the
                // demand value for commodities (in binary flow model).
                let rowlen = rw.nlpcols as usize;
                for j in 0..rowlen {
                    let mut coef = rw.vals[j] * arccapacityscales[a];
                    coef = coef.abs();

                    // Update commodity demands.
                    let col = rw.cols[j];
                    let c = scip.cols[col].lppos as usize;
                    let k = colcommodity[c];
                    if k >= 0 {
                        comdemands[k as usize] = coef;
                    }

                    // Insert coefficients of integer variables into deltas
                    // array; coefficients should not be too small and not be
                    // too big.
                    if !scip.set.is_zero(1.0 / coef) && !scip.set.is_feas_zero(coef) && scip.cols[col].integral {
                        // Binary search if we already know this coefficient.
                        let mut exists = false;
                        let mut left: i32 = 0;
                        let mut right: i32 = deltas.len() as i32 - 1;
                        while left <= right {
                            let mid = ((left + right) / 2) as usize;
                            // Take deltas that are not too close.
                            if (deltas[mid] / coef - 1.0).abs() < 1e-03 {
                                exists = true;
                                break;
                            } else if coef < deltas[mid] {
                                right = mid as i32 - 1;
                            } else {
                                left = mid as i32 + 1;
                            }
                        }

                        // Insert new candidate value.
                        if !exists {
                            deltas.insert(left as usize, coef);
                        }
                    }
                }
            }

            // Set weights of node flow conservation constraints in c-MIR
            // aggregation.
            for v in 0..nnodes {
                let vu = v as usize;
                // Aggregate flow conservation constraints of the 'active'
                // commodities.
                for k in 0..ncommodities {
                    // If commodity was not hit by the capacity constraints of
                    // the cut in the graph, ignore the commodity.
                    if comdemands[k] == 0.0 {
                        continue;
                    }

                    let scale = comdemands[k];
                    if modeltype == MCFMODELTYPE_DIRECTED {
                        // In the directed case, use rows of commodities with
                        // positive demand (negative -d_k).
                        if !scip.set.is_negative(comcutdemands[k]) {
                            continue;
                        }

                        // Check if node belongs to S.
                        if !node_in_partition(nodepartition, partition, inverted, v) {
                            // Node belongs to T.
                            continue;
                        }
                    } else {
                        // In the undirected case, use rows of commodities
                        // with non-zero demand.
                        if scip.set.is_zero(comcutdemands[k]) {
                            continue;
                        }

                        // If the demand (-d_k) is negative (i.e., points
                        // into the wrong direction), we use the flow in the
                        // opposite direction, i.e., sum over all nodes in T
                        // instead of S.
                        if comcutdemands[k] > 0.0 {
                            // Check if node belongs to T.
                            if node_in_partition(nodepartition, partition, inverted, v) {
                                // Node belongs to S.
                                continue;
                            }
                        } else if !node_in_partition(nodepartition, partition, inverted, v) {
                            // Node belongs to T.
                            continue;
                        }
                    }
                    let row = nodeflowrows[vu][k];
                    if row == NONE {
                        continue;
                    }

                    let r = scip.rows[row].lppos;
                    if r >= 0 {
                        // Row might have been removed from LP in the
                        // meantime.
                        let r = r as usize;

                        // Ignore rows with slack.
                        let feasibility = crate::scip_lp::get_row_sol_feasibility(scip, row, sol);
                        if !scip.set.is_feas_positive(feasibility) {
                            rowweights[r] = scale * nodeflowscales[vu][k];
                            if nodeflowinverted[vu][k] {
                                rowweights[r] *= -1.0;
                            }
                            if separateflowcutset {
                                let rw = &scip.rows[row];
                                if nodeflowscales[vu][k] > 0.0 {
                                    baserhs += rowweights[r] * (rw.rhs - rw.constant);
                                } else {
                                    baserhs += rowweights[r] * (rw.lhs - rw.constant);
                                }
                            }
                        }
                    }
                }
            }

            // Try out deltas to generate c-MIR cuts: use larger deltas
            // first. Use best delta for flowcutset separation.
            let mut bestefficacy = crate::def::REAL_MIN;

            let ndeltas = deltas.len() as i32;
            if separateflowcutset && ndeltas > 0 {
                bestdelta = deltas[ndeltas as usize - 1]; // if nothing else is found, use maxdelta
            }

            oldncuts = *ncuts; // save number of cuts

            let mut success = crate::cuts::aggr_row_sum_rows(scip, &mut aggrrow, &rowweights, None, false, allowlocal, 2, maxaggrlen(nvars) as i32);

            if success {
                let mut d = ndeltas - 1;
                while d >= 0 && d >= ndeltas.saturating_sub(maxtestdelta) {
                    let mut cutrhs = 0.0;
                    let mut cutefficacy = 0.0;
                    let mut cutislocal = false;
                    let mut cutrank = 0;
                    let mut cutnnz = 0;

                    // We should not have too small deltas.
                    debug_assert!(!scip.set.is_feas_zero(deltas[d as usize]));
                    // We should not have too large deltas.
                    debug_assert!(!scip.set.is_zero(1.0 / deltas[d as usize]));

                    let out = crate::cuts::CutOut {
                        cutcoefs: &mut cutcoefs,
                        cutrhs: &mut cutrhs,
                        cutinds: &mut cutinds,
                        cutnnz: &mut cutnnz,
                        cutefficacy: Some(&mut cutefficacy),
                        cutrank: Some(&mut cutrank),
                        cutislocal: &mut cutislocal,
                    };
                    success = crate::cuts::calc_mir(
                        scip,
                        sol,
                        POSTPROCESS,
                        BOUNDSWITCH,
                        VARTYPEUSEVBDS,
                        allowlocal,
                        fixintegralrhs,
                        None,
                        None,
                        MINFRAC,
                        MAXFRAC,
                        1.0 / deltas[d as usize],
                        &aggrrow,
                        out,
                    )?;

                    // No success means row was too long or empty, there is a
                    // free variable or for numerical reasons, it does not
                    // mean that the cMIR cut was not violated.
                    if !success {
                        d -= 1;
                        continue;
                    }

                    if separateflowcutset && cutefficacy > bestefficacy {
                        bestdelta = deltas[d as usize];
                        bestefficacy = cutefficacy;
                    }

                    if api::is_efficacious(scip, cutefficacy) {
                        add_cut(scip, sepa, sol, &cutcoefs, cutrhs, &cutinds, cutnnz, cutislocal, cutrank, ncuts, cutoff)?;
                        if *cutoff {
                            break;
                        }
                    }
                    d -= 1;
                }
            }

            // Only separate flowcutset inequalities if no cutset
            // inequalities have been found.
            if separateflowcutset && oldncuts == *ncuts && !*cutoff {
                // Try to separate flow cuts for the best delta.
                let f0 = scip.set.frac(baserhs / bestdelta);
                if MINFRAC <= f0 && f0 <= MAXFRAC {
                    let mut totalviolationdelta = 0.0;
                    let onedivoneminsf0 = 1.0 / (1.0 - f0);
                    for a in 0..narcs {
                        // Arc might be uncapacitated.
                        let caprow = arccapacityrows[a];
                        if caprow == NONE {
                            continue;
                        }

                        let r = scip.rows[caprow].lppos;

                        // Row might have been removed from LP in the
                        // meantime.
                        if r == -1 {
                            continue;
                        }
                        let r = r as usize;

                        // Ignore rows that are not in the aggregation.
                        if rowweights[r] == 0.0 {
                            continue;
                        }

                        // Check if removing the capacity inequality will lead
                        // to a more violated MIR inequality: in a "perfect"
                        // MCF model, adding the capacity constraints means to
                        // cancel the flow variables of the capacity
                        // constraints and instead to add the capacity
                        // variables. Thus, removing it means to add the flow
                        // variables (with negative sign) and to remove the
                        // capacity variables.
                        let rw = &scip.rows[caprow];
                        let rowlen = rw.nlpcols as usize;
                        let rowweight = rowweights[r] / bestdelta;
                        let rowlhs = rw.lhs;
                        let rowrhs = rw.rhs;
                        let rowconstant = rw.constant;
                        let mut violationdelta = if scip.set.is_infinity(rowrhs) || (!scip.set.is_infinity(-rowlhs) && rowweight < 0.0) {
                            rowweight * (rowlhs - rowconstant)
                        } else {
                            rowweight * (rowrhs - rowconstant)
                        };

                        for j in 0..rowlen {
                            let mut coef = rw.vals[j] * rowweight;

                            let c = scip.cols[rw.cols[j]].lppos as usize;
                            let var = scip.cols[rw.cols[j]].var;

                            // Variable is flow variable: if we are not using
                            // the capacity constraint, this would appear with
                            // negative coefficient in the base inequality
                            // instead of being canceled. Variable is capacity
                            // variable: if we are not using the capacity
                            // constraint, this would not appear in the base
                            // inequality.
                            if colcommodity[c] >= 0 {
                                coef *= -1.0;
                            }

                            let mircoef = if crate::var::var_is_integral(scip, var) {
                                let fj = scip.set.frac(coef);
                                if fj <= f0 {
                                    scip.set.floor(coef)
                                } else {
                                    scip.set.floor(coef) + (fj - f0) * onedivoneminsf0
                                }
                            } else if coef >= 0.0 {
                                0.0
                            } else {
                                coef * onedivoneminsf0
                            };

                            // Add flow variable MIR coefficients, and
                            // subtract capacity variable MIR coefficients.
                            let solval = api::get_sol_val(scip, sol, var);
                            if colcommodity[c] >= 0 {
                                violationdelta += mircoef * solval;
                            } else {
                                violationdelta -= mircoef * solval;
                            }
                        }

                        if scip.set.is_positive(violationdelta) {
                            rowweights[r] = 0.0;
                            totalviolationdelta += violationdelta;
                        }
                    }

                    // If we removed a capacity constraint from the
                    // aggregation, try the new aggregation.
                    if totalviolationdelta > 0.0 {
                        let mut cutrhs = 0.0;
                        let mut cutefficacy = 0.0;
                        let mut cutislocal = false;
                        let mut cutnnz = 0;
                        let mut cutrank = 0;

                        success = crate::cuts::aggr_row_sum_rows(scip, &mut aggrrow, &rowweights, None, false, allowlocal, 2, maxaggrlen(nvars) as i32);

                        if success {
                            let out = crate::cuts::CutOut {
                                cutcoefs: &mut cutcoefs,
                                cutrhs: &mut cutrhs,
                                cutinds: &mut cutinds,
                                cutnnz: &mut cutnnz,
                                cutefficacy: Some(&mut cutefficacy),
                                cutrank: Some(&mut cutrank),
                                cutislocal: &mut cutislocal,
                            };
                            success = crate::cuts::calc_mir(
                                scip,
                                sol,
                                POSTPROCESS,
                                BOUNDSWITCH,
                                VARTYPEUSEVBDS,
                                allowlocal,
                                fixintegralrhs,
                                None,
                                None,
                                MINFRAC,
                                MAXFRAC,
                                1.0 / bestdelta,
                                &aggrrow,
                                out,
                            )?;

                            if success && api::is_efficacious(scip, cutefficacy) {
                                add_cut(scip, sepa, sol, &cutcoefs, cutrhs, &cutinds, cutnnz, cutislocal, cutrank, ncuts, cutoff)?;
                            }
                        }
                    }
                }
            }

            // inverted++ (the loop runs FALSE, then TRUE if useinverted).
            if inverted {
                break;
            }
            inverted = true;
            if !useinverted {
                break;
            }
        }
        partition += 1;
    }

    Ok(())
}

/// `separateCuts`: searches and adds MCF network cuts that separate the
/// given primal solution.
fn separate_cuts(scip: &mut Scip, sepa: SepaId, sol: SolId, allowlocal: bool, depth: i32, result: &mut ScipResultCode) -> ScipResult {
    debug_assert!(*result == ScipResultCode::DidNotRun);

    let mut ncuts = 0;
    let mut cutoff = false;

    // Check for number of columns, number of variables, column/row ratio.
    let nrows = lp_rows(scip).len() as i32;
    let ncols = n_lp_cols(scip);
    let nvars = api::get_n_vars(scip);

    // Exit if not all variables are in the LP (e.g. dynamic columns).
    if ncols != nvars {
        return Ok(());
    }

    // Exit if number of columns is too large (expensive detection).
    if ncols > MAXCOLS {
        return Ok(());
    }

    let colrowratio = ncols as f64 / (nrows as f64 + 1e-9);

    // If separation was not delayed before and we had no success in previous
    // round then delay the separation.
    if !scip.sepas[sepa].lpwasdelayed && !data(scip, sepa).lastroundsuccess {
        *result = ScipResultCode::Delayed;
        return Ok(());
    }

    if !(MINCOLROWRATIO..=MAXCOLROWRATIO).contains(&colrowratio) {
        return Ok(());
    }

    // Get or extract network flow structure.
    if data(scip, sepa).nmcfnetworks == -1 {
        *result = ScipResultCode::DidNotFind;

        let (mcfnetworks, effortlevel) = mcfnetwork_extract(scip, sepa)?;
        let d = data_mut(scip, sepa);
        d.nmcfnetworks = mcfnetworks.len() as i32;
        d.mcfnetworks = mcfnetworks;
        d.effortlevel = effortlevel;
    }
    debug_assert!(data(scip, sepa).nmcfnetworks >= 0);

    // Separation.
    let (nmcfnetworks, effortlevel, separatesinglenodecuts, nclusters) = {
        let d = data(scip, sepa);
        (d.nmcfnetworks, d.effortlevel, d.separatesinglenodecuts, d.nclusters)
    };
    if nmcfnetworks > 0 && effortlevel != EffortLevel::Off {
        // Separate cuts.
        *result = ScipResultCode::DidNotFind;
        data_mut(scip, sepa).lastroundsuccess = false;

        // The networks stay in the separator data; take them out while
        // separating so they can be borrowed alongside `scip`.
        let mcfnetworks = std::mem::take(&mut data_mut(scip, sepa).mcfnetworks);
        let mut ret = Ok(());
        let mut i = 0;
        while i < nmcfnetworks as usize && !cutoff {
            let mcfnetwork = &mcfnetworks[i];
            i += 1;

            // If the network does not have at least 2 nodes and 1 arc, we
            // did not create it.
            debug_assert!(mcfnetwork.nnodes >= 2);
            debug_assert!(mcfnetwork.narcs >= 1);

            let arcnoderatio = mcfnetwork.narcs as f64 / mcfnetwork.nnodes as f64;

            // Do not allow networks exceeding the arcs/nodes ratio ( =
            // average node degree / 2 (directed)).
            if arcnoderatio > MAXARCNODERATIO {
                continue;
            }

            // Enumerate single node cuts.
            if separatesinglenodecuts {
                ret = generate_cluster_cuts(scip, sepa, sol, allowlocal, depth, mcfnetwork, None, &mut ncuts, &mut cutoff);
                if ret.is_err() {
                    break;
                }
            }

            if !cutoff {
                // Partition nodes into a small number of clusters.
                let nodepartition = nodepartition_create(scip, mcfnetwork, if effortlevel == EffortLevel::Default { nclusters } else { 2 * nclusters });

                // Enumerate cuts between subsets of the clusters.
                ret = generate_cluster_cuts(scip, sepa, sol, allowlocal, depth, mcfnetwork, Some(&nodepartition), &mut ncuts, &mut cutoff);
                if ret.is_err() {
                    break;
                }
            }

            // Adjust result code.
            if cutoff {
                *result = ScipResultCode::Cutoff;
                data_mut(scip, sepa).lastroundsuccess = true;
            } else if ncuts > 0 {
                *result = ScipResultCode::Separated;
                data_mut(scip, sepa).lastroundsuccess = true;
            }
        }
        data_mut(scip, sepa).mcfnetworks = mcfnetworks;
        ret?;
    }

    Ok(())
}

/*
 * Callback methods of separator
 */

/// `sepaCopyMcf`: copy method for separator plugins.
fn sepa_copy_mcf(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    include_sepa_mcf(scip)
}

/// `sepaInitsolMcf`: solving process initialization method of separator.
fn sepa_initsol_mcf(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    let d = data_mut(scip, sepa);
    d.lastroundsuccess = true;
    d.effortlevel = EffortLevel::Off;
    Ok(())
}

/// `sepaExitsolMcf`: solving process deinitialization method of separator.
fn sepa_exitsol_mcf(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Free MCF networks.
    let mut mcfnetworks = std::mem::take(&mut data_mut(scip, sepa).mcfnetworks);
    for net in mcfnetworks.iter_mut() {
        mcfnetwork_free(scip, net)?;
    }
    data_mut(scip, sepa).nmcfnetworks = -1;
    Ok(())
}

/// `sepaExeclpMcf`: LP solution separation method of separator.
fn sepa_execlp_mcf(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // Only call separator, if there are fractional variables.
    if api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    // Separate cuts on the LP solution.
    separate_cuts(scip, sepa, NONE, allowlocal, depth, result)
}

/// `sepaExecsolMcf`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_mcf(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Separate cuts on the given primal solution.
    separate_cuts(scip, sepa, sol, allowlocal, depth, result)
}

/*
 * Separator specific interface methods
 */

/// `SCIPincludeSepaMcf`: creates the mcf separator and includes it in SCIP.
pub fn include_sepa_mcf(scip: &mut Scip) -> ScipResult {
    // Create mcf separator data.
    let sepadata = SepaData {
        nmcfnetworks: -1,
        lastroundsuccess: true,
        effortlevel: EffortLevel::Off,
        ..Default::default()
    };

    // Include separator.
    let sepa = api::include_sepa_basic(
        scip,
        SEPA_NAME,
        SEPA_DESC,
        SEPA_PRIORITY,
        SEPA_FREQ,
        SEPA_MAXBOUNDDIST,
        SEPA_USESSUBSCIP,
        SEPA_DELAY,
        Some(sepa_execlp_mcf),
        Some(sepa_execsol_mcf),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    scip.sepas[sepa].sepacopy = Some(sepa_copy_mcf);
    scip.sepas[sepa].sepainitsol = Some(sepa_initsol_mcf);
    scip.sepas[sepa].sepaexitsol = Some(sepa_exitsol_mcf);

    // Add mcf separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    let p = scip;
    paramset_add_int(
        p,
        "separating/mcf/nclusters",
        "number of clusters to generate in the shrunken network -- default separation",
        target!(i32, nclusters),
        true,
        DEFAULT_NCLUSTERS,
        2,
        (8 * std::mem::size_of::<u32>()) as i32,
        None,
    )?;
    paramset_add_real(
        p,
        "separating/mcf/maxweightrange",
        "maximal valid range max(|weights|)/min(|weights|) of row weights",
        target!(f64, maxweightrange),
        true,
        DEFAULT_MAXWEIGHTRANGE,
        1.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_int(
        p,
        "separating/mcf/maxtestdelta",
        "maximal number of different deltas to try (-1: unlimited)  -- default separation",
        target!(i32, maxtestdelta),
        true,
        DEFAULT_MAXTESTDELTA,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/trynegscaling",
        "should negative values also be tested in scaling?",
        target!(bool, trynegscaling),
        true,
        DEFAULT_TRYNEGSCALING,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/fixintegralrhs",
        "should an additional variable be complemented if f0 = 0?",
        target!(bool, fixintegralrhs),
        true,
        DEFAULT_FIXINTEGRALRHS,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/dynamiccuts",
        "should generated cuts be removed from the LP if they are no longer tight?",
        target!(bool, dynamiccuts),
        false,
        DEFAULT_DYNAMICCUTS,
        None,
    )?;
    paramset_add_int(
        p,
        "separating/mcf/modeltype",
        "model type of network (0: auto, 1:directed, 2:undirected)",
        target!(i32, modeltype),
        true,
        DEFAULT_MODELTYPE,
        0,
        2,
        None,
    )?;
    paramset_add_int(
        p,
        "separating/mcf/maxsepacuts",
        "maximal number of mcf cuts separated per separation round",
        target!(i32, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        p,
        "separating/mcf/maxsepacutsroot",
        "maximal number of mcf cuts separated per separation round in the root node  -- default separation",
        target!(i32, maxsepacutsroot),
        false,
        DEFAULT_MAXSEPACUTSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        p,
        "separating/mcf/maxinconsistencyratio",
        "maximum inconsistency ratio for separation at all",
        target!(f64, maxinconsistencyratio),
        true,
        DEFAULT_MAXINCONSISTENCYRATIO,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_real(
        p,
        "separating/mcf/maxarcinconsistencyratio",
        "maximum inconsistency ratio of arcs not to be deleted",
        target!(f64, maxarcinconsistencyratio),
        true,
        DEFAULT_MAXARCINCONSISTENCYRATIO,
        0.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/checkcutshoreconnectivity",
        "should we separate only if the cuts shores are connected?",
        target!(bool, checkcutshoreconnectivity),
        true,
        DEFAULT_CHECKCUTSHORECONNECTIVITY,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/separatesinglenodecuts",
        "should we separate inequalities based on single-node cuts?",
        target!(bool, separatesinglenodecuts),
        true,
        DEFAULT_SEPARATESINGLENODECUTS,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/separateflowcutset",
        "should we separate flowcutset inequalities on the network cuts?",
        target!(bool, separateflowcutset),
        true,
        DEFAULT_SEPARATEFLOWCUTSET,
        None,
    )?;
    paramset_add_bool(
        p,
        "separating/mcf/separateknapsack",
        "should we separate knapsack cover inequalities on the network cuts?",
        target!(bool, separateknapsack),
        true,
        DEFAULT_SEPARATEKNAPSACK,
        None,
    )?;

    Ok(())
}
