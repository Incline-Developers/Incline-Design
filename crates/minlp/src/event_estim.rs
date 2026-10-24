//! Port of `event_estim.c`: event handler for tree size estimation and
//! restarts.
//!
//! Not ported: reading a user regression forest (`estimation/regforestfilename`,
//! default "-", none) and the statistics table and display column, which
//! only print.

use std::{any::Any, collections::HashMap, rc::Rc};

use crate::{
    api,
    def::INVALID,
    event::{EVENTTYPE_NODEBRANCHED, EVENTTYPE_NODEDELETE, Event, EventType},
    paramset::Target,
    pqueue::PQueue,
    prob::{ORIGPROB, TRANSPROB},
    retcode::{Retcode, ScipResult},
    scip::{Scip, Stage},
    tree::tree_was_node_last_branch_parent,
    types::{EventhdlrId, NodeId, NodeType},
    var::NONE,
};

const EVENTHDLR_NAME: &str = "estim";
const EVENTHDLR_DESC: &str = "event handler for tree size estimation and restarts";
const EVENTTYPE_ESTIM: EventType = EVENTTYPE_NODEDELETE | EVENTTYPE_NODEBRANCHED;

/// Restart policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartPolicy {
    /// Never restart (disable this event handler).
    Never = 0,
    /// Always restart (can be fine tuned by using minimum number of nodes
    /// and restart limit).
    Always = 1,
    /// Base restart on the estimation method.
    Estimation = 2,
    /// Trigger restart based on search completion approximation.
    Completion = 3,
}

const RESTARTPOLICY_CHAR_NEVER: u8 = b'n';
const RESTARTPOLICY_CHAR_ALWAYS: u8 = b'a';
const RESTARTPOLICY_CHAR_COMPLETION: u8 = b'c';
const RESTARTPOLICY_CHAR_ESTIMATION: u8 = b'e';

/// Should the trend be used in the level update?
const DES_USETRENDINLEVEL: bool = true;

const INITIALSIZE: usize = 100;
/// Coefficient of single exponential smoothing of estimation.
const SESCOEFF: f64 = 0.75;

/* double exponential smoothing parameters for different time series */
const DES_ALPHA_TREEWEIGHT: f64 = 0.65;
const DES_BETA_TREEWEIGHT: f64 = 0.15;

const DES_ALPHA_GAP: f64 = 0.6;
const DES_BETA_GAP: f64 = 0.15;

const DES_ALPHA_LEAFFREQUENCY: f64 = 0.3;
const DES_BETA_LEAFFREQUENCY: f64 = 0.33;

const DES_ALPHA_SSG: f64 = 0.6;
const DES_BETA_SSG: f64 = 0.15;

const DES_ALPHA_OPENNODES: f64 = 0.6;
const DES_BETA_OPENNODES: f64 = 0.15;

/* computation of search completion */
/// Automatic (regression forest if available, else monotone regression on
/// binary and SSG on nonbinary trees).
const COMPLETIONTYPE_AUTO: u8 = b'a';
/// Regression forest (must be provided by user).
const COMPLETIONTYPE_REGFOREST: u8 = b'r';
/// Monotone regression (using tree weight and SSG).
const COMPLETIONTYPE_MONOREG: u8 = b'm';
/// Use tree weight value as approximation of search tree completion.
const COMPLETIONTYPE_TREEWEIGHT: u8 = b'w';
/// Use SSG value as approximation of search tree completion.
const COMPLETIONTYPE_SSG: u8 = b's';
/// Use gap value as approximation of search tree completion.
const COMPLETIONTYPE_GAP: u8 = b'g';

/* tree size estimation method */
/// Estimation based on projection of current search completion.
const ESTIMMETHOD_COMPL: u8 = b'c';
/// Weighted backtrack estimation.
const ESTIMMETHOD_WBE: u8 = b'b';
/// Estimation based on an ensemble of the individual estimations.
const ESTIMMETHOD_ENSMBL: u8 = b'e';
/// Estimation based on double exponential smoothing for open nodes.
const ESTIMMETHOD_GAP: u8 = b'g';
/// Estimation based on double exponential smoothing for leaf frequency.
const ESTIMMETHOD_LFREQ: u8 = b'l';
/// Estimation based on double exponential smoothing for open nodes.
const ESTIMMETHOD_OPEN: u8 = b'o';
/// Estimation based on double exponential smoothing for sum of subtree
/// gaps.
const ESTIMMETHOD_SSG: u8 = b's';
/// Estimation based on tree profile method.
const ESTIMMETHOD_TPROF: u8 = b't';
/// Estimation based on double exponential smoothing for tree weight.
const ESTIMMETHOD_TREEWEIGHT: u8 = b'w';

const ESTIMMETHODS: &str = "bceglostw";

/* constants and default values for treeprofile parameters */
/// Minimum size (depth) that tree profile can hold.
const TREEPROFILE_MINSIZE: i32 = 512;
/// Initial value of primal bound used within SSG.
const SSG_STARTPRIMBOUND: f64 = INVALID;

const NTIMESERIES: usize = 5;

/* time series position in event handler time series array */
/// Time series position of gap.
const TSPOS_GAP: usize = 0;
/// Time series position of tree weight.
const TSPOS_TREEWEIGHT: usize = 1;
/// Time series position of leaf frequency.
const TSPOS_LFREQ: usize = 2;
/// Time series position of SSG.
const TSPOS_SSG: usize = 3;
/// Time series position of open nodes.
const TSPOS_OPEN: usize = 4;

/* default values of user parameters */
/// Use leaf nodes as basic observations for time series, or all nodes?
const DEFAULT_USELEAFTS: bool = true;
/// Report frequency on estimation: -1: never, 0: always, k >= 1: k times
/// evenly during search.
const DEFAULT_REPORTFREQ: i32 = -1;
/// Default file name of user regression forest in RFCSV format.
const DEFAULT_REGFORESTFILENAME: &str = "-";
/// Coefficient of tree weight in monotone approximation of search
/// completion.
const DEFAULT_COEFMONOWEIGHT: f64 = 0.3667;
/// Coefficient of 1 - SSG in monotone approximation of search completion.
const DEFAULT_COEFMONOSSG: f64 = 0.6333;
/// Default computation of search tree completion.
const DEFAULT_COMPLETIONTYPE: u8 = COMPLETIONTYPE_AUTO;
/// Default tree size estimation method: (c)ompletion, (e)nsemble, time
/// series forecasts on either (g)ap, (l)eaf frequency, (o)open nodes, tree
/// (w)eight, (s)sg, or (t)ree profile or w(b)e.
const DEFAULT_ESTIMMETHOD: u8 = ESTIMMETHOD_TREEWEIGHT;
/// Should the event handler collect data?
const DEFAULT_TREEPROFILE_ENABLED: bool = false;
/// Minimum average number of nodes at each depth before producing
/// estimations.
const DEFAULT_TREEPROFILE_MINNODESPERDEPTH: f64 = 20.0;
/// Default restart policy: (a)lways, (c)ompletion, (e)stimation, (n)ever.
const DEFAULT_RESTARTPOLICY: u8 = b'e';
/// Default restart limit.
const DEFAULT_RESTARTLIMIT: i32 = 1;
/// Minimum number of nodes before restart.
const DEFAULT_MINNODES: i64 = 1000;
/// Should only leaves count for the minnodes parameter?
const DEFAULT_COUNTONLYLEAVES: bool = false;
/// Factor by which the estimated number of nodes should exceed the current
/// number of nodes.
const DEFAULT_RESTARTFACTOR: f64 = 50.0;
/// Whether to apply a restart when nonlinear constraints are present.
const DEFAULT_RESTARTNONLINEAR: bool = false;
/// Whether to apply a restart when active pricers are used.
const DEFAULT_RESTARTACTPRICERS: bool = false;
/// Limit on the number of successive samples to really trigger a restart.
const DEFAULT_HITCOUNTERLIM: i32 = 50;
/// The maximum number of individual SSG subtrees; the old split is kept if
/// a new split exceeds this number of subtrees; -1: no limit.
const DEFAULT_SSG_NMAXSUBTREES: i32 = -1;
/// Minimum number of nodes to process between two consecutive SSG splits.
const DEFAULT_SSG_NMINNODESLASTSPLIT: i64 = 0;
/// Should statistics be shown at the end?
const DEFAULT_SHOWSTATS: bool = false;

/// C's `long double` on x86-64 (the x87 80-bit format: 64-bit significand,
/// round to nearest even), which the tree weight is summed in. Only what
/// the tree weight needs: nonnegative values, addition, comparison with a
/// double and conversion to double.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LongDouble {
    /// Significand with its top bit set, or 0 for zero.
    m: u64,
    /// The value is `m * 2^e`.
    e: i32,
}

impl LongDouble {
    const ZERO: LongDouble = LongDouble { m: 0, e: 0 };

