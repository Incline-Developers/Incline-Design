//! Port of `scip_expr.c`: public functions to work with algebraic
//! expressions.

use std::{any::Any, collections::HashMap};

use crate::{
    expr::{self, ExprCurv, ExprEval, ExprId, ExprMapExpr, ExprOwnerCreate, ExprhdlrId},
    misc::{SPACECONTROL, c_isspace, skip_space, str_copy_section, str_to_real_value},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::VarId,
    var::NONE,
};

/// `SCIPincludeExprhdlr`: creates the data structure for an expression
/// handler and includes it into SCIP.
pub fn include_exprhdlr(scip: &mut Scip, name: &str, desc: Option<&str>, precedence: u32, eval: ExprEval, data: Option<Box<dyn Any>>) -> ScipResult<ExprhdlrId> {
    let exprhdlr = expr::exprhdlr_create(scip, name, desc, precedence, eval, data);
    expr::set_include_exprhdlr(scip, exprhdlr);
    Ok(exprhdlr)
}

/// `SCIPcallExprSimplify`: calls the simplification method of an
/// expression handler.
pub fn call_expr_simplify(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    // Use simplification of expression handlers.
    let h = scip.exprs[expr].exprhdlr;
    expr::exprhdlr_simplify_expr(scip, h, expr, simplifiedexpr, ownercreate, ownercreatedata)
}

/// `SCIPduplicateExpr`: duplicates the given expression and its children.
pub fn duplicate_expr(
    scip: &mut Scip,
    expr: ExprId,
    mapexpr: Option<ExprMapExpr>,
    mapexprdata: usize,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    expr::expr_copy(scip, expr, mapexpr, mapexprdata, ownercreate, ownercreatedata)
}

