//! SCIP's global settings (`set.c`): every core parameter as a field, the
//! numerics, and the plugins.

use crate::{
    cvec::CVec,
    def::{self, eps_ceil, eps_eq, eps_floor, eps_frac, eps_ge, eps_gt, eps_is_int, eps_le, eps_lt, eps_n, eps_p, eps_round, eps_z, max, min},
    misc::{c_f64_to_i32, rel_diff},
    paramset::{ParamSet, paramset_add_bool, paramset_add_char, paramset_add_int, paramset_add_longint, paramset_add_real, paramset_add_string},
    retcode::ScipResult,
    scip::{Scip, Stage},
    set_target,
};

/// `SCIP_SET`.
#[derive(Default)]
pub struct Set {
    pub stage: Stage,
    pub paramset: ParamSet,
    pub limitchanged: bool,
    pub subscipsoff: bool,

    // The included plugins (ids into their arenas on `Scip`); each list
    // has its C count.
    pub pricers: CVec<usize>,
    pub npricers: i32,
    pub nactivepricers: i32,
    pub conshdlrs: CVec<usize>,
    pub conshdlrs_sepa: CVec<usize>,
    pub conshdlrs_enfo: CVec<usize>,
    pub conshdlrs_include: CVec<usize>,
    pub nconshdlrs: i32,
    pub conflicthdlrs: CVec<usize>,
    pub nconflicthdlrs: i32,
    pub presols: CVec<usize>,
    pub npresols: i32,
    pub relaxs: CVec<usize>,
    pub nrelaxs: i32,
    pub sepas: CVec<usize>,
    pub nsepas: i32,
    pub cutsels: CVec<usize>,
    pub ncutsels: i32,
    pub props: CVec<usize>,
    pub props_presol: CVec<usize>,
    pub nprops: i32,
    pub heurs: CVec<usize>,
    pub nheurs: i32,
    pub comprs: CVec<usize>,
    pub ncomprs: i32,
    pub eventhdlrs: CVec<usize>,
    pub neventhdlrs: i32,
    pub nodesels: CVec<usize>,
    pub nnodesels: i32,
    /// The active node selector, or `NONE`.
    pub nodesel: usize,
    pub branchrules: CVec<usize>,
    pub nbranchrules: i32,
    /// Expression handlers (`set->exprhdlrs`, `nexprhdlrs` its length).
    pub exprhdlrs: Vec<usize>,
    /// The handlers of variable, value, sum, product and power expressions.
    pub exprhdlrvar: Option<usize>,
    pub exprhdlrval: Option<usize>,
    pub exprhdlrsum: Option<usize>,
    pub exprhdlrproduct: Option<usize>,
    pub exprhdlrpow: Option<usize>,
    pub nlpis: CVec<usize>,
    pub nnlpis: i32,
    pub benders: CVec<usize>,
    pub nbenders: i32,
    pub nactivebenders: i32,
    pub banditvtables: CVec<usize>,
    pub nbanditvtables: i32,
    pub pricerssorted: bool,
    pub pricersnamesorted: bool,
    pub conflicthdlrssorted: bool,
    pub conflicthdlrsnamesorted: bool,
    pub presolssorted: bool,
    pub presolsnamesorted: bool,
    pub relaxssorted: bool,
    pub relaxsnamesorted: bool,
    pub sepassorted: bool,
    pub sepasnamesorted: bool,
    pub cutselssorted: bool,
    pub propssorted: bool,
    pub propspresolsorted: bool,
    pub propsnamesorted: bool,
    pub heurssorted: bool,
    pub heursnamesorted: bool,
    pub comprssorted: bool,
    pub comprsnamesorted: bool,
    pub branchrulessorted: bool,
    pub branchrulesnamesorted: bool,
    pub exprhdlrssorted: bool,
    pub nlpissorted: bool,
    pub benderssorted: bool,
    pub bendersnamesorted: bool,
    pub istimelimitfinite: bool,
    pub num_relaxfeastol: f64,
    pub exact_enable: bool,
    pub exact_improvingsols: bool,
    pub exact_safedbmethod: u8,
    pub exact_psdualcolselection: i32,
    pub exact_interleavedbstrat: i32,
    pub exact_lpinfo: bool,
    pub exact_allownegslack: bool,
    pub exact_cutmaxdenom: i64,
    pub exact_cutapproxmaxboundval: i64,
    pub certificate_filename: String,
    pub certificate_maxfilesize: f64,
    pub branch_scorefunc: u8,
    pub branch_scorefac: f64,
    pub branch_preferbinary: bool,
    pub branch_clamp: f64,
    pub branch_midpull: f64,
    pub branch_midpullreldomtrig: f64,
    pub branch_lpgainnorm: u8,
    pub branch_delaypscost: bool,
    pub branch_divingpscost: bool,
    pub branch_collectancpscost: bool,
    pub branch_forceall: bool,
    pub branch_firstsbchild: u8,
    pub branch_checksbsol: bool,
    pub branch_roundsbsol: bool,
    pub branch_sumadjustscore: bool,
    pub compr_enable: bool,
    pub conf_enable: bool,
    pub conf_cleanbnddepend: bool,
    pub conf_uselocalrows: bool,
    pub conf_useprop: bool,
    pub conf_usegenres: bool,
    pub conf_useinflp: u8,
    pub conf_useboundlp: u8,
    pub conf_reduction: u8,
    pub conf_mbreduction: bool,
    pub conf_usesb: bool,
    pub conf_usepseudo: bool,
    pub conf_maxvarsfac: f64,
    pub conf_maxvarsfracres: f64,
    pub conf_minmaxvars: i32,
    pub conf_maxlploops: i32,
    pub conf_lpiterations: i32,
    pub conf_fuiplevels: i32,
    pub conf_resfuiplevels: i32,
    pub conf_interconss: i32,
    pub conf_reconvlevels: i32,
    pub conf_maxconss: i32,
    pub conf_maxstoresize: i32,
    pub conf_preferbinary: bool,
    pub conf_prefinfproof: bool,
    pub conf_allowlocal: bool,
    pub conf_settlelocal: bool,
    pub conf_repropagate: bool,
    pub conf_keepreprop: bool,
    pub conf_separate: bool,
    pub conf_dynamic: bool,
    pub conf_removable: bool,
    pub conf_fixandcontinue: bool,
    pub conf_depthscorefac: f64,
    pub conf_proofscorefac: f64,
    pub conf_uplockscorefac: f64,
    pub conf_downlockscorefac: f64,
    pub conf_scorefac: f64,
    pub conf_restartnum: i32,
    pub conf_restartfac: f64,
    pub conf_ignorerelaxedbd: bool,
    pub conf_maxvarsdetectimpliedbounds: i32,
    pub conf_fullshortenconflict: bool,
    pub conf_conflictweight: f64,
    pub conf_conflictgraphweight: f64,
    pub conf_minimprove: f64,
    pub conf_weightsize: f64,
    pub conf_weightrepropdepth: f64,
    pub conf_weightvaliddepth: f64,
    pub conf_maxcoefquot: f64,
    pub conf_sepaaltproofs: bool,
    pub cons_agelimit: i32,
    pub cons_obsoleteage: i32,
    pub cons_disableenfops: bool,
    pub disp_verblevel: i32,
    pub disp_width: i32,
    pub disp_freq: i32,
    pub disp_headerfreq: i32,
    pub disp_lpinfo: bool,
    pub disp_allviols: bool,
    pub disp_relevantstats: bool,
    pub heur_useuctsubscip: bool,
    pub history_valuebased: bool,
    pub history_allowmerge: bool,
    pub history_allowtransfer: bool,
    pub iisfinder_irreducible: bool,
    pub iisfinder_removebounds: bool,
    pub iisfinder_silent: bool,
    pub iisfinder_stopafterone: bool,
    pub iisfinder_removeunusedvars: bool,
    pub iisfinder_time: f64,
    pub iisfinder_nodes: i64,
    pub limit_time: f64,
    pub limit_nodes: i64,
    pub limit_totalnodes: i64,
    pub limit_stallnodes: i64,
    pub limit_memory: f64,
    pub limit_gap: f64,
    pub limit_absgap: f64,
    pub limit_primal: f64,
    pub limit_dual: f64,
    pub limit_solutions: i32,
    pub limit_bestsol: i32,
    pub limit_maxsol: i32,
    pub limit_maxorigsol: i32,
    pub limit_restarts: i32,
    pub limit_autorestartnodes: i32,
    pub lp_solvefreq: i32,
    pub lp_iterlim: i64,
    pub lp_rootiterlim: i64,
    pub lp_solvedepth: i32,
    pub lp_minsolvedepth: i32,
    pub lp_initalgorithm: u8,
    pub lp_resolvealgorithm: u8,
    pub lp_pricing: u8,
    pub lp_clearinitialprobinglp: bool,
    pub lp_resolverestore: bool,
    pub lp_freesolvalbuffers: bool,
    pub lp_colagelimit: i32,
    pub lp_rowagelimit: i32,
    pub lp_cleanupcols: bool,
    pub lp_cleanupcolsroot: bool,
    pub lp_cleanuprows: bool,
    pub lp_cleanuprowsroot: bool,
    pub lp_checkstability: bool,
    pub lp_conditionlimit: f64,
    pub lp_markowitz: f64,
    pub lp_checkprimfeas: bool,
    pub lp_checkdualfeas: bool,
    pub lp_checkfarkas: bool,
    pub lp_fastmip: i32,
    pub lp_scaling: i32,
    pub lp_presolving: bool,
    pub lp_lexdualalgo: bool,
    pub lp_lexdualrootonly: bool,
    pub lp_lexdualmaxrounds: i32,
    pub lp_lexdualbasic: bool,
    pub lp_lexdualstalling: bool,
    pub lp_disablecutoff: i32,
    pub lp_rowrepswitch: f64,
    pub lp_threads: i32,
    pub lp_resolveiterfac: f64,
    pub lp_resolveitermin: i32,
    pub lp_solutionpolishing: i32,
    pub lp_refactorinterval: i32,
    pub lp_alwaysgetduals: bool,
    pub nlp_solver: String,
    pub nlp_disable: bool,
    pub mem_savefac: f64,
    pub mem_arraygrowfac: f64,
    pub mem_arraygrowinit: i32,
    pub mem_treegrowfac: f64,
    pub mem_treegrowinit: i32,
    pub mem_pathgrowfac: f64,
    pub mem_pathgrowinit: i32,
    pub misc_catchctrlc: bool,
    pub misc_usevartable: bool,
    pub misc_useconstable: bool,
    pub misc_usesmalltables: bool,
    pub misc_resetstat: bool,
    pub misc_improvingsols: bool,
    pub misc_printreason: bool,
    pub misc_estimexternmem: bool,
    pub misc_avoidmemout: bool,
    pub misc_transorigsols: bool,
    pub misc_transsolsorig: bool,
    pub misc_calcintegral: bool,
    pub misc_finitesolstore: bool,
    pub misc_outputorigsol: bool,
    pub misc_allowstrongdualreds: bool,
    pub misc_allowweakdualreds: bool,
    pub misc_scaleobj: bool,
    pub misc_showdivingstats: bool,
    pub misc_referencevalue: f64,
    pub misc_usesymmetry: i32,
    pub random_randomseedshift: i32,
    pub random_randomseedshiftmultiplier: i32,
    pub random_permutationseed: i32,
    pub random_permuteconss: bool,
    pub random_permutevars: bool,
    pub random_randomseed: i32,
    pub nodesel_childsel: u8,
    pub num_infinity: f64,
    pub num_epsilon: f64,
    pub num_sumepsilon: f64,
    pub num_feastol: f64,
    pub num_checkfeastolfac: f64,
    pub num_lpfeastolfactor: f64,
    pub num_dualfeastol: f64,
    pub num_barrierconvtol: f64,
    pub num_boundstreps: f64,
    pub num_pseudocosteps: f64,
    pub num_pseudocostdelta: f64,
    pub num_recompfac: f64,
    pub num_hugeval: f64,
    pub presol_maxrounds: i32,
    pub presol_abortfac: f64,
    pub presol_maxrestarts: i32,
    pub presol_restartfac: f64,
    pub presol_clqtablefac: f64,
    pub presol_immrestartfac: f64,
    pub presol_subrestartfac: f64,
    pub presol_restartminred: f64,
    pub presol_donotmultaggr: bool,
    pub presol_donotaggr: bool,
    pub price_maxvars: i32,
    pub price_maxvarsroot: i32,
    pub price_abortfac: f64,
    pub price_delvars: bool,
    pub price_delvarsroot: bool,
    pub decomp_benderslabels: bool,
    pub decomp_applybenders: bool,
    pub decomp_maxgraphedge: i32,
    pub decomp_disablemeasures: bool,
    pub benders_soltol: f64,
    pub benders_cutlpsol: bool,
    pub benders_copybenders: bool,
    pub prop_maxrounds: i32,
    pub prop_maxroundsroot: i32,
    pub prop_abortoncutoff: bool,
    pub reopt_enable: bool,
    pub reopt_maxsavednodes: i32,
    pub reopt_maxdiffofnodes: i32,
    pub reopt_sepaglbinfsubtrees: bool,
    pub reopt_sepabestsol: bool,
    pub reopt_storevarhistory: bool,
    pub reopt_usepscost: bool,
    pub reopt_solvelp: i32,
    pub reopt_solvelpdiff: i32,
    pub reopt_savesols: i32,
    pub reopt_objsimrootlp: f64,
    pub reopt_objsimsol: f64,
    pub reopt_objsimdelay: f64,
    pub reopt_commontimelimit: bool,
    pub reopt_shrinkinner: bool,
    pub reopt_sbinit: bool,
    pub reopt_reducetofrontier: bool,
    pub reopt_forceheurrestart: i32,
    pub reopt_saveprop: bool,
    pub reopt_usesplitcons: bool,
    pub reopt_varorderinterdiction: u8,
    pub reopt_usecuts: bool,
    pub reopt_maxcutage: i32,
    pub sepa_maxbounddist: f64,
    pub sepa_maxlocalbounddist: f64,
    pub sepa_maxcoefratio: f64,
    pub sepa_maxcoefratiofacrowprep: f64,
    pub sepa_minefficacy: f64,
    pub sepa_minefficacyroot: f64,
    pub sepa_minactivityquot: f64,
    pub sepa_maxcutsgenfactor: f64,
    pub sepa_maxcutsrootgenfactor: f64,
    pub sepa_orthofunc: u8,
    pub sepa_efficacynorm: u8,
    pub sepa_cutselrestart: u8,
    pub sepa_cutselsubscip: u8,
    pub sepa_filtercutpoolrel: bool,
    pub sepa_maxruns: i32,
    pub sepa_maxrounds: i32,
    pub sepa_maxroundsroot: i32,
    pub sepa_maxroundsrootsubrun: i32,
    pub sepa_maxaddrounds: i32,
    pub sepa_maxstallrounds: i32,
    pub sepa_maxstallroundsroot: i32,
    pub sepa_maxcuts: i32,
    pub sepa_maxcutsroot: i32,
    pub sepa_cutagelimit: i32,
    pub sepa_poolfreq: i32,
    pub parallel_mode: i32,
    pub parallel_minnthreads: i32,
    pub parallel_maxnthreads: i32,
    pub concurrent_changeseeds: bool,
    pub concurrent_changechildsel: bool,
    pub concurrent_commvarbnds: bool,
    pub concurrent_presolvebefore: bool,
    pub concurrent_initseed: i32,
    pub concurrent_freqinit: f64,
    pub concurrent_freqmax: f64,
    pub concurrent_freqfactor: f64,
    pub concurrent_targetprogress: f64,
    pub concurrent_maxnsols: i32,
    pub concurrent_maxnsyncdelay: i32,
    pub concurrent_minsyncdelay: f64,
    pub concurrent_nbestsols: i32,
    pub concurrent_paramsetprefix: String,
    pub time_clocktype: i32,
    pub time_enabled: bool,
    pub time_reading: bool,
    pub time_rareclockcheck: bool,
    pub time_statistictiming: bool,
    pub time_nlpieval: bool,
    pub visual_vbcfilename: String,
    pub visual_bakfilename: String,
    pub visual_realtime: bool,
    pub visual_dispsols: bool,
    pub visual_displb: bool,
    pub visual_objextern: bool,
    pub read_initialconss: bool,
    pub read_dynamicconss: bool,
    pub read_dynamiccols: bool,
    pub read_dynamicrows: bool,
    pub write_allconss: bool,
    pub write_printzeros: bool,
    pub write_genoffset: i32,
    pub write_implintlevel: i32,
}

