//! SCIP's problem statistics (`stat.c`): counters, clocks and global
//! histories, many of which steer the search.

use crate::{
    clock::Clock,
    def::{DEFAULT_INFINITY, INVALID, REAL_MAX, REAL_MIN, UNKNOWN},
    history::History,
    misc::Regression,
    scip::Scip,
    types::{BranchDir, HeurId, LpSolStat, Status, VarId},
};

/// `SCIP_STAT`.
#[derive(Clone, Debug)]
pub struct Stat {
    pub regressioncandsobjval: Regression,
    pub nlpiterations: i64,
    pub nrootlpiterations: i64,
    pub nrootfirstlpiterations: i64,
    pub nprimallpiterations: i64,
    pub nduallpiterations: i64,
    pub nlexduallpiterations: i64,
    pub nbarrierlpiterations: i64,
    pub nprimalresolvelpiterations: i64,
    pub ndualresolvelpiterations: i64,
    pub nlexdualresolvelpiterations: i64,
    pub nnodelpiterations: i64,
    pub ninitlpiterations: i64,
    pub ndivinglpiterations: i64,
    pub ndivesetlpiterations: i64,
    pub nsbdivinglpiterations: i64,
    pub nsblpiterations: i64,
    pub nrootsblpiterations: i64,
    pub nconflictlpiterations: i64,
    pub nresolveinstablelps: i64,
    pub nresolveinstablelpiters: i64,
    pub nnodes: i64,
    pub ninternalnodes: i64,
    pub nobjleaves: i64,
    pub nfeasleaves: i64,
    pub ninfeasleaves: i64,
    pub ntotalnodes: i64,
    pub ntotalinternalnodes: i64,
    pub ntotalnodesmerged: i64,
    pub ncreatednodes: i64,
    pub ncreatednodesrun: i64,
    pub nactivatednodes: i64,
    pub ndeactivatednodes: i64,
    pub nearlybacktracks: i64,
    pub nnodesaboverefbound: i64,
    pub nbacktracks: i64,
    pub ndelayedcutoffs: i64,
    pub nreprops: i64,
    pub nrepropboundchgs: i64,
    pub nrepropcutoffs: i64,
    pub nlpsolsfound: i64,
    pub nrelaxsolsfound: i64,
    pub npssolsfound: i64,
    pub nsbsolsfound: i64,
    pub nlpbestsolsfound: i64,
    pub nrelaxbestsolsfound: i64,
    pub npsbestsolsfound: i64,
    pub nsbbestsolsfound: i64,
    pub nexternalsolsfound: i64,
    pub lastdispnode: i64,
    pub lastdivenode: i64,
    pub lastconflictnode: i64,
    pub bestsolnode: i64,
    pub domchgcount: i64,
    pub nboundchgs: i64,
    pub nholechgs: i64,
    pub nprobboundchgs: i64,
    pub nprobholechgs: i64,
    pub nsbdowndomchgs: i64,
    pub nsbupdomchgs: i64,
    pub nsbtimesiterlimhit: i64,
    pub nnodesbeforefirst: i64,
    pub ninitconssadded: i64,
    pub nactiveconssadded: i64,
    pub externmemestim: i64,
    pub exprlastvisitedtag: i64,
    pub exprlastsoltag: i64,
    pub exprlastdifftag: i64,
    pub avgnnz: f64,
    pub firstlpdualbound: f64,
    pub rootlowerbound: f64,
    pub vsidsweight: f64,
    pub firstprimalbound: f64,
    pub firstprimaltime: f64,
    pub firstsolgap: f64,
    pub lastsolgap: f64,
    pub primalzeroittime: f64,
    pub dualzeroittime: f64,
    pub barrierzeroittime: f64,
    pub maxcopytime: f64,
    pub mincopytime: f64,
    pub firstlptime: f64,
    pub lastbranchvalue: f64,
    pub dualrefintegral: f64,
    pub primalrefintegral: f64,
    pub primaldualintegral: f64,
    pub previousgap: f64,
    pub previousdualrefgap: f64,
    pub previousprimalrefgap: f64,
    pub previntegralevaltime: f64,
    pub lastprimalbound: f64,
    pub lastdualbound: f64,
    pub lastlowerbound: f64,
    pub lastupperbound: f64,
    pub rootlpbestestimate: f64,
    pub referencebound: f64,
    pub bestefficacy: f64,
    pub minefficacyfac: f64,
    pub detertimecnt: f64,
    pub boundingerrorbs: f64,
    pub boundingerrorps: f64,
    pub boundingerrorexlp: f64,
    pub timefailexlpinf: f64,
    pub timefailexlp: f64,
    pub solvingtime: Clock,
    pub solvingtimeoverall: Clock,
    pub presolvingtime: Clock,
    pub presolvingtimeoverall: Clock,
    pub primallptime: Clock,
    pub duallptime: Clock,
    pub lexduallptime: Clock,
    pub barrierlptime: Clock,
    pub resolveinstablelptime: Clock,
    pub divinglptime: Clock,
    pub strongbranchtime: Clock,
    pub conflictlptime: Clock,
    pub provedfeaslptime: Clock,
    pub provedinfeaslptime: Clock,
    pub provedfeasbstime: Clock,
    pub provedinfeasbstime: Clock,
    pub provedfeaspstime: Clock,
    pub provedinfeaspstime: Clock,
    pub lpsoltime: Clock,
    pub relaxsoltime: Clock,
    pub pseudosoltime: Clock,
    pub sbsoltime: Clock,
    pub nodeactivationtime: Clock,
    pub nlpsoltime: Clock,
    pub copyclock: Clock,
    pub strongpropclock: Clock,
    pub reoptupdatetime: Clock,
    pub glbhistory: History,
    pub glbhistorycrun: History,
    pub lastbranchvar: VarId,
    pub firstprimalheur: HeurId,
    pub status: Status,
    pub lastbranchdir: BranchDir,
    pub lastsblpsolstats: [LpSolStat; 2],
    pub nnz: i64,
    pub lpcount: i64,
    pub relaxcount: i64,
    pub nlps: i64,
    pub nrootlps: i64,
    pub nprimallps: i64,
    pub nprimalzeroitlps: i64,
    pub nduallps: i64,
    pub ndualzeroitlps: i64,
    pub nlexduallps: i64,
    pub nbarrierlps: i64,
    pub nbarrierzeroitlps: i64,
    pub nprimalresolvelps: i64,
    pub ndualresolvelps: i64,
    pub nlexdualresolvelps: i64,
    pub nnodelps: i64,
    pub nnodezeroitlps: i64,
    pub ninitlps: i64,
    pub ndivinglps: i64,
    pub ndivesetlps: i64,
    pub nsbdivinglps: i64,
    pub nnumtroublelpmsgs: i64,
    pub nstrongbranchs: i64,
    pub nrootstrongbranchs: i64,
    pub nconflictlps: i64,
    pub nexlpinf: i64,
    pub nfailexlpinf: i64,
    pub nexlp: i64,
    pub nexlpinter: i64,
    pub nexlpintfeas: i64,
    pub nexlpboundexc: i64,
    pub nfailexlp: i64,
    pub nboundshift: i64,
    pub nfailboundshift: i64,
    pub nboundshiftinf: i64,
    pub nfailboundshiftinf: i64,
    pub nboundshiftobjlim: i64,
    pub nboundshiftobjlimfail: i64,
    pub nprojshift: i64,
    pub nfailprojshift: i64,
    pub nprojshiftinf: i64,
    pub nfailprojshiftinf: i64,
    pub nprojshiftobjlim: i64,
    pub nprojshiftobjlimfail: i64,
    pub niterationsexlp: i64,
    pub niterationsexlpinf: i64,
    pub nnlps: i64,
    pub nisstoppedcalls: i64,
    pub totaldivesetdepth: i64,
    pub subscipdepth: i32,
    pub ndivesetcalls: i32,
    pub nruns: i32,
    pub ncutpoolfails: i32,
    pub nconfrestarts: i32,
    pub nrootboundchgs: i32,
    pub nrootboundchgsrun: i32,
    pub nrootintfixings: i32,
    pub nrootintfixingsrun: i32,
    pub prevrunnvars: i32,
    pub nvaridx: i32,
    pub ncolidx: i32,
    pub nrowidx: i32,
    pub marked_nvaridx: i32,
    pub marked_ncolidx: i32,
    pub marked_nrowidx: i32,
    pub npricerounds: i32,
    pub nseparounds: i32,
    pub ndisplines: i32,
    pub maxdepth: i32,
    pub maxtotaldepth: i32,
    pub plungedepth: i32,
    pub nactiveconss: i32,
    pub nenabledconss: i32,
    pub nimplications: i32,
    pub npresolrounds: i32,
    pub npresolroundsfast: i32,
    pub npresolroundsmed: i32,
    pub npresolroundsext: i32,
    pub npresolfixedvars: i32,
    pub npresolaggrvars: i32,
    pub npresolchgvartypes: i32,
    pub npresolchgbds: i32,
    pub npresoladdholes: i32,
    pub npresoldelconss: i32,
    pub npresoladdconss: i32,
    pub npresolupgdconss: i32,
    pub npresolchgcoefs: i32,
    pub npresolchgsides: i32,
    pub lastnpresolfixedvars: i32,
    pub lastnpresolaggrvars: i32,
    pub lastnpresolchgvartypes: i32,
    pub lastnpresolchgbds: i32,
    pub lastnpresoladdholes: i32,
    pub lastnpresoldelconss: i32,
    pub lastnpresoladdconss: i32,
    pub lastnpresolupgdconss: i32,
    pub lastnpresolchgcoefs: i32,
    pub lastnpresolchgsides: i32,
    pub lastnpresolimplications: i32,
    pub lastnpresolcliques: i32,
    pub solindex: i32,
    pub nrunsbeforefirst: i32,
    pub firstprimaldepth: i32,
    pub ncopies: i32,
    pub nreoptruns: i32,
    pub nclockskipsleft: i32,
    pub nactiveexpriter: i32,
    pub memsavemode: bool,
    pub userinterrupt: bool,
    pub userrestart: bool,
    pub inrestart: bool,
    pub collectvarhistory: bool,
    pub performpresol: bool,
    pub branchedunbdvar: bool,
    pub disableenforelaxmsg: bool,
}