    /// Exact conversion of a nonnegative finite double.
    fn from_f64(x: f64) -> LongDouble {
        debug_assert!(x >= 0.0 && x.is_finite());
        if x == 0.0 {
            return LongDouble::ZERO;
        }
        let bits = x.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1u64 << 52) - 1);
        let (mant, exp) = if biased == 0 { (frac, -1074) } else { (frac | (1u64 << 52), biased - 1075) };
        let shift = mant.leading_zeros() as i32;
        LongDouble { m: mant << shift, e: exp - shift }
    }

    /// Rounds `s * 2^e` to a 64-bit significand, to nearest even.
    fn round_from(s: u128, e: i32) -> LongDouble {
        if s == 0 {
            return LongDouble::ZERO;
        }
        let nbits = 128 - s.leading_zeros() as i32;
        if nbits <= 64 {
            let shift = 64 - nbits;
            return LongDouble {
                m: (s << shift) as u64,
                e: e - shift,
            };
        }
        let shift = nbits - 64;
        let mut q = s >> shift;
        let rem = s & ((1u128 << shift) - 1);
        let half = 1u128 << (shift - 1);
        let mut e = e + shift;
        if rem > half || (rem == half && q & 1 == 1) {
            q += 1;
            if q == 1u128 << 64 {
                q >>= 1;
                e += 1;
            }
        }
        LongDouble { m: q as u64, e }
    }

    /// `self + other` for nonnegative values.
    fn add(self, other: LongDouble) -> LongDouble {
        if self.m == 0 {
            return other;
        }
        if other.m == 0 {
            return self;
        }
        let (big, small) = if self.e >= other.e { (self, other) } else { (other, self) };
        let d = big.e - small.e;
        // The smaller summand is below half an ulp of the larger one.
        if d >= 65 {
            return big;
        }
        let s = ((big.m as u128) << d) + small.m as u128;
        LongDouble::round_from(s, small.e)
    }

    /// `self += x` for a nonnegative double `x`.
    fn add_f64(&mut self, x: f64) {
        *self = self.add(LongDouble::from_f64(x));
    }

    /// Conversion to double, rounding to nearest even.
    fn to_f64(self) -> f64 {
        if self.m == 0 {
            return 0.0;
        }
        let mut q = self.m >> 11;
        let rem = self.m & 0x7ff;
        let mut e = self.e + 11;
        if rem > 0x400 || (rem == 0x400 && q & 1 == 1) {
            q += 1;
            if q == 1u64 << 53 {
                q >>= 1;
                e += 1;
            }
        }
        // q < 2^53 is exact; scale by 2^e in two steps that stay normal.
        let half = e / 2;
        (q as f64) * 2f64.powi(half) * 2f64.powi(e - half)
    }

    /// Three-way comparison with a nonnegative double, exactly (C promotes
    /// the double to `long double`).
    fn cmp_f64(self, x: f64) -> std::cmp::Ordering {
        let y = LongDouble::from_f64(x);
        match (self.m == 0, y.m == 0) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => (self.e, self.m).cmp(&(y.e, y.m)),
        }
    }

    fn le(self, x: f64) -> bool {
        self.cmp_f64(x) != std::cmp::Ordering::Greater
    }

    fn ge(self, x: f64) -> bool {
        self.cmp_f64(x) != std::cmp::Ordering::Less
    }
}

/// Double exponential smoothing data structure.
#[derive(Clone, Copy, Debug, Default)]
struct DoubleExpSmooth {
    /// Level smoothing constant.
    alpha: f64,
    /// Trend smoothing constant.
    beta: f64,
    /// Estimation of the current level used for smoothing.
    level: f64,
    /// Estimation of the current trend (slope).
    trend: f64,
    /// The level value at 0 observations.
    initialvalue: f64,
    /// Should the trend be used in the level update?
    usetrendinlevel: bool,
    /// Number of observations.
    n: i32,
}

/// The update callback of a time series (`DECL_TIMESERIESUPDATE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimeSeriesUpdate {
    Gap,
    TreeWeight,
    LeafFreq,
    Ssg,
    OpenNodes,
}

/// Time series data structure for leaf time series.
///
/// These time series are the basic ingredient for tree size estimation via
/// forecasting. This general class represents concrete time series such as
/// the closed gap, tree weight, and leaf frequency. Through callbacks for
/// data (de-)initialization and value queries, it provides a common
/// interface to which double exponential smoothing or window forecasts can
/// be applied.
#[derive(Clone, Debug)]
struct TimeSeries {
    /// Double exponential smoothing data structure.
    des: DoubleExpSmooth,
    /// Name of this time series.
    name: String,
    /// Value array of this time series.
    vals: Vec<f64>,
    /// Array of estimations of this time series.
    estimation: Vec<f64>,
    /// Smoothened estimation value.
    smoothestimation: f64,
    /// Target value of this time series.
    targetvalue: f64,
    /// Current value of time series.
    currentvalue: f64,
    /// The initial value of time series.
    initialvalue: f64,
    /// Total number of observations.
    nobs: i64,
    /// Size of value array.
    valssize: i32,
    /// Number of values.
    nvals: i32,
    /// Current (inverse of) resolution.
    resolution: i32,
    /// Should this time series be recorded at leaf nodes, or at every node?
    useleafts: bool,
    /// Update callback at nodes.
    timeseriesupdate: TimeSeriesUpdate,
}

/// Statistics collected from profile used for prediction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TreeProfileStats {
    /// Maximum node depth encountered.
    maxdepth: i32,
    /// Deepest layer for which all nodes have been explored.
    lastfulldepth: i32,
    /// Minimum depth of the waist, i.e. the widest part of the tree.
    minwaistdepth: i32,
    /// Maximum depth of the waist, i.e. the widest part of the tree.
    maxwaistdepth: i32,
}

/// Profile data structure for tree.
#[derive(Clone, Debug, Default)]
struct TreeProfile {
    /// Array to store the tree profile.
    profile: Vec<i64>,
    /// Size of the profile array.
    profilesize: i32,
    /// Statistics collected from profile used for prediction.
    stats: TreeProfileStats,
    /// The last estimate predicted by predictTotalSizeTreeprofile().
    lastestimate: f64,
    /// Tree profile statistics at last estimation.
    lastestimatestats: TreeProfileStats,
}

/// Extended node information for SSG priority queue; the lower bounds and
/// positions live in [`SubtreeSumGap`]'s parallel arrays, indexed like this
/// (C's node pointer in it is only printed).
#[derive(Clone, Copy, Debug)]
struct NodeInfo {
    /// Subtree index of this node.
    subtreeidx: i32,
}

/// Subtree sum gap data structure.
#[derive(Debug, Default)]
struct SubtreeSumGap {
    /// The current subtree sum gap.
    value: f64,
    /// Map between nodes and their subtree indices (into `infos`).
    nodes2info: HashMap<NodeId, usize>,
    /// Array of priority queues, one for each subtree.
    subtreepqueues: Option<Vec<Option<PQueue<usize>>>>,
    /// The current scaling factor.
    scalingfactor: f64,
    /// Primal bound when last split occurred.
    pblastsplit: f64,
    /// Last node at which a subtree split occurred.
    nodelastsplit: i64,
    /// Minimum number of nodes to process between two consecutive SSG
    /// splits.
    nminnodeslastsplit: i64,
    /// The maximum number of individual SSG subtrees; the old split is kept
    /// if a new split exceeds this number of subtrees; -1: no limit.
    nmaxsubtrees: i32,
    /// The current number n of subtrees labeled 0 .. n - 1.
    nsubtrees: i32,
    /// Node infos (C allocates each in block memory).
    infos: Vec<NodeInfo>,
    /// Lower bound of each node info's node at insertion into the priority
    /// queue.
    infolbs: Vec<f64>,
    /// Position of each node info in its priority queue.
    infopos: Vec<i32>,
}

/// Data structure for convenient access of tree information.
#[derive(Debug, Default)]
struct TreeData {
    /// The total number of nodes.
    nnodes: i64,
    /// The current number of open nodes.
    nopen: i64,
    /// The number of inner nodes.
    ninner: i64,
    /// The number of final leaf nodes.
    nleaves: i64,
    /// The number of visited nodes.
    nvisited: i64,
    /// The current tree weight (sum of leaf weights).
    weight: LongDouble,
    /// Subtree sum gap data structure.
    ssg: SubtreeSumGap,
}

/// Event handler data.
struct EventhdlrData {
    /// Array of time series slots.
    timeseries: Vec<TimeSeries>,
    /// Tree data.
    treedata: TreeData,
    /// Tree profile data.
    treeprofile: Option<TreeProfile>,
    /// File name of user regression forest in RFCSV format.
    regforestfilename: String,
    /// Factor by which the estimated number of nodes should exceed the
    /// current number of nodes.
    restartfactor: f64,
    /// Tree weight at which last report was printed.
    weightlastreport: f64,
    /// Minimum average number of nodes at each depth before producing
    /// estimations.
    treeprofile_minnodesperdepth: f64,
    /// Coefficient of tree weight in monotone approximation of search
    /// completion.
    coefmonoweight: f64,
    /// Coefficient of 1 - SSG in monotone approximation of search
    /// completion.
    coefmonossg: f64,
    /// Minimum number of nodes in a run before restart is triggered.
    minnodes: i64,
    /// How often should a restart be triggered? (-1 for no limit)
    restartlimit: i32,
    /// Number of restarts performed so far.
    nrestartsperformed: i32,
    /// The number of successive samples that would trigger a restart.
    restarthitcounter: i32,
    /// Limit on the number of successive samples to really trigger a
    /// restart.
    hitcounterlim: i32,
    /// The number of reports already printed.
    nreports: i32,
    /// Report frequency on estimation: -1: never, 0:always, k >= 1: k times
    /// evenly during search.
    reportfreq: i32,
    /// The last run at which this event handler triggered restart.
    lastrestartrun: i32,
    /// Restart policy parameter.
    restartpolicyparam: u8,
    /// Tree size estimation method.
    estimmethod: u8,
    /// Approximation of search tree completion: (a)uto, (g)ap, tree
    /// (w)eight, (m)onotone regression, (r)egression forest, (s)sg.
    completiontypeparam: u8,
    /// Should only leaves count for the minnodes parameter?
    countonlyleaves: bool,
    /// Use leaf nodes as basic observations for time series, or all nodes?
    useleafts: bool,
    /// Should the event handler collect treeprofile data?
    treeprofile_enabled: bool,
    /// Internal flag if all branching decisions produced 2 children.
    treeisbinary: bool,
    /// Whether to apply a restart when nonlinear constraints are present.
    restartnonlinear: bool,
    /// Whether to apply a restart when active pricers are used.
    restartactpricers: bool,
    /// Should statistics be shown at the end?
    showstats: bool,
}