/// `SCIPsimplifyExpr`: simplifies an expression; returns `(simplified,
/// changed, infeasible)`.
pub fn simplify_expr(scip: &mut Scip, rootexpr: ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<(ExprId, bool, bool)> {
    expr::expr_simplify(scip, rootexpr, ownercreate, ownercreatedata)
}

/* parsing methods (internal)
 *
 * Here is an attempt at defining the grammar of an expression. We use upper
 * case names for variables (in the grammar sense) and terminals are between
 * "". Loosely speaking, a Base will be any "block", a Factor is a Base to a
 * power, a Term is a product of Factors and an Expression is a sum of terms.
 * The actual definition:
 *
 * Expression -> ["+" | "-"] Term { ("+" | "-" | "number *") ] Term }
 * Term       -> Factor { ("*" | "/" ) Factor }
 * Factor     -> Base [ "^" "number" | "^(" "number" ")" ]
 * Base       -> "number" | "<varname>" | "(" Expression ")" | Op "(" OpExpression ")
 *
 * where [a|b] means a or b or none, (a|b) means a or b, {a} means 0 or more
 * a.
 *
 * Note that Op and OpExpression are undefined. Op corresponds to the name of
 * an expression handler and OpExpression to whatever string the expression
 * handler accepts (through its parse method).
 *
 * parse(Expr|Term|Base) returns an SCIP_EXPR
 *
 * @todo We can change the grammar so that Factor becomes base and we allow a
 * Term to be: Term -> Factor { ("*" | "/" | "^") Factor }
 */

/// The byte at `pos`, or 0 past the end (C's terminating NUL).
#[inline]
fn at(s: &[u8], pos: usize) -> u8 {
    s.get(pos).copied().unwrap_or(0)
}

/// The rest of `s` from `pos`, for messages.
fn rest(s: &[u8], pos: usize) -> String {
    String::from_utf8_lossy(&s[pos.min(s.len())..]).into_owned()
}

/// `SCIPparseVarName`: parses variable name in a string; returns the
/// variable (or `NONE` if it does not exist) and the end position (`pos` if
/// no name was found).
pub fn parse_var_name(scip: &mut Scip, s: &[u8], pos: usize) -> ScipResult<(VarId, usize)> {
    let (varname, mut endptr) = str_copy_section(s, pos, b'<', b'>', 1024);

    if endptr == pos {
        return Ok((NONE, endptr));
    }

    let var;
    // Check if we have a negated variable.
    if let Some(name) = varname.strip_prefix('~') {
        // Search for the variable and ignore '~'.
        let v = crate::api::find_var(scip, name);

        var = if v != NONE { crate::scip_var::get_negated_var(scip, v)? } else { v };
    } else {
        // Search for the variable.
        var = crate::api::find_var(scip, &varname);
    }

    let str = endptr;

    // Skip additional variable type marker.
    if at(s, str) == b'[' && matches!(at(s, str + 1), b'B' | b'I' | b'M' | b'C') && at(s, str + 2) == b']' {
        endptr += 3;
    }

    Ok((var, endptr))
}

/// `parseBase`: parses base to build a value, variable, sum, or
/// function-like ("func(...)") expression.
///
/// Base -> "number" | "<varname>" | "(" Expression ")" | Op "(" OpExpression ")
fn parse_base(
    scip: &mut Scip,
    vartoexprvarmap: &mut HashMap<VarId, ExprId>,
    s: &[u8],
    mut pos: usize,
    newpos: &mut usize,
    basetree: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // Ignore whitespace.
    pos = skip_space(s, pos);

    if at(s, pos) == 0 {
        log::error!("Unexpected end of expression string");
        return Err(Retcode::ReadError);
    }

    let c = at(s, pos);
    if c == b'<' {
        // Parse a variable.
        let (var, np) = parse_var_name(scip, s, pos)?;
        *newpos = np;

        if var == NONE {
            log::error!("Could not find variable with name '{}'", rest(s, pos));
            return Err(Retcode::ReadError);
        }

        pos = *newpos;

        // Check if we have already created an expression out of this var.
        if let Some(&e) = vartoexprvarmap.get(&var) {
            *basetree = e;
            expr::expr_capture(scip, *basetree);
        } else {
            // Intentionally not using createExprVar here, since parsed expressions
            // are not part of a constraint (they will be copied when a constraint
            // is created).
            *basetree = crate::expr_var::create_expr_var(scip, var, ownercreate, ownercreatedata)?;
            vartoexprvarmap.insert(var, *basetree);
        }
    } else if c == b'(' {
        // Parse expression.
        pos += 1;
        parse_expr_internal(scip, vartoexprvarmap, s, pos, newpos, basetree, ownercreate, ownercreatedata)?;
        pos = *newpos;

        // Expect ')'.
        if at(s, pos) != b')' {
            log::error!(
                "Read a '(', parsed expression inside --> expecting closing ')'. Got <{}>: rest of string <{}>",
                at(s, pos) as char,
                rest(s, pos)
            );
            expr::expr_release(scip, basetree)?;
            return Err(Retcode::ReadError);
        }
        pos += 1;
    } else if c.is_ascii_digit() {
        // Parse number.
        let Some((value, np)) = str_to_real_value(s, pos) else {
            log::error!("error parsing number from <{}>", rest(s, pos));
            return Err(Retcode::ReadError);
        };
        pos = np;
        if !value.is_finite() || scip.set.is_infinity(value) {
            log::error!("Infinite or nan term value expression within nonlinear expression {}", rest(s, pos));
            return Err(Retcode::InvalidData);
        }
        *basetree = crate::expr_value::create_expr_value(scip, value, ownercreate, ownercreatedata)?;
    } else if c.is_ascii_alphabetic() {
        // A (function) name is coming, should find exprhandler with such name.

        // Get name.
        let mut operatorname = Vec::new();
        while at(s, pos) != b'(' && at(s, pos) != 0 && !c_isspace(at(s, pos)) && !(at(s, pos) == b'\\' && at(s, pos + 1) != 0 && SPACECONTROL.contains(&at(s, pos + 1))) {
            operatorname.push(at(s, pos));
            pos += 1;
        }
        let operatorname = String::from_utf8_lossy(&operatorname).into_owned();

        // After name we must see a '('.
        if at(s, pos) != b'(' {
            log::error!("Expected '(' after operator name <{operatorname}>, but got {}.", rest(s, pos));
            return Err(Retcode::ReadError);
        }

        // Search for expression handler.
        let exprhdlr = expr::set_find_exprhdlr(scip, &operatorname);

        // Check expression handler exists and has a parsing method.
        if exprhdlr == NONE {
            log::error!("No expression handler with name <{operatorname}> found.");
            return Err(Retcode::ReadError);
        }

        pos += 1;
        let mut success = false;
        expr::exprhdlr_parse_expr(scip, exprhdlr, s, pos, newpos, basetree, &mut success, ownercreate, ownercreatedata)?;

        if !success {
            log::error!("Error while expression handler <{operatorname}> was parsing {}", rest(s, pos));
            return Err(Retcode::ReadError);
        }
        pos = *newpos;

        // We should see the ')' of Op "(" OpExpression "); move one character
        // forward.
        pos += 1;
    } else {
        // Base -> "number" | "<varname>" | "(" Expression ")" | Op "(" OpExpression ")
        log::error!(
            "Expected a number, (expression), <varname>, Opname(Opexpr), instead got <{}> from {}",
            c as char,
            rest(s, pos)
        );
        return Err(Retcode::ReadError);
    }

    *newpos = pos;

    Ok(())
}

/// `parseFactor`: parses a factor and builds a product-expression if there
/// is an exponent, otherwise returns the base expression.
///
/// Factor -> Base [ "^" "number" | "^(" "number" ")" ]
#[allow(clippy::too_many_arguments)]
fn parse_factor(
    scip: &mut Scip,
    isdenominator: bool,
    vartoexprvarmap: &mut HashMap<VarId, ExprId>,
    s: &[u8],
    mut pos: usize,
    newpos: &mut usize,
    factortree: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    if at(s, pos) == 0 {
        log::error!("Unexpected end of expression string.");
        return Err(Retcode::ReadError);
    }

    // Parse Base; ignore whitespace.
    pos = skip_space(s, pos);

    let mut basetree = NONE;
    parse_base(scip, vartoexprvarmap, s, pos, newpos, &mut basetree, ownercreate, ownercreatedata)?;
    pos = *newpos;

    // Check if there is an exponent; ignore whitespace.
    pos = skip_space(s, pos);

    let mut exponent;
    if at(s, pos) == b'^' {
        pos += 1;
        pos = skip_space(s, pos);

        if at(s, pos) == 0 {
            log::error!("Unexpected end of expression string after '^'.");
            expr::expr_release(scip, &mut basetree)?;
            return Err(Retcode::ReadError);
        }

        if at(s, pos) == b'(' {
            pos += 1;

            // It is exponent with parenthesis; expect number possibly starting with
            // + or -.
            let Some((e, np)) = str_to_real_value(s, pos) else {
                log::error!("error parsing number from <{}>", rest(s, pos));
                expr::expr_release(scip, &mut basetree)?;
                return Err(Retcode::ReadError);
            };
            exponent = e;
            pos = np;

            // Expect the ')'.
            pos = skip_space(s, pos);
            if at(s, pos) != b')' {
                log::error!("error in parsing exponent: expected ')', received <{}> from <{}>", at(s, pos) as char, rest(s, pos));
                expr::expr_release(scip, &mut basetree)?;
                return Err(Retcode::ReadError);
            }
            pos += 1;
        } else {
            // No parenthesis, we should see just a positive number; expect a digit.
            if at(s, pos).is_ascii_digit() {
                let Some((e, np)) = str_to_real_value(s, pos) else {
                    log::error!("error parsing number from <{}>", rest(s, pos));
                    expr::expr_release(scip, &mut basetree)?;
                    return Err(Retcode::ReadError);
                };
                exponent = e;
                pos = np;
            } else {
                log::error!("error in parsing exponent, expected a digit, received <{}> from <{}>", at(s, pos) as char, rest(s, pos));
                expr::expr_release(scip, &mut basetree)?;
                return Err(Retcode::ReadError);
            }
        }

        if !exponent.is_finite() || scip.set.is_infinity(exponent) {
            log::error!("Infinite or nan term exponent in nonlinear expression {}", rest(s, pos));
            expr::expr_release(scip, &mut basetree)?;
            return Err(Retcode::InvalidData);
        }
    } else {
        // There is no explicit exponent.
        exponent = 1.0;
    }
    *newpos = pos;

    // Multiply with -1 when we are in the denominator.
    if isdenominator {
        exponent *= -1.0;
    }

    // Create power.
    if exponent != 1.0 {
        *factortree = crate::expr_pow::create_expr_pow(scip, basetree, exponent, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut basetree)?;
    } else {
        // Factor consists of this unique Base.
        *factortree = basetree;
    }

    Ok(())
}

/// `parseTerm`: parses a term and builds a product-expression, where each
/// factor is a child.
///
/// Term -> Factor { ("*" | "/" ) Factor }
fn parse_term(
    scip: &mut Scip,
    vartoexprvarmap: &mut HashMap<VarId, ExprId>,
    s: &[u8],
    mut pos: usize,
    newpos: &mut usize,
    termtree: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // Parse Factor; ignore whitespace.
    pos = skip_space(s, pos);

    let mut factortree = NONE;
    parse_factor(scip, false, vartoexprvarmap, s, pos, newpos, &mut factortree, ownercreate, ownercreatedata)?;
    pos = *newpos;

    // Check if Terms has another Factor incoming.
    pos = skip_space(s, pos);
    if at(s, pos) == b'*' || at(s, pos) == b'/' {
        // Initialize termtree as a product expression with a single term, so we
        // can append the extra Factors.
        *termtree = crate::expr_product::create_expr_product(scip, &[factortree], 1.0, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut factortree)?;

        // Loop: parse Factor, find next symbol.
        loop {
            let isdivision = at(s, pos) == b'/';

            pos += 1;
            let retcode = parse_factor(scip, isdivision, vartoexprvarmap, s, pos, newpos, &mut factortree, ownercreate, ownercreatedata);

            // Release termtree, if parseFactor fails with a read-error.
            if retcode == Err(Retcode::ReadError) {
                expr::expr_release(scip, termtree)?;
            }
            retcode?;

            // Append newly created factor.
            expr::expr_append_child(scip, *termtree, factortree);
            expr::expr_release(scip, &mut factortree)?;

            // Find next symbol.
            pos = *newpos;
            pos = skip_space(s, pos);
            if !(at(s, pos) == b'*' || at(s, pos) == b'/') {
                break;
            }
        }
    } else {
        // Term consists of this unique factor.
        *termtree = factortree;
    }

    *newpos = pos;

    Ok(())
}

/// `parseExpr`: parses an expression and builds a sum-expression with
/// children.
///
/// Expression -> ["+" | "-"] Term { ("+" | "-" | "number *") ] Term }
#[allow(clippy::too_many_arguments)]
fn parse_expr_internal(
    scip: &mut Scip,
    vartoexprvarmap: &mut HashMap<VarId, ExprId>,
    s: &[u8],
    mut pos: usize,
    newpos: &mut usize,
    exprtree: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // Ignore whitespace.
    pos = skip_space(s, pos);

    // If '+' or '-', store it.
    let mut sign = 1.0;
    if at(s, pos) == b'+' || at(s, pos) == b'-' {
        sign = if at(s, pos) == b'+' { 1.0 } else { -1.0 };
        pos += 1;
    }

    let mut termtree = NONE;
    parse_term(scip, vartoexprvarmap, s, pos, newpos, &mut termtree, ownercreate, ownercreatedata)?;
    pos = *newpos;

    // Check if Expr has another Term incoming.
    pos = skip_space(s, pos);
    if at(s, pos) == b'+' || at(s, pos) == b'-' {
        if expr::expr_is_value(scip, termtree) {
            // Initialize exprtree as a sum expression with a constant only, so we
            // can append the following terms.
            let v = sign * crate::expr_value::get_value_expr_value(scip, termtree);
            *exprtree = crate::expr_sum::create_expr_sum(scip, &[], None, v, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut termtree)?;
        } else {
            // Initialize exprtree as a sum expression with a single term, so we can
            // append the following terms.
            *exprtree = crate::expr_sum::create_expr_sum(scip, &[termtree], Some(&[sign]), 0.0, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut termtree)?;
        }

        // Loop: parse Term, find next symbol.
        loop {
            let mut coef;

            // Check if we have a "coef * <term>".
            if let Some((c, np)) = str_to_real_value(s, pos) {
                coef = c;
                *newpos = np;
                if !coef.is_finite() || scip.set.is_infinity(coef) {
                    log::error!("Infinite or nan term coefficient in nonlinear expression {}", rest(s, pos));
                    expr::expr_release(scip, exprtree)?;
                    return Err(Retcode::InvalidData);
                }

                *newpos = skip_space(s, *newpos);

                if at(s, *newpos) == b'<' {
                    // Found variable name ('*' is considered optional); keep
                    // coefficient in coef and continue parsing term after coefficient.
                    pos = *newpos;

                    pos = skip_space(s, pos);
                } else if at(s, *newpos) == b'*' {
                    // Keep coefficient in coef and continue parsing term after
                    // coefficient.
                    pos = *newpos + 1;

                    pos = skip_space(s, pos);
                } else {
                    // Term consists of single value; let parseTerm() below parse it.
                    coef = if at(s, pos) == b'+' { 1.0 } else { -1.0 };
                    pos += 1;
                }
            } else {
                coef = if at(s, pos) == b'+' { 1.0 } else { -1.0 };
                pos += 1;
            }

            let retcode = parse_term(scip, vartoexprvarmap, s, pos, newpos, &mut termtree, ownercreate, ownercreatedata);

            // Release exprtree if parseTerm fails with an read-error.
            if retcode == Err(Retcode::ReadError) {
                expr::expr_release(scip, exprtree)?;
            }
            retcode?;

            // Append newly created term.
            crate::expr_sum::append_expr_sum_expr(scip, *exprtree, termtree, coef);
            expr::expr_release(scip, &mut termtree)?;

            // Find next symbol.
            pos = *newpos;
            pos = skip_space(s, pos);
            if !(at(s, pos) == b'+' || at(s, pos) == b'-') {
                break;
            }
        }
    } else {
        // Expr consists of this unique ['+' | '-'] Term.
        if sign < 0.0 {
            *exprtree = crate::expr_sum::create_expr_sum(scip, &[termtree], Some(&[sign]), 0.0, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut termtree)?;
        } else {
            *exprtree = termtree;
        }
    }

    *newpos = pos;

    Ok(())
}

/// `SCIPparseExpr`: creates an expression from a string; returns the
/// expression and the position in `exprstr` where reading finished.
pub fn parse_expr(scip: &mut Scip, exprstr: &[u8], pos: usize, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<(ExprId, usize)> {
    let mut vartoexprvarmap = HashMap::new();
    let mut finalpos = pos;
    let mut expr = NONE;

    parse_expr_internal(scip, &mut vartoexprvarmap, exprstr, pos, &mut finalpos, &mut expr, ownercreate, ownercreatedata)?;

    Ok((expr, finalpos))
}

/* simplify methods (internal) */

/// `COMMONSUBEXPR_HASH_DATA` with the `hashCommonSubexpr*` callbacks:
/// userdata for multihash for common subexpression.
struct CommonSubexprHashData<'a> {
    scip: &'a Scip,
    hashiterator: &'a expr::ExprIter,
}

impl crate::hash::HashOps<ExprId> for CommonSubexprHashData<'_> {
    type Key = ExprId;

    /// `hashCommonSubexprGetKey`: get key of hash element.
    fn get_key(&self, elem: &ExprId) -> ExprId {
        *elem
    }

    /// `hashCommonSubexprEq`: checks if two expressions are structurally the
    /// same.
    fn key_eq(&self, key1: &ExprId, key2: &ExprId) -> bool {
        key1 == key2 || expr::expr_compare(self.scip, *key1, *key2) == 0
    }

    /// `hashCommonSubexprKeyval`: get value of hash element when comparing
    /// with another expression.
    fn key_val(&self, key: &ExprId) -> u64 {
        self.hashiterator.get_expr_user_data(self.scip, *key).uintval() as u64
    }
}

