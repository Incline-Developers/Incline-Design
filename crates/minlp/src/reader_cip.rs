//! The writer half of `reader_cip.c` (`readerWriteCip`), with the print
//! callbacks it reaches for the original problems this port builds:
//! `SCIPvarPrint`, `SCIPconsPrint` with the linear, indicator and
//! nonlinear handlers' `CONSPRINT`, and `SCIPexprPrint` over sum, product,
//! power, variable and value expressions. Its output reads back into the
//! reference SCIP as the same problem, so a model built by an application
//! can be solved again there.

use std::fmt::Write as _;

use crate::{
    misc::fmt_g_prec,
    prob::ORIGPROB,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ConsId, ExprId, ImplintType, ObjSense, VarId, VarStatus, VarType},
};

/// `%.15g`.
fn g15(x: f64) -> String {
    fmt_g_prec(x, 15)
}

/// `%+.15g`.
fn g15_signed(x: f64) -> String {
    let s = g15(x);
    if s.starts_with('-') { s } else { format!("+{s}") }
}

/// `SCIPwriteOrigProblem` in the `cip` format, as text.
pub fn write_orig_problem(scip: &Scip) -> ScipResult<String> {
    let prob = &scip.probs[ORIGPROB];
    let mut out = String::new();
    let nvars = prob.nvars as usize;
    let nconss = prob.nconss as usize;

    let _ = writeln!(out, "STATISTICS");
    let _ = writeln!(out, "  Problem name     : {}", prob.name);
    let _ = writeln!(
        out,
        "  Variables        : {} ({} binary, {} integer, {} implicit integer, {} continuous)",
        prob.nvars,
        prob.nbinvars,
        prob.nintvars,
        prob.nbinimplvars + prob.nintimplvars + prob.ncontimplvars,
        prob.ncontvars
    );
    let _ = writeln!(out, "  Constraints      : {} initial, {} maximal", prob.startnconss, prob.maxnconss);

    let _ = writeln!(out, "OBJECTIVE");
    let _ = writeln!(out, "  Sense            : {}", if prob.objsense == ObjSense::Minimize { "minimize" } else { "maximize" });
    // The original problem's offset and scale are not adapted.
    if prob.objoffset != 0.0 {
        let _ = writeln!(out, "  Offset           : {}", g15_signed(prob.objoffset));
    }
    if prob.objscale != 1.0 {
        let _ = writeln!(out, "  Scale            : {}", g15(prob.objscale));
    }

    // An original problem has no fixed variables.
    debug_assert_eq!(prob.nfixedvars, 0);
    if nvars > 0 {
        let _ = writeln!(out, "VARIABLES");
        for i in 0..nvars {
            var_print(scip, prob.vars.0[i], &mut out)?;
        }
    }

    if nconss > 0 {
        let _ = writeln!(out, "CONSTRAINTS");
        for i in 0..nconss {
            cons_print(scip, prob.conss.0[i], &mut out)?;
            let _ = writeln!(out, ";");
        }
    }
    let _ = writeln!(out, "END");
    Ok(out)
}

/// `printBounds`.
fn print_bounds(scip: &Scip, out: &mut String, lb: f64, ub: f64, name: &str) {
    let bound = |b: f64| {
        if scip.set.is_infinity(b) {
            "+inf".to_string()
        } else if scip.set.is_infinity(-b) {
            "-inf".to_string()
        } else {
            g15(b)
        }
    };
    let _ = write!(out, ", {name}=[{},{}]", bound(lb), bound(ub));
}