/// `SCIPsetCreate`: the settings with every core parameter at its default.
pub fn set_create(scip: &mut Scip) -> ScipResult {
    scip.set.stage = Stage::Init;
    scip.set.limitchanged = false;
    scip.set.subscipsoff = false;
    scip.set.nlp_disable = false;
    scip.set.num_relaxfeastol = def::INVALID;
    scip.set.istimelimitfinite = false;
    paramset_add_char(
        scip,
        "branching/scorefunc",
        "branching score function ('s'um, 'p'roduct, 'q'uotient)",
        set_target!(u8, branch_scorefunc),
        true,
        b'p',
        Some("spq"),
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/scorefac",
        "branching score factor to weigh downward and upward gain prediction in sum score function",
        set_target!(f64, branch_scorefac),
        true,
        0.167,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/preferbinary",
        "should branching on binary variables be preferred?",
        set_target!(bool, branch_preferbinary),
        false,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/clamp",
        "minimal relative distance of branching point to bounds when branching on a continuous variable",
        set_target!(f64, branch_clamp),
        false,
        0.2,
        0.0,
        0.5,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/midpull",
        "fraction by which to move branching point of a continuous variable towards the middle of the domain; a value of 1.0 leads to branching always in the middle of the domain",
        set_target!(f64, branch_midpull),
        false,
        0.75,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "branching/midpullreldomtrig",
        "multiply midpull by relative domain width if the latter is below this value",
        set_target!(f64, branch_midpullreldomtrig),
        false,
        0.5,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/lpgainnormalize",
        "strategy for normalization of LP gain when updating pseudocosts of continuous variables (divide by movement of 'l'p value, reduction in 'd'omain width, or reduction in domain width of 's'ibling)",
        set_target!(u8, branch_lpgainnorm),
        false,
        b's',
        Some("dls"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/delaypscostupdate",
        "should updating pseudo costs for continuous variables be delayed to the time after separation?",
        set_target!(bool, branch_delaypscost),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/divingpscost",
        "should pseudo costs be updated also in diving and probing mode?",
        set_target!(bool, branch_divingpscost),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/collectancpscost",
        "should ancestral pseudo costs be updated?",
        set_target!(bool, branch_collectancpscost),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/forceallchildren",
        "should all strong branching children be regarded even if one is detected to be infeasible? (only with propagation)",
        set_target!(bool, branch_forceall),
        true,
        false,
        None,
    )?;
    paramset_add_char(
        scip,
        "branching/firstsbchild",
        "child node to be regarded first during strong branching (only with propagation): 'u'p child, 'd'own child, 'h'istory-based, or 'a'utomatic",
        set_target!(u8, branch_firstsbchild),
        true,
        b'a',
        Some("aduh"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/checksol",
        "should LP solutions during strong branching with propagation be checked for feasibility?",
        set_target!(bool, branch_checksbsol),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/roundsbsol",
        "should LP solutions during strong branching with propagation be rounded? (only when checksbsol=TRUE)",
        set_target!(bool, branch_roundsbsol),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "branching/sumadjustscore",
        "score adjustment near zero by adding epsilon (TRUE) or using maximum (FALSE)",
        set_target!(bool, branch_sumadjustscore),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "compression/enable",
        "should automatic tree compression after the presolving be enabled?",
        set_target!(bool, compr_enable),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/enable",
        "should conflict analysis be enabled?",
        set_target!(bool, conf_enable),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/cleanboundexceedings",
        "should conflicts based on an old cutoff bound be removed from the conflict pool after improving the primal bound?",
        set_target!(bool, conf_cleanbnddepend),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/uselocalrows",
        "use local rows to construct infeasibility proofs",
        set_target!(bool, conf_uselocalrows),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/useprop",
        "should propagation conflict analysis be used?",
        set_target!(bool, conf_useprop),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/usegenres",
        "should generalized resolution conflict analysis be used?",
        set_target!(bool, conf_usegenres),
        false,
        false,
        None,
    )?;
    paramset_add_char(
        scip,
        "conflict/useinflp",
        "should infeasible LP conflict analysis be used? ('o'ff, 'c'onflict graph, 'd'ual ray, 'b'oth conflict graph and dual ray)",
        set_target!(u8, conf_useinflp),
        false,
        b'b',
        Some("ocdb"),
        None,
    )?;
    paramset_add_char(
        scip,
        "conflict/useboundlp",
        "should bound exceeding LP conflict analysis be used? ('o'ff, 'c'onflict graph, 'd'ual ray, 'b'oth conflict graph and dual ray)",
        set_target!(u8, conf_useboundlp),
        false,
        b'b',
        Some("ocdb"),
        None,
    )?;
    paramset_add_char(
        scip,
        "conflict/reduction",
        "which tightening reduction should be used? ('o'ff, 'm'ir)",
        set_target!(u8, conf_reduction),
        false,
        b'm',
        Some("om"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/mbreduction",
        "should apply the mixed binary reduction?",
        set_target!(bool, conf_mbreduction),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/usesb",
        "should infeasible/bound exceeding strong branching conflict analysis be used?",
        set_target!(bool, conf_usesb),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/usepseudo",
        "should pseudo solution conflict analysis be used?",
        set_target!(bool, conf_usepseudo),
        false,
        true,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/maxvarsfac",
        "maximal fraction of variables involved in a conflict constraint",
        set_target!(f64, conf_maxvarsfac),
        true,
        0.15,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/maxvarsfracres",
        "maximal fraction of variables involved in a resolution conflict constraint",
        set_target!(f64, conf_maxvarsfracres),
        true,
        0.20,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/minmaxvars",
        "minimal absolute maximum of variables involved in a conflict constraint",
        set_target!(i32, conf_minmaxvars),
        true,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/maxlploops",
        "maximal number of LP resolving loops during conflict analysis (-1: no limit)",
        set_target!(i32, conf_maxlploops),
        true,
        2,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/lpiterations",
        "maximal number of LP iterations in each LP resolving loop (-1: no limit)",
        set_target!(i32, conf_lpiterations),
        true,
        10,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/fuiplevels",
        "number of depth levels up to which first UIP's are used in conflict analysis (-1: use All-FirstUIP rule)",
        set_target!(i32, conf_fuiplevels),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/resfuiplevels",
        "number of depth levels up to which first UIP's are used in conflict analysis (-1: use All-FirstUIP rule)",
        set_target!(i32, conf_resfuiplevels),
        true,
        1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/interconss",
        "maximal number of intermediate conflict constraints generated in conflict graph (-1: use every intermediate constraint)",
        set_target!(i32, conf_interconss),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/reconvlevels",
        "number of depth levels up to which UIP reconvergence constraints are generated (-1: generate reconvergence constraints in all depth levels)",
        set_target!(i32, conf_reconvlevels),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/maxconss",
        "maximal number of conflict constraints accepted at an infeasible node (-1: use all generated conflict constraints)",
        set_target!(i32, conf_maxconss),
        true,
        10,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/maxstoresize",
        "maximal size of conflict store (-1: auto, 0: disable storage)",
        set_target!(i32, conf_maxstoresize),
        true,
        10000,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/preferbinary",
        "should binary conflicts be preferred?",
        set_target!(bool, conf_preferbinary),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/prefinfproof",
        "prefer infeasibility proof to boundexceeding proof",
        set_target!(bool, conf_prefinfproof),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/allowlocal",
        "should conflict constraints be generated that are only valid locally?",
        set_target!(bool, conf_allowlocal),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/settlelocal",
        "should conflict constraints be attached only to the local subtree where they can be useful?",
        set_target!(bool, conf_settlelocal),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/repropagate",
        "should earlier nodes be repropagated in order to replace branching decisions by deductions?",
        set_target!(bool, conf_repropagate),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/keepreprop",
        "should constraints be kept for repropagation even if they are too long?",
        set_target!(bool, conf_keepreprop),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/separate",
        "should the conflict constraints be separated?",
        set_target!(bool, conf_separate),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/dynamic",
        "should the conflict constraints be subject to aging?",
        set_target!(bool, conf_dynamic),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/removable",
        "should the conflict's relaxations be subject to LP aging and cleanup?",
        set_target!(bool, conf_removable),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/fixandcontinue",
        "should we fix unresolvable bound changes and continue?",
        set_target!(bool, conf_fixandcontinue),
        true,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/graph/depthscorefac",
        "score factor for depth level in bound relaxation heuristic",
        set_target!(f64, conf_depthscorefac),
        true,
        1.0,
        -f64::MAX,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/proofscorefac",
        "score factor for impact on acticity in bound relaxation heuristic",
        set_target!(f64, conf_proofscorefac),
        true,
        1.0,
        -f64::MAX,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/uplockscorefac",
        "score factor for up locks in bound relaxation heuristic",
        set_target!(f64, conf_uplockscorefac),
        true,
        0.0,
        -f64::MAX,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/downlockscorefac",
        "score factor for down locks in bound relaxation heuristic",
        set_target!(f64, conf_downlockscorefac),
        true,
        0.0,
        -f64::MAX,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/scorefac",
        "factor to decrease importance of variables' earlier conflict scores",
        set_target!(f64, conf_scorefac),
        true,
        0.98,
        1e-6,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/restartnum",
        "number of successful conflict analysis calls that trigger a restart (0: disable conflict restarts)",
        set_target!(i32, conf_restartnum),
        false,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/restartfac",
        "factor to increase restartnum with after each restart",
        set_target!(f64, conf_restartfac),
        false,
        1.5,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/ignorerelaxedbd",
        "should relaxed bounds be ignored?",
        set_target!(bool, conf_ignorerelaxedbd),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "conflict/maxvarsdetectimpliedbounds",
        "maximal number of variables to try to detect global bound implications and shorten the whole conflict set (0: disabled)",
        set_target!(i32, conf_maxvarsdetectimpliedbounds),
        true,
        250,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/fullshortenconflict",
        "try to shorten the whole conflict set or terminate early (depending on the 'maxvarsdetectimpliedbounds' parameter)",
        set_target!(bool, conf_fullshortenconflict),
        true,
        true,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/conflictweight",
        "the weight the VSIDS score is weight by updating the VSIDS for a variable if it is part of a conflict",
        set_target!(f64, conf_conflictweight),
        false,
        0.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/conflictgraphweight",
        "the weight the VSIDS score is weight by updating the VSIDS for a variable if it is part of a conflict graph",
        set_target!(f64, conf_conflictgraphweight),
        false,
        1.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/minimprove",
        "minimal improvement of primal bound to remove conflicts based on a previous incumbent",
        set_target!(f64, conf_minimprove),
        true,
        0.05,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/weightsize",
        "weight of the size of a conflict used in score calculation",
        set_target!(f64, conf_weightsize),
        true,
        0.001,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/weightrepropdepth",
        "weight of the repropagation depth of a conflict used in score calculation",
        set_target!(f64, conf_weightrepropdepth),
        true,
        0.1,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/weightvaliddepth",
        "weight of the valid depth of a conflict used in score calculation",
        set_target!(f64, conf_weightvaliddepth),
        true,
        1.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "conflict/maxcoefquot",
        " largest allowed quotient of max, min coefficient in a conflict constraint generated by generalized resolution",
        set_target!(f64, conf_maxcoefquot),
        false,
        1e+6,
        1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "conflict/sepaaltproofs",
        "apply cut generating functions to construct alternative proofs",
        set_target!(bool, conf_sepaaltproofs),
        false,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/agelimit",
        "maximum age an unnecessary constraint can reach before it is deleted (0: dynamic, -1: keep all constraints)",
        set_target!(i32, cons_agelimit),
        true,
        0,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "constraints/obsoleteage",
        "age of a constraint after which it is marked obsolete (0: dynamic, -1 do not mark constraints obsolete)",
        set_target!(i32, cons_obsoleteage),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "constraints/disableenfops",
        "should enforcement of pseudo solution be disabled?",
        set_target!(bool, cons_disableenfops),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "display/verblevel",
        "verbosity level of output",
        set_target!(i32, disp_verblevel),
        false,
        4,
        0,
        5,
        None,
    )?;
    paramset_add_int(
        scip,
        "display/width",
        "maximal number of characters in a node information line",
        set_target!(i32, disp_width),
        false,
        143,
        0,
        i32::MAX,
        Some(param_chgd_disp_width),
    )?;
    paramset_add_int(
        scip,
        "display/freq",
        "frequency for displaying node information lines",
        set_target!(i32, disp_freq),
        false,
        100,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "display/headerfreq",
        "frequency for displaying header lines (every n'th node information line)",
        set_target!(i32, disp_headerfreq),
        false,
        15,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "display/lpinfo",
        "should the LP solver display status messages?",
        set_target!(bool, disp_lpinfo),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "display/allviols",
        "display all violations for a given start solution / the best solution after the solving process?",
        set_target!(bool, disp_allviols),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "display/relevantstats",
        "should the relevant statistics be displayed at the end of solving?",
        set_target!(bool, disp_relevantstats),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "heuristics/useuctsubscip",
        "should setting of common subscip parameters include the activation of the UCT node selector?",
        set_target!(bool, heur_useuctsubscip),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "history/valuebased",
        "should statistics be collected for variable domain value pairs?",
        set_target!(bool, history_valuebased),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "history/allowmerge",
        "should variable histories be merged from sub-SCIPs whenever possible?",
        set_target!(bool, history_allowmerge),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "history/allowtransfer",
        "should variable histories be transferred to initialize SCIP copies?",
        set_target!(bool, history_allowtransfer),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "iis/irreducible",
        "should the resultant infeasible set be irreducible, i.e., an IIS not an IS",
        set_target!(bool, iisfinder_irreducible),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "iis/removebounds",
        "should bounds of the problem be considered for removal",
        set_target!(bool, iisfinder_removebounds),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "iis/silent",
        "should the IIS finders be run silently and output suppressed",
        set_target!(bool, iisfinder_silent),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "iis/stopafterone",
        "should the IIS search stop after a single IIS finder is run (excluding post processing)",
        set_target!(bool, iisfinder_stopafterone),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "iis/removeunusedvars",
        "should vars that do not feature in any constraints be removed at the end of the IIS process",
        set_target!(bool, iisfinder_removeunusedvars),
        true,
        true,
        None,
    )?;
    paramset_add_real(
        scip,
        "iis/time",
        "maximal time in seconds for all IIS finders to run",
        set_target!(f64, iisfinder_time),
        false,
        1e+20,
        0.0,
        1e+20,
        Some(param_chgd_limit),
    )?;
    paramset_add_longint(
        scip,
        "iis/nodes",
        "maximal number of nodes to process for all IIS finders (-1: no limit)",
        set_target!(i64, iisfinder_nodes),
        false,
        -1,
        -1,
        i64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/time",
        "maximal time in seconds to run",
        set_target!(f64, limit_time),
        false,
        1e+20,
        0.0,
        1e+20,
        Some(param_chgd_limit),
    )?;
    paramset_add_longint(
        scip,
        "limits/nodes",
        "maximal number of nodes to process (-1: no limit)",
        set_target!(i64, limit_nodes),
        false,
        -1,
        -1,
        i64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_longint(
        scip,
        "limits/totalnodes",
        "maximal number of total nodes (incl. restarts) to process (-1: no limit)",
        set_target!(i64, limit_totalnodes),
        false,
        -1,
        -1,
        i64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_longint(
        scip,
        "limits/stallnodes",
        "solving stops, if the given number of nodes was processed since the last improvement of the primal solution value (-1: no limit)",
        set_target!(i64, limit_stallnodes),
        false,
        -1,
        -1,
        i64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/memory",
        "maximal memory usage in MB; reported memory usage is lower than real memory usage!",
        set_target!(f64, limit_memory),
        false,
        (i64::MAX >> 20) as f64,
        0.0,
        (i64::MAX >> 20) as f64,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/gap",
        "solving stops, if the relative gap = |primal - dual|/MIN(|dual|,|primal|) is below the given value, the gap is 'Infinity', if primal and dual bound have opposite signs",
        set_target!(f64, limit_gap),
        false,
        0.0,
        0.0,
        f64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/absgap",
        "solving stops, if the absolute gap = |primalbound - dualbound| is below the given value",
        set_target!(f64, limit_absgap),
        false,
        0.0,
        0.0,
        f64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/primal",
        "solving stops, if primal bound is at least as good as given value",
        set_target!(f64, limit_primal),
        false,
        1e+99,
        -f64::MAX,
        f64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_real(
        scip,
        "limits/dual",
        "solving stops, if dual bound is at least as good as given value",
        set_target!(f64, limit_dual),
        false,
        1e+99,
        -f64::MAX,
        f64::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/solutions",
        "solving stops, if the given number of solutions were found; this limit is first checked in presolving (-1: no limit)",
        set_target!(i32, limit_solutions),
        false,
        -1,
        -1,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/bestsol",
        "solving stops, if the given number of solution improvements were found (-1: no limit)",
        set_target!(i32, limit_bestsol),
        false,
        -1,
        -1,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/maxsol",
        "maximal number of solutions to store in the solution storage",
        set_target!(i32, limit_maxsol),
        false,
        100,
        1,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/maxorigsol",
        "maximal number of solutions candidates to store in the solution storage of the original problem",
        set_target!(i32, limit_maxorigsol),
        false,
        10,
        0,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/restarts",
        "solving stops, if the given number of restarts was triggered (-1: no limit)",
        set_target!(i32, limit_restarts),
        false,
        -1,
        -1,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "limits/autorestartnodes",
        "if solve exceeds this number of nodes for the first time, an automatic restart is triggered (-1: no automatic restart)",
        set_target!(i32, limit_autorestartnodes),
        false,
        -1,
        -1,
        i32::MAX,
        Some(param_chgd_limit),
    )?;
    paramset_add_int(
        scip,
        "lp/solvefreq",
        "frequency for solving LP at the nodes (-1: never; 0: only root LP)",
        set_target!(i32, lp_solvefreq),
        false,
        1,
        -1,
        1073741822,
        None,
    )?;
    paramset_add_longint(
        scip,
        "lp/iterlim",
        "iteration limit for each single LP solve (-1: no limit)",
        set_target!(i64, lp_iterlim),
        true,
        -1,
        -1,
        i64::MAX,
        None,
    )?;
    paramset_add_longint(
        scip,
        "lp/rootiterlim",
        "iteration limit for initial root LP solve (-1: no limit)",
        set_target!(i64, lp_rootiterlim),
        true,
        -1,
        -1,
        i64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/solvedepth",
        "maximal depth for solving LP at the nodes (-1: no depth limit)",
        set_target!(i32, lp_solvedepth),
        false,
        -1,
        -1,
        1073741822,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/minsolvedepth",
        "minimal depth for solving LP at the nodes",
        set_target!(i32, lp_minsolvedepth),
        false,
        0,
        0,
        1073741822,
        None,
    )?;
    paramset_add_char(
        scip,
        "lp/initalgorithm",
        "LP algorithm for solving initial LP relaxations (automatic 's'implex, 'p'rimal simplex, 'd'ual simplex, 'b'arrier, barrier with 'c'rossover)",
        set_target!(u8, lp_initalgorithm),
        false,
        b's',
        Some("spdbc"),
        None,
    )?;
    paramset_add_char(
        scip,
        "lp/resolvealgorithm",
        "LP algorithm for resolving LP relaxations if a starting basis exists (automatic 's'implex, 'p'rimal simplex, 'd'ual simplex, 'b'arrier, barrier with 'c'rossover)",
        set_target!(u8, lp_resolvealgorithm),
        false,
        b's',
        Some("spdbc"),
        None,
    )?;
    paramset_add_char(
        scip,
        "lp/pricing",
        "LP pricing strategy ('l'pi default, 'a'uto, 'f'ull pricing, 'p'artial, 's'teepest edge pricing, 'q'uickstart steepest edge pricing, 'd'evex pricing)",
        set_target!(u8, lp_pricing),
        false,
        b'l',
        Some("lafpsqd"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/clearinitialprobinglp",
        "should lp state be cleared at the end of probing mode when lp was initially unsolved, e.g., when called right after presolving?",
        set_target!(bool, lp_clearinitialprobinglp),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/resolverestore",
        "should the LP be resolved to restore the state at start of diving (if FALSE we buffer the solution values)?",
        set_target!(bool, lp_resolverestore),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/freesolvalbuffers",
        "should the buffers for storing LP solution values during diving be freed at end of diving?",
        set_target!(bool, lp_freesolvalbuffers),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/colagelimit",
        "maximum age a dynamic column can reach before it is deleted from the LP (-1: don't delete columns due to aging)",
        set_target!(i32, lp_colagelimit),
        true,
        10,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/rowagelimit",
        "maximum age a dynamic row can reach before it is deleted from the LP (-1: don't delete rows due to aging)",
        set_target!(i32, lp_rowagelimit),
        true,
        10,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/cleanupcols",
        "should new non-basic columns be removed after LP solving?",
        set_target!(bool, lp_cleanupcols),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/cleanupcolsroot",
        "should new non-basic columns be removed after root LP solving?",
        set_target!(bool, lp_cleanupcolsroot),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/cleanuprows",
        "should new basic rows be removed after LP solving?",
        set_target!(bool, lp_cleanuprows),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/cleanuprowsroot",
        "should new basic rows be removed after root LP solving?",
        set_target!(bool, lp_cleanuprowsroot),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/checkstability",
        "should LP solver's return status be checked for stability?",
        set_target!(bool, lp_checkstability),
        true,
        true,
        None,
    )?;
    paramset_add_real(
        scip,
        "lp/conditionlimit",
        "maximum condition number of LP basis counted as stable (-1.0: no limit)",
        set_target!(f64, lp_conditionlimit),
        true,
        -1.0,
        -1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "lp/minmarkowitz",
        "minimal Markowitz threshold to control sparsity/stability in LU factorization",
        set_target!(f64, lp_markowitz),
        true,
        0.01,
        1e-4,
        0.9999,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/checkprimfeas",
        "should LP solutions be checked for primal feasibility, resolving LP when numerical troubles occur?",
        set_target!(bool, lp_checkprimfeas),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/checkdualfeas",
        "should LP solutions be checked for dual feasibility, resolving LP when numerical troubles occur?",
        set_target!(bool, lp_checkdualfeas),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/checkfarkas",
        "should infeasibility proofs from the LP be checked?",
        set_target!(bool, lp_checkfarkas),
        true,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/fastmip",
        "which FASTMIP setting of LP solver should be used? 0: off, 1: low",
        set_target!(i32, lp_fastmip),
        true,
        1,
        0,
        1,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/scaling",
        "LP scaling (0: none, 1: normal, 2: aggressive)",
        set_target!(i32, lp_scaling),
        true,
        1,
        0,
        2,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/presolving",
        "should presolving of LP solver be used?",
        set_target!(bool, lp_presolving),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/lexdualalgo",
        "should the lexicographic dual algorithm be used?",
        set_target!(bool, lp_lexdualalgo),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/lexdualrootonly",
        "should the lexicographic dual algorithm be applied only at the root node",
        set_target!(bool, lp_lexdualrootonly),
        true,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/lexdualmaxrounds",
        "maximum number of rounds in the lexicographic dual algorithm (-1: unbounded)",
        set_target!(i32, lp_lexdualmaxrounds),
        true,
        2,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/lexdualbasic",
        "choose fractional basic variables in lexicographic dual algorithm?",
        set_target!(bool, lp_lexdualbasic),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/lexdualstalling",
        "turn on the lex dual algorithm only when stalling?",
        set_target!(bool, lp_lexdualstalling),
        true,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/disablecutoff",
        "disable the cutoff bound in the LP solver? (0: enabled, 1: disabled, 2: auto)",
        set_target!(i32, lp_disablecutoff),
        true,
        2,
        0,
        2,
        None,
    )?;
    paramset_add_real(
        scip,
        "lp/rowrepswitch",
        "simplex algorithm shall use row representation of the basis if number of rows divided by number of columns exceeds this value (-1.0 to disable row representation)",
        set_target!(f64, lp_rowrepswitch),
        true,
        1.2,
        -1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/threads",
        "number of threads used for solving the LP (0: automatic)",
        set_target!(i32, lp_threads),
        true,
        0,
        0,
        64,
        None,
    )?;
    paramset_add_real(
        scip,
        "lp/resolveiterfac",
        "factor of average LP iterations that is used as LP iteration limit for LP resolve (-1: unlimited)",
        set_target!(f64, lp_resolveiterfac),
        true,
        -1.0,
        -1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/resolveitermin",
        "minimum number of iterations that are allowed for LP resolve",
        set_target!(i32, lp_resolveitermin),
        true,
        1000,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/solutionpolishing",
        "LP solution polishing method (0: disabled, 1: only root, 2: always, 3: auto)",
        set_target!(i32, lp_solutionpolishing),
        true,
        3,
        0,
        3,
        None,
    )?;
    paramset_add_int(
        scip,
        "lp/refactorinterval",
        "LP refactorization interval (0: auto)",
        set_target!(i32, lp_refactorinterval),
        true,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "lp/alwaysgetduals",
        "should the Farkas duals always be collected when an LP is found to be infeasible?",
        set_target!(bool, lp_alwaysgetduals),
        false,
        false,
        None,
    )?;
    paramset_add_string(
        scip,
        "nlp/solver",
        "solver to use for solving NLPs; leave empty to select NLPI with highest priority",
        set_target!(String, nlp_solver),
        false,
        "",
        None,
    )?;
    paramset_add_bool(
        scip,
        "nlp/disable",
        "should the NLP relaxation be always disabled (also for NLPs/MINLPs)?",
        set_target!(bool, nlp_disable),
        false,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "memory/savefac",
        "fraction of maximal memory usage resulting in switch to memory saving mode",
        set_target!(f64, mem_savefac),
        false,
        0.8,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "memory/arraygrowfac",
        "memory growing factor for dynamically allocated arrays",
        set_target!(f64, mem_arraygrowfac),
        true,
        1.2,
        1.0,
        10.0,
        Some(param_chgd_arraygrowfac),
    )?;
    paramset_add_int(
        scip,
        "memory/arraygrowinit",
        "initial size of dynamically allocated arrays",
        set_target!(i32, mem_arraygrowinit),
        true,
        4,
        0,
        i32::MAX,
        Some(param_chgd_arraygrowinit),
    )?;
    paramset_add_real(
        scip,
        "memory/treegrowfac",
        "memory growing factor for tree array",
        set_target!(f64, mem_treegrowfac),
        true,
        2.0,
        1.0,
        10.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "memory/treegrowinit",
        "initial size of tree array",
        set_target!(i32, mem_treegrowinit),
        true,
        65536,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "memory/pathgrowfac",
        "memory growing factor for path array",
        set_target!(f64, mem_pathgrowfac),
        true,
        2.0,
        1.0,
        10.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "memory/pathgrowinit",
        "initial size of path array",
        set_target!(i32, mem_pathgrowinit),
        true,
        256,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/catchctrlc",
        "should the CTRL-C interrupt be caught by SCIP?",
        set_target!(bool, misc_catchctrlc),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/usevartable",
        "should a hashtable be used to map from variable names to variables?",
        set_target!(bool, misc_usevartable),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/useconstable",
        "should a hashtable be used to map from constraint names to constraints?",
        set_target!(bool, misc_useconstable),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/usesmalltables",
        "should smaller hashtables be used? yields better performance for small problems with about 100 variables",
        set_target!(bool, misc_usesmalltables),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/resetstat",
        "should the statistics be reset if the transformed problem is freed (in case of a Benders' decomposition this parameter should be set to FALSE)",
        set_target!(bool, misc_resetstat),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/improvingsols",
        "should only solutions be checked which improve the primal bound",
        set_target!(bool, misc_improvingsols),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/printreason",
        "should the reason be printed if a given start solution is infeasible",
        set_target!(bool, misc_printreason),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/estimexternmem",
        "should the usage of external memory be estimated?",
        set_target!(bool, misc_estimexternmem),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/avoidmemout",
        "try to avoid running into memory limit by restricting plugins like heuristics?",
        set_target!(bool, misc_avoidmemout),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/transorigsols",
        "should SCIP try to transfer original solutions to the transformed space (after presolving)?",
        set_target!(bool, misc_transorigsols),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/transsolsorig",
        "should SCIP try to transfer transformed solutions to the original space (after solving)?",
        set_target!(bool, misc_transsolsorig),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/calcintegral",
        "should SCIP calculate the primal dual integral value?",
        set_target!(bool, misc_calcintegral),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/finitesolutionstore",
        "should SCIP try to remove infinite fixings from solutions copied to the solution store?",
        set_target!(bool, misc_finitesolstore),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/outputorigsol",
        "should the best solution be transformed to the orignal space and be output in command line run?",
        set_target!(bool, misc_outputorigsol),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/allowstrongdualreds",
        "should strong dual reductions be allowed in propagation and presolving?",
        set_target!(bool, misc_allowstrongdualreds),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/allowweakdualreds",
        "should weak dual reductions be allowed in propagation and presolving?",
        set_target!(bool, misc_allowweakdualreds),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/scaleobj",
        "should the objective function be scaled so that it is always integer?",
        set_target!(bool, misc_scaleobj),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "misc/showdivingstats",
        "should detailed statistics for diving heuristics be shown?",
        set_target!(bool, misc_showdivingstats),
        false,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "misc/referencevalue",
        "objective value for reference purposes",
        set_target!(f64, misc_referencevalue),
        false,
        1e99,
        -f64::MAX,
        f64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "misc/usesymmetry",
        "bitset describing used symmetry handling technique: (0: off; 1: constraint-based (orbitopes, symresacks); lexicographic and orbitopal reduction) if dynamic; 2: orbital reduction; 3: orbitopes and symresacks, and lexicographic/orbital reduction; 4: Schreier Sims cuts; 5: Schreier Sims cuts, orbitopes, symresacks, and/or lexicographic reduction; 6: Schreier Sims cuts, orbital reduction; 7: Schreier Sims cuts, orbitopes, symresacks, and/or lexicographic/orbital reduction;) See type_symmetry.h.",
        set_target!(i32, misc_usesymmetry),
        false,
        7,
        0,
        7,
        Some(param_chgd_usesymmetry),
    )?;
    paramset_add_int(
        scip,
        "randomization/randomseedshift",
        "global shift of all random seeds in the plugins and the LP random seed",
        set_target!(i32, random_randomseedshift),
        false,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "randomization/randomseedshiftmultiplier",
        "multiplier for global shift of all random seeds in the plugins and the LP random seed; this multiplier will be changed with every SCIP major release",
        set_target!(i32, random_randomseedshiftmultiplier),
        false,
        10,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "randomization/permutationseed",
        "seed value for permuting the problem after reading/transformation (0: no permutation)",
        set_target!(i32, random_permutationseed),
        false,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "randomization/permuteconss",
        "should order of constraints be permuted (depends on permutationseed)?",
        set_target!(bool, random_permuteconss),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "randomization/permutevars",
        "should order of variables be permuted (depends on permutationseed)?",
        set_target!(bool, random_permutevars),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "randomization/lpseed",
        "random seed for LP solver, e.g. for perturbations in the simplex (0: LP default)",
        set_target!(i32, random_randomseed),
        false,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "nodeselection/childsel",
        "child selection rule ('d'own, 'u'p, 'p'seudo costs, 'i'nference, 'l'p value, 'r'oot LP value difference, 'h'ybrid inference/root LP value difference)",
        set_target!(u8, nodesel_childsel),
        false,
        b'h',
        Some("dupilrh"),
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/infinity",
        "values larger than this are considered infinity",
        set_target!(f64, num_infinity),
        false,
        1e+20,
        1e+10,
        1e+99 / 10.0,
        Some(param_chgd_infinity),
    )?;
    paramset_add_real(
        scip,
        "numerics/epsilon",
        "absolute values smaller than this are considered zero",
        set_target!(f64, num_epsilon),
        false,
        1e-09,
        1e-20,
        1e-03,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/sumepsilon",
        "absolute values of sums smaller than this are considered zero",
        set_target!(f64, num_sumepsilon),
        false,
        1e-06,
        (1e-20) * 1e+03,
        1e-03,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/feastol",
        "feasibility tolerance for constraints",
        set_target!(f64, num_feastol),
        false,
        1e-06,
        (1e-20) * 1e+03,
        1e-03,
        Some(param_chgd_feastol),
    )?;
    paramset_add_real(
        scip,
        "numerics/checkfeastolfac",
        "feasibility tolerance factor; for checking the feasibility of the best solution",
        set_target!(f64, num_checkfeastolfac),
        false,
        1.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/lpfeastolfactor",
        "factor w.r.t. primal feasibility tolerance that determines default (and maximal) primal feasibility tolerance of LP solver",
        set_target!(f64, num_lpfeastolfactor),
        false,
        1.0,
        1e-6,
        1.0,
        Some(param_chgd_lp_feastol_factor),
    )?;
    paramset_add_real(
        scip,
        "numerics/dualfeastol",
        "feasibility tolerance for reduced costs in LP solution",
        set_target!(f64, num_dualfeastol),
        false,
        1e-07,
        (1e-20) * 1e+03,
        1e-03,
        Some(param_chgd_dualfeastol),
    )?;
    paramset_add_real(
        scip,
        "numerics/barrierconvtol",
        "LP convergence tolerance used in barrier algorithm",
        set_target!(f64, num_barrierconvtol),
        true,
        1e-10,
        (1e-20) * 1e+03,
        1e-03,
        Some(param_chgd_barrierconvtol),
    )?;
    paramset_add_real(
        scip,
        "numerics/boundstreps",
        "minimal relative improve for strengthening bounds",
        set_target!(f64, num_boundstreps),
        true,
        0.05,
        (1e-20) * 1e+03,
        1e+99 / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/pseudocosteps",
        "minimal variable distance value to use for branching pseudo cost updates",
        set_target!(f64, num_pseudocosteps),
        true,
        1e-01,
        (1e-20) * 1e+03,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/pseudocostdelta",
        "minimal objective distance value to use for branching pseudo cost updates",
        set_target!(f64, num_pseudocostdelta),
        true,
        1e-04,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/recomputefac",
        "minimal decrease factor that causes the recomputation of a value (e.g., pseudo objective) instead of an update",
        set_target!(f64, num_recompfac),
        true,
        1e+06,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "numerics/hugeval",
        "values larger than this are considered huge and should be handled separately (e.g., in activity computation)",
        set_target!(f64, num_hugeval),
        true,
        1e+15,
        0.0,
        1e+99 / 10.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/maxrounds",
        "maximal number of presolving rounds (-1: unlimited, 0: off)",
        set_target!(i32, presol_maxrounds),
        false,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/abortfac",
        "abort presolve, if at most this fraction of the problem was changed in last presolve round",
        set_target!(f64, presol_abortfac),
        true,
        8e-04,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_int(
        scip,
        "presolving/maxrestarts",
        "maximal number of restarts (-1: unlimited)",
        set_target!(i32, presol_maxrestarts),
        false,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/restartfac",
        "fraction of integer variables that were fixed in the root node triggering a restart with preprocessing after root node evaluation",
        set_target!(f64, presol_restartfac),
        true,
        0.025,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/clqtablefac",
        "limit on number of entries in clique table relative to number of problem nonzeros",
        set_target!(f64, presol_clqtablefac),
        true,
        2.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/immrestartfac",
        "fraction of integer variables that were fixed in the root node triggering an immediate restart with preprocessing",
        set_target!(f64, presol_immrestartfac),
        true,
        0.05,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/subrestartfac",
        "fraction of integer variables that were globally fixed during the solving process triggering a restart with preprocessing",
        set_target!(f64, presol_subrestartfac),
        true,
        1.00,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "presolving/restartminred",
        "minimal fraction of integer variables removed after restart to allow for an additional restart",
        set_target!(f64, presol_restartminred),
        true,
        0.05,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/donotmultaggr",
        "should multi-aggregation of variables be forbidden?",
        set_target!(bool, presol_donotmultaggr),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "presolving/donotaggr",
        "should aggregation of variables be forbidden?",
        set_target!(bool, presol_donotaggr),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "pricing/maxvars",
        "maximal number of variables priced in per pricing round",
        set_target!(i32, price_maxvars),
        false,
        100,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "pricing/maxvarsroot",
        "maximal number of priced variables at the root node",
        set_target!(i32, price_maxvarsroot),
        false,
        2000,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "pricing/abortfac",
        "pricing is aborted, if fac * pricing/maxvars pricing candidates were found",
        set_target!(f64, price_abortfac),
        false,
        2.0,
        1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "pricing/delvars",
        "should variables created at the current node be deleted when the node is solved in case they are not present in the LP anymore?",
        set_target!(bool, price_delvars),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "pricing/delvarsroot",
        "should variables created at the root node be deleted when the root is solved in case they are not present in the LP anymore?",
        set_target!(bool, price_delvarsroot),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "decomposition/benderslabels",
        "should the variables be labelled for the application of Benders' decomposition?",
        set_target!(bool, decomp_benderslabels),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "decomposition/applybenders",
        "if a decomposition exists, should Benders' decomposition be applied?",
        set_target!(bool, decomp_applybenders),
        false,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "decomposition/maxgraphedge",
        "maximum number of edges in block graph computation (-1: no limit, 0: disable block graph computation)",
        set_target!(i32, decomp_maxgraphedge),
        false,
        10000,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "decomposition/disablemeasures",
        "disable expensive measures",
        set_target!(bool, decomp_disablemeasures),
        false,
        false,
        None,
    )?;
    paramset_add_real(
        scip,
        "benders/solutiontol",
        "the tolerance used for checking optimality in Benders' decomposition. tol where optimality is given by LB + tol > UB.",
        set_target!(f64, benders_soltol),
        false,
        1e-6,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "benders/cutlpsol",
        "should Benders' cuts be generated from the solution to the LP relaxation?",
        set_target!(bool, benders_cutlpsol),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "benders/copybenders",
        "should Benders' decomposition be copied for use in sub-SCIPs?",
        set_target!(bool, benders_copybenders),
        false,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/maxrounds",
        "maximal number of propagation rounds per node (-1: unlimited)",
        set_target!(i32, prop_maxrounds),
        false,
        100,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "propagating/maxroundsroot",
        "maximal number of propagation rounds in the root node (-1: unlimited)",
        set_target!(i32, prop_maxroundsroot),
        false,
        1000,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/abortoncutoff",
        "should propagation be aborted immediately? setting this to FALSE could help conflict analysis to produce more conflict constraints",
        set_target!(bool, prop_abortoncutoff),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/enable",
        "should reoptimization used?",
        set_target!(bool, reopt_enable),
        false,
        false,
        Some(param_chgd_enable_reopt),
    )?;
    paramset_add_int(
        scip,
        "reoptimization/maxsavednodes",
        "maximal number of saved nodes",
        set_target!(i32, reopt_maxsavednodes),
        true,
        i32::MAX,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/maxdiffofnodes",
        "maximal number of bound changes between two stored nodes on one path",
        set_target!(i32, reopt_maxdiffofnodes),
        true,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/globalcons/sepainfsubtrees",
        "save global constraints to separate infeasible subtrees.",
        set_target!(bool, reopt_sepaglbinfsubtrees),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/sepabestsol",
        "separate the optimal solution, i.e., for constrained shortest path",
        set_target!(bool, reopt_sepabestsol),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/storevarhistory",
        "use variable history of the previous solve if the objctive function has changed only slightly",
        set_target!(bool, reopt_storevarhistory),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/usepscost",
        "re-use pseudo costs if the objective function changed only slightly ",
        set_target!(bool, reopt_usepscost),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/solvelp",
        "at which reopttype should the LP be solved? (1: transit, 3: strong branched, 4: w/ added logicor, 5: only leafs).",
        set_target!(i32, reopt_solvelp),
        true,
        1,
        1,
        5,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/solvelpdiff",
        "maximal number of bound changes at node to skip solving the LP",
        set_target!(i32, reopt_solvelpdiff),
        true,
        1,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/savesols",
        "number of best solutions which should be saved for the following runs. (-1: save all)",
        set_target!(i32, reopt_savesols),
        true,
        i32::MAX,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "reoptimization/objsimrootLP",
        "similarity of two sequential objective function to disable solving the root LP.",
        set_target!(f64, reopt_objsimrootlp),
        true,
        0.8,
        -1.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "reoptimization/objsimsol",
        "similarity of two objective functions to re-use stored solutions",
        set_target!(f64, reopt_objsimsol),
        true,
        -1.0,
        -1.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "reoptimization/delay",
        "minimum similarity for using reoptimization of the search tree.",
        set_target!(f64, reopt_objsimdelay),
        true,
        -1.0,
        -1.0,
        1.0,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/commontimelimit",
        "time limit over all reoptimization rounds?.",
        set_target!(bool, reopt_commontimelimit),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/shrinkinner",
        "replace branched inner nodes by their child nodes, if the number of bound changes is not to large",
        set_target!(bool, reopt_shrinkinner),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/strongbranchinginit",
        "try to fix variables at the root node before reoptimizing by probing like strong branching",
        set_target!(bool, reopt_sbinit),
        true,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/reducetofrontier",
        "delete stored nodes which were not reoptimized",
        set_target!(bool, reopt_reducetofrontier),
        true,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/forceheurrestart",
        "force a restart if the last n optimal solutions were found by heuristic reoptsols",
        set_target!(i32, reopt_forceheurrestart),
        true,
        3,
        1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/saveconsprop",
        "save constraint and propagator propagations",
        set_target!(bool, reopt_saveprop),
        true,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/usesplitcons",
        "use constraints to reconstruct the subtree pruned be dual reduction when reactivating the node",
        set_target!(bool, reopt_usesplitcons),
        true,
        true,
        None,
    )?;
    paramset_add_char(
        scip,
        "reoptimization/varorderinterdiction",
        "use 'd'efault, 'r'andom or a variable ordering based on 'i'nference score for interdiction branching used during reoptimization",
        set_target!(u8, reopt_varorderinterdiction),
        true,
        b'd',
        Some("dir"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "reoptimization/usecuts",
        "reoptimize cuts found at the root node",
        set_target!(bool, reopt_usecuts),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "reoptimization/maxcutage",
        "maximal age of a cut to be use for reoptimization",
        set_target!(i32, reopt_maxcutage),
        true,
        0,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxbounddist",
        "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for applying separation (0.0: only on current best node, 1.0: on all nodes)",
        set_target!(f64, sepa_maxbounddist),
        false,
        1.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxlocalbounddist",
        "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for applying local separation (0.0: only on current best node, 1.0: on all nodes)",
        set_target!(f64, sepa_maxlocalbounddist),
        false,
        0.0,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxcoefratio",
        "maximal ratio between coefficients in strongcg, cmir, and flowcover cuts",
        set_target!(f64, sepa_maxcoefratio),
        false,
        1e+4,
        1.0,
        1e+99 / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxcoefratiofacrowprep",
        "maximal ratio between coefficients (as factor of 1/feastol) to ensure in rowprep cleanup",
        set_target!(f64, sepa_maxcoefratiofacrowprep),
        false,
        10.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/minefficacy",
        "minimal efficacy for a cut to enter the LP",
        set_target!(f64, sepa_minefficacy),
        false,
        1e-4,
        0.0,
        1e+99 / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/minefficacyroot",
        "minimal efficacy for a cut to enter the LP in the root node",
        set_target!(f64, sepa_minefficacyroot),
        false,
        1e-4,
        0.0,
        1e+99 / 10.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/minactivityquot",
        "minimum cut activity quotient to convert cuts into constraints during a restart (0.0: all cuts are converted)",
        set_target!(f64, sepa_minactivityquot),
        false,
        0.8,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxcutsgenfactor",
        "factor w.r.t. maxcuts for maximal number of cuts generated per separation round (-1.0: no limit, >= 0.0: valid finite limit)",
        set_target!(f64, sepa_maxcutsgenfactor),
        false,
        2.0,
        -1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/maxcutsrootgenfactor",
        "factor w.r.t. maxcutsroot for maximal number of generated cuts per separation round at the root node (-1.0: no limit, >= 0.0: valid finite limit)",
        set_target!(f64, sepa_maxcutsrootgenfactor),
        false,
        2.0,
        -1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_char(
        scip,
        "separating/orthofunc",
        "function used for calc. scalar prod. in orthogonality test ('e'uclidean, 'd'iscrete)",
        set_target!(u8, sepa_orthofunc),
        true,
        b'e',
        Some("ed"),
        None,
    )?;
    paramset_add_char(
        scip,
        "separating/efficacynorm",
        "row norm to use for efficacy calculation ('e'uclidean, 'm'aximum, 's'um, 'd'iscrete)",
        set_target!(u8, sepa_efficacynorm),
        true,
        b'e',
        Some("emsd"),
        None,
    )?;
    paramset_add_char(
        scip,
        "separating/cutselrestart",
        "cut selection during restart ('a'ge, activity 'q'uotient)",
        set_target!(u8, sepa_cutselrestart),
        true,
        b'a',
        Some("aq"),
        None,
    )?;
    paramset_add_char(
        scip,
        "separating/cutselsubscip",
        "cut selection for sub SCIPs  ('a'ge, activity 'q'uotient)",
        set_target!(u8, sepa_cutselsubscip),
        true,
        b'a',
        Some("aq"),
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/filtercutpoolrel",
        "should cutpool separate only cuts with high relative efficacy?",
        set_target!(bool, sepa_filtercutpoolrel),
        true,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxruns",
        "maximal number of runs for which separation is enabled (-1: unlimited)",
        set_target!(i32, sepa_maxruns),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxrounds",
        "maximal number of separation rounds per node (-1: unlimited)",
        set_target!(i32, sepa_maxrounds),
        false,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxroundsroot",
        "maximal number of separation rounds in the root node (-1: unlimited)",
        set_target!(i32, sepa_maxroundsroot),
        false,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxroundsrootsubrun",
        "maximal number of separation rounds in the root node of a subsequent run (-1: unlimited)",
        set_target!(i32, sepa_maxroundsrootsubrun),
        true,
        -1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxaddrounds",
        "maximal additional number of separation rounds in subsequent price-and-cut loops (-1: no additional restriction)",
        set_target!(i32, sepa_maxaddrounds),
        true,
        1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxstallrounds",
        "maximal number of consecutive separation rounds without objective or integrality improvement in local nodes (-1: no additional restriction)",
        set_target!(i32, sepa_maxstallrounds),
        false,
        1,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxstallroundsroot",
        "maximal number of consecutive separation rounds without objective or integrality improvement in the root node (-1: no additional restriction)",
        set_target!(i32, sepa_maxstallroundsroot),
        false,
        10,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxcuts",
        "maximal number of cuts separated per separation round (0: disable local separation)",
        set_target!(i32, sepa_maxcuts),
        false,
        100,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/maxcutsroot",
        "maximal number of separated cuts per separation round at the root node (0: disable root node separation)",
        set_target!(i32, sepa_maxcutsroot),
        false,
        2000,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/cutagelimit",
        "maximum age a cut can reach before it is deleted from the global cut pool, or -1 to keep all cuts",
        set_target!(i32, sepa_cutagelimit),
        true,
        80,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/poolfreq",
        "separation frequency for the global cut pool (-1: disable global cut pool, 0: only separate pool at the root)",
        set_target!(i32, sepa_poolfreq),
        false,
        10,
        -1,
        1073741822,
        None,
    )?;
    paramset_add_int(
        scip,
        "parallel/mode",
        "parallel optimisation mode, 0: opportunistic or 1: deterministic.",
        set_target!(i32, parallel_mode),
        false,
        1,
        0,
        1,
        None,
    )?;
    paramset_add_int(
        scip,
        "parallel/minnthreads",
        "the minimum number of threads used during parallel solve",
        set_target!(i32, parallel_minnthreads),
        false,
        1,
        0,
        64,
        None,
    )?;
    paramset_add_int(
        scip,
        "parallel/maxnthreads",
        "the maximum number of threads used during parallel solve",
        set_target!(i32, parallel_maxnthreads),
        false,
        8,
        0,
        64,
        None,
    )?;
    paramset_add_bool(
        scip,
        "concurrent/changeseeds",
        "set different random seeds in each concurrent solver?",
        set_target!(bool, concurrent_changeseeds),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "concurrent/changechildsel",
        "use different child selection rules in each concurrent solver?",
        set_target!(bool, concurrent_changechildsel),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "concurrent/commvarbnds",
        "should the concurrent solvers communicate global variable bound changes?",
        set_target!(bool, concurrent_commvarbnds),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "concurrent/presolvebefore",
        "should the problem be presolved before it is copied to the concurrent solvers?",
        set_target!(bool, concurrent_presolvebefore),
        false,
        true,
        None,
    )?;
    paramset_add_int(
        scip,
        "concurrent/initseed",
        "maximum number of solutions that will be shared in a one synchronization",
        set_target!(i32, concurrent_initseed),
        false,
        5131912,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "concurrent/sync/freqinit",
        "initial frequency of synchronization with other threads",
        set_target!(f64, concurrent_freqinit),
        false,
        10.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "concurrent/sync/freqmax",
        "maximal frequency of synchronization with other threads",
        set_target!(f64, concurrent_freqmax),
        false,
        10.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "concurrent/sync/freqfactor",
        "factor by which the frequency of synchronization is changed",
        set_target!(f64, concurrent_freqfactor),
        false,
        1.5,
        1.0,
        f64::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "concurrent/sync/targetprogress",
        "when adapting the synchronization frequency this value is the targeted relative difference by which the absolute gap decreases per synchronization",
        set_target!(f64, concurrent_targetprogress),
        false,
        0.001,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "concurrent/sync/maxnsols",
        "maximum number of solutions that will be shared in a single synchronization",
        set_target!(i32, concurrent_maxnsols),
        false,
        3,
        0,
        1000,
        None,
    )?;
    paramset_add_int(
        scip,
        "concurrent/sync/maxnsyncdelay",
        "maximum number of synchronizations before reading is enforced regardless of delay",
        set_target!(i32, concurrent_maxnsyncdelay),
        true,
        7,
        0,
        100,
        None,
    )?;
    paramset_add_real(
        scip,
        "concurrent/sync/minsyncdelay",
        "minimum delay before synchronization data is read",
        set_target!(f64, concurrent_minsyncdelay),
        false,
        10.0,
        0.0,
        f64::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "concurrent/sync/nbestsols",
        "how many of the N best solutions should be considered for synchronization?",
        set_target!(i32, concurrent_nbestsols),
        false,
        10,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_string(
        scip,
        "concurrent/paramsetprefix",
        "path prefix for parameter setting files of concurrent solvers",
        set_target!(String, concurrent_paramsetprefix),
        false,
        "",
        None,
    )?;
    paramset_add_int(
        scip,
        "timing/clocktype",
        "default clock type (1: CPU user seconds, 2: wall clock time)",
        set_target!(i32, time_clocktype),
        false,
        2,
        1,
        2,
        None,
    )?;
    paramset_add_bool(scip, "timing/enabled", "is timing enabled?", set_target!(bool, time_enabled), false, true, None)?;
    paramset_add_bool(
        scip,
        "timing/reading",
        "belongs reading time to solving time?",
        set_target!(bool, time_reading),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "timing/rareclockcheck",
        "should clock checks of solving time be performed less frequently (note: time limit could be exceeded slightly)",
        set_target!(bool, time_rareclockcheck),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "timing/statistictiming",
        "should timing for statistic output be performed?",
        set_target!(bool, time_statistictiming),
        false,
        true,
        Some(param_chgd_statistictiming),
    )?;
    paramset_add_bool(
        scip,
        "timing/nlpieval",
        "should time for evaluation in NLP solves be measured?",
        set_target!(bool, time_nlpieval),
        false,
        false,
        None,
    )?;
    paramset_add_string(
        scip,
        "visual/vbcfilename",
        "name of the VBC tool output file, or - if no VBC tool output should be created",
        set_target!(String, visual_vbcfilename),
        false,
        "-",
        None,
    )?;
    paramset_add_string(
        scip,
        "visual/bakfilename",
        "name of the BAK tool output file, or - if no BAK tool output should be created",
        set_target!(String, visual_bakfilename),
        false,
        "-",
        None,
    )?;
    paramset_add_bool(
        scip,
        "visual/realtime",
        "should the real solving time be used instead of a time step counter in visualization?",
        set_target!(bool, visual_realtime),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "visual/dispsols",
        "should the node where solutions are found be visualized?",
        set_target!(bool, visual_dispsols),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "visual/displb",
        "should lower bound information be visualized?",
        set_target!(bool, visual_displb),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "visual/objextern",
        "should be output the external value of the objective?",
        set_target!(bool, visual_objextern),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reading/initialconss",
        "should model constraints be marked as initial?",
        set_target!(bool, read_initialconss),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reading/dynamicconss",
        "should model constraints be subject to aging?",
        set_target!(bool, read_dynamicconss),
        false,
        true,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reading/dynamiccols",
        "should columns be added and removed dynamically to the LP?",
        set_target!(bool, read_dynamiccols),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "reading/dynamicrows",
        "should rows be added and removed dynamically to the LP?",
        set_target!(bool, read_dynamicrows),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "write/allconss",
        "should all constraints be written (including the redundant constraints)?",
        set_target!(bool, write_allconss),
        false,
        false,
        None,
    )?;
    paramset_add_bool(
        scip,
        "write/printzeros",
        "should variables set to zero be printed?",
        set_target!(bool, write_printzeros),
        false,
        false,
        None,
    )?;
    paramset_add_int(
        scip,
        "write/genericnamesoffset",
        "when writing a generic problem the index for the first variable should start with?",
        set_target!(i32, write_genoffset),
        false,
        0,
        0,
        i32::MAX / 2,
        None,
    )?;
    paramset_add_int(
        scip,
        "write/implintlevel",
        "should integrality constraints (i.c.) be written for implied integral variables? (0: use original i.c., 1: add i.c. to strongly implied integral vars, 2: add i.c. to all implied integral vars, -1: remove i.c. from strongly implied integral vars, -2: remove i.c. from all implied integral vars)",
        set_target!(i32, write_implintlevel),
        false,
        0,
        -2,
        2,
        None,
    )?;
    // Built without exact solving: the exact parameters keep their defaults
    // but are not parameters.
    scip.set.exact_enable = false;
    scip.set.exact_improvingsols = true;
    scip.set.exact_safedbmethod = b'a';
    scip.set.exact_psdualcolselection = 1;
    scip.set.exact_interleavedbstrat = 1;
    scip.set.exact_lpinfo = false;
    scip.set.exact_allownegslack = true;
    scip.set.exact_cutmaxdenom = 131072;
    scip.set.exact_cutapproxmaxboundval = 10000;
    scip.set.certificate_filename = "-".to_owned();
    scip.set.certificate_maxfilesize = (i64::MAX >> 20) as f64;
    Ok(())
}

/// `SCIPsetSetLimitChanged`.
pub fn set_set_limit_changed(scip: &mut Scip) {
    scip.set.limitchanged = true;
    scip.set.istimelimitfinite = scip.set.limit_time < 1e+20;
}

/// `paramChgdFeastol`.
fn param_chgd_feastol(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    todo!("SCIPchgFeastol")
}

/// `paramChgdLPFeastolFactor`.
fn param_chgd_lp_feastol_factor(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    todo!("SCIPresetLPFeastol")
}

/// `paramChgdDualfeastol`.
fn param_chgd_dualfeastol(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    todo!("SCIPchgDualfeastol")
}

/// `paramChgdBarrierconvtol`.
fn param_chgd_barrierconvtol(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    todo!("SCIPchgBarrierconvtol")
}

/// `paramChgdInfinity`: the LP solver's infinity must be at least as large.
fn param_chgd_infinity(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    todo!("check against SCIPlpiInfinity")
}

/// `SCIPparamChgdDispWidth`: display is not ported.
fn param_chgd_disp_width(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    Ok(())
}

/// `SCIPparamChgdLimit`: `SCIPmarkLimitChanged`.
fn param_chgd_limit(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    set_set_limit_changed(scip);
    Ok(())
}

/// `paramChgdArraygrowfac`: C's buffer memory; nothing to do here.
fn param_chgd_arraygrowfac(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    Ok(())
}

/// `paramChgdArraygrowinit`: C's buffer memory; nothing to do here.
fn param_chgd_arraygrowinit(_scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    Ok(())
}

/// `paramChgdEnableReopt`: reoptimisation is not ported.
fn param_chgd_enable_reopt(scip: &mut Scip, param: crate::paramset::ParamId) -> ScipResult {
    if crate::paramset::param_get_bool(scip, param) {
        return Err(crate::retcode::Retcode::ParameterWrongVal);
    }
    Ok(())
}

/// `paramChgdUsesymmetry`.
fn param_chgd_usesymmetry(scip: &mut Scip, param: crate::paramset::ParamId) -> ScipResult {
    if scip.set.stage >= Stage::InitPresolve && scip.set.stage <= Stage::Solved && crate::paramset::param_get_int(scip, param) > 0 {
        log::error!("Cannot turn on symmetry handling during (pre)solving or change method.");
        return Err(crate::retcode::Retcode::ParameterWrongVal);
    }
    Ok(())
}

/// `paramChgdStatistictiming`.
fn param_chgd_statistictiming(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    enable_or_disable_statistic_timing(scip);
    Ok(())
}

/// `SCIPenableOrDisableStatisticTiming` (`scip_timing.c`): enables or
/// disables the plugin and statistics clocks as `timing/statistictiming`
/// says.
pub fn enable_or_disable_statistic_timing(scip: &mut Scip) {
    let enabled = scip.set.time_statistictiming;
    set_enable_or_disable_plugin_clocks(scip, enabled);
    if scip.set.stage > Stage::Init {
        crate::stat::stat_enable_or_disable_stat_clocks(scip, enabled);
    }
    if scip.set.stage >= Stage::Transforming
        && let Some(conflict) = scip.conflict.as_mut()
    {
        crate::conflict::conflict_enable_or_disable_clocks(conflict, enabled);
    }
}

/// `SCIPsetEnableOrDisablePluginClocks`: enables or disables the clocks of
/// all plugins (readers, pricers and IIS finders are not ported).
pub fn set_enable_or_disable_plugin_clocks(scip: &mut Scip, enabled: bool) {
    for i in (0..scip.set.nconshdlrs).rev() {
        let h = scip.set.conshdlrs[i];
        let c = &mut scip.conshdlrs[h];
        for clock in [
            &mut c.setuptime,
            &mut c.checktime,
            &mut c.enfolptime,
            &mut c.enfopstime,
            &mut c.enforelaxtime,
            &mut c.presoltime,
            &mut c.proptime,
            &mut c.resproptime,
            &mut c.sbproptime,
            &mut c.sepatime,
        ] {
            clock.enable_or_disable(enabled);
        }
    }
    for i in (0..scip.set.nconflicthdlrs).rev() {
        let h = scip.set.conflicthdlrs[i];
        let c = &mut scip.conflicthdlrs[h];
        c.setuptime.enable_or_disable(enabled);
        c.conflicttime.enable_or_disable(enabled);
    }
    for i in (0..scip.set.npresols).rev() {
        let p = scip.set.presols[i];
        crate::presol::presol_enable_or_disable_clocks(scip, p, enabled);
    }
    for i in (0..scip.set.nrelaxs).rev() {
        let r = scip.set.relaxs[i];
        crate::relax::relax_enable_or_disable_clocks(scip, r, enabled);
    }
    for i in (0..scip.set.nsepas).rev() {
        let s = scip.set.sepas[i];
        crate::sepa::sepa_enable_or_disable_clocks(scip, s, enabled);
    }
    for i in (0..scip.set.nprops).rev() {
        let p = scip.set.props[i];
        crate::prop::prop_enable_or_disable_clocks(scip, p, enabled);
    }
    for i in (0..scip.set.nheurs).rev() {
        let h = scip.set.heurs[i];
        crate::heur::heur_enable_or_disable_clocks(scip, h, enabled);
    }
    for i in (0..scip.set.neventhdlrs).rev() {
        let e = scip.set.eventhdlrs[i];
        crate::event::eventhdlr_enable_or_disable_clocks(&mut scip.eventhdlrs[e], enabled);
    }
    for i in (0..scip.set.nnodesels).rev() {
        let n = scip.set.nodesels[i];
        crate::nodesel::nodesel_enable_or_disable_clocks(scip, n, enabled);
    }
    for i in (0..scip.set.ncutsels).rev() {
        let c = scip.set.cutsels[i];
        crate::cutsel::cutsel_enable_or_disable_clocks(scip, c, enabled);
    }
    for i in (0..scip.set.nbranchrules).rev() {
        let b = scip.set.branchrules[i];
        crate::branch::branchrule_enable_or_disable_clocks(scip, b, enabled);
    }
}

/// The numerics of `set.h` as compiled with `NDEBUG`, where they are
/// macros.
impl Set {
    /// `SCIPsetInfinity`.
    #[inline(always)]
    pub fn infinity(&self) -> f64 {
        self.num_infinity
    }

    /// `SCIPsetGetHugeValue`.
    #[inline(always)]
    pub fn get_huge_value(&self) -> f64 {
        self.num_hugeval
    }

    /// `SCIPsetEpsilon`.
    #[inline(always)]
    pub fn epsilon(&self) -> f64 {
        self.num_epsilon
    }

    /// `SCIPsetSumepsilon`.
    #[inline(always)]
    pub fn sumepsilon(&self) -> f64 {
        self.num_sumepsilon
    }

    /// `SCIPsetFeastol`.
    #[inline(always)]
    pub fn feastol(&self) -> f64 {
        self.num_feastol
    }

    /// `SCIPsetLPFeastolFactor`.
    #[inline(always)]
    pub fn lp_feastol_factor(&self) -> f64 {
        self.num_lpfeastolfactor
    }

    /// `SCIPsetDualfeastol`.
    #[inline(always)]
    pub fn dualfeastol(&self) -> f64 {
        self.num_dualfeastol
    }

    /// `SCIPsetBarrierconvtol`.
    #[inline(always)]
    pub fn barrierconvtol(&self) -> f64 {
        self.num_barrierconvtol
    }

    /// `SCIPsetPseudocosteps`.
    #[inline(always)]
    pub fn pseudocosteps(&self) -> f64 {
        self.num_pseudocosteps
    }

    /// `SCIPsetPseudocostdelta`.
    #[inline(always)]
    pub fn pseudocostdelta(&self) -> f64 {
        self.num_pseudocostdelta
    }

    /// `SCIPsetRelaxfeastol`.
    #[inline(always)]
    pub fn relaxfeastol(&self) -> f64 {
        self.num_relaxfeastol
    }

    /// `SCIPsetCutoffbounddelta`.
    #[inline(always)]
    pub fn cutoffbounddelta(&self) -> f64 {
        min(100.0 * self.num_feastol, 0.0001)
    }

    /// `SCIPsetRecompfac`.
    #[inline(always)]
    pub fn recompfac(&self) -> f64 {
        self.num_recompfac
    }

    /// `SCIPsetIsInfinity`.
    #[inline(always)]
    pub fn is_infinity(&self, val: f64) -> bool {
        val >= self.num_infinity
    }

    /// `SCIPsetIsHugeValue`.
    #[inline(always)]
    pub fn is_huge_value(&self, val: f64) -> bool {
        val >= self.num_hugeval
    }

    /// `SCIPsetIsScalingIntegral`.
    #[inline(always)]
    pub fn is_scaling_integral(&self, val: f64, scalar: f64) -> bool {
        eps_is_int(scalar * val, max(scalar.abs(), 1.0) * self.num_epsilon)
    }

    /// `SCIPsetIsLbBetter`.
    #[inline(always)]
    pub fn is_lb_better(&self, newlb: f64, oldlb: f64, oldub: f64) -> bool {
        (oldlb < 0.0 && newlb >= 0.0) || eps_gt(newlb, oldlb, self.num_boundstreps * max(min(oldub - oldlb, oldlb.abs()), 1e-3))
    }

    /// `SCIPsetIsUbBetter`.
    #[inline(always)]
    pub fn is_ub_better(&self, newub: f64, oldlb: f64, oldub: f64) -> bool {
        (oldub > 0.0 && newub <= 0.0) || eps_lt(newub, oldub, self.num_boundstreps * max(min(oldub - oldlb, oldub.abs()), 1e-3))
    }

    /// `SCIPsetIsEfficacious`.
    #[inline(always)]
    pub fn is_efficacious(&self, root: bool, efficacy: f64) -> bool {
        if root {
            eps_p(efficacy, self.sepa_minefficacyroot)
        } else {
            eps_p(efficacy, self.sepa_minefficacy)
        }
    }

    /// `SCIPsetIsUpdateUnreliable`.
    #[inline(always)]
    pub fn is_update_unreliable(&self, newvalue: f64, oldvalue: f64) -> bool {
        (oldvalue.abs() / max(newvalue.abs(), self.num_epsilon)) >= self.num_recompfac
    }

    /// `SCIPsetInitializeRandomSeed`: modifies an initial seed value with
    /// the global shift of random seeds. The reference is built without
    /// `NDEBUG`, so this is the function, not set.h's macro (which drops the
    /// `1 + 6 *`).
    #[inline(always)]
    pub fn initialize_random_seed(&self, val: u32) -> u32 {
        val.wrapping_add((1u32.wrapping_add(6u32.wrapping_mul(self.random_randomseedshiftmultiplier as u32))).wrapping_mul(self.random_randomseedshift as u32))
    }

    /// `SCIPsetGetSubscipsOff`.
    #[inline(always)]
    pub fn get_subscips_off(&self) -> bool {
        self.subscipsoff
    }

    /// `SCIPsetCalcMemGrowSize`.
    pub fn calc_mem_grow_size(&self, num: i32) -> i32 {
        calc_grow_size(self.mem_arraygrowinit, self.mem_arraygrowfac, num)
    }

    /// `SCIPsetCalcTreeGrowSize`.
    pub fn calc_tree_grow_size(&self, num: i32) -> i32 {
        calc_grow_size(self.mem_treegrowinit, self.mem_treegrowfac, num)
    }

    /// `SCIPsetCalcPathGrowSize`.
    pub fn calc_path_grow_size(&self, num: i32) -> i32 {
        calc_grow_size(self.mem_pathgrowinit, self.mem_pathgrowfac, num)
    }

    /// `SCIPsetGetSepaMaxcutsGenFactor`: returns the maximal ratio of cuts
    /// generated to cuts added per separation round.
    pub fn get_sepa_maxcuts_gen_factor(&self, root: bool) -> f64 {
        if root { self.sepa_maxcutsrootgenfactor } else { self.sepa_maxcutsgenfactor }
    }
    /// `SCIPsetGetSepaMaxcuts`.
    pub fn get_sepa_maxcuts(&self, root: bool) -> i32 {
        if root { self.sepa_maxcutsroot } else { self.sepa_maxcuts }
    }

    /// `SCIPsetGetSepaMaxCoefRatioRowprep`.
    pub fn get_sepa_max_coef_ratio_rowprep(&self) -> f64 {
        let mut maxcoefrange = self.sepa_maxcoefratiofacrowprep / self.num_feastol;
        if maxcoefrange < 1.0 {
            maxcoefrange = 1.0;
        }
        maxcoefrange
    }

    /// `SCIPsetGetPriceMaxvars`.
    pub fn get_price_maxvars(&self, root: bool) -> i32 {
        if root { self.price_maxvarsroot } else { self.price_maxvars }
    }

    /// `SCIPsetIsEQ`.
    #[inline(always)]
    pub fn is_eq(&self, val1: f64, val2: f64) -> bool {
        eps_eq(val1, val2, self.num_epsilon)
    }
    /// `SCIPsetIsLT`.
    #[inline(always)]
    pub fn is_lt(&self, val1: f64, val2: f64) -> bool {
        eps_lt(val1, val2, self.num_epsilon)
    }
    /// `SCIPsetIsLE`.
    #[inline(always)]
    pub fn is_le(&self, val1: f64, val2: f64) -> bool {
        eps_le(val1, val2, self.num_epsilon)
    }
    /// `SCIPsetIsGT`.
    #[inline(always)]
    pub fn is_gt(&self, val1: f64, val2: f64) -> bool {
        eps_gt(val1, val2, self.num_epsilon)
    }
    /// `SCIPsetIsGE`.
    #[inline(always)]
    pub fn is_ge(&self, val1: f64, val2: f64) -> bool {
        eps_ge(val1, val2, self.num_epsilon)
    }
    /// `SCIPsetIsZero`.
    #[inline(always)]
    pub fn is_zero(&self, val: f64) -> bool {
        eps_z(val, self.num_epsilon)
    }
    /// `SCIPsetIsPositive`.
    #[inline(always)]
    pub fn is_positive(&self, val: f64) -> bool {
        eps_p(val, self.num_epsilon)
    }
    /// `SCIPsetIsNegative`.
    #[inline(always)]
    pub fn is_negative(&self, val: f64) -> bool {
        eps_n(val, self.num_epsilon)
    }
    /// `SCIPsetIsIntegral`.
    #[inline(always)]
    pub fn is_integral(&self, val: f64) -> bool {
        eps_is_int(val, self.num_epsilon)
    }
    /// `SCIPsetIsFracIntegral`.
    #[inline(always)]
    pub fn is_frac_integral(&self, val: f64) -> bool {
        !eps_p(val, self.num_epsilon)
    }
    /// `SCIPsetFloor`.
    #[inline(always)]
    pub fn floor(&self, val: f64) -> f64 {
        eps_floor(val, self.num_epsilon)
    }
    /// `SCIPsetCeil`.
    #[inline(always)]
    pub fn ceil(&self, val: f64) -> f64 {
        eps_ceil(val, self.num_epsilon)
    }
    /// `SCIPsetRound`.
    #[inline(always)]
    pub fn round(&self, val: f64) -> f64 {
        eps_round(val, self.num_epsilon)
    }
    /// `SCIPsetFrac`.
    #[inline(always)]
    pub fn frac(&self, val: f64) -> f64 {
        eps_frac(val, self.num_epsilon)
    }
    /// `SCIPsetIsSumEQ`.
    #[inline(always)]
    pub fn is_sum_eq(&self, val1: f64, val2: f64) -> bool {
        eps_eq(val1, val2, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumLT`.
    #[inline(always)]
    pub fn is_sum_lt(&self, val1: f64, val2: f64) -> bool {
        eps_lt(val1, val2, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumLE`.
    #[inline(always)]
    pub fn is_sum_le(&self, val1: f64, val2: f64) -> bool {
        eps_le(val1, val2, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumGT`.
    #[inline(always)]
    pub fn is_sum_gt(&self, val1: f64, val2: f64) -> bool {
        eps_gt(val1, val2, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumGE`.
    #[inline(always)]
    pub fn is_sum_ge(&self, val1: f64, val2: f64) -> bool {
        eps_ge(val1, val2, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumZero`.
    #[inline(always)]
    pub fn is_sum_zero(&self, val: f64) -> bool {
        eps_z(val, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumPositive`.
    #[inline(always)]
    pub fn is_sum_positive(&self, val: f64) -> bool {
        eps_p(val, self.num_sumepsilon)
    }
    /// `SCIPsetIsSumNegative`.
    #[inline(always)]
    pub fn is_sum_negative(&self, val: f64) -> bool {
        eps_n(val, self.num_sumepsilon)
    }
    /// `SCIPsetSumFloor`.
    #[inline(always)]
    pub fn sum_floor(&self, val: f64) -> f64 {
        eps_floor(val, self.num_sumepsilon)
    }
    /// `SCIPsetSumCeil`.
    #[inline(always)]
    pub fn sum_ceil(&self, val: f64) -> f64 {
        eps_ceil(val, self.num_sumepsilon)
    }
    /// `SCIPsetSumRound`.
    #[inline(always)]
    pub fn sum_round(&self, val: f64) -> f64 {
        eps_round(val, self.num_sumepsilon)
    }
    /// `SCIPsetSumFrac`.
    #[inline(always)]
    pub fn sum_frac(&self, val: f64) -> f64 {
        eps_frac(val, self.num_sumepsilon)
    }
    /// `SCIPsetIsFeasEQ`.
    #[inline(always)]
    pub fn is_feas_eq(&self, val1: f64, val2: f64) -> bool {
        eps_z(rel_diff(val1, val2), self.num_feastol)
    }
    /// `SCIPsetIsFeasLT`.
    #[inline(always)]
    pub fn is_feas_lt(&self, val1: f64, val2: f64) -> bool {
        eps_n(rel_diff(val1, val2), self.num_feastol)
    }
    /// `SCIPsetIsFeasLE`.
    #[inline(always)]
    pub fn is_feas_le(&self, val1: f64, val2: f64) -> bool {
        !eps_p(rel_diff(val1, val2), self.num_feastol)
    }
    /// `SCIPsetIsFeasGT`.
    #[inline(always)]
    pub fn is_feas_gt(&self, val1: f64, val2: f64) -> bool {
        eps_p(rel_diff(val1, val2), self.num_feastol)
    }
    /// `SCIPsetIsFeasGE`.
    #[inline(always)]
    pub fn is_feas_ge(&self, val1: f64, val2: f64) -> bool {
        !eps_n(rel_diff(val1, val2), self.num_feastol)
    }
    /// `SCIPsetIsFeasZero`.
    #[inline(always)]
    pub fn is_feas_zero(&self, val: f64) -> bool {
        eps_z(val, self.num_feastol)
    }
    /// `SCIPsetIsFeasPositive`.
    #[inline(always)]
    pub fn is_feas_positive(&self, val: f64) -> bool {
        eps_p(val, self.num_feastol)
    }
    /// `SCIPsetIsFeasNegative`.
    #[inline(always)]
    pub fn is_feas_negative(&self, val: f64) -> bool {
        eps_n(val, self.num_feastol)
    }
    /// `SCIPsetIsFeasIntegral`.
    #[inline(always)]
    pub fn is_feas_integral(&self, val: f64) -> bool {
        eps_is_int(val, self.num_feastol)
    }
    /// `SCIPsetIsFeasFracIntegral`.
    #[inline(always)]
    pub fn is_feas_frac_integral(&self, val: f64) -> bool {
        !eps_p(val, self.num_feastol)
    }
    /// `SCIPsetFeasFloor`.
    #[inline(always)]
    pub fn feas_floor(&self, val: f64) -> f64 {
        eps_floor(val, self.num_feastol)
    }
    /// `SCIPsetFeasCeil`.
    #[inline(always)]
    pub fn feas_ceil(&self, val: f64) -> f64 {
        eps_ceil(val, self.num_feastol)
    }
    /// `SCIPsetFeasRound`.
    #[inline(always)]
    pub fn feas_round(&self, val: f64) -> f64 {
        eps_round(val, self.num_feastol)
    }
    /// `SCIPsetFeasFrac`.
    #[inline(always)]
    pub fn feas_frac(&self, val: f64) -> f64 {
        eps_frac(val, self.num_feastol)
    }
    /// `SCIPsetIsDualfeasEQ`.
    #[inline(always)]
    pub fn is_dualfeas_eq(&self, val1: f64, val2: f64) -> bool {
        eps_z(rel_diff(val1, val2), self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasLT`.
    #[inline(always)]
    pub fn is_dualfeas_lt(&self, val1: f64, val2: f64) -> bool {
        eps_n(rel_diff(val1, val2), self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasLE`.
    #[inline(always)]
    pub fn is_dualfeas_le(&self, val1: f64, val2: f64) -> bool {
        !eps_p(rel_diff(val1, val2), self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasGT`.
    #[inline(always)]
    pub fn is_dualfeas_gt(&self, val1: f64, val2: f64) -> bool {
        eps_p(rel_diff(val1, val2), self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasGE`.
    #[inline(always)]
    pub fn is_dualfeas_ge(&self, val1: f64, val2: f64) -> bool {
        !eps_n(rel_diff(val1, val2), self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasZero`.
    #[inline(always)]
    pub fn is_dualfeas_zero(&self, val: f64) -> bool {
        eps_z(val, self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasPositive`.
    #[inline(always)]
    pub fn is_dualfeas_positive(&self, val: f64) -> bool {
        eps_p(val, self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasNegative`.
    #[inline(always)]
    pub fn is_dualfeas_negative(&self, val: f64) -> bool {
        eps_n(val, self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasIntegral`.
    #[inline(always)]
    pub fn is_dualfeas_integral(&self, val: f64) -> bool {
        eps_is_int(val, self.num_dualfeastol)
    }
    /// `SCIPsetIsDualfeasFracIntegral`.
    #[inline(always)]
    pub fn is_dualfeas_frac_integral(&self, val: f64) -> bool {
        !eps_p(val, self.num_dualfeastol)
    }
    /// `SCIPsetDualfeasFloor`.
    #[inline(always)]
    pub fn dualfeas_floor(&self, val: f64) -> f64 {
        eps_floor(val, self.num_dualfeastol)
    }
    /// `SCIPsetDualfeasCeil`.
    #[inline(always)]
    pub fn dualfeas_ceil(&self, val: f64) -> f64 {
        eps_ceil(val, self.num_dualfeastol)
    }
    /// `SCIPsetDualfeasRound`.
    #[inline(always)]
    pub fn dualfeas_round(&self, val: f64) -> f64 {
        eps_round(val, self.num_dualfeastol)
    }
    /// `SCIPsetDualfeasFrac`.
    #[inline(always)]
    pub fn dualfeas_frac(&self, val: f64) -> f64 {
        eps_frac(val, self.num_dualfeastol)
    }
    /// `SCIPsetIsRelEQ`.
    #[inline(always)]
    pub fn is_rel_eq(&self, val1: f64, val2: f64) -> bool {
        eps_z(rel_diff(val1, val2), self.num_epsilon)
    }
    /// `SCIPsetIsRelLT`.
    #[inline(always)]
    pub fn is_rel_lt(&self, val1: f64, val2: f64) -> bool {
        eps_n(rel_diff(val1, val2), self.num_epsilon)
    }
    /// `SCIPsetIsRelLE`.
    #[inline(always)]
    pub fn is_rel_le(&self, val1: f64, val2: f64) -> bool {
        !eps_p(rel_diff(val1, val2), self.num_epsilon)
    }
    /// `SCIPsetIsRelGT`.
    #[inline(always)]
    pub fn is_rel_gt(&self, val1: f64, val2: f64) -> bool {
        eps_p(rel_diff(val1, val2), self.num_epsilon)
    }
    /// `SCIPsetIsRelGE`.
    #[inline(always)]
    pub fn is_rel_ge(&self, val1: f64, val2: f64) -> bool {
        !eps_n(rel_diff(val1, val2), self.num_epsilon)
    }
    /// `SCIPsetIsSumRelEQ`.
    #[inline(always)]
    pub fn is_sum_rel_eq(&self, val1: f64, val2: f64) -> bool {
        eps_z(rel_diff(val1, val2), self.num_sumepsilon)
    }
    /// `SCIPsetIsSumRelLT`.
    #[inline(always)]
    pub fn is_sum_rel_lt(&self, val1: f64, val2: f64) -> bool {
        eps_n(rel_diff(val1, val2), self.num_sumepsilon)
    }
    /// `SCIPsetIsSumRelLE`.
    #[inline(always)]
    pub fn is_sum_rel_le(&self, val1: f64, val2: f64) -> bool {
        !eps_p(rel_diff(val1, val2), self.num_sumepsilon)
    }
    /// `SCIPsetIsSumRelGT`.
    #[inline(always)]
    pub fn is_sum_rel_gt(&self, val1: f64, val2: f64) -> bool {
        eps_p(rel_diff(val1, val2), self.num_sumepsilon)
    }
    /// `SCIPsetIsSumRelGE`.
    #[inline(always)]
    pub fn is_sum_rel_ge(&self, val1: f64, val2: f64) -> bool {
        !eps_n(rel_diff(val1, val2), self.num_sumepsilon)
    }
}

/// `calcGrowSize`: the size the arrays of `set.c` grow to, computed by a
/// loop so the sizes are always the same.
pub fn calc_grow_size(mut initsize: i32, growfac: f64, num: i32) -> i32 {
    let mut size;
    if growfac == 1.0 {
        size = max(initsize, num);
    } else {
        initsize = max(initsize, 4);
        size = initsize;
        let mut oldsize = size - 1;
        while size < num && size > oldsize {
            oldsize = size;
            size = c_f64_to_i32(growfac * size as f64 + initsize as f64);
        }
        if size <= oldsize {
            size = num;
        }
    }
    size
}

/// `SCIPsetIncludeEventhdlr`: inserts event handler in event handler list.
pub fn set_include_eventhdlr(scip: &mut Scip, eventhdlr: usize) {
    let set = &mut scip.set;
    let n = set.neventhdlrs as usize;
    if set.eventhdlrs.len() <= n {
        let newsize = set.calc_mem_grow_size(set.neventhdlrs + 1);
        set.eventhdlrs.0.resize(newsize as usize, usize::MAX);
    }
    set.eventhdlrs[n] = eventhdlr;
    set.neventhdlrs += 1;
}

/// `SCIPsetFindEventhdlr`: returns the event handler of the given name, or
/// `NONE` if not existing.
pub fn set_find_eventhdlr(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.neventhdlrs as usize {
        let h = scip.set.eventhdlrs[i];
        if scip.eventhdlrs[h].name == name {
            return h;
        }
    }
    usize::MAX
}

/// Appends `id` to a plugin list holding `n` entries, growing it as
/// `SCIPsetCalcMemGrowSize` does.
fn set_list_push(calc: impl Fn(i32) -> i32, list: &mut CVec<usize>, n: &mut i32, id: usize) {
    if list.len() <= *n as usize {
        let newsize = calc(*n + 1);
        list.0.resize(newsize as usize, usize::MAX);
    }
    list[*n as usize] = id;
    *n += 1;
}

/// `SCIPsetIncludeConshdlr`: inserts constraint handler in constraint
/// handler list.
pub fn set_include_conshdlr(scip: &mut Scip, conshdlr: usize) {
    let n = scip.set.nconshdlrs as usize;

    // Allocate memory.
    if scip.set.conshdlrs.len() <= n {
        let newsize = scip.set.calc_mem_grow_size(scip.set.nconshdlrs + 1) as usize;
        let set = &mut scip.set;
        set.conshdlrs.0.resize(newsize, usize::MAX);
        set.conshdlrs_sepa.0.resize(newsize, usize::MAX);
        set.conshdlrs_enfo.0.resize(newsize, usize::MAX);
        set.conshdlrs_include.0.resize(newsize, usize::MAX);
    }

    let hdlrs = &scip.conshdlrs;
    let set = &mut scip.set;

    // Sort constraint handler into conshdlrs array sorted by check priority.
    let priority = hdlrs[conshdlr].checkpriority;
    let mut i = n;
    while i > 0 && hdlrs[set.conshdlrs[i - 1]].checkpriority < priority {
        set.conshdlrs[i] = set.conshdlrs[i - 1];
        i -= 1;
    }
    set.conshdlrs[i] = conshdlr;

    // Sort constraint handler into conshdlrs_sepa array sorted by sepa
    // priority.
    let priority = hdlrs[conshdlr].sepapriority;
    let mut i = n;
    while i > 0 && hdlrs[set.conshdlrs_sepa[i - 1]].sepapriority < priority {
        set.conshdlrs_sepa[i] = set.conshdlrs_sepa[i - 1];
        i -= 1;
    }
    set.conshdlrs_sepa[i] = conshdlr;

    // Sort constraint handler into conshdlrs_enfo array sorted by enfo
    // priority.
    let priority = hdlrs[conshdlr].enfopriority;
    let mut i = n;
    while i > 0 && hdlrs[set.conshdlrs_enfo[i - 1]].enfopriority < priority {
        set.conshdlrs_enfo[i] = set.conshdlrs_enfo[i - 1];
        i -= 1;
    }
    set.conshdlrs_enfo[i] = conshdlr;

    // Add constraint handler into conshdlrs_include array sorted by inclusion
    // order.
    set.conshdlrs_include[n] = conshdlr;

    set.nconshdlrs += 1;
}

/// `SCIPsetFindPresol`: returns the presolver of the given name, or NONE if
/// not existing.
pub fn set_find_presol(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.npresols as usize {
        let p = scip.set.presols[i];
        if scip.presols[p].name == name {
            return p;
        }
    }
    usize::MAX
}

/// `SCIPsetFindProp`: returns the propagator of the given name, or NONE if
/// not existing.
pub fn set_find_prop(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nprops as usize {
        let p = scip.set.props[i];
        if scip.props[p].name == name {
            return p;
        }
    }
    usize::MAX
}

/// `SCIPsetFindRelax`: returns the relaxation handler of the given name,
/// or `NONE` if not existing.
pub fn set_find_relax(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nrelaxs as usize {
        let r = scip.set.relaxs[i];
        if scip.relaxs[r].name == name {
            return r;
        }
    }
    usize::MAX
}

/// `SCIPsetFindHeur`: returns the primal heuristic of the given name, or
/// `NONE` if not existing.
pub fn set_find_heur(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nheurs as usize {
        let h = scip.set.heurs[i];
        if scip.heurs[h].name == name {
            return h;
        }
    }
    usize::MAX
}

/// `SCIPsetFindConshdlr`: returns the constraint handler of the given name,
/// or `NONE` if not existing.
pub fn set_find_conshdlr(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nconshdlrs as usize {
        let h = scip.set.conshdlrs[i];
        if scip.conshdlrs[h].name == name {
            return h;
        }
    }
    usize::MAX
}

/// `SCIPsetIncludeConflicthdlr`: inserts conflict handler in conflict
/// handler list.
pub fn set_include_conflicthdlr(scip: &mut Scip, h: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.conflicthdlrs, &mut s.nconflicthdlrs, h);
    s.conflicthdlrssorted = false;
}

/// `SCIPsetIncludePresol`: inserts presolver in presolver list.
pub fn set_include_presol(scip: &mut Scip, p: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.presols, &mut s.npresols, p);
    s.presolssorted = false;
}

/// `SCIPsetIncludeRelax`: inserts relaxator in relaxator list.
pub fn set_include_relax(scip: &mut Scip, r: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.relaxs, &mut s.nrelaxs, r);
    s.relaxssorted = false;
}

/// `SCIPsetIncludeSepa`: inserts separator in separator list.
pub fn set_include_sepa(scip: &mut Scip, sepa: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.sepas, &mut s.nsepas, sepa);
    s.sepassorted = false;
}

/// `SCIPsetIncludeCutsel`: inserts cut selector in cut selector list.
pub fn set_include_cutsel(scip: &mut Scip, c: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.cutsels, &mut s.ncutsels, c);
    s.cutselssorted = false;
}

/// `SCIPsetIncludeProp`: inserts propagator in propagator list.
pub fn set_include_prop(scip: &mut Scip, p: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    let mut n = s.nprops;
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.props, &mut n, p);
    let mut n2 = s.nprops;
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.props_presol, &mut n2, p);
    s.nprops = n;
    s.propssorted = false;
    s.propspresolsorted = false;
}

/// `SCIPsetIncludeHeur`: inserts primal heuristic in primal heuristic list.
pub fn set_include_heur(scip: &mut Scip, h: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.heurs, &mut s.nheurs, h);
    s.heurssorted = false;
}

/// `SCIPsetIncludeNodesel`: inserts node selector in node selector list.
pub fn set_include_nodesel(scip: &mut Scip, nodesel: usize) {
    let n = scip.set.nnodesels as usize;
    if scip.set.nodesels.len() <= n {
        let newsize = scip.set.calc_mem_grow_size(scip.set.nnodesels + 1) as usize;
        scip.set.nodesels.0.resize(newsize, usize::MAX);
    }

    let nodeselstdprio = scip.nodesels[nodesel].stdpriority;
    let mut i = n;
    while i > 0 && nodeselstdprio > scip.nodesels[scip.set.nodesels[i - 1]].stdpriority {
        scip.set.nodesels[i] = scip.set.nodesels[i - 1];
        i -= 1;
    }
    scip.set.nodesels[i] = nodesel;
    scip.set.nnodesels += 1;
}

/// `SCIPsetIncludeBranchrule`: inserts branching rule in branching rule
/// list.
pub fn set_include_branchrule(scip: &mut Scip, b: usize) {
    let s = &mut scip.set;
    let (g, f) = (s.mem_arraygrowinit, s.mem_arraygrowfac);
    set_list_push(|n| calc_grow_size(g, f, n), &mut s.branchrules, &mut s.nbranchrules, b);
    s.branchrulessorted = false;
}

/// `SCIPsetReinsertConshdlrSepaPrio`: reinserts a constraint handler with
/// modified sepa priority into the sorted list of separating constraint
/// handlers.
pub fn set_reinsert_conshdlr_sepa_prio(scip: &mut Scip, conshdlr: usize, oldpriority: i32) {
    let hdlrs = &scip.conshdlrs;
    let set = &mut scip.set;
    let newpriority = hdlrs[conshdlr].sepapriority;
    let mut newpos: i32 = -1;
    let n = set.nconshdlrs;
    let name = &hdlrs[conshdlr].name;

    // Search for the old position of constraint handler; determine its new
    // position at the same time.
    if newpriority > oldpriority {
        let mut i: i32 = 0;
        while i < n && hdlrs[set.conshdlrs_sepa[i]].name != *name {
            let priorityatpos = hdlrs[set.conshdlrs_sepa[i]].sepapriority;

            // Current index is the position to insert the constraint handler.
            if newpriority > priorityatpos && newpos == -1 {
                newpos = i;
            }
            i += 1;
        }

        // Constraint must change its position in array.
        if newpos != -1 {
            // Shift all constraint handlers between old and new position by one,
            // and insert constraint handler.
            while i > newpos {
                set.conshdlrs_sepa[i] = set.conshdlrs_sepa[i - 1];
                i -= 1;
            }
            set.conshdlrs_sepa[newpos] = conshdlr;
        }
    } else if newpriority < oldpriority {
        let mut i: i32 = n - 1;
        while i >= 0 && hdlrs[set.conshdlrs_sepa[i]].name != *name {
            let priorityatpos = hdlrs[set.conshdlrs_sepa[i]].sepapriority;

            // Current index is the position to insert the constraint handler.
            if newpriority < priorityatpos && newpos == -1 {
                newpos = i;
            }
            i -= 1;
        }

        // Constraint must change its position in array.
        if newpos != -1 {
            // Shift all constraint handlers between old and new position by one,
            // and insert constraint handler.
            while i < newpos {
                set.conshdlrs_sepa[i] = set.conshdlrs_sepa[i + 1];
                i += 1;
            }
            set.conshdlrs_sepa[newpos] = conshdlr;
        }
    }
}

/// `SCIPsetFindConflicthdlr`: returns the conflict handler of the given
/// name, or `usize::MAX` if not existing.
pub fn set_find_conflicthdlr(scip: &Scip, name: &str) -> usize {
    let s = &scip.set;
    for i in 0..s.nconflicthdlrs as usize {
        let h = s.conflicthdlrs[i];
        if scip.conflicthdlrs[h].name == name {
            return h;
        }
    }
    usize::MAX
}

/// Snapshots a plugin list of the set (the callbacks may not change it, but
/// they take `&mut Scip`).
fn plugin_list(list: &CVec<usize>, n: i32) -> Vec<usize> {
    list.0[..n as usize].to_vec()
}

/// `SCIPsetInitPlugins`: calls init methods of all plugins (pricers,
/// Benders, compressions, displays, tables, expression handlers and NLPIs
/// are not ported).
pub fn set_init_plugins(scip: &mut Scip) -> ScipResult {
    let s = &scip.set;
    let (conshdlrs, conflicthdlrs, presols, relaxs) = (
        plugin_list(&s.conshdlrs, s.nconshdlrs),
        plugin_list(&s.conflicthdlrs, s.nconflicthdlrs),
        plugin_list(&s.presols, s.npresols),
        plugin_list(&s.relaxs, s.nrelaxs),
    );
    let (sepas, cutsels, props, heurs) = (
        plugin_list(&s.sepas, s.nsepas),
        plugin_list(&s.cutsels, s.ncutsels),
        plugin_list(&s.props, s.nprops),
        plugin_list(&s.heurs, s.nheurs),
    );
    let (eventhdlrs, nodesels, branchrules) = (
        plugin_list(&s.eventhdlrs, s.neventhdlrs),
        plugin_list(&s.nodesels, s.nnodesels),
        plugin_list(&s.branchrules, s.nbranchrules),
    );

    for h in conshdlrs {
        crate::cons::conshdlr_init(scip, h)?;
    }
    for h in conflicthdlrs {
        crate::conflict::conflicthdlr_init(scip, h)?;
    }
    for p in presols {
        crate::presol::presol_init(scip, p)?;
    }
    for r in relaxs {
        crate::relax::relax_init(scip, r)?;
    }
    for s in sepas {
        crate::sepa::sepa_init(scip, s)?;
    }
    for c in cutsels {
        crate::cutsel::cutsel_init(scip, c)?;
    }
    for p in props {
        crate::prop::prop_init(scip, p)?;
    }
    for h in heurs {
        crate::heur::heur_init(scip, h)?;
    }
    for e in eventhdlrs {
        crate::event::eventhdlr_init(scip, e)?;
    }
    for n in nodesels {
        crate::nodesel::nodesel_init(scip, n)?;
    }
    for b in branchrules {
        crate::branch::branchrule_init(scip, b)?;
    }

    Ok(())
}

/// `SCIPsetExitPlugins`: calls exit methods of all plugins.
pub fn set_exit_plugins(scip: &mut Scip) -> ScipResult {
    let s = &scip.set;
    let (conshdlrs, conflicthdlrs, presols, relaxs) = (
        plugin_list(&s.conshdlrs, s.nconshdlrs),
        plugin_list(&s.conflicthdlrs, s.nconflicthdlrs),
        plugin_list(&s.presols, s.npresols),
        plugin_list(&s.relaxs, s.nrelaxs),
    );
    let (sepas, cutsels, props, heurs) = (
        plugin_list(&s.sepas, s.nsepas),
        plugin_list(&s.cutsels, s.ncutsels),
        plugin_list(&s.props, s.nprops),
        plugin_list(&s.heurs, s.nheurs),
    );
    let (eventhdlrs, nodesels, branchrules) = (
        plugin_list(&s.eventhdlrs, s.neventhdlrs),
        plugin_list(&s.nodesels, s.nnodesels),
        plugin_list(&s.branchrules, s.nbranchrules),
    );

    for h in conshdlrs {
        crate::cons::conshdlr_exit(scip, h)?;
    }
    for h in conflicthdlrs {
        crate::conflict::conflicthdlr_exit(scip, h)?;
    }
    for p in presols {
        crate::presol::presol_exit(scip, p)?;
    }
    for r in relaxs {
        crate::relax::relax_exit(scip, r)?;
    }
    for s in sepas {
        crate::sepa::sepa_exit(scip, s)?;
    }
    for c in cutsels {
        crate::cutsel::cutsel_exit(scip, c)?;
    }
    for p in props {
        crate::prop::prop_exit(scip, p)?;
    }
    for h in heurs {
        crate::heur::heur_exit(scip, h)?;
    }
    for e in eventhdlrs {
        crate::event::eventhdlr_exit(scip, e)?;
    }
    for n in nodesels {
        crate::nodesel::nodesel_exit(scip, n)?;
    }
    for b in branchrules {
        crate::branch::branchrule_exit(scip, b)?;
    }

    Ok(())
}

/// `SCIPsetInitprePlugins`: calls initpre methods of all plugins.
pub fn set_initpre_plugins(scip: &mut Scip) -> ScipResult {
    let s = &scip.set;
    let (presols, props, conshdlrs) = (
        plugin_list(&s.presols, s.npresols),
        plugin_list(&s.props, s.nprops),
        plugin_list(&s.conshdlrs, s.nconshdlrs),
    );

    for p in presols {
        crate::presol::presol_initpre(scip, p)?;
    }
    for p in props {
        crate::prop::prop_initpre(scip, p)?;
    }
    for h in conshdlrs {
        crate::cons::conshdlr_initpre(scip, h)?;
    }

    Ok(())
}

/// `SCIPsetExitprePlugins`: calls exitpre methods of all plugins.
pub fn set_exitpre_plugins(scip: &mut Scip) -> ScipResult {
    let s = &scip.set;
    let (presols, props, conshdlrs) = (
        plugin_list(&s.presols, s.npresols),
        plugin_list(&s.props, s.nprops),
        plugin_list(&s.conshdlrs, s.nconshdlrs),
    );

    for p in presols {
        crate::presol::presol_exitpre(scip, p)?;
    }
    for p in props {
        crate::prop::prop_exitpre(scip, p)?;
    }
    for h in conshdlrs {
        crate::cons::conshdlr_exitpre(scip, h)?;
    }

    Ok(())
}

/// `SCIPsetInitsolPlugins`: calls initsol methods of all plugins.
pub fn set_initsol_plugins(scip: &mut Scip) -> ScipResult {
    scip.set.num_relaxfeastol = def::INVALID;

    let s = &scip.set;
    let (conshdlrs, conflicthdlrs, relaxs, sepas) = (
        plugin_list(&s.conshdlrs, s.nconshdlrs),
        plugin_list(&s.conflicthdlrs, s.nconflicthdlrs),
        plugin_list(&s.relaxs, s.nrelaxs),
        plugin_list(&s.sepas, s.nsepas),
    );
    let (cutsels, props, heurs) = (plugin_list(&s.cutsels, s.ncutsels), plugin_list(&s.props, s.nprops), plugin_list(&s.heurs, s.nheurs));
    let (eventhdlrs, nodesels, branchrules) = (
        plugin_list(&s.eventhdlrs, s.neventhdlrs),
        plugin_list(&s.nodesels, s.nnodesels),
        plugin_list(&s.branchrules, s.nbranchrules),
    );

    for h in conshdlrs {
        crate::cons::conshdlr_initsol(scip, h)?;
    }
    for h in conflicthdlrs {
        crate::conflict::conflicthdlr_initsol(scip, h)?;
    }
    for r in relaxs {
        crate::relax::relax_initsol(scip, r)?;
    }
    for s in sepas {
        crate::sepa::sepa_initsol(scip, s)?;
    }
    for c in cutsels {
        crate::cutsel::cutsel_initsol(scip, c)?;
    }
    for p in props {
        crate::prop::prop_initsol(scip, p)?;
    }
    for h in heurs {
        crate::heur::heur_initsol(scip, h)?;
    }
    for e in eventhdlrs {
        crate::event::eventhdlr_initsol(scip, e)?;
    }
    for n in nodesels {
        crate::nodesel::nodesel_initsol(scip, n)?;
    }
    for b in branchrules {
        crate::branch::branchrule_initsol(scip, b)?;
    }

    Ok(())
}

/// `SCIPsetExitsolPlugins`: calls exitsol methods of all plugins.
pub fn set_exitsol_plugins(scip: &mut Scip, restart: bool) -> ScipResult {
    let s = &scip.set;
    let (conshdlrs, conflicthdlrs, relaxs, sepas) = (
        plugin_list(&s.conshdlrs, s.nconshdlrs),
        plugin_list(&s.conflicthdlrs, s.nconflicthdlrs),
        plugin_list(&s.relaxs, s.nrelaxs),
        plugin_list(&s.sepas, s.nsepas),
    );
    let (cutsels, props, heurs) = (plugin_list(&s.cutsels, s.ncutsels), plugin_list(&s.props, s.nprops), plugin_list(&s.heurs, s.nheurs));
    let (eventhdlrs, nodesels, branchrules) = (
        plugin_list(&s.eventhdlrs, s.neventhdlrs),
        plugin_list(&s.nodesels, s.nnodesels),
        plugin_list(&s.branchrules, s.nbranchrules),
    );

    for h in conshdlrs {
        crate::cons::conshdlr_exitsol(scip, h, restart)?;
    }
    for h in conflicthdlrs {
        crate::conflict::conflicthdlr_exitsol(scip, h)?;
    }
    for r in relaxs {
        crate::relax::relax_exitsol(scip, r)?;
    }
    for s in sepas {
        crate::sepa::sepa_exitsol(scip, s)?;
    }
    for c in cutsels {
        crate::cutsel::cutsel_exitsol(scip, c)?;
    }
    for p in props {
        crate::prop::prop_exitsol(scip, p, restart)?;
    }
    for h in heurs {
        crate::heur::heur_exitsol(scip, h)?;
    }
    for e in eventhdlrs {
        crate::event::eventhdlr_exitsol(scip, e)?;
    }
    for n in nodesels {
        crate::nodesel::nodesel_exitsol(scip, n)?;
    }
    for b in branchrules {
        crate::branch::branchrule_exitsol(scip, b)?;
    }

    Ok(())
}

/// `SCIPsetFindNodesel`: returns the node selector of the given name, or
/// `NONE` if not existing.
pub fn set_find_nodesel(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nnodesels as usize {
        let n = scip.set.nodesels[i];
        if scip.nodesels[n].name == name {
            return n;
        }
    }
    usize::MAX
}

/// `SCIPsetFindBranchrule`: returns the branching rule of the given name,
/// or `NONE` if not existing.
pub fn set_find_branchrule(scip: &Scip, name: &str) -> usize {
    for i in 0..scip.set.nbranchrules as usize {
        let b = scip.set.branchrules[i];
        if scip.branchrules[b].name == name {
            return b;
        }
    }
    usize::MAX
}