/// `findEqualExpr`: returns an equivalent expression for a given expression
/// if possible (`NONE` if there is none).
///
/// It adds the expression to key2expr if the map does not contain the key.
fn find_equal_expr(scip: &Scip, hashiterator: &expr::ExprIter, expr: ExprId, key2expr: &mut crate::hash::MultiHash<ExprId>) -> ExprId {
    let ops = CommonSubexprHashData { scip, hashiterator };
    let mut multihashlist = None;

    // Search for an equivalent expression.
    match key2expr.retrieve_next(&ops, &mut multihashlist, &expr) {
        None => {
            // Processed all expressions like expr from hash table, so insert expr.
            key2expr.insert(&ops, expr);
            NONE
        }
        Some(newexpr) if expr != newexpr => newexpr,
        // Can not replace expr since it is already contained in the
        // hashtablelist.
        Some(_) => NONE,
    }
}

/// `hashExpr`: hashes an expression using an already existing iterator.
///
/// The iterator must by of type DFS with allowrevisit=FALSE and only the
/// leaveexpr stage enabled. The hashes of all visited expressions will be
/// stored in the iterators expression data.
fn hash_expr_internal(scip: &mut Scip, expr: ExprId, hashiterator: &mut expr::ExprIter, mut nvisitedexprs: Option<&mut i32>) -> ScipResult {
    let mut childrenhashes: Vec<u32> = Vec::with_capacity(5);

    let mut e = hashiterator.restart_dfs(scip, expr);
    while !hashiterator.is_end() {
        if let Some(n) = nvisitedexprs.as_deref_mut() {
            *n += 1;
        }

        // Collect hashes of children.
        childrenhashes.clear();
        for i in 0..scip.exprs[e].children.len() {
            let child = scip.exprs[e].children[i];
            childrenhashes.push(hashiterator.get_expr_user_data(scip, child).uintval());
        }

        let mut hashval = 0u32;
        expr::exprhdlr_hash_expr(scip, e, &mut hashval, &childrenhashes)?;

        hashiterator.set_current_user_data(scip, expr::ExprIterUserData(hashval as u64));

        e = hashiterator.get_next(scip);
    }

    Ok(())
}