impl Default for Stat {
    fn default() -> Self {
        Self {
            regressioncandsobjval: Regression::default(),
            nlpiterations: 0,
            nrootlpiterations: 0,
            nrootfirstlpiterations: 0,
            nprimallpiterations: 0,
            nduallpiterations: 0,
            nlexduallpiterations: 0,
            nbarrierlpiterations: 0,
            nprimalresolvelpiterations: 0,
            ndualresolvelpiterations: 0,
            nlexdualresolvelpiterations: 0,
            nnodelpiterations: 0,
            ninitlpiterations: 0,
            ndivinglpiterations: 0,
            ndivesetlpiterations: 0,
            nsbdivinglpiterations: 0,
            nsblpiterations: 0,
            nrootsblpiterations: 0,
            nconflictlpiterations: 0,
            nresolveinstablelps: 0,
            nresolveinstablelpiters: 0,
            nnodes: 0,
            ninternalnodes: 0,
            nobjleaves: 0,
            nfeasleaves: 0,
            ninfeasleaves: 0,
            ntotalnodes: 0,
            ntotalinternalnodes: 0,
            ntotalnodesmerged: 0,
            ncreatednodes: 0,
            ncreatednodesrun: 0,
            nactivatednodes: 0,
            ndeactivatednodes: 0,
            nearlybacktracks: 0,
            nnodesaboverefbound: 0,
            nbacktracks: 0,
            ndelayedcutoffs: 0,
            nreprops: 0,
            nrepropboundchgs: 0,
            nrepropcutoffs: 0,
            nlpsolsfound: 0,
            nrelaxsolsfound: 0,
            npssolsfound: 0,
            nsbsolsfound: 0,
            nlpbestsolsfound: 0,
            nrelaxbestsolsfound: 0,
            npsbestsolsfound: 0,
            nsbbestsolsfound: 0,
            nexternalsolsfound: 0,
            lastdispnode: 0,
            lastdivenode: 0,
            lastconflictnode: 0,
            bestsolnode: 0,
            domchgcount: 0,
            nboundchgs: 0,
            nholechgs: 0,
            nprobboundchgs: 0,
            nprobholechgs: 0,
            nsbdowndomchgs: 0,
            nsbupdomchgs: 0,
            nsbtimesiterlimhit: 0,
            nnodesbeforefirst: 0,
            ninitconssadded: 0,
            nactiveconssadded: 0,
            externmemestim: 0,
            exprlastvisitedtag: 0,
            exprlastsoltag: 0,
            exprlastdifftag: 0,
            avgnnz: 0.0,
            firstlpdualbound: 0.0,
            rootlowerbound: 0.0,
            vsidsweight: 0.0,
            firstprimalbound: 0.0,
            firstprimaltime: 0.0,
            firstsolgap: 0.0,
            lastsolgap: 0.0,
            primalzeroittime: 0.0,
            dualzeroittime: 0.0,
            barrierzeroittime: 0.0,
            maxcopytime: 0.0,
            mincopytime: 0.0,
            firstlptime: 0.0,
            lastbranchvalue: 0.0,
            dualrefintegral: 0.0,
            primalrefintegral: 0.0,
            primaldualintegral: 0.0,
            previousgap: 0.0,
            previousdualrefgap: 0.0,
            previousprimalrefgap: 0.0,
            previntegralevaltime: 0.0,
            lastprimalbound: 0.0,
            lastdualbound: 0.0,
            lastlowerbound: 0.0,
            lastupperbound: 0.0,
            rootlpbestestimate: 0.0,
            referencebound: 0.0,
            bestefficacy: 0.0,
            minefficacyfac: 0.0,
            detertimecnt: 0.0,
            boundingerrorbs: 0.0,
            boundingerrorps: 0.0,
            boundingerrorexlp: 0.0,
            timefailexlpinf: 0.0,
            timefailexlp: 0.0,
            solvingtime: Clock::new(),
            solvingtimeoverall: Clock::new(),
            presolvingtime: Clock::new(),
            presolvingtimeoverall: Clock::new(),
            primallptime: Clock::new(),
            duallptime: Clock::new(),
            lexduallptime: Clock::new(),
            barrierlptime: Clock::new(),
            resolveinstablelptime: Clock::new(),
            divinglptime: Clock::new(),
            strongbranchtime: Clock::new(),
            conflictlptime: Clock::new(),
            provedfeaslptime: Clock::new(),
            provedinfeaslptime: Clock::new(),
            provedfeasbstime: Clock::new(),
            provedinfeasbstime: Clock::new(),
            provedfeaspstime: Clock::new(),
            provedinfeaspstime: Clock::new(),
            lpsoltime: Clock::new(),
            relaxsoltime: Clock::new(),
            pseudosoltime: Clock::new(),
            sbsoltime: Clock::new(),
            nodeactivationtime: Clock::new(),
            nlpsoltime: Clock::new(),
            copyclock: Clock::new(),
            strongpropclock: Clock::new(),
            reoptupdatetime: Clock::new(),
            glbhistory: History::new(),
            glbhistorycrun: History::new(),
            lastbranchvar: crate::var::NONE,
            firstprimalheur: crate::var::NONE,
            status: Status::Unknown,
            lastbranchdir: BranchDir::Downwards,
            lastsblpsolstats: [LpSolStat::NotSolved; 2],
            nnz: 0,
            lpcount: 0,
            relaxcount: 0,
            nlps: 0,
            nrootlps: 0,
            nprimallps: 0,
            nprimalzeroitlps: 0,
            nduallps: 0,
            ndualzeroitlps: 0,
            nlexduallps: 0,
            nbarrierlps: 0,
            nbarrierzeroitlps: 0,
            nprimalresolvelps: 0,
            ndualresolvelps: 0,
            nlexdualresolvelps: 0,
            nnodelps: 0,
            nnodezeroitlps: 0,
            ninitlps: 0,
            ndivinglps: 0,
            ndivesetlps: 0,
            nsbdivinglps: 0,
            nnumtroublelpmsgs: 0,
            nstrongbranchs: 0,
            nrootstrongbranchs: 0,
            nconflictlps: 0,
            nexlpinf: 0,
            nfailexlpinf: 0,
            nexlp: 0,
            nexlpinter: 0,
            nexlpintfeas: 0,
            nexlpboundexc: 0,
            nfailexlp: 0,
            nboundshift: 0,
            nfailboundshift: 0,
            nboundshiftinf: 0,
            nfailboundshiftinf: 0,
            nboundshiftobjlim: 0,
            nboundshiftobjlimfail: 0,
            nprojshift: 0,
            nfailprojshift: 0,
            nprojshiftinf: 0,
            nfailprojshiftinf: 0,
            nprojshiftobjlim: 0,
            nprojshiftobjlimfail: 0,
            niterationsexlp: 0,
            niterationsexlpinf: 0,
            nnlps: 0,
            nisstoppedcalls: 0,
            totaldivesetdepth: 0,
            subscipdepth: 0,
            ndivesetcalls: 0,
            nruns: 0,
            ncutpoolfails: 0,
            nconfrestarts: 0,
            nrootboundchgs: 0,
            nrootboundchgsrun: 0,
            nrootintfixings: 0,
            nrootintfixingsrun: 0,
            prevrunnvars: 0,
            nvaridx: 0,
            ncolidx: 0,
            nrowidx: 0,
            marked_nvaridx: 0,
            marked_ncolidx: 0,
            marked_nrowidx: 0,
            npricerounds: 0,
            nseparounds: 0,
            ndisplines: 0,
            maxdepth: 0,
            maxtotaldepth: 0,
            plungedepth: 0,
            nactiveconss: 0,
            nenabledconss: 0,
            nimplications: 0,
            npresolrounds: 0,
            npresolroundsfast: 0,
            npresolroundsmed: 0,
            npresolroundsext: 0,
            npresolfixedvars: 0,
            npresolaggrvars: 0,
            npresolchgvartypes: 0,
            npresolchgbds: 0,
            npresoladdholes: 0,
            npresoldelconss: 0,
            npresoladdconss: 0,
            npresolupgdconss: 0,
            npresolchgcoefs: 0,
            npresolchgsides: 0,
            lastnpresolfixedvars: 0,
            lastnpresolaggrvars: 0,
            lastnpresolchgvartypes: 0,
            lastnpresolchgbds: 0,
            lastnpresoladdholes: 0,
            lastnpresoldelconss: 0,
            lastnpresoladdconss: 0,
            lastnpresolupgdconss: 0,
            lastnpresolchgcoefs: 0,
            lastnpresolchgsides: 0,
            lastnpresolimplications: 0,
            lastnpresolcliques: 0,
            solindex: 0,
            nrunsbeforefirst: 0,
            firstprimaldepth: 0,
            ncopies: 0,
            nreoptruns: 0,
            nclockskipsleft: 0,
            nactiveexpriter: 0,
            memsavemode: false,
            userinterrupt: false,
            userrestart: false,
            inrestart: false,
            collectvarhistory: false,
            performpresol: false,
            branchedunbdvar: false,
            disableenforelaxmsg: false,
        }
    }
}