/// `SCIPvarPrint` for an original variable without exact data or holes.
fn var_print(scip: &Scip, var: VarId, out: &mut String) -> ScipResult {
    let v = &scip.vars[var];
    let mut vartype = v.vartype;
    let impltype = v.varimpltype;

    // Change integrality constraints of implied integral variables based on
    // the writing settings.
    let implintlevel = scip.set.write_implintlevel;
    if vartype == VarType::Continuous {
        if impltype as i32 > 2 - implintlevel {
            vartype = VarType::Integer;
        }
    } else if impltype as i32 > 2 + implintlevel {
        vartype = VarType::Continuous;
    }

    match vartype {
        VarType::Binary => out.push_str("  [binary]"),
        VarType::Integer => out.push_str("  [integer]"),
        VarType::Continuous => out.push_str("  [continuous]"),
        VarType::ImplInt => {
            log::error!("unknown variable type");
            return Err(Retcode::InvalidData);
        }
    }

    let _ = write!(out, " <{}>:", v.name);
    let _ = write!(out, " obj={}", g15(v.obj));

    if v.varstatus != VarStatus::Original {
        unimplemented!("SCIPvarPrint of a transformed variable");
    }
    print_bounds(scip, out, v.data.original.origdom.lb, v.data.original.origdom.ub, "original bounds");
    let (lb, ub) = (v.lazylb, v.lazyub);
    if !scip.set.is_infinity(-lb) || !scip.set.is_infinity(ub) {
        print_bounds(scip, out, lb, ub, "lazy bounds");
    }
    if v.data.original.origdom.holelist.is_some() {
        unimplemented!("printHolelist");
    }

    match impltype {
        ImplintType::None => {}
        ImplintType::Weak => out.push_str(", implied: weak"),
        ImplintType::Strong => out.push_str(", implied: strong"),
    }

    out.push('\n');
    Ok(())
}

/// `SCIPconsPrint`.
fn cons_print(scip: &Scip, cons: ConsId, out: &mut String) -> ScipResult {
    let hdlr = &scip.conshdlrs[scip.conss[cons].conshdlr].name;
    let _ = write!(out, "  [{hdlr}] <{}>: ", scip.conss[cons].name);
    match hdlr.as_str() {
        "linear" => print_linear(scip, cons, out),
        "indicator" => print_indicator(scip, cons, out),
        "nonlinear" => print_nonlinear(scip, cons, out),
        other => unimplemented!("CONSPRINT of constraint handler <{other}>"),
    }
    Ok(())
}

/// The sides as `consdataPrint` (`cons_linear.c`) and `consPrintNonlinear`
/// print them around the body.
fn print_sides(scip: &Scip, out: &mut String, lhs: f64, rhs: f64, body: impl FnOnce(&mut String)) {
    // Print left hand side for ranged rows.
    if !scip.set.is_infinity(-lhs) && !scip.set.is_infinity(rhs) && !scip.set.is_eq(lhs, rhs) {
        let _ = write!(out, "{} <= ", g15(lhs));
    }
    body(out);
    if scip.set.is_eq(lhs, rhs) {
        let _ = write!(out, " == {}", g15(rhs));
    } else if !scip.set.is_infinity(rhs) {
        let _ = write!(out, " <= {}", g15(rhs));
    } else if !scip.set.is_infinity(-lhs) {
        let _ = write!(out, " >= {}", g15(lhs));
    } else {
        out.push_str(" [free]");
    }
}

/// `consdataPrint` (`cons_linear.c`).
fn print_linear(scip: &Scip, cons: ConsId, out: &mut String) {
    let consdata = crate::cons_linear::cd(scip, cons);
    print_sides(scip, out, consdata.lhs, consdata.rhs, |out| {
        let n = consdata.nvars as usize;
        if n == 0 {
            out.push('0');
        } else {
            write_vars_linearsum(scip, out, &consdata.vars.0[..n], &consdata.vals.0[..n], true);
        }
    });
}

/// `SCIPwriteVarsLinearsum`.
fn write_vars_linearsum(scip: &Scip, out: &mut String, vars: &[VarId], vals: &[f64], vartype: bool) {
    for v in 0..vars.len() {
        if vals[v] == 1.0 {
            if v > 0 {
                out.push_str(" +");
            }
        } else if vals[v] == -1.0 {
            out.push_str(" -");
        } else {
            let _ = write!(out, " {}", g15_signed(vals[v]));
        }
        write_var_name(scip, out, vars[v], vartype);
    }
}

/// `SCIPwriteVarName`.
fn write_var_name(scip: &Scip, out: &mut String, var: VarId, vartype: bool) {
    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Negated {
        let _ = write!(out, "<~{}>", scip.vars[v.negatedvar].name);
    } else {
        let _ = write!(out, "<{}>", v.name);
    }
    if vartype {
        let c = if v.varimpltype != ImplintType::None {
            'I'
        } else {
            match v.vartype {
                VarType::Binary => 'B',
                VarType::Integer => 'I',
                _ => 'C',
            }
        };
        let _ = write!(out, "[{c}]");
    }
}