/// `SCIPcreateExprQuadratic`: creates and captures an expression
/// representing a quadratic function.
#[allow(clippy::too_many_arguments)]
pub fn create_expr_quadratic(
    scip: &mut Scip,
    linvars: &[VarId],
    lincoefs: &[f64],
    quadvars1: &[VarId],
    quadvars2: &[VarId],
    quadcoefs: &[f64],
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    let nlinvars = linvars.len();
    let nquadterms = quadvars1.len();
    let mut children = vec![NONE; nquadterms + nlinvars];
    let mut coefs = vec![0.0; nquadterms + nlinvars];

    // Create children for quadratic terms.
    for i in 0..nquadterms {
        // Quadratic term.
        if quadvars1[i] == quadvars2[i] {
            // Create variable expression; intentionally not using createExprVar
            // here, since expression created here is not part of a constraint (they
            // will be copied when a constraint is created).
            let mut xexpr = crate::expr_var::create_expr_var(scip, quadvars1[i], ownercreate, ownercreatedata)?;

            // Create pow expression.
            children[i] = crate::expr_pow::create_expr_pow(scip, xexpr, 2.0, ownercreate, ownercreatedata)?;

            // Release variable expression; note that the variable expression is
            // still captured by children[i].
            expr::expr_release(scip, &mut xexpr)?;
        } else {
            // Bilinear term.

            // Create variable expressions; intentionally not using createExprVar
            // here, since expression created here is not part of a constraint (they
            // will be copied when a constraint is created).
            let mut e0 = crate::expr_var::create_expr_var(scip, quadvars1[i], ownercreate, ownercreatedata)?;
            let mut e1 = crate::expr_var::create_expr_var(scip, quadvars2[i], ownercreate, ownercreatedata)?;

            // Create product expression.
            children[i] = crate::expr_product::create_expr_product(scip, &[e0, e1], 1.0, ownercreate, ownercreatedata)?;

            // Release variable expressions; note that the variable expressions are
            // still captured by children[i].
            expr::expr_release(scip, &mut e1)?;
            expr::expr_release(scip, &mut e0)?;
        }

        // Store coefficient.
        coefs[i] = quadcoefs[i];
    }

    // Create children for linear terms.
    for i in 0..nlinvars {
        // Create variable expression; intentionally not using createExprVar here,
        // since expression created here is not part of a constraint (they will be
        // copied when a constraint is created); release variable expression after
        // the sum expression has been created.
        children[nquadterms + i] = crate::expr_var::create_expr_var(scip, linvars[i], ownercreate, ownercreatedata)?;

        // Store coefficient.
        coefs[nquadterms + i] = lincoefs[i];
    }

    // Create sum expression.
    let expr = crate::expr_sum::create_expr_sum(scip, &children, Some(&coefs), 0.0, ownercreate, ownercreatedata)?;

    // Release children.
    for child in children.iter_mut() {
        expr::expr_release(scip, child)?;
    }

    Ok(expr)
}