/// `SCIPstatCreate`: called with the settings in place, as C does.
pub fn stat_create(scip: &mut Scip) {
    scip.stat = Stat::default();
    let statistictiming = scip.set.time_statistictiming;
    stat_enable_or_disable_stat_clocks(scip, statistictiming);
    scip.stat.status = Status::Unknown;
    scip.stat.marked_nvaridx = 0;
    scip.stat.marked_ncolidx = 0;
    scip.stat.marked_nrowidx = 0;
    scip.stat.subscipdepth = 0;
    scip.stat.detertimecnt = 0.0;
    scip.stat.nreoptruns = 0;
    stat_reset(scip);
}

/// `SCIPstatDisableVarHistory`.
pub fn stat_disable_var_history(scip: &mut Scip) {
    scip.stat.collectvarhistory = false;
}

/// `SCIPstatEnableVarHistory`.
pub fn stat_enable_var_history(scip: &mut Scip) {
    scip.stat.collectvarhistory = true;
}

/// `SCIPstatMark`.
pub fn stat_mark(scip: &mut Scip) {
    scip.stat.marked_nvaridx = scip.stat.nvaridx;
    scip.stat.marked_ncolidx = scip.stat.ncolidx;
    scip.stat.marked_nrowidx = scip.stat.nrowidx;
}