fn edm(scip: &mut Scip, eventhdlr: EventhdlrId) -> &mut EventhdlrData {
    scip.eventhdlrs[eventhdlr].eventhdlrdata.as_mut().unwrap().downcast_mut::<EventhdlrData>().unwrap()
}

/// Takes the handler data out while code that also needs `&mut Scip` runs.
fn take_data(scip: &mut Scip, eventhdlr: EventhdlrId) -> Box<EventhdlrData> {
    scip.eventhdlrs[eventhdlr].eventhdlrdata.take().unwrap().downcast::<EventhdlrData>().unwrap()
}

fn put_data(scip: &mut Scip, eventhdlr: EventhdlrId, data: Box<EventhdlrData>) {
    scip.eventhdlrs[eventhdlr].eventhdlrdata = Some(data as Box<dyn Any>);
}

/*
 * Local methods
 */

/// `real2String`: convert number to string and treat SCIP_INVALID as '-'.
fn real2string(num: f64, digits: usize) -> String {
    if num == INVALID {
        // C: SCIPsnprintf(buf, 1, "-") leaves the buffer empty.
        String::new()
    } else if num >= 1e+20 {
        // C: SCIPsnprintf(buf, 3, "inf") keeps "in".
        "in".to_string()
    } else {
        format!("{:10.*}", digits, num)
    }
}

/// `extendMemoryTreeProfile`: extend tree profile to deeper tree.
fn extend_memory_tree_profile(scip: &Scip, treeprofile: &mut TreeProfile, mindepth: i32) {
    if mindepth < treeprofile.profilesize {
        return;
    }

    if treeprofile.profile.is_empty() {
        treeprofile.profile = vec![0; mindepth as usize];
        treeprofile.profilesize = mindepth;
    } else {
        let newsize = scip.set.calc_mem_grow_size(mindepth + 1);
        treeprofile.profile.resize(newsize as usize, 0);
        treeprofile.profilesize = newsize;
    }
}

/// `createTreeProfile`: create a tree profile.
fn create_tree_profile(scip: &Scip) -> TreeProfile {
    let mut treeprofile = TreeProfile {
        profile: Vec::new(),
        profilesize: 0,
        ..Default::default()
    };
    extend_memory_tree_profile(scip, &mut treeprofile, TREEPROFILE_MINSIZE);

    treeprofile.stats = TreeProfileStats::default();
    treeprofile.lastestimatestats = TreeProfileStats::default();

    treeprofile.lastestimate = -1.0;

    treeprofile
}

/// `updateTreeProfile`: update tree profile.
fn update_tree_profile(scip: &Scip, treeprofile: Option<&mut TreeProfile>, node: NodeId) {
    let Some(treeprofile) = treeprofile else {
        return;
    };

    let nodedepth = scip.nodes[node].depth as i32;
    let maxnodes = treeprofile.profile[treeprofile.stats.minwaistdepth as usize];

    // Ensure that the memory can hold at least this depth.
    extend_memory_tree_profile(scip, treeprofile, nodedepth);

    treeprofile.profile[nodedepth as usize] += 1;
    let nodedepthcnt = treeprofile.profile[nodedepth as usize];

    // Is this level fully explored? We assume binary branching. The first
    // condition ensures that the bit shift operation of the second condition
    // represents a feasible power of unsigned int. The largest power of 2
    // representable by unsigned int is 2^{8*sizeof(unsigned int) - 1}.
    let nbits = 8 * std::mem::size_of::<u32>() as u32;
    if (nodedepth as u32) < nbits && nodedepthcnt == (1u32 << nodedepth) as i64 {
        treeprofile.stats.lastfulldepth = nodedepth;
    }

    // Update maximum depth.
    if treeprofile.stats.maxdepth < nodedepth {
        treeprofile.stats.maxdepth = nodedepth;
    }

    // Minimum and maximum waist now coincide.
    if nodedepthcnt > maxnodes {
        treeprofile.stats.minwaistdepth = nodedepth;
        treeprofile.stats.maxwaistdepth = nodedepth;
    } else if nodedepthcnt == maxnodes {
        // Enlarge the interval in which the waist lies.
        if treeprofile.stats.minwaistdepth > nodedepth {
            treeprofile.stats.minwaistdepth = nodedepth;
        } else if treeprofile.stats.maxwaistdepth < nodedepth {
            treeprofile.stats.maxwaistdepth = nodedepth;
        }
    }
}

/// `predictTotalSizeTreeProfile`: make a prediction of the total tree size
/// based on the current tree profile.
fn predict_total_size_tree_profile(scip: &Scip, treeprofile: Option<&mut TreeProfile>, minnodesperdepth: f64) -> f64 {
    // Prediction is disabled.
    let Some(treeprofile) = treeprofile else {
        return -1.0;
    };

    // Two few nodes to make a prediction.
    if minnodesperdepth * treeprofile.stats.maxdepth as f64 > api::get_n_nodes(scip) as f64 {
        return -1.0;
    }

    // Reuse previous estimation if tree profile hasn't changed.
    if treeprofile.lastestimatestats == treeprofile.stats {
        return treeprofile.lastestimate;
    }

    // Compute the (depth of the) waist as convex combination between the
    // minimum and maximum waist depths.
    let s = treeprofile.stats;
    let waist = (2 * s.maxwaistdepth + s.minwaistdepth) / 3;

    let mut growthfac = 2.0;
    let mut estimate = 1.0;

    // Loop over all full levels.
    let mut d = 1;
    while d < s.lastfulldepth {
        let gamma_d = 2.0;

        estimate += growthfac;
        growthfac *= gamma_d;
        d += 1;
    }

    // Loop until the waist is reached.
    while d < waist {
        let gamma_d = 2.0 - (d as f64 - s.lastfulldepth as f64 + 1.0) / (waist as f64 - s.lastfulldepth as f64 + 1.0);

        estimate += growthfac;
        growthfac *= gamma_d;
        d += 1;
    }

    // Loop over the remaining levels.
    while d <= s.maxdepth {
        let gamma_d = 1.0 - (d as f64 - waist as f64 + 1.0) / (s.maxdepth as f64 - waist as f64 + 1.0);

        estimate += growthfac;
        growthfac *= gamma_d;
        d += 1;
    }

    // Copy tree profile statistics.
    treeprofile.lastestimatestats = treeprofile.stats;

    treeprofile.lastestimate = estimate;

    estimate
}

/// `subtreeSumGapDelSubtrees`: clean subtrees stored as priority queues.
fn subtree_sum_gap_del_subtrees(ssg: &mut SubtreeSumGap) {
    // Free all previous priority queues (with the node infos in them; the
    // map from nodes to them has been cleared by every caller).
    if ssg.nsubtrees > 1 {
        ssg.infos.clear();
        ssg.infolbs.clear();
        ssg.infopos.clear();
    }

    ssg.subtreepqueues = None;
}

/// `subtreeSumGapReset`: reset subtree sum gap.
fn subtree_sum_gap_reset(ssg: &mut SubtreeSumGap) {
    ssg.nodes2info.clear();

    subtree_sum_gap_del_subtrees(ssg);

    ssg.value = 1.0;
    ssg.scalingfactor = 1.0;
    ssg.nsubtrees = 1;
    ssg.subtreepqueues = None;
    ssg.pblastsplit = SSG_STARTPRIMBOUND;
    ssg.nodelastsplit = -1;
}

/// `subtreeSumGapCreate`: create a subtree sum gap.
fn subtree_sum_gap_create() -> SubtreeSumGap {
    let mut ssg = SubtreeSumGap {
        nodes2info: HashMap::with_capacity(INITIALSIZE),
        ..Default::default()
    };

    // Explicitly set this to skip removal of subtrees during reset.
    ssg.nsubtrees = 0;

    // Reset ssg.
    subtree_sum_gap_reset(&mut ssg);

    ssg
}

/// `compareNodeInfos`: compare two node infos by comparing their lower
/// bound.
fn compare_node_infos(lbs: &[f64], a: usize, b: usize) -> i32 {
    if lbs[a] < lbs[b] {
        -1
    } else if lbs[a] > lbs[b] {
        1
    } else {
        0
    }
}

/// `subtreeSumGapStoreNode`: store node in SSG data structure.
fn subtree_sum_gap_store_node(scip: &Scip, ssg: &mut SubtreeSumGap, node: NodeId, subtreeidx: i32) {
    // Create a new node info; store node information in data structure and
    // insert into priority queue.
    let info = ssg.infos.len();
    ssg.infos.push(NodeInfo { subtreeidx });
    ssg.infolbs.push(scip.nodes[node].lowerbound);
    ssg.infopos.push(-1);

    // Store node information in Hash Map.
    debug_assert!(!ssg.nodes2info.contains_key(&node));
    ssg.nodes2info.insert(node, info);

    // Create the corresponding priority queue, if it does not exist yet.
    let queues = ssg.subtreepqueues.as_mut().unwrap();
    let pq = queues[subtreeidx as usize].get_or_insert_with(|| PQueue::new(5));

    // Insert node and ensure that its position is up to date.
    let (lbs, poss) = (&ssg.infolbs, &mut ssg.infopos);
    pq.insert(info, |&a, &b| compare_node_infos(lbs, a, b), |e, _old, new| poss[e] = new as i32);
}