/// `SCIPcreateExprMonomial`: creates and captures an expression
/// representing a monomial.
///
/// @note In deviation from the actual definition of monomials, we also allow
/// for negative and rational exponents. So this function actually creates an
/// expression for a signomial that has exactly one term.
pub fn create_expr_monomial(scip: &mut Scip, vars: &[VarId], exponents: Option<&[f64]>, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let nfactors = vars.len();

    // Return 1 as constant expression if there are no factors.
    if nfactors == 0 {
        return crate::expr_value::create_expr_value(scip, 1.0, ownercreate, ownercreatedata);
    }

    if nfactors == 1 {
        // Only one factor and exponent is 1 => return factors[0].
        if exponents.is_none_or(|e| e[0] == 1.0) {
            // Intentionally not using createExprVar here, since expression created
            // here is not part of a constraint (they will be copied when a
            // constraint is created).
            return crate::expr_var::create_expr_var(scip, vars[0], ownercreate, ownercreatedata);
        }

        // Create variable and power expression; intentionally not using
        // createExprVar here, since expression created here is not part of a
        // constraint (they will be copied when a constraint is created).
        let mut varexpr = crate::expr_var::create_expr_var(scip, vars[0], ownercreate, ownercreatedata)?;
        let expr = crate::expr_pow::create_expr_pow(scip, varexpr, exponents.unwrap()[0], ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut varexpr)?;
        return Ok(expr);
    }

    // Create children.
    let mut children = vec![NONE; nfactors];
    for i in 0..nfactors {
        // Check whether to create a power expression or not, i.e., exponent == 1.
        if exponents.is_none_or(|e| e[i] == 1.0) {
            children[i] = crate::expr_var::create_expr_var(scip, vars[i], ownercreate, ownercreatedata)?;
        } else {
            // Create variable and pow expression.
            let mut varexpr = crate::expr_var::create_expr_var(scip, vars[i], ownercreate, ownercreatedata)?;
            children[i] = crate::expr_pow::create_expr_pow(scip, varexpr, exponents.unwrap()[i], ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut varexpr)?;
        }
    }

    // Create product expression.
    let expr = crate::expr_product::create_expr_product(scip, &children, 1.0, ownercreate, ownercreatedata)?;

    // Release children.
    for child in children.iter_mut() {
        expr::expr_release(scip, child)?;
    }

    Ok(expr)
}