/// `SCIPstatReset`.
pub fn stat_reset(scip: &mut Scip) {
    scip.stat.solvingtime.reset();
    scip.stat.presolvingtime.reset();
    scip.stat.primallptime.reset();
    scip.stat.duallptime.reset();
    scip.stat.lexduallptime.reset();
    scip.stat.barrierlptime.reset();
    scip.stat.resolveinstablelptime.reset();
    scip.stat.divinglptime.reset();
    scip.stat.strongbranchtime.reset();
    scip.stat.conflictlptime.reset();
    scip.stat.provedfeaslptime.reset();
    scip.stat.provedinfeaslptime.reset();
    scip.stat.provedfeasbstime.reset();
    scip.stat.provedinfeasbstime.reset();
    scip.stat.provedfeaspstime.reset();
    scip.stat.provedinfeaspstime.reset();
    scip.stat.lpsoltime.reset();
    scip.stat.relaxsoltime.reset();
    scip.stat.pseudosoltime.reset();
    scip.stat.sbsoltime.reset();
    scip.stat.nodeactivationtime.reset();
    scip.stat.nlpsoltime.reset();
    scip.stat.copyclock.reset();
    scip.stat.strongpropclock.reset();
    scip.stat.glbhistory.reset();
    scip.stat.lastsblpsolstats = [LpSolStat::NotSolved; 2];
    scip.stat.vsidsweight = 1.0;
    scip.stat.nlpiterations = 0;
    scip.stat.nrootlpiterations = 0;
    scip.stat.nrootfirstlpiterations = 0;
    scip.stat.nprimallpiterations = 0;
    scip.stat.nduallpiterations = 0;
    scip.stat.nlexduallpiterations = 0;
    scip.stat.nbarrierlpiterations = 0;
    scip.stat.nprimalresolvelpiterations = 0;
    scip.stat.ndualresolvelpiterations = 0;
    scip.stat.nlexdualresolvelpiterations = 0;
    scip.stat.nnodelpiterations = 0;
    scip.stat.ninitlpiterations = 0;
    scip.stat.ndivinglpiterations = 0;
    scip.stat.nsbdivinglpiterations = 0;
    scip.stat.nsblpiterations = 0;
    scip.stat.nsbtimesiterlimhit = 0;
    scip.stat.nrootsblpiterations = 0;
    scip.stat.nconflictlpiterations = 0;
    scip.stat.nresolveinstablelps = 0;
    scip.stat.nresolveinstablelpiters = 0;
    scip.stat.ntotalnodes = 0;
    scip.stat.ntotalinternalnodes = 0;
    scip.stat.ntotalnodesmerged = 0;
    scip.stat.ncreatednodes = 0;
    scip.stat.nlpsolsfound = 0;
    scip.stat.nrelaxsolsfound = 0;
    scip.stat.npssolsfound = 0;
    scip.stat.nsbsolsfound = 0;
    scip.stat.nlpbestsolsfound = 0;
    scip.stat.nrelaxbestsolsfound = 0;
    scip.stat.npsbestsolsfound = 0;
    scip.stat.nsbbestsolsfound = 0;
    scip.stat.nexternalsolsfound = 0;
    scip.stat.domchgcount = 0;
    scip.stat.nboundchgs = 0;
    scip.stat.nholechgs = 0;
    scip.stat.nprobboundchgs = 0;
    scip.stat.nprobholechgs = 0;
    scip.stat.nsbdowndomchgs = 0;
    scip.stat.nsbupdomchgs = 0;
    scip.stat.nruns = 0;
    scip.stat.nconfrestarts = 0;
    scip.stat.nrootboundchgs = 0;
    scip.stat.nrootintfixings = 0;
    scip.stat.prevrunnvars = 0;
    scip.stat.nvaridx = scip.stat.marked_nvaridx;
    scip.stat.ncolidx = scip.stat.marked_ncolidx;
    scip.stat.nrowidx = scip.stat.marked_nrowidx;
    scip.stat.nnz = 0;
    scip.stat.avgnnz = 0.0;
    scip.stat.lpcount = 0;
    scip.stat.relaxcount = 0;
    scip.stat.nlps = 0;
    scip.stat.nrootlps = 0;
    scip.stat.nprimallps = 0;
    scip.stat.nprimalzeroitlps = 0;
    scip.stat.nduallps = 0;
    scip.stat.ndualzeroitlps = 0;
    scip.stat.nlexduallps = 0;
    scip.stat.nbarrierlps = 0;
    scip.stat.nbarrierzeroitlps = 0;
    scip.stat.nprimalresolvelps = 0;
    scip.stat.ndualresolvelps = 0;
    scip.stat.nlexdualresolvelps = 0;
    scip.stat.nnodelps = 0;
    scip.stat.nnodezeroitlps = 0;
    scip.stat.nisstoppedcalls = 0;
    scip.stat.ninitlps = 0;
    scip.stat.ndivinglps = 0;
    scip.stat.nsbdivinglps = 0;
    scip.stat.nnumtroublelpmsgs = 0;
    scip.stat.nstrongbranchs = 0;
    scip.stat.nrootstrongbranchs = 0;
    scip.stat.nconflictlps = 0;
    scip.stat.nexlp = 0;
    scip.stat.nexlpinter = 0;
    scip.stat.nexlpboundexc = 0;
    scip.stat.nexlpintfeas = 0;
    scip.stat.timefailexlpinf = 0.0;
    scip.stat.timefailexlp = 0.0;
    scip.stat.nfailexlp = 0;
    scip.stat.nboundshift = 0;
    scip.stat.nfailboundshift = 0;
    scip.stat.nboundshiftinf = 0;
    scip.stat.nfailboundshiftinf = 0;
    scip.stat.nboundshiftobjlim = 0;
    scip.stat.nboundshiftobjlimfail = 0;
    scip.stat.nprojshift = 0;
    scip.stat.nfailprojshift = 0;
    scip.stat.nprojshiftinf = 0;
    scip.stat.nfailprojshiftinf = 0;
    scip.stat.nprojshiftobjlim = 0;
    scip.stat.nprojshiftobjlimfail = 0;
    scip.stat.niterationsexlp = 0;
    scip.stat.niterationsexlpinf = 0;
    scip.stat.nexlpinf = 0;
    scip.stat.nfailexlpinf = 0;
    scip.stat.nnlps = 0;
    scip.stat.maxtotaldepth = -1;
    scip.stat.nactiveconss = 0;
    scip.stat.nenabledconss = 0;
    scip.stat.solindex = 0;
    scip.stat.memsavemode = false;
    scip.stat.nnodesbeforefirst = -1;
    scip.stat.ninitconssadded = 0;
    scip.stat.nactiveconssadded = 0;
    scip.stat.externmemestim = 0;
    scip.stat.exprlastvisitedtag = 0;
    scip.stat.exprlastsoltag = 0;
    scip.stat.exprlastdifftag = 0;
    scip.stat.nrunsbeforefirst = -1;
    scip.stat.firstprimalheur = crate::var::NONE;
    scip.stat.firstprimaltime = DEFAULT_INFINITY;
    scip.stat.firstprimalbound = DEFAULT_INFINITY;
    scip.stat.firstprimaldepth = -1;
    scip.stat.firstsolgap = DEFAULT_INFINITY;
    scip.stat.lastsolgap = DEFAULT_INFINITY;
    scip.stat.primalzeroittime = 0.0;
    scip.stat.dualzeroittime = 0.0;
    scip.stat.barrierzeroittime = 0.0;
    scip.stat.maxcopytime = REAL_MIN;
    scip.stat.mincopytime = REAL_MAX;
    scip.stat.firstlptime = 0.0;
    scip.stat.firstlpdualbound = UNKNOWN;
    scip.stat.ncopies = 0;
    scip.stat.nclockskipsleft = 0;
    scip.stat.nactiveexpriter = 0;
    scip.stat.marked_nvaridx = -1;
    scip.stat.marked_ncolidx = -1;
    scip.stat.marked_nrowidx = -1;
    scip.stat.branchedunbdvar = false;
    scip.stat.bestefficacy = 0.0;
    scip.stat.minefficacyfac = 0.5;
    scip.stat.ncutpoolfails = 0;
    scip.stat.ndivesetlpiterations = 0;
    scip.stat.ndivesetcalls = 0;
    scip.stat.ndivesetlps = 0;
    scip.stat.totaldivesetdepth = 0;
    scip.stat.userinterrupt = false;
    scip.stat.userrestart = false;
    scip.stat.inrestart = false;
    scip.stat.collectvarhistory = true;
    scip.stat.performpresol = false;
    scip.stat.disableenforelaxmsg = false;
    stat_reset_implications(scip);
    stat_reset_presolving(scip);
    stat_reset_primal_dual_integrals(scip, false);
}