/// `subtreeSumGapSplit`: split the open nodes of the current tree.
fn subtree_sum_gap_split(scip: &Scip, ssg: &mut SubtreeSumGap, addfocusnode: bool) {
    // Query the open nodes of SCIP: leaves, children, siblings.
    let tree = scip.tree.as_ref().unwrap();
    let opennodes: [Vec<NodeId>; 3] = [
        crate::nodesel::nodepq_nodes(&tree.leaves).to_vec(),
        tree.children.0[..tree.nchildren as usize].to_vec(),
        tree.siblings.0[..tree.nsiblings as usize].to_vec(),
    ];

    let nnewsubtrees = (opennodes[0].len() + opennodes[1].len() + opennodes[2].len()) as i32 + if addfocusnode { 1 } else { 0 };

    // Clear hash map from entries.
    ssg.nodes2info.clear();

    // Delete all subtrees.
    subtree_sum_gap_del_subtrees(ssg);

    ssg.nsubtrees = nnewsubtrees;

    // Create priority queue array.
    if ssg.nsubtrees > 1 {
        ssg.subtreepqueues = Some((0..ssg.nsubtrees).map(|_| None).collect());
    } else {
        ssg.subtreepqueues = None;

        return;
    }

    // Loop over node types (leaves, siblings, children).
    let mut label = 0;
    for nodes in &opennodes {
        // Label each open node as new, separate subtree.
        for &node in nodes {
            subtree_sum_gap_store_node(scip, ssg, node, label);
            label += 1;
        }
    }

    if addfocusnode {
        let focusnode = tree.focusnode;
        subtree_sum_gap_store_node(scip, ssg, focusnode, label);
    }
}

/// `calcGap`: compute a gap between a lower bound and the current upper
/// bound.
fn calc_gap(scip: &Scip, lowerbound: f64) -> f64 {
    if scip.set.is_infinity(lowerbound) || lowerbound >= api::get_upperbound(scip) {
        return 0.0;
    }

    if scip.set.is_infinity(api::get_upperbound(scip)) {
        return 1.0;
    }

    let db = crate::prob::prob_extern_objval(scip, TRANSPROB, ORIGPROB, lowerbound);
    let pb = api::get_primalbound(scip);

    if scip.set.is_eq(db, pb) {
        return 0.0;
    }

    let abspb = pb.abs();
    let absdb = db.abs();
    let mut gap = (pb - db).abs() / if abspb >= absdb { abspb } else { absdb };
    gap = if gap <= 1.0 { gap } else { 1.0 };

    gap
}

/// `subtreeSumGapRemoveNode`: remove node from the subtree sum gap (because
/// it has been solved by branching or is a leaf).
fn subtree_sum_gap_remove_node(scip: &Scip, ssg: &mut SubtreeSumGap, mut node: NodeId) {
    if ssg.nsubtrees <= 1 {
        return;
    }

    let mut info = ssg.nodes2info.get(&node).copied();

    // It can happen that the node was not created via branching; search for
    // the most recent ancestor in the queue.
    if info.is_none() {
        loop {
            node = scip.nodes[node].parent;
            if node == NONE {
                break;
            }
            info = ssg.nodes2info.get(&node).copied();
            if info.is_some() {
                break;
            }
        }

        // No ancestor found.
        if info.is_none() {
            return;
        }
    }
    let info = info.unwrap();

    // Get open nodes of this subtree stored as priority queue.
    let subtreeidx = ssg.infos[info].subtreeidx as usize;

    // Delete the element from the priority queue.
    let pos = ssg.infopos[info];
    {
        let pq = ssg.subtreepqueues.as_mut().unwrap()[subtreeidx].as_mut().unwrap();
        let (lbs, poss) = (&ssg.infolbs, &mut ssg.infopos);
        pq.del_pos(pos as usize, |&a, &b| compare_node_infos(lbs, a, b), |e, _old, new| poss[e] = new as i32);
    }

    // Update ssg if removed node was the lower bound defining node of its
    // subtree.
    if pos == 0 {
        let oldgap = calc_gap(scip, ssg.infolbs[info]);
        let first = ssg.subtreepqueues.as_ref().unwrap()[subtreeidx].as_ref().unwrap().first();
        let newgap = calc_gap(scip, if let Some(f) = first { ssg.infolbs[f] } else { scip.set.infinity() });

        // The SSG value is always up-to-date because it is recomputed when
        // the primal bound changes.
        let d = newgap - oldgap;
        ssg.value += ssg.scalingfactor * if d <= 0.0 { d } else { 0.0 };
    }

    ssg.nodes2info.remove(&node);
}

/// `subtreeSumGapInsertChildren`: insert children into subtree sum gap.
fn subtree_sum_gap_insert_children(scip: &Scip, ssg: &mut SubtreeSumGap) {
    if ssg.nsubtrees == 1 {
        return;
    }

    let tree = scip.tree.as_ref().unwrap();
    let nchildren = tree.nchildren as usize;
    if nchildren == 0 {
        return;
    }
    let children = tree.children.0[..nchildren].to_vec();

    let focusnode = tree.focusnode;

    // A rare case: the search has been stopped at some point, and the
    // current focus node is the only descendant of its parent node.
    let parentnode = if !ssg.nodes2info.contains_key(&focusnode) {
        let mut parentnode = focusnode;
        loop {
            parentnode = scip.nodes[parentnode].parent;
            if parentnode == NONE || ssg.nodes2info.contains_key(&parentnode) {
                break;
            }
        }
        parentnode
    } else {
        focusnode
    };

    let parentnodeinfo = ssg.nodes2info[&parentnode];
    let parentnodelabel = ssg.infos[parentnodeinfo].subtreeidx;

    // Loop over children and insert the focus node label.
    for &child in &children {
        subtree_sum_gap_store_node(scip, ssg, child, parentnodelabel);
    }

    // Remove focus node from hash map.
    subtree_sum_gap_remove_node(scip, ssg, parentnode);
}

/// `subtreeSumGapComputeFromScratchEfficiently`: compute subtree sum gap
/// from scratch efficiently (linear effort in the number of subtrees).
fn subtree_sum_gap_compute_from_scratch_efficiently(scip: &Scip, ssg: &mut SubtreeSumGap, updatescaling: bool) {
    let mut gapsum = 0.0;

    // Treat trivial cases: only 1 subtree, no incumbent solution.
    if scip.set.is_infinity(api::get_upperbound(scip)) {
        ssg.value = 1.0;

        return;
    }

    if ssg.nsubtrees == 1 {
        ssg.value = calc_gap(scip, api::get_lowerbound(scip));

        return;
    }

    // Compute subtree gaps in original space; sum them up.
    for l in 0..ssg.nsubtrees as usize {
        let first = ssg.subtreepqueues.as_ref().unwrap()[l].as_ref().unwrap().first();

        // Skip subtrees with infinite lower bound; they are empty and
        // contribute 0.0 to the gap sum term.
        let Some(info) = first else {
            continue;
        };
        if scip.set.is_infinity(ssg.infolbs[info]) {
            continue;
        }

        let subtreegap = calc_gap(scip, ssg.infolbs[info]);

        gapsum += subtreegap;
    }

    // Update the scaling factor by using the previous SSG value divided by
    // the current gapsum.
    if updatescaling {
        ssg.scalingfactor = ssg.value / if gapsum >= 1e-6 { gapsum } else { 1e-6 };
    }

    // Update and store SSG value by considering scaling factor.
    ssg.value = ssg.scalingfactor * gapsum;
}

/// `subtreeSumGapUpdate`: update the subtree sum gap after a node event
/// (branching or deletion of a node).
fn subtree_sum_gap_update(scip: &mut Scip, ssg: &mut SubtreeSumGap, node: NodeId, nchildren: i32, nsolvednodes: i64) {
    let insertchildren = ssg.nsubtrees > 1 && nchildren > 0;

    // If the instance is solved or a node is cutoff at the initsolve stage
    // or we are unbounded, the ssg is 0.
    if scip.set.stage == Stage::Solved || scip.set.stage == Stage::InitSolve || scip.set.is_infinity(-api::get_upperbound(scip)) {
        ssg.value = 0.0;

        return;
    }

    // Make a new tree split if the primal bound has changed.
    if !scip.set.is_infinity(api::get_upperbound(scip)) && !scip.set.is_eq(api::get_primalbound(scip), ssg.pblastsplit) {
        let focusnode = scip.tree.as_ref().unwrap().focusnode;
        let nchildrenscip = crate::tree::tree_get_n_children(scip);
        let addfocusnode = focusnode != NONE && nchildrenscip == 0 && !tree_was_node_last_branch_parent(scip, focusnode);
        let nnewsubtrees = crate::tree::tree_get_n_siblings(scip) + crate::tree::tree_get_n_leaves(scip) + nchildrenscip + if addfocusnode { 1 } else { 0 };

        // Check if number of new subtrees does not exceed maximum number of
        // subtrees; always split if no split happened, yet.
        if ssg.nsubtrees <= 1 || ((ssg.nmaxsubtrees == -1 || nnewsubtrees <= ssg.nmaxsubtrees) && (nsolvednodes - ssg.nodelastsplit >= ssg.nminnodeslastsplit)) {
            subtree_sum_gap_split(scip, ssg, addfocusnode);

            // Remember time stamp.
            ssg.nodelastsplit = nsolvednodes;
        } else {
            // No new split has happened; insert the new children to their
            // SSG subtree.
            if insertchildren {
                subtree_sum_gap_insert_children(scip, ssg);
            }
        }

        ssg.pblastsplit = api::get_primalbound(scip);

        // Compute the current SSG value from scratch.
        subtree_sum_gap_compute_from_scratch_efficiently(scip, ssg, true);
    }
    // Otherwise, if new children have been created, label them.
    else if insertchildren {
        subtree_sum_gap_insert_children(scip, ssg);
    }

    // Remove the node from the hash map if it is a leaf.
    if nchildren == 0 {
        subtree_sum_gap_remove_node(scip, ssg, node);
    }
}

/// `resetTreeData`: reset tree data.
fn reset_tree_data(treedata: &mut TreeData) {
    // Simply set everything to 0.
    treedata.ninner = 0;
    treedata.nleaves = 0;
    treedata.nvisited = 0;
    treedata.weight = LongDouble::ZERO;

    // Set up root node.
    treedata.nnodes = 1;
    treedata.nopen = 1;

    subtree_sum_gap_reset(&mut treedata.ssg);
}