/// `SCIPhashExpr`: compute the hash value of an expression.
pub fn hash_expr(scip: &mut Scip, expr: ExprId) -> ScipResult<u32> {
    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    hash_expr_internal(scip, expr, &mut it, None)?;

    let hashval = it.get_expr_user_data(scip, expr).uintval();

    it.free(scip);

    Ok(hashval)
}

/// `SCIPreplaceCommonSubexpressions`: replaces common sub-expressions in a
/// given expression graph by using a hash key for each expression; returns
/// whether any root expression (expression in exprs) was replaced.
///
/// The algorithm consists of two steps:
///
/// 1. traverse through all given expressions and compute for each of them a
///    (not necessarily unique) hash
///
/// 2. initialize an empty hash table and traverse through all expression;
///    check for each of them if we can find a structural equivalent
///    expression in the hash table; if yes we replace the expression by the
///    expression inside the hash table, otherwise we add it to the hash table
///
/// @note the hash keys of the expressions are used for the hashing inside
/// the hash table; to compute if two expressions (with the same hash) are
/// structurally the same we use the function SCIPexprCompare().
pub fn replace_common_subexpressions(scip: &mut Scip, exprs: &mut [ExprId]) -> ScipResult<bool> {
    let mut replacedroot = false;
    let mut nvisitedexprs = 0;

    if exprs.is_empty() {
        return Ok(false);
    }

    let mut hashiterator = expr::ExprIter::create();
    hashiterator.init(scip, NONE, expr::ExprIterType::Dfs, false)?;
    hashiterator.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    // Compute all hashes for each sub-expression.
    for &e in exprs.iter() {
        hash_expr_internal(scip, e, &mut hashiterator, Some(&mut nvisitedexprs))?;
    }

    // Replace equivalent sub-expressions.
    let mut key2expr = crate::hash::MultiHash::new(nvisitedexprs);

    let mut repliterator = expr::ExprIter::create();

    for root in exprs.iter_mut() {
        // Check the root for equivalence separately first.
        let newroot = find_equal_expr(scip, &hashiterator, *root, &mut key2expr);

        if newroot != NONE {
            expr::expr_release(scip, root)?;

            *root = newroot;
            expr::expr_capture(scip, newroot);

            replacedroot = true;

            continue;
        }

        // Replace equivalent sub-expressions in the tree.
        repliterator.init(scip, *root, expr::ExprIterType::Dfs, false)?;
        repliterator.set_stages_dfs(scip, expr::EXPRITER_VISITINGCHILD);

        while !repliterator.is_end() {
            let child = repliterator.get_child_expr_dfs(scip);

            // Try to find an equivalent expression.
            let newchild = find_equal_expr(scip, &hashiterator, child, &mut key2expr);

            // Replace child with newchild.
            if newchild != NONE {
                let cur = repliterator.get_current();
                let idx = repliterator.get_child_idx_dfs(scip) as usize;
                expr::expr_replace_child(scip, cur, idx, newchild)?;

                repliterator.skip_dfs(scip);
            } else {
                repliterator.get_next(scip);
            }
        }
    }

    // Free memory.
    repliterator.free(scip);
    hashiterator.free(scip);

    Ok(replacedroot)
}