/// `SCIPstatResetImplications`.
pub fn stat_reset_implications(scip: &mut Scip) {
    scip.stat.nimplications = 0;
}

/// `SCIPstatResetPresolving`.
pub fn stat_reset_presolving(scip: &mut Scip) {
    scip.stat.npresolrounds = 0;
    scip.stat.npresolroundsfast = 0;
    scip.stat.npresolroundsmed = 0;
    scip.stat.npresolroundsext = 0;
    scip.stat.npresolfixedvars = 0;
    scip.stat.npresolaggrvars = 0;
    scip.stat.npresolchgvartypes = 0;
    scip.stat.npresolchgbds = 0;
    scip.stat.npresoladdholes = 0;
    scip.stat.npresoldelconss = 0;
    scip.stat.npresoladdconss = 0;
    scip.stat.npresolupgdconss = 0;
    scip.stat.npresolchgcoefs = 0;
    scip.stat.npresolchgsides = 0;
    stat_reset_current_run(scip, false);
}

/// `SCIPstatResetPrimalDualIntegrals`.
pub fn stat_reset_primal_dual_integrals(scip: &mut Scip, partialreset: bool) {
    scip.stat.previousgap = 100.0;
    scip.stat.previousdualrefgap = 100.0;
    scip.stat.previousprimalrefgap = 100.0;
    scip.stat.lastprimalbound = UNKNOWN;
    scip.stat.lastdualbound = UNKNOWN;
    scip.stat.lastlowerbound = -scip.set.infinity();
    scip.stat.lastupperbound = scip.set.infinity();
    if !partialreset {
        scip.stat.previntegralevaltime = 0.0;
        scip.stat.dualrefintegral = 0.0;
        scip.stat.primalrefintegral = 0.0;
        scip.stat.primaldualintegral = 0.0;
    }
}

