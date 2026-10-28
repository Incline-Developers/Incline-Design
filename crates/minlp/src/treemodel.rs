//! Port of `treemodel.c`: the parameters of the Treemodel branching rules
//! and their switch. The candidate selection itself
//! (`SCIPtreemodelSelectCandidate`) is not ported: `branching/treemodel/enable`
//! is off at the settings Improve uses.

use std::rc::Rc;

use crate::{
    paramset::{Target, paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_real},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{BranchruleId, VarId},
};

const DEFAULT_ENABLE: bool = false;
const DEFAULT_HIGHRULE: u8 = b'r';
const DEFAULT_LOWRULE: u8 = b'r';
const DEFAULT_HEIGHT: i32 = 10;
const DEFAULT_FILTERHIGH: u8 = b'a';
const DEFAULT_FILTERLOW: u8 = b'a';
const DEFAULT_MAXFPITER: i32 = 24;
const DEFAULT_MAXSVTSHEIGHT: i32 = 100;
const DEFAULT_FALLBACKINF: u8 = b'r';
const DEFAULT_FALLBACKNOPRIM: u8 = b'r';
const DEFAULT_SMALLPSCOST: f64 = 0.1;

/// `SCIP_TREEMODEL`: the Treemodel parameter data structure.
#[derive(Default)]
pub struct Treemodel {
    /// Should candidate branching variables be scored using the Treemodel
    /// rule?
    pub enabled: bool,
    /// Scoring function to use at nodes predicted to be high in the tree
    /// ('d'efault, 's'vts, 'r'atio, 't'ree sample).
    pub highrule: u8,
    /// Scoring function to use at nodes predicted to be low in the tree
    /// ('d'efault, 's'vts, 'r'atio, 't'ree sample).
    pub lowrule: u8,
    /// Estimated tree height at which we switch from using the low rule to
    /// the high rule.
    pub height: i32,
    /// Should dominated candidates be filtered before using the high scoring
    /// function? ('a'uto, 't'rue, 'f'alse)
    pub filterhigh: u8,
    /// Should dominated candidates be filtered before using the low scoring
    /// function? ('a'uto, 't'rue, 'f'alse)
    pub filterlow: u8,
    /// Maximum number of fixed-point iterations when computing the ratio.
    pub maxfpiter: i32,
    /// Maximum height to compute the SVTS score exactly before approximating.
    pub maxsvtsheight: i32,
    /// Which method should be used as a fallback if the tree size estimates
    /// are infinite? ('d'efault, 'r'atio)
    pub fallbackinf: u8,
    /// Which method should be used as a fallback if there is no primal bound
    /// available? ('d'efault, 'r'atio)
    pub fallbacknoprim: u8,
    /// Threshold at which pseudocosts are considered small, making hybrid
    /// scores more likely to be the deciding factor in branching.
    pub smallpscost: f64,
}

/// Where a branching rule keeps its [`Treemodel`]: the C pointer to it.
pub type TreemodelGet = fn(&Scip, BranchruleId) -> &Treemodel;
/// The mutable access of [`TreemodelGet`].
pub type TreemodelGetMut = fn(&mut Scip, BranchruleId) -> &mut Treemodel;

macro_rules! tm_target {
    ($ty:ty, $get:expr, $get_mut:expr, $id:expr, $field:ident) => {{
        let (get, get_mut, id): (TreemodelGet, TreemodelGetMut, BranchruleId) = ($get, $get_mut, $id);
        Some(Rc::new(Target::<$ty> {
            get: Box::new(move |scip: &Scip| &get(scip, id).$field),
            get_mut: Box::new(move |scip: &mut Scip| &mut get_mut(scip, id).$field),
        }))
    }};
}