/// `SCIPcomputeExprCurvature`: computes the curvature of a given expression
/// and all its subexpressions.
///
/// @note this function also evaluates all subexpressions w.r.t. current
/// variable bounds
/// @note this function relies on information from the curvature callback of
/// expression handlers only, consider using function SCIPhasExprCurvature()
/// of the convex-nlhdlr instead, as that uses more information to deduce
/// convexity
pub fn compute_expr_curvature(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let trialcurv = [ExprCurv::Linear, ExprCurv::Convex, ExprCurv::Concave];
    let mut childcurv = vec![ExprCurv::Unknown; 5];

    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    let mut e = it.get_current();
    while !it.is_end() {
        let mut curv = ExprCurv::Unknown;
        let h = scip.exprs[e].exprhdlr;

        if scip.exprhdlrs[h].curvature.is_none() {
            // Set curvature in expression.
            scip.exprs[e].curvature = curv;
            e = it.get_next(scip);
            continue;
        }

        let nchildren = scip.exprs[e].children.len();
        if nchildren > childcurv.len() {
            childcurv.resize(nchildren, ExprCurv::Unknown);
        }

        for &trial in &trialcurv {
            // Check if expression can have a curvature trialcurv[i].
            let mut success = false;
            expr::exprhdlr_curvature_expr(scip, h, e, trial, &mut success, &mut childcurv)?;
            if !success {
                continue;
            }

            // Check if conditions on children are satisfied.
            for c in 0..nchildren {
                let childc = scip.exprs[scip.exprs[e].children[c]].curvature;
                if (childcurv[c].bits() & childc.bits()) != childcurv[c].bits() {
                    success = false;
                    break;
                }
            }

            if success {
                curv = trial;
                break;
            }
        }

        // Set curvature in expression.
        scip.exprs[e].curvature = curv;

        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPcomputeExprIntegrality`: computes integrality information of a given
/// expression and all its subexpressions.
pub fn compute_expr_integrality(scip: &mut Scip, expr: ExprId) -> ScipResult {
    let mut integrality = crate::types::ImplintType::None;

    // Shortcut for expr without children.
    if scip.exprs[expr].children.is_empty() {
        // Compute integrality information.
        let h = scip.exprs[expr].exprhdlr;
        expr::exprhdlr_integrality_expr(scip, h, expr, &mut integrality)?;
        scip.exprs[expr].integrality = integrality;

        return Ok(());
    }

    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;
    it.set_stages_dfs(scip, expr::EXPRITER_LEAVEEXPR);

    let mut e = it.get_current();
    while !it.is_end() {
        // Compute integrality information.
        let h = scip.exprs[e].exprhdlr;
        expr::exprhdlr_integrality_expr(scip, h, e, &mut integrality)?;
        scip.exprs[e].integrality = integrality;

        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(())
}

/// `SCIPgetExprNVars`: returns the total number of variable expressions in
/// an expression.
///
/// The function counts variable expressions in common sub-expressions only
/// once, but counts variables appearing in several variable expressions
/// multiple times.
pub fn get_expr_n_vars(scip: &mut Scip, expr: ExprId) -> ScipResult<i32> {
    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;

    let mut nvars = 0;
    let mut e = expr;
    while !it.is_end() {
        if expr::expr_is_var(scip, e) {
            nvars += 1;
        }
        e = it.get_next(scip);
    }

    it.free(scip);

    Ok(nvars)
}

/// `SCIPgetExprVarExprs`: returns all variable expressions contained in a
/// given expression.
///
/// @note function captures variable expressions
pub fn get_expr_var_exprs(scip: &mut Scip, expr: ExprId) -> ScipResult<Vec<ExprId>> {
    let mut it = expr::ExprIter::create();
    it.init(scip, expr, expr::ExprIterType::Dfs, false)?;

    let mut varexprs = Vec::new();
    let mut e = expr;
    while !it.is_end() {
        if expr::expr_is_var(scip, e) {
            varexprs.push(e);

            // Capture expression.
            expr::expr_capture(scip, e);
        }
        e = it.get_next(scip);
    }

    // @todo sort variable expressions here?

    it.free(scip);

    Ok(varexprs)
}

/// `SCIPevalExprQuadratic`: evaluates quadratic term in a solution w.r.t.
/// auxiliary variables.
///
/// @note This requires that every expr used in the quadratic data is a
/// variable expression.
pub fn eval_expr_quadratic(scip: &Scip, expr: ExprId, sol: crate::types::SolId) -> f64 {
    let quaddata = expr::expr_get_quadratic_data(scip, expr);
    let val = |e: ExprId| crate::api::get_sol_val(scip, sol, crate::expr_var::get_var_expr_var(scip, e));

    let mut auxvalue = quaddata.constant;

    // Linear terms.
    for i in 0..quaddata.linexprs.len() {
        auxvalue += quaddata.lincoefs[i] * val(quaddata.linexprs[i]);
    }

    // Quadratic terms.
    for term in &quaddata.quadexprterms {
        let solval = val(term.expr);
        auxvalue += (term.lincoef + term.sqrcoef * solval) * solval;
    }

    // Bilinear terms.
    for term in &quaddata.bilinexprterms {
        auxvalue += term.coef * val(term.expr1) * val(term.expr2);
    }

    auxvalue
}

/// `SCIPgetExprMonomialData` (`SCIPexprGetMonomialData`): gets the
/// coefficient, the exponents and the factor expressions of a product
/// expression seen as a monomial c * prod_i x_i^alpha_i.
pub fn get_expr_monomial_data(scip: &Scip, expr: ExprId) -> (f64, Vec<f64>, Vec<ExprId>) {
    let coef = crate::expr_product::get_coef_expr_product(scip, expr);
    let nexprs = scip.exprs[expr].children.len();
    let mut exponents = vec![0.0; nexprs];
    let mut factors = vec![NONE; nexprs];

    for c in 0..nexprs {
        let child = scip.exprs[expr].children[c];

        if expr::expr_is_power(scip, child) {
            exponents[c] = crate::expr_pow::get_exponent_expr_pow(scip, child);
            factors[c] = scip.exprs[child].children[0];
        } else {
            exponents[c] = 1.0;
            factors[c] = child;
        }
    }

    (coef, exponents, factors)
}

/// `SCIPcopyExpr`: copies an expression including subexpressions from
/// `source` to `target` (`SCIPexprCopy` with the mapping `copyVarExpr`,
/// which turns each variable expression into one of the variable's copy).
/// Returns the copy (`NONE` if an expression handler is missing in
/// `target`) and whether all variables were validly copied.
#[allow(clippy::too_many_arguments)]
pub fn copy_expr(
    source: &mut Scip,
    target: &mut Scip,
    sourceexpr: ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
    mut maps: Option<&mut crate::copy::CopyMaps>,
    global: bool,
) -> ScipResult<(ExprId, bool)> {
    use expr::{EXPRITER_ENTEREXPR, EXPRITER_VISITEDCHILD, ExprIter, ExprIterType, ExprIterUserData};

    let mut copyvalid = true;

    let mut it = ExprIter::create();
    // TODO use FALSE, i.e., don't duplicate common subexpr?
    it.init(source, sourceexpr, ExprIterType::Dfs, true)?;
    it.set_stages_dfs(source, EXPRITER_ENTEREXPR | EXPRITER_VISITEDCHILD);

    let mut e = sourceexpr;
    while !it.is_end() {
        match it.get_stage_dfs() {
            EXPRITER_ENTEREXPR => {
                // Create expr that will hold the copy; copyVarExpr: map var exprs to
                // var-exprs of the copied variables.
                if expr::expr_is_var(source, e) {
                    let sourcevar = crate::expr_var::get_var_expr_var(source, e);
                    let (targetvar, valid) = crate::copy::get_var_copy(source, target, sourcevar, maps.as_deref_mut(), global)?;

                    // If copy was not valid, store so in mapvar data.
                    if !valid {
                        copyvalid = false;
                    }

                    let exprcopy = crate::expr_var::create_expr_var(target, targetvar, ownercreate, ownercreatedata)?;

                    // Map callback gave us an expression to use for the copy; store
                    // targetexpr.
                    it.set_current_user_data(source, ExprIterUserData::from_ptr(exprcopy));

                    // Skip subexpression (assume that exprcopy is a complete copy) and
                    // continue.
                    e = it.skip_dfs(source);
                    continue;
                }

                // Get the exprhdlr of the target scip.
                let name = source.exprhdlrs[source.exprs[e].exprhdlr].name.clone();
                let targetexprhdlr = expr::set_find_exprhdlr(target, &name);
                if targetexprhdlr == NONE {
                    // Expression handler not in target scip (probably did not have a copy
                    // callback) -> abort.
                    it.set_current_user_data(source, ExprIterUserData::from_ptr(NONE));
                    e = it.skip_dfs(source);
                    continue;
                }

                // Copy expression data.
                let targetexprdata = if source.exprs[e].exprdata.is_some() {
                    let h = source.exprs[e].exprhdlr;
                    Some((source.exprhdlrs[h].copydata.unwrap())(source, e)?)
                } else {
                    None
                };

                // Create in targetexpr an expression of the same type as expr, but
                // without children for now.
                let exprcopy = expr::expr_create(target, targetexprhdlr, targetexprdata, &[], ownercreate, ownercreatedata)?;

                // Store targetexpr.
                it.set_current_user_data(source, ExprIterUserData::from_ptr(exprcopy));
            }
            EXPRITER_VISITEDCHILD => {
                // Just visited child so a copy of himself should be available; append
                // it.
                let mut exprcopy = it.get_current_user_data(source).ptrval();

                // Get copy of child.
                let mut childcopy = it.get_child_user_data_dfs(source).ptrval();
                if childcopy == NONE {
                    // Abort: release exprcopy (should free also the already copied
                    // children).
                    expr::expr_release(target, &mut exprcopy)?;

                    it.set_current_user_data(source, ExprIterUserData::from_ptr(NONE));

                    e = it.skip_dfs(source);
                    continue;
                }

                // Append child to exprcopy.
                expr::expr_append_child(target, exprcopy, childcopy);

                // Release childcopy (still captured by exprcopy).
                expr::expr_release(target, &mut childcopy)?;
            }
            _ => unreachable!("we should never be called in this stage"),
        }

        e = it.get_next(source);
    }

    // The target expression should be stored in the userdata of the
    // sourceexpr (can be NULL if aborted).
    let targetexpr = it.get_expr_user_data(source, sourceexpr).ptrval();

    it.free(source);

    Ok((targetexpr, copyvalid))
}