/// `consPrintIndicator`.
fn print_indicator(scip: &Scip, cons: ConsId, out: &mut String) {
    let consdata = crate::cons_indicator::cd(scip, cons);
    let mut binvar = consdata.binvar;
    let mut rhs = 1;
    if scip.vars[binvar].varstatus == VarStatus::Negated {
        rhs = 0;
        binvar = scip.vars[binvar].negatedvar;
    }
    let _ = write!(out, "<{}> = {rhs}", scip.vars[binvar].name);
    let _ = write!(out, " -> <{}> = 0", scip.vars[consdata.slackvar].name);
    let _ = write!(out, " (<{}>)", scip.conss[consdata.lincons].name);
}

/// `consPrintNonlinear`.
fn print_nonlinear(scip: &Scip, cons: ConsId, out: &mut String) {
    let consdata = crate::cons_nonlinear::cd(scip, cons);
    print_sides(scip, out, consdata.lhs, consdata.rhs, |out| print_expr(scip, consdata.expr, 0, out));
}

/// `SCIPexprPrint`: the depth-first walk with every stage, written as the
/// recursion it is, calling each handler's `EXPRPRINT` at entering the
/// expression, before and after each child and at leaving it.
fn print_expr(scip: &Scip, expr: ExprId, parentprecedence: u32, out: &mut String) {
    let hdlr = &scip.exprhdlrs[scip.exprs[expr].exprhdlr];
    let precedence = hdlr.precedence;
    let children = &scip.exprs[expr].children;
    match hdlr.name.as_str() {
        // `printSum`.
        "sum" => {
            let constant = crate::expr_sum::get_constant_expr_sum(scip, expr);
            let coefs = crate::expr_sum::get_coefs_expr_sum(scip, expr);
            if precedence <= parentprecedence {
                out.push('(');
            }
            if constant != 0.0 {
                out.push_str(&g15(constant));
            }
            for (currentchild, &child) in children.iter().enumerate() {
                let coef = coefs[currentchild];
                if coef == 1.0 {
                    if constant != 0.0 || currentchild > 0 {
                        out.push('+');
                    }
                } else if coef == -1.0 {
                    out.push('-');
                } else {
                    let _ = write!(out, "{}*", if constant != 0.0 || currentchild > 0 { g15_signed(coef) } else { g15(coef) });
                }
                print_expr(scip, child, precedence, out);
            }
            if precedence <= parentprecedence {
                out.push(')');
            }
        }
        // `printProduct`.
        "prod" => {
            let coefficient = crate::expr_product::get_coef_expr_product(scip, expr);
            if precedence <= parentprecedence {
                out.push('(');
            }
            if coefficient != 1.0 {
                if coefficient < 0.0 && precedence > parentprecedence {
                    let _ = write!(out, "({})", g15(coefficient));
                } else {
                    out.push_str(&g15(coefficient));
                }
            }
            for (currentchild, &child) in children.iter().enumerate() {
                if coefficient != 1.0 || currentchild > 0 {
                    out.push('*');
                }
                print_expr(scip, child, precedence, out);
            }
            if precedence <= parentprecedence {
                out.push(')');
            }
        }
        // `printPow`.
        "pow" => {
            out.push('(');
            print_expr(scip, children[0], precedence, out);
            let exponent = crate::expr_pow::get_exponent_expr_pow(scip, expr);
            if exponent >= 0.0 {
                let _ = write!(out, ")^{}", g15(exponent));
            } else {
                let _ = write!(out, ")^({})", g15(exponent));
            }
        }
        // `printVar`.
        "var" => {
            let _ = write!(out, "<{}>", scip.vars[crate::expr_var::get_var_expr_var(scip, expr)].name);
        }
        // `printValue`.
        "val" => {
            let v = crate::expr_value::get_value_expr_value(scip, expr);
            if v < 0.0 && precedence <= parentprecedence {
                let _ = write!(out, "({})", g15(v));
            } else {
                out.push_str(&g15(v));
            }
        }
        other => unimplemented!("EXPRPRINT of expression handler <{other}>"),
    }
}