/// `createTreeData`: create tree data structure.
fn create_tree_data() -> TreeData {
    let mut treedata = TreeData {
        ssg: subtree_sum_gap_create(),
        ..Default::default()
    };

    reset_tree_data(&mut treedata);

    treedata
}

/// `updateTreeData`: update tree data structure after a node has been
/// solved/is about to be deleted.
fn update_tree_data(scip: &mut Scip, treedata: &mut TreeData, node: NodeId, nchildren: i32) {
    treedata.nvisited += 1;
    treedata.nopen -= 1;

    if nchildren == 0 {
        let depth = scip.nodes[node].depth as i32;
        treedata.nleaves += 1;
        treedata.weight.add_f64(0.5f64.powf(depth as f64));
    } else {
        treedata.nnodes += nchildren as i64;
        treedata.nopen += nchildren as i64;
        treedata.ninner += 1;
    }

    // Update the subtree sum gap.
    if !scip.stat.inrestart {
        let nvisited = treedata.nvisited;
        subtree_sum_gap_update(scip, &mut treedata.ssg, node, nchildren, nvisited);
    }
}

/// `treeDataGetWbe`: get weighted backtrack estimation from this tree data.
fn tree_data_get_wbe(treedata: &TreeData) -> f64 {
    if treedata.weight.le(0.0) || treedata.nleaves == 0 {
        return -1.0;
    }

    2.0 * treedata.nleaves as f64 / treedata.weight.to_f64() - 1.0
}

/// `doubleExpSmoothReset`: reset double exponential smoothing.
fn double_exp_smooth_reset(des: &mut DoubleExpSmooth, initialvalue: f64) {
    des.n = 0;
    des.level = INVALID;
    des.trend = INVALID;
    des.initialvalue = initialvalue;
}

/// `doubleExpSmoothInit`: initialize a double exponential smoothing data
/// structure.
fn double_exp_smooth_init(des: &mut DoubleExpSmooth, x1: f64) {
    des.n = 1;
    des.level = x1;
    des.trend = x1 - des.initialvalue;

    des.usetrendinlevel = DES_USETRENDINLEVEL;
}

/// `doubleExpSmoothUpdate`: update a double exponential smoothing data
/// structure.
fn double_exp_smooth_update(des: &mut DoubleExpSmooth, xnew: f64) {
    if des.n == 0 {
        double_exp_smooth_init(des, xnew);
    } else {
        let newlevel = des.alpha * xnew + (1.0 - des.alpha) * (des.level + if des.usetrendinlevel { des.trend } else { 0.0 });
        let newtrend = des.beta * (newlevel - des.level) + (1.0 - des.beta) * des.trend;

        des.level = newlevel;
        des.trend = newtrend;
    }
}

/// `doubleExpSmoothGetTrend`: get the current trend (slope) computed by this
/// double exponential smoothing.
fn double_exp_smooth_get_trend(des: &DoubleExpSmooth) -> f64 {
    if des.n == 0 {
        return INVALID;
    }

    des.trend
}

/// `timeSeriesReset`: reset time series.
fn time_series_reset(timeseries: &mut TimeSeries) {
    timeseries.resolution = 1;
    timeseries.nvals = 0;
    timeseries.nobs = 0;
    timeseries.currentvalue = timeseries.initialvalue;
    timeseries.smoothestimation = INVALID;

    double_exp_smooth_reset(&mut timeseries.des, timeseries.initialvalue);
}

/// `timeSeriesCreate`: create a time series object.
fn time_series_create(name: &str, targetvalue: f64, initialvalue: f64, alpha: f64, beta: f64, timeseriesupdate: TimeSeriesUpdate) -> TimeSeries {
    let valssize = 1024;
    let mut timeseries = TimeSeries {
        des: DoubleExpSmooth::default(),
        name: name.to_string(),
        vals: vec![0.0; valssize],
        estimation: vec![0.0; valssize],
        smoothestimation: 0.0,
        targetvalue,
        currentvalue: 0.0,
        initialvalue,
        nobs: 0,
        valssize: valssize as i32,
        nvals: 0,
        resolution: 0,
        useleafts: false,
        timeseriesupdate,
    };

    time_series_reset(&mut timeseries);

    timeseries.des.alpha = alpha;
    timeseries.des.beta = beta;

    timeseries
}

/// `timeSeriesEstimate`: estimate tree size at which time series reaches
/// target value.
fn time_series_estimate(timeseries: &TimeSeries, treedata: &TreeData) -> f64 {
    let tolerance = 1e-6;

    // If no observations have been made yet, return infinity.
    if timeseries.nobs == 0 {
        return -1.0;
    }

    let val = timeseries.currentvalue;
    let targetval = timeseries.targetvalue;

    // If the value has reached the target value already, return the number
    // of observations.
    let d = val - targetval;
    if d <= tolerance && d >= -tolerance {
        return treedata.nnodes as f64;
    }

    let trend = double_exp_smooth_get_trend(&timeseries.des);

    // Get current value and trend; the linear trend estimation may not
    // point towards the target; in this case, return infinity.
    if (targetval > val && trend <= tolerance) || (targetval < val && trend >= -tolerance) {
        return -1.0;
    }

    // Compute after how many additional steps the current trend reaches the
    // target value; multiply by resolution.
    let estimated = timeseries.resolution as f64 * (timeseries.nvals as f64 + (targetval - val) / trend);
    if timeseries.useleafts { 2.0 * estimated - 1.0 } else { estimated }
}

/// `timeSeriesUpdateSmoothEstimation`: update time series smoothened
/// estimation.
fn time_series_update_smooth_estimation(timeseries: &mut TimeSeries, estimation: f64) {
    if timeseries.smoothestimation == INVALID {
        timeseries.smoothestimation = estimation;
    } else {
        timeseries.smoothestimation *= 1.0 - SESCOEFF;
        timeseries.smoothestimation += SESCOEFF * estimation;
    }
}

/// `timeSeriesResample`: resample to lower resolution.
fn time_series_resample(timeseries: &mut TimeSeries) {
    double_exp_smooth_reset(&mut timeseries.des, timeseries.initialvalue);

    // Compress vals array to store only every second entry.
    for i in 0..(timeseries.nvals / 2) as usize {
        timeseries.vals[i] = timeseries.vals[2 * i];
        timeseries.estimation[i] = timeseries.estimation[2 * i];
        let v = timeseries.vals[i];
        double_exp_smooth_update(&mut timeseries.des, v);
        let e = timeseries.estimation[i];
        time_series_update_smooth_estimation(timeseries, e);
    }

    timeseries.resolution *= 2;
    timeseries.nvals /= 2;
}

/// The time series update callbacks (`timeseriesUpdateGap` and the others).
fn time_series_value(scip: &Scip, ts: &TimeSeries, treedata: &TreeData) -> f64 {
    match ts.timeseriesupdate {
        TimeSeriesUpdate::Gap => {
            // Avoid to call SCIPgetDualbound during a restart where the queue
            // is simply emptied.
            if scip.stat.inrestart {
                return ts.currentvalue;
            }

            let primalbound = api::get_primalbound(scip);
            let dualbound = api::get_dualbound(scip);
            let mut value = if scip.set.is_infinity(primalbound.abs()) || scip.set.is_infinity(dualbound.abs()) {
                0.0
            } else if scip.set.is_eq(primalbound, dualbound) {
                1.0
            } else {
                let abspb = primalbound.abs();
                let absdb = dualbound.abs();
                1.0 - (primalbound - dualbound).abs() / if abspb >= absdb { abspb } else { absdb }
            };

            // Using this max, we set the closed gap to 0 in the case where
            // the primal and dual bound differ in their sign.
            value = if value >= 0.0 { value } else { 0.0 };
            value
        }
        TimeSeriesUpdate::TreeWeight => treedata.weight.to_f64(),
        TimeSeriesUpdate::LeafFreq => {
            if treedata.nvisited == 0 {
                -0.5
            } else {
                (treedata.nleaves as f64 - 0.5) / treedata.nvisited as f64
            }
        }
        TimeSeriesUpdate::Ssg => {
            if treedata.nvisited == 0 {
                1.0
            } else {
                treedata.ssg.value
            }
        }
        TimeSeriesUpdate::OpenNodes => {
            if treedata.nvisited == 0 {
                0.0
            } else {
                treedata.nopen as f64
            }
        }
    }
}

/// `timeSeriesUpdate`: update time series.
fn time_series_update(scip: &Scip, timeseries: &mut TimeSeries, treedata: &TreeData, isleaf: bool) {
    // Call update callback.
    let value = time_series_value(scip, timeseries, treedata);

    // Store the value as current value.
    timeseries.currentvalue = value;

    if timeseries.useleafts && !isleaf {
        return;
    }

    timeseries.nobs += 1;

    // If this is a leaf that matches the time series resolution, store the
    // value.
    if timeseries.nobs % timeseries.resolution as i64 == 0 {
        let tspos = timeseries.nvals as usize;
        timeseries.nvals += 1;
        timeseries.vals[tspos] = value;
        double_exp_smooth_update(&mut timeseries.des, value);
        let estimate = time_series_estimate(timeseries, treedata);
        timeseries.estimation[tspos] = estimate;
        time_series_update_smooth_estimation(timeseries, estimate);
    }

    // If the time series has reached its capacity, resample and increase the
    // resolution.
    if timeseries.nvals == timeseries.valssize {
        time_series_resample(timeseries);
    }
}

/// `resetTimeSeries`: reset all time series.
fn reset_time_series(eventhdlrdata: &mut EventhdlrData) {
    // Loop over time series and reset them.
    let useleafts = eventhdlrdata.useleafts;
    for ts in &mut eventhdlrdata.timeseries {
        time_series_reset(ts);

        ts.useleafts = useleafts;
    }
}

