//! Solve an MPS file with HiGHS, printing its developer log.
//!
//! `highs_log <file.mps> [log_dev_level] [presolve] [solver]`: presolve is
//! off unless the third argument is `on`; the fourth names HiGHS's solver.
use highs_sys::*;

fn main() {
    let path = std::ffi::CString::new(std::env::args().nth(1).expect("an MPS file")).unwrap();
    let level: HighsInt = std::env::args().nth(2).map_or(3, |level| level.parse().unwrap());
    // SAFETY: plain calls on a fresh instance.
    unsafe {
        let highs = Highs_create();
        Highs_setIntOptionValue(highs, c"threads".as_ptr(), 1);
        Highs_setIntOptionValue(highs, c"log_dev_level".as_ptr(), level);
        if std::env::args().nth(3).as_deref() != Some("on") {
            Highs_setStringOptionValue(highs, c"presolve".as_ptr(), c"off".as_ptr());
        }
        Highs_setBoolOptionValue(highs, c"presolve_rule_logging".as_ptr(), 1);
        if let Some(solver) = std::env::args().nth(4) {
            let solver = std::ffi::CString::new(solver).unwrap();
            Highs_setStringOptionValue(highs, c"solver".as_ptr(), solver.as_ptr());
        }
        Highs_readModel(highs, path.as_ptr());
        Highs_run(highs);
        Highs_destroy(highs);
    }
}