/// `SCIPstatResetCurrentRun`.
pub fn stat_reset_current_run(scip: &mut Scip, solved: bool) {
    scip.stat.nnodes = 0;
    scip.stat.ninternalnodes = 0;
    scip.stat.ncreatednodesrun = 0;
    scip.stat.nactivatednodes = 0;
    scip.stat.ndeactivatednodes = 0;
    scip.stat.nbacktracks = 0;
    scip.stat.ndelayedcutoffs = 0;
    scip.stat.nreprops = 0;
    scip.stat.nrepropboundchgs = 0;
    scip.stat.nrepropcutoffs = 0;
    scip.stat.lastdivenode = 0;
    scip.stat.lastconflictnode = 0;
    scip.stat.bestsolnode = 0;
    scip.stat.rootlowerbound = REAL_MIN;
    scip.stat.lastbranchvalue = UNKNOWN;
    scip.stat.rootlpbestestimate = INVALID;
    scip.stat.boundingerrorbs = 0.0;
    scip.stat.boundingerrorps = 0.0;
    scip.stat.boundingerrorexlp = 0.0;
    scip.stat.lastbranchvar = crate::var::NONE;
    scip.stat.lastbranchdir = BranchDir::Downwards;
    scip.stat.nrootboundchgsrun = 0;
    scip.stat.nrootintfixingsrun = 0;
    scip.stat.npricerounds = 0;
    scip.stat.nseparounds = 0;
    scip.stat.maxdepth = -1;
    scip.stat.plungedepth = 0;
    scip.stat.nobjleaves = 0;
    scip.stat.ninfeasleaves = 0;
    scip.stat.nfeasleaves = 0;
    scip.stat.branchedunbdvar = false;
    scip.stat.nnumtroublelpmsgs = 0;
    scip.stat.nearlybacktracks = 0;
    scip.stat.nnodesaboverefbound = 0;
    if scip.probs.contains(crate::prob::TRANSPROB) && !scip.set.is_infinity(scip.set.misc_referencevalue) {
        scip.stat.referencebound = crate::prob::prob_intern_objval(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, scip.set.misc_referencevalue);
    } else {
        scip.stat.referencebound = scip.set.infinity();
    }
    if !solved {
        scip.stat.status = Status::Unknown;
    }
    scip.stat.glbhistorycrun.reset();
    scip.stat.regressioncandsobjval.reset();
    stat_reset_display(scip);
}