/// `getEnsembleEstimation`: get ensemble tree size estimation as a
/// combination of the individual time series estimations.
///
/// The coefficients have been computed based on a nonlinear fit on a broad
/// set of publicly available MIP instances; please refer to the publication
/// at the top of `event_estim.c` for further details.
fn get_ensemble_estimation(eventhdlrdata: &EventhdlrData) -> f64 {
    let treedata = &eventhdlrdata.treedata;

    let tsposs = [TSPOS_GAP, TSPOS_TREEWEIGHT, TSPOS_LFREQ, TSPOS_SSG, TSPOS_OPEN];

    // Coefficients for the early stage (tree weight <= 0.3).
    let coeffs_early = [
        0.002, // gap
        0.381, // tree weight
        0.469, // leaf-frequency
        0.292, // SSG
        0.004, // open-nodes
    ];

    // Coefficients for the intermediate stage (0.3 < tree weight <= 0.6).
    let coeffs_intermediate = [
        0.011, // gap
        0.193, // tree weight
        0.351, // leaf-frequency
        0.012, // SSG
        0.051, // open-nodes
    ];

    // Coefficients for the late stage (tree weight > 0.6).
    let coeffs_late = [
        0.000, // gap
        0.033, // tree weight
        0.282, // leaf-frequency
        0.003, // SSG
        0.024, // open-nodes
    ];

    // Assign coeffs based on stage.
    let (coeffs, mut estim) = if treedata.weight.le(0.3) {
        (&coeffs_early, 0.0)
    } else if treedata.weight.le(0.6) {
        // Initialize by intermediate WBE coefficient.
        (&coeffs_intermediate, 0.156 * tree_data_get_wbe(treedata))
    } else {
        // Initialize by late WBE coefficient.
        (&coeffs_late, 0.579 * tree_data_get_wbe(treedata))
    };

    // Combine estimation using the stage-dependent coefficients.
    for t in 0..NTIMESERIES {
        let tspos = tsposs[t];
        let mut testim = time_series_estimate(&eventhdlrdata.timeseries[tspos], treedata);

        if testim < 0.0 {
            testim = treedata.nnodes as f64;
        }

        estim += coeffs[t] * testim;
    }

    if estim < treedata.nnodes as f64 { treedata.nnodes as f64 } else { estim }
}

/// `getSearchCompletion`: get approximation of search tree completion
/// depending on the selected method.
fn get_search_completion(eventhdlrdata: &EventhdlrData) -> ScipResult<f64> {
    let treedata = &eventhdlrdata.treedata;
    let mut completiontype = eventhdlrdata.completiontypeparam;

    // Infer automatic completion type:
    //
    //  use regression forest if available,
    //  or
    //  use monotone regression if both SSG and tree weight are meaningful;
    //  or
    //  use tree weight or SSG, depending which one is available,
    //  or
    //  use gap, which is always available
    if completiontype == COMPLETIONTYPE_AUTO {
        let useweight = eventhdlrdata.treeisbinary;
        let usessg = treedata.ssg.pblastsplit != SSG_STARTPRIMBOUND;

        // No regression forest is ever read (see the module comment).
        completiontype = if useweight && usessg {
            COMPLETIONTYPE_MONOREG
        } else if useweight {
            COMPLETIONTYPE_TREEWEIGHT
        } else if usessg {
            COMPLETIONTYPE_SSG
        } else {
            COMPLETIONTYPE_GAP
        };
    }

    // Compute the search tree completion based on the selected method.
    let completed = match completiontype {
        // Use regression forest (none is ever read).
        COMPLETIONTYPE_REGFOREST => {
            log::error!("estimation/completiontype 'r' needs a regression forest, which is not ported");
            return Err(Retcode::NotImplemented);
        }
        // Interpolate between ssg and tree weight.
        COMPLETIONTYPE_MONOREG => eventhdlrdata.coefmonoweight * treedata.weight.to_f64() + eventhdlrdata.coefmonossg * (1.0 - treedata.ssg.value),
        COMPLETIONTYPE_TREEWEIGHT => treedata.weight.to_f64(),
        // Gap is stored as 1 - gap.
        COMPLETIONTYPE_GAP => eventhdlrdata.timeseries[TSPOS_GAP].currentvalue,
        // SSG is decreasing.
        COMPLETIONTYPE_SSG => 1.0 - treedata.ssg.value,
        c => {
            log::error!("Unsupported completion type '{}'", c as char);
            return Err(Retcode::ParameterWrongVal);
        }
    };
    Ok(completed)
}

/// `getEstimCompletion`: tree size estimation based on search tree
/// completion.
fn get_estim_completion(scip: &Scip, eventhdlrdata: &EventhdlrData) -> ScipResult<f64> {
    let mut estim = -1.0;

    let mut completed = get_search_completion(eventhdlrdata)?;

    completed = if completed <= 1.0 { completed } else { 1.0 };

    if completed > 0.0 {
        estim = api::get_n_nodes(scip) as f64 / completed;
    }

    Ok(estim)
}

/// `includeTimeseries`: include time series to forecast into event handler.
fn include_timeseries() -> Vec<TimeSeries> {
    vec![
        // Include gap time series.
        time_series_create("gap", 1.0, 0.0, DES_ALPHA_GAP, DES_BETA_GAP, TimeSeriesUpdate::Gap),
        // Include tree weight time series.
        time_series_create("tree-weight", 1.0, 0.0, DES_ALPHA_TREEWEIGHT, DES_BETA_TREEWEIGHT, TimeSeriesUpdate::TreeWeight),
        // Include leaf time series.
        time_series_create("leaf-frequency", 0.5, -0.5, DES_ALPHA_LEAFFREQUENCY, DES_BETA_LEAFFREQUENCY, TimeSeriesUpdate::LeafFreq),
        // Include SSG time series.
        time_series_create("ssg", 0.0, 1.0, DES_ALPHA_SSG, DES_BETA_SSG, TimeSeriesUpdate::Ssg),
        // Include open nodes time series.
        time_series_create("open-nodes", 0.0, 0.0, DES_ALPHA_OPENNODES, DES_BETA_OPENNODES, TimeSeriesUpdate::OpenNodes),
    ]
}

/// `getRestartPolicy`: get restartpolicy based on the value of the restart
/// parameter.
fn get_restart_policy(eventhdlrdata: &EventhdlrData) -> RestartPolicy {
    match eventhdlrdata.restartpolicyparam {
        RESTARTPOLICY_CHAR_ALWAYS => RestartPolicy::Always,
        RESTARTPOLICY_CHAR_NEVER => RestartPolicy::Never,
        RESTARTPOLICY_CHAR_COMPLETION => RestartPolicy::Completion,
        RESTARTPOLICY_CHAR_ESTIMATION => RestartPolicy::Estimation,
        c => {
            log::error!("Unknown restart policy {}", c as char);
            RestartPolicy::Never
        }
    }
}

/// `isRestartApplicable`: check if a restart is applicable considering limit
/// and threshold user parameters.
fn is_restart_applicable(scip: &Scip, eventhdlrdata: &EventhdlrData) -> bool {
    // If there are no root node integer fixings, restart is usually not
    // helpful.
    if scip.stat.nrootintfixingsrun == 0 {
        return false;
    }

    // Check whether to apply restarts when there are active pricers
    // available.
    if scip.set.nactivepricers > 0 && !eventhdlrdata.restartactpricers {
        return false;
    }

    // Check whether to apply a restart when nonlinear constraints are
    // present.
    if api::is_nlp_constructed(scip) && !eventhdlrdata.restartnonlinear {
        return false;
    }

    // Check if max number of restarts has been reached.
    if eventhdlrdata.restartlimit != -1 && eventhdlrdata.nrestartsperformed >= eventhdlrdata.restartlimit {
        return false;
    }

    // Check if number of nodes exceeds the minimum number of nodes.
    let nnodes = if eventhdlrdata.countonlyleaves {
        eventhdlrdata.treedata.nleaves
    } else {
        eventhdlrdata.treedata.nvisited
    };

    if nnodes < eventhdlrdata.minnodes {
        return false;
    }

    true
}

/// `shouldApplyRestartCompletion`: should a restart be applied based on the
/// value of the selected completion method?
fn should_apply_restart_completion(eventhdlrdata: &EventhdlrData) -> ScipResult<bool> {
    let completion = get_search_completion(eventhdlrdata)?;

    // If the estimation exceeds the current number of nodes by a dramatic
    // factor, restart.
    Ok(completion < 1.0 / eventhdlrdata.restartfactor)
}

/// `shouldApplyRestartEstimation`: should a restart be applied based on the
/// value of the selected completion method?
fn should_apply_restart_estimation(scip: &Scip, eventhdlrdata: &mut EventhdlrData) -> ScipResult<bool> {
    let estimation = treesize_estimation(scip, eventhdlrdata)?;

    if estimation < 0.0 {
        return Ok(true);
    }

    // If the estimation exceeds the current number of nodes by a dramatic
    // factor, restart.
    Ok(estimation > eventhdlrdata.treedata.nnodes as f64 * eventhdlrdata.restartfactor)
}

/// `shouldApplyRestart`: check if a restart should be performed based on the
/// given restart policy.
fn should_apply_restart(scip: &Scip, eventhdlrdata: &mut EventhdlrData) -> ScipResult<bool> {
    match get_restart_policy(eventhdlrdata) {
        RestartPolicy::Always => Ok(true),
        RestartPolicy::Never => Ok(false),
        RestartPolicy::Completion => should_apply_restart_completion(eventhdlrdata),
        RestartPolicy::Estimation => should_apply_restart_estimation(scip, eventhdlrdata),
    }
}