/// `SCIPtreemodelInit`: initialises the Treemodel parameter data structure
/// (kept by the branching rule `branchrule`, reached through `get` and
/// `get_mut`).
pub fn treemodel_init(scip: &mut Scip, branchrule: BranchruleId, get: TreemodelGet, get_mut: TreemodelGetMut) -> ScipResult {
    paramset_add_bool(
        scip,
        "branching/treemodel/enable",
        "should candidate branching variables be scored using the Treemodel branching rules?",
        tm_target!(bool, get, get_mut, branchrule, enabled),
        false,
        DEFAULT_ENABLE,
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/highrule",
        "scoring function to use at nodes predicted to be high in the tree ('d'efault, 's'vts, 'r'atio, 't'ree sample)",
        tm_target!(u8, get, get_mut, branchrule, highrule),
        false,
        DEFAULT_HIGHRULE,
        Some("dsrt"),
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/lowrule",
        "scoring function to use at nodes predicted to be low in the tree ('d'efault, 's'vts, 'r'atio, 't'ree sample)",
        tm_target!(u8, get, get_mut, branchrule, lowrule),
        false,
        DEFAULT_LOWRULE,
        Some("dsrt"),
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/treemodel/height",
        "estimated tree height at which we switch from using the low rule to the high rule",
        tm_target!(i32, get, get_mut, branchrule, height),
        false,
        DEFAULT_HEIGHT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/filterhigh",
        "should dominated candidates be filtered before using the high scoring function? ('a'uto, 't'rue, 'f'alse)",
        tm_target!(u8, get, get_mut, branchrule, filterhigh),
        true,
        DEFAULT_FILTERHIGH,
        Some("atf"),
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/filterlow",
        "should dominated candidates be filtered before using the low scoring function? ('a'uto, 't'rue, 'f'alse)",
        tm_target!(u8, get, get_mut, branchrule, filterlow),
        true,
        DEFAULT_FILTERLOW,
        Some("atf"),
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/treemodel/maxfpiter",
        "maximum number of fixed-point iterations when computing the ratio",
        tm_target!(i32, get, get_mut, branchrule, maxfpiter),
        true,
        DEFAULT_MAXFPITER,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "branching/treemodel/maxsvtsheight",
        "maximum height to compute the SVTS score exactly before approximating",
        tm_target!(i32, get, get_mut, branchrule, maxsvtsheight),
        true,
        DEFAULT_MAXSVTSHEIGHT,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/fallbackinf",
        "which method should be used as a fallback if the tree size estimates are infinite? ('d'efault, 'r'atio)",
        tm_target!(u8, get, get_mut, branchrule, fallbackinf),
        true,
        DEFAULT_FALLBACKINF,
        Some("dr"),
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/treemodel/fallbacknoprim",
        "which method should be used as a fallback if there is no primal bound available? ('d'efault, 'r'atio)",
        tm_target!(u8, get, get_mut, branchrule, fallbacknoprim),
        true,
        DEFAULT_FALLBACKNOPRIM,
        Some("dr"),
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/treemodel/smallpscost",
        "threshold at which pseudocosts are considered small, making hybrid scores more likely to be the deciding factor in branching",
        tm_target!(f64, get, get_mut, branchrule, smallpscost),
        true,
        DEFAULT_SMALLPSCOST,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;

    Ok(())
}

/// `SCIPtreemodelIsEnabled`: returns TRUE if the Treemodel branching rules
/// are enabled.
pub fn treemodel_is_enabled(treemodel: &Treemodel) -> bool {
    treemodel.enabled
}

/// `SCIPtreemodelSelectCandidate`: apply the Treemodel branching rules to
/// attempt to select a better branching candidate than the one selected by
/// pseudocost branching. Not ported (see the module documentation).
pub fn treemodel_select_candidate(
    _scip: &mut Scip,
    _treemodel: &Treemodel,
    _branchcands: &[VarId],
    _mingains: &[f64],
    _maxgains: &[f64],
    _scoresfromothers: &[f64],
    _bestcand: &mut usize,
) -> ScipResult {
    Err(Retcode::NotImplemented)
}