/// `SCIPstatResetDisplay`.
pub fn stat_reset_display(scip: &mut Scip) {
    scip.stat.lastdispnode = 0;
    scip.stat.ndisplines = 0;
}

/// `SCIPstatEnforceLPUpdates`.
pub fn stat_enforce_lp_updates(scip: &mut Scip) {
    scip.stat.lpcount += 1;
}

/// `SCIPstatEnableOrDisableStatClocks`.
pub fn stat_enable_or_disable_stat_clocks(scip: &mut Scip, enable: bool) {
    let stat = &mut scip.stat;
    for clock in [
        &mut stat.primallptime,
        &mut stat.duallptime,
        &mut stat.lexduallptime,
        &mut stat.barrierlptime,
        &mut stat.resolveinstablelptime,
        &mut stat.divinglptime,
        &mut stat.strongbranchtime,
        &mut stat.conflictlptime,
        &mut stat.provedfeaslptime,
        &mut stat.provedinfeaslptime,
        &mut stat.provedfeasbstime,
        &mut stat.provedinfeasbstime,
        &mut stat.provedfeaspstime,
        &mut stat.provedinfeaspstime,
        &mut stat.lpsoltime,
        &mut stat.relaxsoltime,
        &mut stat.pseudosoltime,
        &mut stat.sbsoltime,
        &mut stat.nodeactivationtime,
        &mut stat.nlpsoltime,
        &mut stat.copyclock,
        &mut stat.strongpropclock,
    ] {
        if enable { clock.enable() } else { clock.disable() }
    }
}

/// `SCIPstatComputeRootLPBestEstimate`: recomputes the root LP best
/// estimate from the root LP value and the variables' pseudocost scores
/// (integral variables come first).
pub fn stat_compute_root_lp_best_estimate(scip: &mut Scip, rootlpobjval: f64, vars: &[crate::types::VarId]) {
    scip.stat.rootlpbestestimate = rootlpobjval;

    // Compute the pseudo costs of the variables' root solutions.
    for &var in vars {
        if !crate::var::var_is_integral(scip, var) {
            break;
        }
        let rootlpsol = crate::var::var_get_root_sol(scip, var);
        let varminpseudoscore = crate::var::var_get_min_pseudocost_score(scip, var, rootlpsol);
        scip.stat.rootlpbestestimate += varminpseudoscore;
    }
}

/// `SCIPstatUpdateVarRootLPBestEstimate`: updates the root LP estimate
/// after the variable's pseudocosts changed.
pub fn stat_update_var_root_lp_best_estimate(scip: &mut Scip, var: crate::types::VarId, oldrootpscostscore: f64) -> crate::retcode::ScipResult {
    // Entering this method is only useful if the root LP estimate is
    // valid.
    if scip.stat.rootlpbestestimate == crate::def::INVALID {
        return Ok(());
    }
    let rootlpsol = crate::var::var_get_root_sol(scip, var);

    // The root LP estimate only changes for fractional root solutions.
    if scip.set.is_feas_integral(rootlpsol) {
        return Ok(());
    }

    // Subtract the old pseudo cost contribution, add the new one.
    scip.stat.rootlpbestestimate -= oldrootpscostscore;
    let varminpseudoscore = crate::var::var_get_min_pseudocost_score(scip, var, rootlpsol);
    scip.stat.rootlpbestestimate += varminpseudoscore;
    Ok(())
}