/// `updateTimeseries`: update all time series.
fn update_timeseries(scip: &Scip, eventhdlrdata: &mut EventhdlrData, isleaf: bool) {
    // Loop over time series.
    let EventhdlrData { timeseries, treedata, .. } = eventhdlrdata;
    for ts in timeseries.iter_mut() {
        time_series_update(scip, ts, treedata, isleaf);
    }
}

/// `printReport`: print a treesize estimation report into a string.
fn print_report(scip: &Scip, eventhdlrdata: &mut EventhdlrData, reportnum: i32) -> ScipResult<String> {
    let mut s = String::new();

    // Print report number.
    if reportnum > 0 {
        s += &format!("Report {}\nTime Elapsed: {:.2}\n", reportnum, scip.stat.solvingtime.get_time());
    }

    s += &format!("Estim. Tree Size   :{:11}\n", treesize_estimation(scip, eventhdlrdata)? as i64);

    let mut completed = get_search_completion(eventhdlrdata)?;

    completed = if 1.0 <= completed { 1.0 } else { completed };
    completed = if 0.0 >= completed { 0.0 } else { completed };

    // Print tree data.
    let treedata = &eventhdlrdata.treedata;
    s += &format!(
        "{:<19}: {} nodes ({} visited, {} internal, {} leaves, {} open), weight: {:.4} completed {:.4}\n",
        "Estimation Tree",
        treedata.nnodes,
        treedata.nvisited,
        treedata.ninner,
        treedata.nleaves,
        treedata.nopen,
        treedata.weight.to_f64(),
        completed
    );

    // Print estimations.
    s += &format!("Estimations        : {:>10} {:>10} {:>10} {:>10} {:>10}", "estim", "value", "trend", "resolution", "smooth");
    s += "\n";

    let wbeestim = tree_data_get_wbe(&eventhdlrdata.treedata);
    s += &format!("  wbe              : {:>10} {:>10} {:>10} {:>10} {:>10}\n", real2string(wbeestim, 0), "-", "-", "-", "-");

    let minnodesperdepth = eventhdlrdata.treeprofile_minnodesperdepth;
    let tprof = predict_total_size_tree_profile(scip, eventhdlrdata.treeprofile.as_mut(), minnodesperdepth);
    s += &format!("  tree-profile     : {:10.0} {:>10} {:>10} {:>10} {:>10}\n", tprof, "-", "-", "-", "-");

    // Print time series forecasts.
    for ts in &eventhdlrdata.timeseries {
        let trend = double_exp_smooth_get_trend(&ts.des);
        let smoothestim = ts.smoothestimation;

        s += &format!(
            "  {:<17}: {:10.0} {:10.5} {:>10} {:10} {:>10}\n",
            ts.name,
            time_series_estimate(ts, &eventhdlrdata.treedata),
            ts.currentvalue,
            real2string(trend, 5),
            ts.resolution,
            real2string(smoothestim, 0)
        );
    }

    if reportnum > 0 {
        s += &format!("End of Report {reportnum}\n");
    }

    Ok(s)
}

/// `eventCopyEstim`: copy method for event handler plugins.
fn event_copy_estim(scip: &mut Scip, _sourcescip: &Scip, _eventhdlr: EventhdlrId) -> ScipResult {
    include_event_hdlr_estim(scip)
}

/// `eventInitEstim`: initialization method of event handler.
fn event_init_estim(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    let d = edm(scip, eventhdlr);

    // Test if user specified a regression forest.
    if !d.regforestfilename.starts_with(DEFAULT_REGFORESTFILENAME) {
        log::error!("estimation/regforestfilename: reading a regression forest is not ported");
        return Err(Retcode::NotImplemented);
    }

    d.lastrestartrun = 0;
    d.nrestartsperformed = 0;

    Ok(())
}

/// `eventInitsolEstim`: solving process initialization method of event
/// handler.
fn event_initsol_estim(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    let treeprofile = if edm(scip, eventhdlr).treeprofile_enabled {
        Some(create_tree_profile(scip))
    } else {
        None
    };
    let d = edm(scip, eventhdlr);

    d.restarthitcounter = 0;
    d.weightlastreport = 0.0;
    d.nreports = 0;

    // Reset tree data.
    reset_tree_data(&mut d.treedata);

    reset_time_series(d);

    api::catch_event(scip, EVENTTYPE_ESTIM, eventhdlr, 0, None)?;

    let d = edm(scip, eventhdlr);
    if treeprofile.is_some() {
        d.treeprofile = treeprofile;
    }

    d.treeisbinary = true;

    Ok(())
}

/// `eventExitsolEstim`: solving process deinitialization method of event
/// handler.
fn event_exitsol_estim(scip: &mut Scip, eventhdlr: EventhdlrId) -> ScipResult {
    edm(scip, eventhdlr).treeprofile = None;

    api::drop_event(scip, EVENTTYPE_ESTIM, eventhdlr, 0, -1)
}

/// `eventExecEstim`: execution method of event handler.
fn event_exec_estim(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, _eventdata: usize) -> ScipResult {
    let mut d = take_data(scip, eventhdlr);
    let r = event_exec_estim_with(scip, &mut d, event);
    put_data(scip, eventhdlr, d);
    r
}

fn event_exec_estim_with(scip: &mut Scip, d: &mut EventhdlrData, event: &Event) -> ScipResult {
    let eventtype = event.eventtype;

    // Actual leaf nodes for our tree data are children/siblings/leaves or
    // the focus node itself (deadend) if it has not been branched on.
    let isleafevent = eventtype == EVENTTYPE_NODEDELETE && {
        let node = event.data.node;
        let nodetype = scip.nodes[node].nodetype;
        nodetype == NodeType::Child
            || nodetype == NodeType::Sibling
            || nodetype == NodeType::Leaf
            || (nodetype == NodeType::DeadEnd && !tree_was_node_last_branch_parent(scip, node))
    };
    if eventtype == EVENTTYPE_NODEBRANCHED || isleafevent {
        let mut nchildren = 0;

        if eventtype == EVENTTYPE_NODEBRANCHED {
            nchildren = crate::tree::tree_get_n_children(scip);

            // Update whether the tree is still binary.
            if nchildren != 2 {
                d.treeisbinary = false;
            }
        }

        let eventnode = event.data.node;
        update_tree_data(scip, &mut d.treedata, eventnode, nchildren);
        update_tree_profile(scip, d.treeprofile.as_mut(), eventnode);

        update_timeseries(scip, d, nchildren == 0);

        // Should a new report be printed?
        if d.reportfreq >= 0
            && api::get_status(scip) == crate::types::Status::Unknown
            && (d.reportfreq == 0 || d.treedata.weight.ge(d.weightlastreport + 1.0 / d.reportfreq as f64))
        {
            d.nreports += 1;
            let report = print_report(scip, d, d.nreports)?;
            log::info!("{report}");

            if d.reportfreq > 0 {
                d.weightlastreport = 1.0 / d.reportfreq as f64 * scip.set.floor(d.treedata.weight.to_f64() * d.reportfreq as f64);
            } else {
                d.weightlastreport = d.treedata.weight.to_f64();
            }
        }
    }

    // If nodes have been pruned, things are progressing, don't restart
    // right now.
    if eventtype == EVENTTYPE_NODEDELETE {
        return Ok(());
    }

    // Check if all conditions are met such that the event handler should
    // run.
    if !is_restart_applicable(scip, d) {
        return Ok(());
    }

    // Test if a restart should be applied.
    if should_apply_restart(scip, d)? {
        d.restarthitcounter += 1;

        if d.restarthitcounter >= d.hitcounterlim {
            // Safe that we triggered a restart at this run.
            if api::get_n_runs(scip) > d.lastrestartrun {
                d.nrestartsperformed += 1;

                log::info!(
                    "Restart triggered after {} consecutive estimations that the remaining tree will be large",
                    d.restarthitcounter
                );
            }

            d.lastrestartrun = api::get_n_runs(scip);

            crate::scip_solve::restart_solve(scip);
        }
    } else {
        d.restarthitcounter = 0;
    }

    Ok(())
}

/// The SSG of the handler data (for its parameters' targets).
fn ssg_of(scip: &Scip, eventhdlr: EventhdlrId) -> &SubtreeSumGap {
    &scip.eventhdlrs[eventhdlr]
        .eventhdlrdata
        .as_ref()
        .unwrap()
        .downcast_ref::<EventhdlrData>()
        .unwrap()
        .treedata
        .ssg
}

fn ssg_of_mut(scip: &mut Scip, eventhdlr: EventhdlrId) -> &mut SubtreeSumGap {
    &mut edm(scip, eventhdlr).treedata.ssg
}

/// Target for a parameter stored in the SSG (C binds
/// `&eventhdlrdata->treedata->ssg->field`).
fn ssg_target<T: 'static>(eventhdlr: EventhdlrId, get: fn(&SubtreeSumGap) -> &T, get_mut: fn(&mut SubtreeSumGap) -> &mut T) -> Option<Rc<Target<T>>> {
    Some(Rc::new(Target::<T> {
        get: Box::new(move |scip: &Scip| get(ssg_of(scip, eventhdlr))),
        get_mut: Box::new(move |scip: &mut Scip| get_mut(ssg_of_mut(scip, eventhdlr))),
    }))
}