/// `getGap`: returns the gap bounded by 100.
fn get_gap(set: &crate::set::Set, primalbound: f64, dualbound: f64, upperbound: f64, lowerbound: f64) -> f64 {
    // Computation of the gap, special cases are handled first.
    if primalbound >= UNKNOWN || dualbound >= UNKNOWN {
        100.0
    }
    // The gap is 0.0 if bounds coincide.
    else if set.is_ge(lowerbound, upperbound) || set.is_eq(primalbound, dualbound) {
        0.0
    }
    // The gap is 100.0 if bounds have different signs.
    else if primalbound * dualbound <= 0.0 {
        100.0
    } else if !set.is_infinity(primalbound.abs()) && !set.is_infinity(dualbound.abs()) {
        let absprim = primalbound.abs();
        let absdual = dualbound.abs();
        // The gap in the definition of the primal-dual integral differs from
        // the default SCIP gap function. Here, the MAX(primalbound,
        // dualbound) is taken for gap quotient in order to ensure a gap <=
        // 100.
        100.0 * (primalbound - dualbound).abs() / crate::def::max(absprim, absdual)
    } else {
        100.0
    }
}

/// `SCIPstatUpdatePrimalDualIntegrals`: update the primal-dual,
/// primal-reference, and reference-dual integral statistics. Accepts + and
/// - infinity as values for upper and lower bound, respectively.
pub fn stat_update_primal_dual_integrals(scip: &mut Scip, transprob: usize, origprob: usize, mut upperbound: f64, mut lowerbound: f64) {
    let solvingtime = crate::def::max(scip.stat.solvingtime.get_time(), scip.stat.previntegralevaltime);

    let primalbound;
    if !scip.set.is_infinity(upperbound) {
        // Get value in original space for gap calculation.
        let p = crate::prob::prob_extern_objval(scip, transprob, origprob, upperbound);
        primalbound = if scip.set.is_zero(p) { 0.0 } else { p };
    } else {
        // No new upper bound: use stored values from last update.
        upperbound = scip.stat.lastupperbound;
        primalbound = scip.stat.lastprimalbound;
    }

    let dualbound;
    if !scip.set.is_infinity(-lowerbound) {
        // Get value in original space for gap calculation.
        let d = crate::prob::prob_extern_objval(scip, transprob, origprob, lowerbound);
        dualbound = if scip.set.is_zero(d) { 0.0 } else { d };
    } else {
        // No new lower bound: use stored values from last update.
        lowerbound = scip.stat.lastlowerbound;
        dualbound = scip.stat.lastdualbound;
    }

    // Calculate primal-dual and dual reference gap.
    let currentgap = get_gap(&scip.set, primalbound, dualbound, upperbound, lowerbound);

    // Update the integral based on previous information.
    let stat = &mut scip.stat;
    let deltatime = solvingtime - stat.previntegralevaltime;
    stat.primaldualintegral += deltatime * stat.previousgap;
    stat.dualrefintegral += deltatime * stat.previousdualrefgap;
    stat.primalrefintegral += deltatime * stat.previousprimalrefgap;

    let (currentdualrefgap, currentprimalrefgap);
    let referencevalue = scip.set.misc_referencevalue;
    if !scip.set.is_infinity(referencevalue.abs()) {
        currentdualrefgap = get_gap(&scip.set, referencevalue, dualbound, upperbound, lowerbound);
        currentprimalrefgap = get_gap(&scip.set, primalbound, referencevalue, upperbound, lowerbound);
    } else {
        currentdualrefgap = 100.0;
        currentprimalrefgap = 100.0;
    }

    // Update all relevant information for next evaluation.
    let stat = &mut scip.stat;
    stat.previousgap = currentgap;
    stat.previousdualrefgap = currentdualrefgap;
    stat.previousprimalrefgap = currentprimalrefgap;
    stat.previntegralevaltime = solvingtime;
    stat.lastprimalbound = primalbound;
    stat.lastdualbound = dualbound;
    stat.lastlowerbound = lowerbound;
    stat.lastupperbound = upperbound;
}

/// `SCIPstatGetPrimalDualIntegral`: optionally update and return the
/// primal-dual integral statistic.
pub fn stat_get_primal_dual_integral(scip: &mut Scip, transprob: usize, origprob: usize, update: bool) -> f64 {
    if update {
        let inf = scip.set.infinity();
        stat_update_primal_dual_integrals(scip, transprob, origprob, inf, -inf);
    }
    scip.stat.primaldualintegral
}

/// `SCIPstatUpdateMemsaveMode`: updates the memory saving mode. SCIP's
/// block memory use is not measured here (it reads as 0); Improve keeps the
/// default memory limit, which no instance reaches, so the mode never
/// switches on.
pub fn stat_update_memsave_mode(scip: &mut Scip) {
    if scip.set.is_lt(scip.set.mem_savefac, 1.0) {
        let memused = 0.0;
        let limit = scip.set.mem_savefac * scip.set.limit_memory * 1024.0 * 1024.0;

        if !scip.stat.memsavemode && memused >= limit {
            // Switch to memory saving mode.
            scip.stat.memsavemode = true;
            scip.set.nodesel = crate::var::NONE;
        } else if scip.stat.memsavemode && memused < 0.5 * limit {
            // Switch to standard mode.
            scip.stat.memsavemode = false;
            scip.set.nodesel = crate::var::NONE;
        }
    } else {
        scip.stat.memsavemode = false;
    }
}