/// `SCIPincludeEventHdlrEstim`: creates event handler for tree size
/// estimation.
pub fn include_event_hdlr_estim(scip: &mut Scip) -> ScipResult {
    // Create estim event handler data.
    let eventhdlrdata = EventhdlrData {
        timeseries: Vec::new(),
        treedata: create_tree_data(),
        treeprofile: None,
        regforestfilename: String::new(),
        restartfactor: 0.0,
        weightlastreport: 0.0,
        treeprofile_minnodesperdepth: 0.0,
        coefmonoweight: 0.0,
        coefmonossg: 0.0,
        minnodes: 0,
        restartlimit: 0,
        nrestartsperformed: 0,
        restarthitcounter: 0,
        hitcounterlim: 0,
        nreports: 0,
        reportfreq: 0,
        lastrestartrun: 0,
        restartpolicyparam: 0,
        estimmethod: 0,
        completiontypeparam: 0,
        countonlyleaves: false,
        useleafts: false,
        treeprofile_enabled: false,
        treeisbinary: false,
        restartnonlinear: false,
        restartactpricers: false,
        showstats: false,
    };

    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_estim, Some(Box::new(eventhdlrdata) as Box<dyn Any>))?;

    // Set non fundamental callbacks via setter functions (the free callback
    // only frees memory, and the exit callback frees the regression forest,
    // which is never read).
    let h = &mut scip.eventhdlrs[eventhdlr];
    h.eventcopy = Some(event_copy_estim);
    h.eventinit = Some(event_init_estim);
    h.eventinitsol = Some(event_initsol_estim);
    h.eventexitsol = Some(event_exitsol_estim);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, eventhdlrs, eventhdlr, eventhdlrdata, EventhdlrData, $f)
        };
    }

    // Add estimation event handler parameters.
    use crate::{
        def::REAL_MAX,
        paramset::{paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_longint, paramset_add_real, paramset_add_string},
    };
    paramset_add_char(
        scip,
        "estimation/restarts/restartpolicy",
        "restart policy: (a)lways, (c)ompletion, (e)stimation, (n)ever",
        t!(u8, restartpolicyparam),
        false,
        DEFAULT_RESTARTPOLICY,
        Some("acen"),
        None,
    )?;

    paramset_add_char(
        scip,
        "estimation/method",
        "tree size estimation method: (c)ompletion, (e)nsemble, time series forecasts on either (g)ap, (l)eaf frequency, (o)open nodes, tree (w)eight, (s)sg, or (t)ree profile or w(b)e",
        t!(u8, estimmethod),
        false,
        DEFAULT_ESTIMMETHOD,
        Some(ESTIMMETHODS),
        None,
    )?;

    paramset_add_int(
        scip,
        "estimation/restarts/restartlimit",
        "restart limit",
        t!(i32, restartlimit),
        false,
        DEFAULT_RESTARTLIMIT,
        -1,
        i32::MAX,
        None,
    )?;

    paramset_add_longint(
        scip,
        "estimation/restarts/minnodes",
        "minimum number of nodes before restart",
        t!(i64, minnodes),
        false,
        DEFAULT_MINNODES,
        -1,
        i64::MAX,
        None,
    )?;

    // C passes DEFAULT_COUNTONLYLEAVES as isadvanced and FALSE as the
    // default.
    paramset_add_bool(
        scip,
        "estimation/restarts/countonlyleaves",
        "should only leaves count for the minnodes parameter?",
        t!(bool, countonlyleaves),
        DEFAULT_COUNTONLYLEAVES,
        false,
        None,
    )?;

    paramset_add_real(
        scip,
        "estimation/restarts/restartfactor",
        "factor by which the estimated number of nodes should exceed the current number of nodes",
        t!(f64, restartfactor),
        false,
        DEFAULT_RESTARTFACTOR,
        1.0,
        REAL_MAX,
        None,
    )?;

    paramset_add_bool(
        scip,
        "estimation/restarts/restartnonlinear",
        "whether to apply a restart when nonlinear constraints are present",
        t!(bool, restartnonlinear),
        false,
        DEFAULT_RESTARTNONLINEAR,
        None,
    )?;

    paramset_add_bool(
        scip,
        "estimation/restarts/restartactpricers",
        "whether to apply a restart when active pricers are used",
        t!(bool, restartactpricers),
        false,
        DEFAULT_RESTARTACTPRICERS,
        None,
    )?;

    paramset_add_real(
        scip,
        "estimation/coefmonoweight",
        "coefficient of tree weight in monotone approximation of search completion",
        t!(f64, coefmonoweight),
        false,
        DEFAULT_COEFMONOWEIGHT,
        0.0,
        1.0,
        None,
    )?;

    paramset_add_real(
        scip,
        "estimation/coefmonossg",
        "coefficient of 1 - SSG in monotone approximation of search completion",
        t!(f64, coefmonossg),
        false,
        DEFAULT_COEFMONOSSG,
        0.0,
        1.0,
        None,
    )?;

    paramset_add_int(
        scip,
        "estimation/restarts/hitcounterlim",
        "limit on the number of successive samples to really trigger a restart",
        t!(i32, hitcounterlim),
        false,
        DEFAULT_HITCOUNTERLIM,
        1,
        i32::MAX,
        None,
    )?;

    paramset_add_int(
        scip,
        "estimation/reportfreq",
        "report frequency on estimation: -1: never, 0:always, k >= 1: k times evenly during search",
        t!(i32, reportfreq),
        true,
        DEFAULT_REPORTFREQ,
        -1,
        i32::MAX / 2,
        None,
    )?;

    paramset_add_string(
        scip,
        "estimation/regforestfilename",
        "user regression forest in RFCSV format",
        t!(String, regforestfilename),
        false,
        DEFAULT_REGFORESTFILENAME,
        None,
    )?;

    paramset_add_char(
        scip,
        "estimation/completiontype",
        "approximation of search tree completion: (a)uto, (g)ap, tree (w)eight, (m)onotone regression, (r)egression forest, (s)sg",
        t!(u8, completiontypeparam),
        false,
        DEFAULT_COMPLETIONTYPE,
        Some("agmrsw"),
        None,
    )?;

    paramset_add_bool(
        scip,
        "estimation/treeprofile/enabled",
        "should the event handler collect data?",
        t!(bool, treeprofile_enabled),
        false,
        DEFAULT_TREEPROFILE_ENABLED,
        None,
    )?;

    paramset_add_real(
        scip,
        "estimation/treeprofile/minnodesperdepth",
        "minimum average number of nodes at each depth before producing estimations",
        t!(f64, treeprofile_minnodesperdepth),
        false,
        DEFAULT_TREEPROFILE_MINNODESPERDEPTH,
        1.0,
        REAL_MAX,
        None,
    )?;

    paramset_add_bool(
        scip,
        "estimation/useleafts",
        "use leaf nodes as basic observations for time series, or all nodes?",
        t!(bool, useleafts),
        true,
        DEFAULT_USELEAFTS,
        None,
    )?;

    paramset_add_bool(
        scip,
        "estimation/showstats",
        "should statistics be shown at the end?",
        t!(bool, showstats),
        true,
        DEFAULT_SHOWSTATS,
        None,
    )?;

    // SSG parameters.
    paramset_add_int(
        scip,
        "estimation/ssg/nmaxsubtrees",
        "the maximum number of individual SSG subtrees; -1: no limit",
        ssg_target(eventhdlr, |s| &s.nmaxsubtrees, |s| &mut s.nmaxsubtrees),
        false,
        DEFAULT_SSG_NMAXSUBTREES,
        -1,
        i32::MAX / 2,
        None,
    )?;

    paramset_add_longint(
        scip,
        "estimation/ssg/nminnodeslastsplit",
        "minimum number of nodes to process between two consecutive SSG splits",
        ssg_target(eventhdlr, |s| &s.nminnodeslastsplit, |s| &mut s.nminnodeslastsplit),
        false,
        DEFAULT_SSG_NMINNODESLASTSPLIT,
        0,
        i64::MAX,
        None,
    )?;

    // Include time series into event handler.
    edm(scip, eventhdlr).timeseries = include_timeseries();

    Ok(())
}

/// `SCIPgetTreesizeEstimation` on handler data the caller holds.
fn treesize_estimation(scip: &Scip, eventhdlrdata: &mut EventhdlrData) -> ScipResult<f64> {
    let tspos = match eventhdlrdata.estimmethod {
        ESTIMMETHOD_COMPL => return get_estim_completion(scip, eventhdlrdata),

        ESTIMMETHOD_ENSMBL => return Ok(get_ensemble_estimation(eventhdlrdata)),

        // For the requested time series methods, we specify the array
        // position.
        ESTIMMETHOD_GAP => TSPOS_GAP,

        ESTIMMETHOD_LFREQ => TSPOS_LFREQ,

        ESTIMMETHOD_OPEN => TSPOS_OPEN,

        ESTIMMETHOD_TREEWEIGHT => TSPOS_TREEWEIGHT,

        ESTIMMETHOD_SSG => TSPOS_SSG,

        // Tree profile estimation.
        ESTIMMETHOD_TPROF => {
            let minnodesperdepth = eventhdlrdata.treeprofile_minnodesperdepth;
            return Ok(predict_total_size_tree_profile(scip, eventhdlrdata.treeprofile.as_mut(), minnodesperdepth));
        }

        // Weighted backtrack estimation.
        ESTIMMETHOD_WBE => return Ok(tree_data_get_wbe(&eventhdlrdata.treedata)),

        c => {
            log::error!("Unknown estimation '{}' method specified, should be one of [{}]", c as char, ESTIMMETHODS);
            return Ok(-1.0);
        }
    };

    Ok(time_series_estimate(&eventhdlrdata.timeseries[tspos], &eventhdlrdata.treedata))
}

/// `SCIPgetTreesizeEstimation`: return an estimation of the final tree
/// size.
pub fn get_treesize_estimation(scip: &mut Scip) -> f64 {
    let eventhdlr = crate::set::set_find_eventhdlr(scip, EVENTHDLR_NAME);
    if eventhdlr == NONE {
        log::warn!("SCIPgetTreesizeEstimation() called, but event handler estim is missing.");
        return -1.0;
    }

    let mut d = take_data(scip, eventhdlr);
    let r = treesize_estimation(scip, &mut d);
    put_data(scip, eventhdlr, d);
    r.expect("tree size estimation")
}
