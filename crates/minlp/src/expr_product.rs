//! Port of `expr_product.c`: product expression handler.

use std::any::Any;

use crate::{
    def::eps_is_int,
    expr::{self, ExprCurv, ExprId, ExprIntEvalVar, ExprOwnerCreate, ExprhdlrId, INTERVAL_INFINITY, Monotone},
    expr_sum::{append_expr_sum_expr, create_expr_sum, get_coefs_expr_sum, get_constant_expr_sum},
    exprcurv::exprcurv_multiply,
    interval::{self, Interval},
    retcode::ScipResult,
    scip::Scip,
    scip_expr::call_expr_simplify,
    types::{ImplintType, SolId},
};

const EXPRHDLR_NAME: &str = "prod";
const EXPRHDLR_DESC: &str = "product expression";
const EXPRHDLR_PRECEDENCE: u32 = 50000;

/// `SCIP_MAXVERTEXPOLYDIM` (`cons_nonlinear.h`): maximal dimension of
/// vertex-polyhedral function to compute facet for.
const MAXVERTEXPOLYDIM: usize = 14;

/// `EXPRHDLR_HASHKEY`.
fn exprhdlr_hashkey() -> u32 {
    crate::misc::calc_fib_hash(54949.0)
}

/// `SCIP_ExprData`: expression data.
struct ExprData {
    /// Coefficient.
    coefficient: f64,
}

/// `SCIP_ExprhdlrData`.
struct ExprhdlrData {
    /// Nonlinear constraint handler (to compute estimates for > 2-dim
    /// products).
    conshdlr: Option<usize>,
    /// Whether to expand products of a sum and several factors in simplify
    /// (SP12b).
    expandalways: bool,
}

fn hdlrdata(scip: &Scip, expr: ExprId) -> &ExprhdlrData {
    let h = scip.exprs[expr].exprhdlr;
    scip.exprhdlrs[h].data.as_ref().unwrap().downcast_ref::<ExprhdlrData>().unwrap()
}

/* methods for handling linked list of expressions */

/// End of a list (`NULL`).
const NIL: usize = usize::MAX;

/// `struct exprnode`: node for linked list of expressions. The nodes of
/// one simplification live in one arena, and a list is the index of its
/// first node.
struct ExprNode {
    /// Expression in node.
    expr: ExprId,
    /// Next node.
    next: usize,
}

type Nodes = Vec<ExprNode>;

/// `insertFirstList`: inserts newnode at beginning of list.
fn insert_first_list(nodes: &mut Nodes, newnode: usize, list: &mut usize) {
    nodes[newnode].next = *list;
    *list = newnode;
}

/// `listPopFirst`: removes first element of list and returns it.
fn list_pop_first(nodes: &mut Nodes, list: &mut usize) -> usize {
    if *list == NIL {
        return NIL;
    }

    let first = *list;
    *list = nodes[first].next;
    nodes[first].next = NIL;

    first
}

/// `listLength`: returns length of list.
fn list_length(nodes: &Nodes, mut list: usize) -> usize {
    if list == NIL {
        return 0;
    }

    let mut length = 1;
    loop {
        list = nodes[list].next;
        if list == NIL {
            break;
        }
        length += 1;
    }

    length
}

/// `createExprNode`: creates expression node and captures expression.
fn create_expr_node(scip: &mut Scip, nodes: &mut Nodes, expr: ExprId) -> usize {
    nodes.push(ExprNode { expr, next: NIL });
    expr::expr_capture(scip, expr);
    nodes.len() - 1
}

/// `createExprlistFromExprs`: creates expression list from expressions.
fn create_exprlist_from_exprs(scip: &mut Scip, nodes: &mut Nodes, exprs: &[ExprId], list: &mut usize) {
    for i in (0..exprs.len()).rev() {
        let newnode = create_expr_node(scip, nodes, exprs[i]);
        insert_first_list(nodes, newnode, list);
    }
}

/// `freeExprNode`: frees expression node and releases expressions.
fn free_expr_node(scip: &mut Scip, nodes: &mut Nodes, node: &mut usize) -> ScipResult {
    expr::expr_release(scip, &mut nodes[*node].expr)?;
    *node = NIL;
    Ok(())
}

/// `freeExprlist`: frees an expression list.
fn free_exprlist(scip: &mut Scip, nodes: &mut Nodes, exprlist: &mut usize) -> ScipResult {
    if *exprlist == NIL {
        return Ok(());
    }

    let mut current = *exprlist;
    while current != NIL {
        let mut tofree = current;
        current = nodes[current].next;
        free_expr_node(scip, nodes, &mut tofree)?;
    }
    *exprlist = NIL;

    Ok(())
}

/* helper functions for simplifying expressions */

/// `createExprProductFromExprlist`: creates a product expression with the
/// elements of exprlist as its children.
fn create_expr_product_from_exprlist(
    scip: &mut Scip,
    nodes: &Nodes,
    mut exprlist: usize,
    coef: f64,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult<ExprId> {
    // Asserts SP8.
    let nchildren = list_length(nodes, exprlist);

    let mut children = Vec::with_capacity(nchildren);
    for _ in 0..nchildren {
        children.push(nodes[exprlist].expr);
        exprlist = nodes[exprlist].next;
    }

    create_expr_product(scip, &children, coef, ownercreate, ownercreatedata)
}

/// Whether `expr` belongs to the expression handler called `name` (the
/// `SCIPisExpr...` test of handlers that are not ported).
fn is_expr_named(scip: &Scip, expr: ExprId, name: &str) -> bool {
    scip.exprhdlrs[scip.exprs[expr].exprhdlr].name == name
}

/// `simplifyFactor`: simplifies a factor of a product expression: base, so
/// that it is a valid children of a simplified product expr.
///
/// @note In contrast to other simplify methods, this does *not* return a
/// simplified expression. Instead, the method is intended to be called only
/// when simplifying a product expression. Since in general, base is not a
/// simplified child of a product expression, this method returns a list of
/// expressions L, such that (prod L) = baset *and* each expression in L is a
/// valid child of a simplified product expression.
fn simplify_factor(scip: &mut Scip, nodes: &mut Nodes, factor: ExprId, simplifiedcoef: &mut f64, simplifiedfactor: &mut usize, changed: &mut bool) {
    // Enforces SP7.
    if expr::expr_is_value(scip, factor) {
        *changed = true;
        *simplifiedcoef *= crate::expr_value::get_value_expr_value(scip, factor);
        return;
    }

    // Enforces SP2.
    if expr::expr_is_product(scip, factor) {
        *changed = true;

        // Assert SP8.
        let children = scip.exprs[factor].children.clone();
        create_exprlist_from_exprs(scip, nodes, &children, simplifiedfactor);

        return;
    }

    // Enforces SP13: a sum with a unique child and no constant -> take the
    // coefficient and use its child as factor.
    if expr::expr_is_sum(scip, factor) && scip.exprs[factor].children.len() == 1 && get_constant_expr_sum(scip, factor) == 0.0 {
        *changed = true;

        // Assert SS8 and SS7.
        let child0 = scip.exprs[factor].children[0];
        if expr::expr_is_product(scip, child0) {
            // If child is a product, then add its children to exprlist.
            let children = scip.exprs[child0].children.clone();
            create_exprlist_from_exprs(scip, nodes, &children, simplifiedfactor);
            *simplifiedcoef *= get_coef_expr_product(scip, child0);
        } else {
            create_exprlist_from_exprs(scip, nodes, &[child0], simplifiedfactor);
        }
        *simplifiedcoef *= get_coefs_expr_sum(scip, factor)[0];

        return;
    }

    // The given (simplified) expression `factor`, can be a child of a
    // simplified product.
    *simplifiedfactor = create_expr_node(scip, nodes, factor);
}

/// The base, exponent and whether it is a signpower of a factor seen as
/// `base^exponent`.
fn factor_power(scip: &Scip, expr: ExprId) -> (ExprId, f64, bool) {
    if expr::expr_is_power(scip, expr) {
        (scip.exprs[expr].children[0], crate::expr_pow::get_exponent_expr_pow(scip, expr), false)
    } else if crate::expr_pow::is_expr_signpower(scip, expr) {
        (scip.exprs[expr].children[0], crate::expr_pow::get_exponent_expr_pow(scip, expr), true)
    } else {
        (expr, 1.0, false)
    }
}

/// `mergeProductExprlist`: merges tomerge into finalchildren.
///
/// Both, tomerge and finalchildren contain expressions that could be the
/// children of a simplified product (except for SP8 and SP10 which are
/// enforced later). However, the concatenation of both lists will not in
/// general yield a simplified product expression, because SP4, SP5 and SP14
/// could be violated. So the purpose of this method is to enforce SP4, SP5
/// and SP14. In the process of enforcing SP4, it could happen that SP2 is
/// violated. Since enforcing SP2 could generate further violations, we
/// remove the affected children from finalchildren and include them in
/// unsimplifiedchildren for further processing.
///
/// @note if tomerge has more than one element, then they are the children
/// of a simplified product expression.
fn merge_product_exprlist(
    scip: &mut Scip,
    nodes: &mut Nodes,
    tomerge: usize,
    finalchildren: &mut usize,
    unsimplifiedchildren: &mut usize,
    changed: &mut bool,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    if tomerge == NIL {
        return Ok(());
    }

    if *finalchildren == NIL {
        *finalchildren = tomerge;
        return Ok(());
    }

    let mut tomergenode = tomerge;
    let mut current = *finalchildren;
    let mut previous = NIL;

    while tomergenode != NIL && current != NIL {
        // We are going to multiply the two exprs: current and tomergenode; we
        // first check if they are both exponentials; if so, we multiply them;
        // otherwise, we interpret them as base^exponent; the base of an expr is
        // the expr itself if type(expr) != pow, otherwise it is the child of pow.

        // If both are exponentials, create a new exponential with the sum of
        // their children.
        if is_expr_named(scip, nodes[current].expr, "exp") && is_expr_named(scip, nodes[tomergenode].expr, "exp") {
            unimplemented!("the exponential expression handler is not ported");
        }

        // They were not exponentials, so collect bases and exponents.
        let (base1, expo1, issignpower1) = factor_power(scip, nodes[current].expr);
        let (base2, expo2, issignpower2) = factor_power(scip, nodes[tomergenode].expr);

        if expr::expr_compare(scip, base1, base2) == 0 {
            // The bases are the same, so we should try to merge the multiplication
            // of the powers.
            let mut power = NIL;

            if !issignpower1 && !issignpower2 {
                // And both are normal power, then add to unsimplifiedchildren the
                // resulting expr of simplify(base^(expo1 + expo2)).
                power = crate::expr_pow::create_expr_pow(scip, base1, expo1 + expo2, ownercreate, ownercreatedata)?;
            } else if issignpower1 ^ issignpower2 {
                // Exactly one is signpower: sign(x) |x|^expo1 x^expo2 = sign(x)^(1+expo2)
                // |x|^(expo1+expo2), with x = base.
                if eps_is_int(expo2, 0.0) {
                    if (expo2 as i32) % 2 == 0 {
                        // If expo2 is even, then sign(x)^(1+expo2) = sign(x), so we have
                        // signpower: sign(x) |x|^(expo1+expo2).
                        // TODO: we can remove this case distinction once the
                        // simplification of power expressions tranform |expr|^even ->
                        // expr^even, since the call to SCIPcallExprSimplify(scip,
                        // conshdlr, power, &simplifiedpower) below will take care of this.
                        power = crate::expr_pow::create_expr_signpower(scip, base1, expo1 + expo2, ownercreate, ownercreatedata)?;
                    } else {
                        // If expo2 is odd, then sign(x)^(1+expo2) = 1, so we have
                        // |x|^(expo1+expo2).
                        unimplemented!("the absolute value expression handler is not ported");
                    }
                } else if !eps_is_int(expo1 + expo2, 0.0) {
                    // If expo2 is fractional and expo1+expo2 is fractional, then we need
                    // x >= 0, so we can use x^(expo1+expo2).
                    power = crate::expr_pow::create_expr_pow(scip, base1, expo1 + expo2, ownercreate, ownercreatedata)?;
                }
                // Else: expo2 is fractional but expo1+expo2 is integral, then we
                // better do not do anything for now (leave power at NULL).
            } else {
                // If both are signpower, then we have |base|^(expo1+expo2); if
                // expo1+expo2 is even, then we can change this to base^(expo1+expo2).
                if eps_is_int(expo1 + expo2, 0.0) && ((expo1 + expo2) as i32) % 2 == 0 {
                    power = crate::expr_pow::create_expr_pow(scip, base1, expo1 + expo2, ownercreate, ownercreatedata)?;
                } else {
                    unimplemented!("the absolute value expression handler is not ported");
                }
            }

            if power != NIL {
                // We have created a new power: simplify again and continue; call
                // simplifyPow or simplifySignpower.
                let mut simplifiedpower = NIL;
                call_expr_simplify(scip, power, &mut simplifiedpower, ownercreate, ownercreatedata)?;
                expr::expr_release(scip, &mut power)?;

                // Replace tomergenode's expression with simplifiedpower.
                expr::expr_release(scip, &mut nodes[tomergenode].expr)?;
                nodes[tomergenode].expr = simplifiedpower;

                *changed = true;

                // Move tomergenode to unsimplifiedchildren.
                let aux = tomergenode;
                tomergenode = nodes[tomergenode].next;
                insert_first_list(nodes, aux, unsimplifiedchildren);

                // Remove current.
                let mut aux;
                if current == *finalchildren {
                    aux = list_pop_first(nodes, finalchildren);
                    current = *finalchildren;
                } else {
                    aux = current;
                    current = nodes[current].next;
                    nodes[previous].next = current;
                }
                free_expr_node(scip, nodes, &mut aux)?;

                continue;
            }
        }

        // Bases are not the same, or we do not want to merge them then
        // expressions cannot be the same therefore we need to insert tomergenode
        // in finalchildren for this, we need to take care of the order.
        let compareres = expr::expr_compare(scip, nodes[current].expr, nodes[tomergenode].expr);
        if compareres == -1 {
            // current < tomergenode => move current.
            previous = current;
            current = nodes[current].next;
        } else {
            *changed = true;

            // Insert: if current is the first node, then insert at beginning;
            // otherwise, insert between previous and current.
            if current == *finalchildren {
                let aux = tomergenode;
                tomergenode = nodes[tomergenode].next;
                insert_first_list(nodes, aux, finalchildren);
                previous = *finalchildren;
            } else {
                // Extract.
                let aux = tomergenode;
                tomergenode = nodes[tomergenode].next;
                // Insert.
                nodes[previous].next = aux;
                nodes[aux].next = current;
                previous = aux;
            }
        }
    }

    // If all nodes of tomerge were merged, we are done.
    if tomergenode == NIL {
        return Ok(());
    }

    // If all nodes of finalchildren were cancelled by nodes of tomerge (i.e.,
    // transfered to unsimplifiedchildren), then the rest of tomerge is
    // finalchildren.
    if *finalchildren == NIL {
        *finalchildren = tomergenode;
        return Ok(());
    }

    // There are still nodes of tomerge unmerged; these nodes are larger than
    // finalchildren, so append at end.
    nodes[previous].next = tomergenode;

    Ok(())
}

/// `simplifyMultiplyChildren`: simplifies the given (simplified) exprs so
/// that they can be factors of a simplified product.
///
/// In particular, it will sort and multiply factors whose product leads to
/// new expressions.
#[allow(clippy::too_many_arguments)]
fn simplify_multiply_children(
    scip: &mut Scip,
    nodes: &mut Nodes,
    exprs: &[ExprId],
    simplifiedcoef: &mut f64,
    finalchildren: &mut usize,
    changed: &mut bool,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // Set up list of current children (when looking at each of them
    // individually, they are simplified, but as children of a product
    // expression they might be unsimplified).
    let mut unsimplifiedchildren = NIL;
    create_exprlist_from_exprs(scip, nodes, exprs, &mut unsimplifiedchildren);

    *changed = false;

    // While there are still children to process.
    *finalchildren = NIL;
    while unsimplifiedchildren != NIL {
        let mut first = list_pop_first(nodes, &mut unsimplifiedchildren);

        // Enforces SP2, SP7 and SP13.
        let mut tomerge = NIL;
        let firstexpr = nodes[first].expr;
        simplify_factor(scip, nodes, firstexpr, simplifiedcoef, &mut tomerge, changed);

        // Enforces SP4 and SP5 note: merge frees (or uses) the nodes of the
        // tomerge list.
        merge_product_exprlist(scip, nodes, tomerge, finalchildren, &mut unsimplifiedchildren, changed, ownercreate, ownercreatedata)?;

        // Free first.
        free_exprlist(scip, nodes, &mut first)?;

        // If the simplified coefficient is 0, we can return value 0.
        if *simplifiedcoef == 0.0 {
            *changed = true;
            free_exprlist(scip, nodes, finalchildren)?;
            free_exprlist(scip, nodes, &mut unsimplifiedchildren)?;
            break;
        }
    }
    Ok(())
}

/// `enforceSP10`: make sure product has at least two children
/// - if it is empty; return value
/// - if it has one child and coef = 1; return child
/// - if it has one child and coef != 1; return (sum 0 coef expr)
fn enforce_sp10(
    scip: &mut Scip,
    nodes: &Nodes,
    simplifiedcoef: f64,
    finalchildren: usize,
    simplifiedexpr: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // Empty list --> return value.
    if finalchildren == NIL {
        *simplifiedexpr = crate::expr_value::create_expr_value(scip, simplifiedcoef, ownercreate, ownercreatedata)?;
        return Ok(());
    }

    // One child and coef equal to 1 --> return child.
    if nodes[finalchildren].next == NIL && simplifiedcoef == 1.0 {
        *simplifiedexpr = nodes[finalchildren].expr;
        expr::expr_capture(scip, *simplifiedexpr);
        return Ok(());
    }

    // One child and coef different from 1 --> return (sum 0 coef child).
    if nodes[finalchildren].next == NIL {
        let mut sum = create_expr_sum(scip, &[nodes[finalchildren].expr], Some(&[simplifiedcoef]), 0.0, ownercreate, ownercreatedata)?;

        // Simplifying here is necessary, the product could have sums as children
        // e.g., (prod 2 (sum 1 <x>)) -> (sum 0 2 (sum 1 <x>)) and that needs to be
        // simplified to (sum 0 2 <x>).
        call_expr_simplify(scip, sum, simplifiedexpr, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut sum)?;
        return Ok(());
    }

    Ok(())
}

/// `enforceSP11`: checks if it is entropy expression.
fn enforce_sp11(scip: &mut Scip, nodes: &Nodes, _simplifiedcoef: f64, finalchildren: usize, _simplifiedexpr: &mut ExprId) -> ScipResult {
    if !(finalchildren != NIL && nodes[finalchildren].next != NIL && nodes[nodes[finalchildren].next].next == NIL) {
        return Ok(());
    }

    let first = nodes[finalchildren].expr;
    let second = nodes[nodes[finalchildren].next].expr;
    let mut entropicchild = NIL;

    // Could be log(expr) * expr, e.g., log(sin(x)) * sin(x) (OR11).
    if is_expr_named(scip, first, "log") {
        if 0 == expr::expr_compare(scip, scip.exprs[first].children[0], second) {
            entropicchild = second;
        }
    }
    // Could be expr * log(expr), e.g., (1 + abs(x)) log(1 + abs(x)) (OR11).
    else if is_expr_named(scip, second, "log") && 0 == expr::expr_compare(scip, scip.exprs[second].children[0], first) {
        entropicchild = first;
    }

    // Success --> replace finalchildren by entropy expression.
    if entropicchild != NIL {
        unimplemented!("the entropy expression handler is not ported");
    }

    Ok(())
}

/// `enforceSP12`: expands product of two sums or one sum and another
/// expression
/// - two sums: (prod (sum c1 s1 ... sn) (sum c2 t1 ... tm)
///   Builds a sum representing the expansion, where all of its children are
///   simplified, and then simplify the sum
///   - constant != 0 --> c1 ti or c2 * sj is simplified (ti, sj are not
///     sums, because they are children of a simplified sum)
///   - sj * ti may be not be simplified, so put them in a product list and
///     simplify them from there
/// - one sum: (prod factor (sum c s1 ... sn))
///   - c != 0 --> c * factor is simplified (i.e. factor is not sum!)
///   - factor * si may be not be simplified, so put them in a product list
///     and simplify them from there
#[allow(clippy::too_many_arguments)]
fn enforce_sp12(
    scip: &mut Scip,
    nodes: &mut Nodes,
    simplifiedcoef: f64,
    finalchildren: usize,
    expandalways: bool,
    simplifiedexpr: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    // We need only two children.
    if !(finalchildren != NIL && nodes[finalchildren].next != NIL && nodes[nodes[finalchildren].next].next == NIL) {
        return Ok(());
    }

    let first = nodes[finalchildren].expr;
    let second = nodes[nodes[finalchildren].next].expr;

    // Handle both sums case.
    if expr::expr_is_sum(scip, first) && expr::expr_is_sum(scip, second) {
        let c1 = get_constant_expr_sum(scip, first);
        let c2 = get_constant_expr_sum(scip, second);
        let nchildren1 = scip.exprs[first].children.len();
        let nchildren2 = scip.exprs[second].children.len();

        let mut expanded = create_expr_sum(scip, &[], None, c1 * c2 * simplifiedcoef, ownercreate, ownercreatedata)?;

        // Multiply c1 * sum2.
        if c1 != 0.0 {
            for i in 0..nchildren2 {
                let term = scip.exprs[second].children[i];
                let c = get_coefs_expr_sum(scip, second)[i] * c1 * simplifiedcoef;
                append_expr_sum_expr(scip, expanded, term, c);
                // We are just re-using a child here, so do not release term!
            }
        }
        // Multiply c2 * sum1.
        if c2 != 0.0 {
            for i in 0..nchildren1 {
                let term = scip.exprs[first].children[i];
                let c = get_coefs_expr_sum(scip, first)[i] * c2 * simplifiedcoef;
                append_expr_sum_expr(scip, expanded, term, c);
                // We are just re-using a child here, so do not release term!
            }
        }
        // Multiply sum1 * sum2 without constants.
        for j in 0..nchildren1 {
            let coef1 = get_coefs_expr_sum(scip, first)[j];
            let factor0 = scip.exprs[first].children[j];
            for k in 0..nchildren2 {
                let coef2 = get_coefs_expr_sum(scip, second)[k];
                let factor1 = scip.exprs[second].children[k];

                let mut factorscoef = coef1 * coef2;
                let mut finalfactors = NIL;
                let mut dummy = false;
                simplify_multiply_children(
                    scip,
                    nodes,
                    &[factor0, factor1],
                    &mut factorscoef,
                    &mut finalfactors,
                    &mut dummy,
                    ownercreate,
                    ownercreatedata,
                )?;

                let mut term = NIL;
                build_simplified_product(scip, nodes, 1.0, &mut finalfactors, expandalways, true, &mut term, ownercreate, ownercreatedata)?;

                append_expr_sum_expr(scip, expanded, term, factorscoef * simplifiedcoef);

                expr::expr_release(scip, &mut term)?;
            }
        }

        // Simplify the sum.
        call_expr_simplify(scip, expanded, simplifiedexpr, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut expanded)?;

        return Ok(());
    }

    // Handle one sum case.
    if expr::expr_is_sum(scip, first) || expr::expr_is_sum(scip, second) {
        let (sum, factor0) = if expr::expr_is_sum(scip, first) { (first, second) } else { (second, first) };
        let constant = simplifiedcoef * get_constant_expr_sum(scip, sum);
        let nchildren = scip.exprs[sum].children.len();

        let mut expanded = create_expr_sum(scip, &[factor0], Some(&[constant]), 0.0, ownercreate, ownercreatedata)?;
        // We are just re-using a child here, so do not release factor!

        for j in 0..nchildren {
            let coef = get_coefs_expr_sum(scip, sum)[j];
            let factor1 = scip.exprs[sum].children[j];

            let mut termcoef = coef;
            let mut finalfactors = NIL;
            let mut dummy = false;
            simplify_multiply_children(scip, nodes, &[factor0, factor1], &mut termcoef, &mut finalfactors, &mut dummy, ownercreate, ownercreatedata)?;

            let mut term = NIL;
            build_simplified_product(scip, nodes, 1.0, &mut finalfactors, expandalways, true, &mut term, ownercreate, ownercreatedata)?;

            append_expr_sum_expr(scip, expanded, term, termcoef * simplifiedcoef);
            expr::expr_release(scip, &mut term)?;
        }

        // Simplify the sum.
        call_expr_simplify(scip, expanded, simplifiedexpr, ownercreate, ownercreatedata)?;
        expr::expr_release(scip, &mut expanded)?;
    }

    Ok(())
}

/// `enforceSP12b`: expands product of one sum and other expressions
/// - (prod factor ... factor (sum c s1 ... sn) factor ... factor )
///   - c != 0 --> c * factor is simplified (i.e. factor is not sum!)
///   - factor ... factor * si may be not be simplified, so put them in a
///     product list and simplify them from there
fn enforce_sp12b(
    scip: &mut Scip,
    nodes: &mut Nodes,
    simplifiedcoef: f64,
    finalchildren: usize,
    simplifiedexpr: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    let mut sum_node = NIL;
    let mut nfactors = 0;

    // Check whether there is exactly one sum, calc number of factors.
    let mut n = finalchildren;
    while n != NIL {
        if expr::expr_is_sum(scip, nodes[n].expr) {
            if sum_node == NIL {
                sum_node = n;
            } else {
                return Ok(()); // More than one sum.
            }
        } else {
            nfactors += 1;
        }
        n = nodes[n].next;
    }
    if sum_node == NIL || nfactors == 0 {
        // No sum or no other factors.
        return Ok(());
    }

    // Collect exprs of all factors other than the sum.
    let mut factors = Vec::with_capacity(nfactors + 1);
    let mut n = finalchildren;
    while n != NIL {
        if n != sum_node {
            factors.push(nodes[n].expr);
        }
        n = nodes[n].next;
    }
    factors.push(NIL);

    // Build new sum expression.
    let sumexpr = nodes[sum_node].expr;
    let nchildren = scip.exprs[sumexpr].children.len();
    let mut expanded = create_expr_sum(scip, &[], None, 0.0, ownercreate, ownercreatedata)?;

    // Handle constant-from-sum * factors.
    if get_constant_expr_sum(scip, sumexpr) != 0.0 {
        if nfactors == 1 {
            let c = simplifiedcoef * get_constant_expr_sum(scip, sumexpr);
            append_expr_sum_expr(scip, expanded, factors[0], c);
        } else {
            let mut termcoef = 1.0;
            let mut dummy = false;
            let mut finalfactors = NIL;

            simplify_multiply_children(
                scip,
                nodes,
                &factors[..nfactors],
                &mut termcoef,
                &mut finalfactors,
                &mut dummy,
                ownercreate,
                ownercreatedata,
            )?;

            let mut term = NIL;
            build_simplified_product(scip, nodes, 1.0, &mut finalfactors, true, true, &mut term, ownercreate, ownercreatedata)?;

            let c = termcoef * simplifiedcoef * get_constant_expr_sum(scip, sumexpr);
            append_expr_sum_expr(scip, expanded, term, c);
            expr::expr_release(scip, &mut term)?;
        }
    }

    for j in 0..nchildren {
        let coef = get_coefs_expr_sum(scip, sumexpr)[j];
        factors[nfactors] = scip.exprs[sumexpr].children[j];

        let mut termcoef = coef;
        let mut dummy = false;
        let mut finalfactors = NIL;
        simplify_multiply_children(
            scip,
            nodes,
            &factors[..nfactors + 1],
            &mut termcoef,
            &mut finalfactors,
            &mut dummy,
            ownercreate,
            ownercreatedata,
        )?;

        let mut term = NIL;
        build_simplified_product(scip, nodes, 1.0, &mut finalfactors, true, true, &mut term, ownercreate, ownercreatedata)?;

        append_expr_sum_expr(scip, expanded, term, termcoef * simplifiedcoef);
        expr::expr_release(scip, &mut term)?;
    }

    // Simplify the sum.
    call_expr_simplify(scip, expanded, simplifiedexpr, ownercreate, ownercreatedata)?;
    expr::expr_release(scip, &mut expanded)?;

    Ok(())
}

/// `buildSimplifiedProduct`: builds a simplified product from
/// simplifiedfactors.
///
/// @note this function also releases simplifiedfactors.
#[allow(clippy::too_many_arguments)]
fn build_simplified_product(
    scip: &mut Scip,
    nodes: &mut Nodes,
    simplifiedcoef: f64,
    simplifiedfactors: &mut usize,
    expandalways: bool,
    changed: bool,
    simplifiedexpr: &mut ExprId,
    ownercreate: Option<ExprOwnerCreate>,
    ownercreatedata: usize,
) -> ScipResult {
    let finalchildren = *simplifiedfactors;

    // Build product expression from finalchildren and post-simplify.
    *simplifiedexpr = NIL;

    'cleanup: {
        enforce_sp11(scip, nodes, simplifiedcoef, *simplifiedfactors, simplifiedexpr)?;
        if *simplifiedexpr != NIL {
            break 'cleanup;
        }

        enforce_sp12(scip, nodes, simplifiedcoef, *simplifiedfactors, expandalways, simplifiedexpr, ownercreate, ownercreatedata)?;
        if *simplifiedexpr != NIL {
            break 'cleanup;
        }

        if expandalways {
            enforce_sp12b(scip, nodes, simplifiedcoef, *simplifiedfactors, simplifiedexpr, ownercreate, ownercreatedata)?;
            if *simplifiedexpr != NIL {
                break 'cleanup;
            }
        }

        enforce_sp10(scip, nodes, simplifiedcoef, *simplifiedfactors, simplifiedexpr, ownercreate, ownercreatedata)?;
        if *simplifiedexpr != NIL {
            break 'cleanup;
        }

        // Enforces SP8: if simplifiedcoef != 1.0, transform it into a sum with
        // the (simplified) product as child.
        if simplifiedcoef != 1.0 {
            // Create sum.
            let mut aux = create_expr_product_from_exprlist(scip, nodes, finalchildren, 1.0, ownercreate, ownercreatedata)?;
            let mut sum = create_expr_sum(scip, &[aux], Some(&[simplifiedcoef]), 0.0, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut aux)?;

            // Simplify sum.
            call_expr_simplify(scip, sum, simplifiedexpr, ownercreate, ownercreatedata)?;
            expr::expr_release(scip, &mut sum)?;

            break 'cleanup;
        }

        // Build product expression from list.
        if changed {
            *simplifiedexpr = create_expr_product_from_exprlist(scip, nodes, finalchildren, simplifiedcoef, ownercreate, ownercreatedata)?;
            break 'cleanup;
        }
    }

    free_exprlist(scip, nodes, simplifiedfactors)?;
    Ok(())
}

/// `estimateVertexPolyhedralProduct`: computes an estimator for a product
/// as a vertex polyhedral function.
///
/// Since the product is multilinear, its convex and concave envelopes are
/// piecewise linear.
#[allow(clippy::too_many_arguments)]
fn estimate_vertex_polyhedral_product(
    scip: &mut Scip,
    conshdlr: usize,
    nfactors: usize,
    bounds: &[Interval],
    constantfactor: f64,
    refpoint: Option<&[f64]>,
    overestimate: bool,
    targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    success: &mut bool,
) -> ScipResult {
    *success = false;

    // Assemble box, check for unbounded variables, assemble xstar.
    let mut boxv = vec![0.0; 2 * nfactors];
    let mut xstar = vec![0.0; nfactors];
    let mut nfixed = 0;
    for i in 0..nfactors {
        if scip.set.is_infinity(-bounds[i].inf) || scip.set.is_infinity(bounds[i].sup) {
            return Ok(());
        }

        boxv[2 * i] = bounds[i].inf;
        boxv[2 * i + 1] = bounds[i].sup;

        xstar[i] = match refpoint {
            Some(r) => r[i],
            None => 0.5 * (boxv[2 * i] + boxv[2 * i + 1]),
        };

        if scip.set.is_rel_eq(boxv[2 * i], boxv[2 * i + 1]) {
            nfixed += 1;
        }
    }

    if nfixed < nfactors && nfactors - nfixed <= MAXVERTEXPOLYDIM {
        // `prodfunction`: evaluation callback for (vertex-polyhedral) functions used
        // as input for facet computation of its envelopes; the function data is
        // the coefficient.
        let mut prodfunction = |_: &mut Scip, args: &[f64]| -> f64 {
            let mut ret = constantfactor;
            for &a in args {
                ret *= a;
            }
            ret
        };
        *success = crate::cons_nonlinear::compute_facet_vertex_polyhedral_nonlinear(
            scip,
            conshdlr,
            overestimate,
            &mut prodfunction,
            &xstar,
            &boxv,
            nfactors,
            targetvalue,
            coefs,
            constant,
        )?;
    }

    Ok(())
}

/// `simplifyProduct`: simplifies a product expression.
///
/// Summary: we first build a list of expressions (called finalchildren)
/// which will be the children of the simplified product and then we process
/// this list in order to enforce SP8 and SP10.
///
/// Description: In order to build finalchildren, we first build a list of
/// unsimplified children (called unsimplifiedchildren) with the children of
/// the product. Each node of the list is manipulated (see simplifyFactor) in
/// order to satisfy SP2 and SP7 as follows:
/// - SP7: if the node's expression is a value, multiply the value to the
///   products's coef
/// - SP2: if the node's expression is a product, then build a list with the
///   child's children
///
/// Then, we merge the built list (or the simplified node) into
/// finalchildren. While merging, nodes from finalchildren can go back to
/// unsimplifiedchildren for further processing (see mergeProductExprlist()
/// for more details). After building finalchildren, we create the
/// simplified product out of it, taking care that SP8 and SP10 are
/// satisfied.
fn simplify_product(scip: &mut Scip, expr: ExprId, simplifiedexpr: &mut ExprId, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult {
    let mut nodes = Nodes::new();
    let mut simplifiedcoef = get_coef_expr_product(scip, expr);
    let mut changed = false;
    let mut finalchildren = NIL;

    // Simplify and multiply factors.
    let children = scip.exprs[expr].children.clone();
    simplify_multiply_children(
        scip,
        &mut nodes,
        &children,
        &mut simplifiedcoef,
        &mut finalchildren,
        &mut changed,
        ownercreate,
        ownercreatedata,
    )?;

    // Get simplified product from simplified factors in finalchildren.
    let expandalways = hdlrdata(scip, expr).expandalways;
    build_simplified_product(
        scip,
        &mut nodes,
        simplifiedcoef,
        &mut finalchildren,
        expandalways,
        changed,
        simplifiedexpr,
        ownercreate,
        ownercreatedata,
    )?;

    if *simplifiedexpr == NIL {
        *simplifiedexpr = expr;

        // We have to capture it, since it must simulate a "normal" simplified
        // call in which a new expression is created.
        expr::expr_capture(scip, *simplifiedexpr);
    }

    Ok(())
}

/// `compareProduct`: compare two product expressions.
///
/// The order of two product expressions, u and v, is a lexicographical
/// order on the factors.
///
/// Starting from the *last*, we find the first child where they differ,
/// say, the i-th. Then u < v <=> u_i < v_i. If there is no such children and
/// they have different number of children, then u < v <=> nchildren(u) <
/// nchildren(v). If all children are the same and they have the same number
/// of children, then u < v <=> coeff(u) < coeff(v). Otherwise, they are the
/// same.
///
/// Note: we are assuming expression are simplified, so within u, we have
/// u_1 < u_2, etc.
///
/// Example: y * z < x * y * z.
fn compare_product(scip: &Scip, expr1: ExprId, expr2: ExprId) -> i32 {
    let children1 = &scip.exprs[expr1].children;
    let children2 = &scip.exprs[expr2].children;

    let mut i = children1.len() as i32 - 1;
    let mut j = children2.len() as i32 - 1;
    while i >= 0 && j >= 0 {
        let compareresult = expr::expr_compare(scip, children1[i as usize], children2[j as usize]);
        if compareresult != 0 {
            return compareresult;
        }
        // Expressions are equal, continue.
        i -= 1;
        j -= 1;
    }

    // All children of one expression are children of the other expression,
    // use number of children as a tie-breaker.
    if i < j {
        // expr1 has less elements, hence expr1 < expr2.
        return -1;
    }
    if i > j {
        // expr1 has more elements, hence expr1 > expr2.
        return 1;
    }

    // Everything is equal, use coefficient as tie-breaker.
    if get_coef_expr_product(scip, expr1) < get_coef_expr_product(scip, expr2) {
        return -1;
    }
    if get_coef_expr_product(scip, expr1) > get_coef_expr_product(scip, expr2) {
        return 1;
    }

    // They are equal.
    0
}

/// `copyhdlrProduct`.
fn copyhdlr_product(scip: &mut Scip, _source: &Scip, _exprhdlr: ExprhdlrId) -> ScipResult {
    include_exprhdlr_product(scip)
}

/// `freehdlrProduct`.
fn freehdlr_product(scip: &mut Scip, exprhdlr: ExprhdlrId) -> ScipResult {
    scip.exprhdlrs[exprhdlr].data = None;
    Ok(())
}

/// `copydataProduct`.
fn copydata_product(scip: &mut Scip, sourceexpr: ExprId) -> ScipResult<Box<dyn Any>> {
    Ok(Box::new(ExprData {
        coefficient: get_coef_expr_product(scip, sourceexpr),
    }))
}

/// `freedataProduct`.
fn freedata_product(scip: &mut Scip, expr: ExprId) -> ScipResult {
    scip.exprs[expr].exprdata = None;
    Ok(())
}

/// `hashProduct`.
fn hash_product(scip: &mut Scip, expr: ExprId, hashkey: &mut u32, childrenhashes: &[u32]) -> ScipResult {
    *hashkey = exprhdlr_hashkey();
    *hashkey ^= crate::misc::calc_fib_hash(get_coef_expr_product(scip, expr));

    for c in 0..scip.exprs[expr].children.len() {
        *hashkey ^= childrenhashes[c];
    }

    Ok(())
}

/// `evalProduct`.
fn eval_product(scip: &mut Scip, expr: ExprId, val: &mut f64, _sol: SolId) -> ScipResult {
    *val = get_coef_expr_product(scip, expr);
    let children = &scip.exprs[expr].children;
    let mut c = 0;
    while c < children.len() && *val != 0.0 {
        let childval = scip.exprs[children[c]].evalvalue;

        *val *= childval;
        c += 1;
    }

    Ok(())
}

/// `fwdiffProduct`: derivative evaluation callback computing <gradient,
/// children.dot>.
///
/// If expr is prod_i x_i, then computes sum_j prod_{i!=j} x_i x^dot_j.
fn fwdiff_product(scip: &mut Scip, expr: ExprId, dot: &mut f64, _direction: SolId) -> ScipResult {
    // TODO add special handling for nchildren == 2
    let children = &scip.exprs[expr].children;
    let exprval = scip.exprs[expr].evalvalue;

    *dot = 0.0;
    for c in 0..children.len() {
        let child = children[c];

        if scip.exprs[child].dot == 0.0 {
            continue;
        }

        if scip.exprs[child].evalvalue != 0.0 {
            *dot += exprval / scip.exprs[child].evalvalue * scip.exprs[child].dot;
        } else {
            let mut partial = get_coef_expr_product(scip, expr);
            let mut i = 0;
            while i < children.len() && partial != 0.0 {
                if i != c {
                    partial *= scip.exprs[children[i]].evalvalue;
                }
                i += 1;
            }
            *dot += partial * scip.exprs[child].dot;
        }
    }

    Ok(())
}

/// `bwfwdiffProduct`: expression backward forward derivative evaluation
/// callback.
///
/// Computes d/d childidx ( <gradient, children.dot> ).
///
/// If expr is prod_i x_i, and childidx is k then computes
///   d_k sum_j prod_{i != j} x_i x^dot_j = sum_{j != k} prod_{i != j, k} x_i x^dot_j
fn bwfwdiff_product(scip: &mut Scip, expr: ExprId, childidx: i32, bardot: &mut f64, _direction: SolId) -> ScipResult {
    let childidx = childidx as usize;
    let children = &scip.exprs[expr].children;
    let exprval = scip.exprs[expr].evalvalue;
    let partialchild = children[childidx];

    // TODO add special handling for nchildren == 2
    *bardot = 0.0;
    for c in 0..children.len() {
        if c == childidx {
            continue;
        }

        let child = children[c];

        if scip.exprs[child].dot == 0.0 {
            continue;
        }

        if scip.exprs[child].evalvalue != 0.0 && scip.exprs[partialchild].evalvalue != 0.0 {
            *bardot += exprval / (scip.exprs[child].evalvalue * scip.exprs[partialchild].evalvalue) * scip.exprs[child].dot;
        } else {
            let mut partial = get_coef_expr_product(scip, expr);
            let mut i = 0;
            while i < children.len() && partial != 0.0 {
                if i != c && i != childidx {
                    partial *= scip.exprs[children[i]].evalvalue;
                }
                i += 1;
            }
            *bardot += partial * scip.exprs[child].dot;
        }
    }

    Ok(())
}

/// `bwdiffProduct`.
fn bwdiff_product(scip: &mut Scip, expr: ExprId, childidx: i32, val: &mut f64) -> ScipResult {
    let childidx = childidx as usize;
    let children = &scip.exprs[expr].children;
    let child = children[childidx];

    // TODO add special handling for nchildren == 2
    if !scip.set.is_zero(scip.exprs[child].evalvalue) {
        *val = scip.exprs[expr].evalvalue / scip.exprs[child].evalvalue;
    } else {
        *val = get_coef_expr_product(scip, expr);
        let mut i = 0;
        while i < children.len() && *val != 0.0 {
            if i != childidx {
                *val *= scip.exprs[children[i]].evalvalue;
            }
            i += 1;
        }
    }

    Ok(())
}

/// `intevalProduct`.
fn inteval_product(scip: &mut Scip, expr: ExprId, interval: &mut Interval, _intevalvar: Option<ExprIntEvalVar>, _intevalvardata: usize) -> ScipResult {
    interval.set(get_coef_expr_product(scip, expr));

    for &child in &scip.exprs[expr].children {
        let childinterval = scip.exprs[child].activity;
        if interval::is_empty(INTERVAL_INFINITY, childinterval) {
            interval.set_empty();
            break;
        }

        // Multiply childinterval with the so far computed interval.
        let cur = *interval;
        interval::mul(INTERVAL_INFINITY, interval, cur, childinterval);
    }

    Ok(())
}

/// `estimateProduct`: estimates a multilinear function of the form f(x) :=
/// a prod_{i = 1}^n x_i.
///
/// x_i are the auxiliary variables of the children. If !overestimate, then
/// we look for an affine underestimator of f(x) which has a value above
/// targetvalue at x^*, i.e., g(x) := alpha^T x + beta <= f(x) for all x in
/// the domain, such that alpha x^* + beta > targetvalue.
///
/// Since f(x) is componentwise linear, its convex envelope is piecewise
/// linear and its value can be computed by finding the largest affine
/// underestimator. This is done either explicitly (if n=2) or by solving an
/// LP, see SCIPcomputeFacetVertexPolyhedralNonlinear().
#[allow(clippy::too_many_arguments)]
fn estimate_product(
    scip: &mut Scip,
    expr: ExprId,
    localbounds: &[Interval],
    _globalbounds: &[Interval],
    refpoint: &[f64],
    overestimate: bool,
    targetvalue: f64,
    coefs: &mut [f64],
    constant: &mut f64,
    islocal: &mut bool,
    success: &mut bool,
    _branchcand: &mut [bool],
) -> ScipResult {
    let coefficient = get_coef_expr_product(scip, expr);

    *success = false;
    *islocal = true;

    let nchildren = scip.exprs[expr].children.len();

    // Bilinear term.
    if nchildren == 2 {
        // Collect first variable.
        let mut refpointx = refpoint[0];
        let bndx = localbounds[0];

        // Collect second variable.
        let mut refpointy = refpoint[1];
        let bndy = localbounds[1];

        // Adjust the reference points.
        refpointx = cmin(cmax(refpointx, bndx.inf), bndx.sup);
        refpointy = cmin(cmax(refpointy, bndy.inf), bndy.sup);

        coefs[0] = 0.0;
        coefs[1] = 0.0;
        *constant = 0.0;
        *success = true;

        let (c0, c1) = coefs.split_at_mut(1);
        crate::nlhdlr_bilinear::add_bilin_mccormick(
            scip,
            coefficient,
            bndx.inf,
            bndx.sup,
            refpointx,
            bndy.inf,
            bndy.sup,
            refpointy,
            overestimate,
            &mut c0[0],
            &mut c1[0],
            constant,
            success,
        );
    } else if let Some(conshdlr) = hdlrdata(scip, expr).conshdlr {
        estimate_vertex_polyhedral_product(
            scip,
            conshdlr,
            nchildren,
            localbounds,
            coefficient,
            Some(refpoint),
            overestimate,
            targetvalue,
            coefs,
            constant,
            success,
        )?;
    }

    Ok(())
}

/// `initestimatesProduct`.
fn initestimates_product(scip: &mut Scip, expr: ExprId, bounds: &[Interval], overestimate: bool, coefs: &mut [Vec<f64>], constant: &mut [f64], nreturned: &mut i32) -> ScipResult {
    let mut success = true;

    let nchildren = scip.exprs[expr].children.len();
    let coefficient = get_coef_expr_product(scip, expr);

    if nchildren == 2 {
        let bndx = bounds[0];
        let bndy = bounds[1];

        constant[0] = 0.0;
        coefs[0][0] = 0.0;
        coefs[0][1] = 0.0;

        // Build estimator.
        let (c0, c1) = coefs[0].split_at_mut(1);
        crate::nlhdlr_bilinear::add_bilin_mccormick(
            scip,
            coefficient,
            bndx.inf,
            bndx.sup,
            (bndx.inf + bndx.sup) / 2.0,
            bndy.inf,
            bndy.sup,
            (bndy.inf + bndy.sup) / 2.0,
            overestimate,
            &mut c0[0],
            &mut c1[0],
            &mut constant[0],
            &mut success,
        );
    } else if let Some(conshdlr) = hdlrdata(scip, expr).conshdlr {
        let target = if overestimate { scip.set.infinity() } else { -scip.set.infinity() };
        estimate_vertex_polyhedral_product(
            scip,
            conshdlr,
            nchildren,
            bounds,
            coefficient,
            None,
            overestimate,
            target,
            &mut coefs[0],
            &mut constant[0],
            &mut success,
        )?;
    }

    if success {
        *nreturned = 1;
    }

    Ok(())
}

/// `reversepropProduct`.
fn reverseprop_product(scip: &mut Scip, expr: ExprId, bounds: Interval, childrenbounds: &mut [Interval], infeasible: &mut bool) -> ScipResult {
    *infeasible = false;

    let nchildren = scip.exprs[expr].children.len();

    // Too expensive (runtime here is quadratic in number of children)
    // TODO implement something faster for larger numbers of factors, e.g.,
    // split product into smaller products.
    if nchildren > 10 {
        return Ok(());
    }

    // Not possible to learn bounds on children if expression bounds are
    // unbounded in both directions.
    if interval::is_entire(INTERVAL_INFINITY, bounds) {
        return Ok(());
    }

    let coefficient = get_coef_expr_product(scip, expr);

    let mut zero = Interval::default();
    zero.set(0.0);

    // f = const * prod_k c_k => c_i solves c_i * (const * prod_{j:j!=i} c_j) =
    // f.
    let mut i = 0;
    while i < nchildren && !*infeasible {
        let mut otherfactor = Interval::default();
        otherfactor.set(coefficient);

        // Compute prod_{j:j!=i} c_j.
        for j in 0..nchildren {
            if i == j {
                continue;
            }

            // TODO we should compute these only one time instead of repeating this
            // for almost every i.
            let childbounds = childrenbounds[j];
            if interval::is_empty(INTERVAL_INFINITY, childbounds) {
                *infeasible = true;
                return Ok(());
            }

            let cur = otherfactor;
            interval::mul(INTERVAL_INFINITY, &mut otherfactor, cur, childbounds);
        }

        let mut childbounds = childrenbounds[i];
        if interval::is_empty(INTERVAL_INFINITY, childbounds) {
            *infeasible = true;
            return Ok(());
        }

        // Solve x*otherfactor = f for x in c_i.
        let xbnds = childbounds;
        interval::solve_univariate_quad_expression(INTERVAL_INFINITY, &mut childbounds, zero, otherfactor, bounds, xbnds);

        // Store computed bounds of the expression.
        let cur = childrenbounds[i];
        interval::intersect(&mut childrenbounds[i], cur, childbounds);
        if interval::is_empty(INTERVAL_INFINITY, childrenbounds[i]) {
            *infeasible = true;
            return Ok(());
        }
        i += 1;
    }

    Ok(())
}

/// `curvatureProduct`.
fn curvature_product(scip: &mut Scip, expr: ExprId, exprcurvature: ExprCurv, success: &mut bool, childcurv: &mut [ExprCurv]) -> ScipResult {
    if scip.exprs[expr].children.len() == 1 {
        childcurv[0] = exprcurv_multiply(get_coef_expr_product(scip, expr), exprcurvature);
        *success = true;
    } else {
        *success = false;
    }

    Ok(())
}

/// `monotonicityProduct`.
fn monotonicity_product(scip: &mut Scip, expr: ExprId, childidx: i32, result: &mut Monotone) -> ScipResult {
    let coef = get_coef_expr_product(scip, expr);

    // Count the number of negative children (except for childidx); if some
    // children changes sign -> monotonicity unknown.
    let mut nneg = 0;
    for i in 0..scip.exprs[expr].children.len() {
        if i as i32 == childidx {
            continue;
        }

        let child = scip.exprs[expr].children[i];
        expr::expr_eval_activity(scip, child)?;
        let interval = scip.exprs[child].activity;

        if interval.sup <= 0.0 {
            nneg += 1;
        } else if interval.inf < 0.0 {
            *result = Monotone::Unknown;
            return Ok(());
        }
    }

    // Note that the monotonicity depends on the sign of the coefficient.
    *result = if nneg % 2 == 0 {
        if coef >= 0.0 { Monotone::Inc } else { Monotone::Dec }
    } else if coef >= 0.0 {
        Monotone::Dec
    } else {
        Monotone::Inc
    };

    Ok(())
}

/// `integralityProduct`.
fn integrality_product(scip: &mut Scip, expr: ExprId, integrality: &mut ImplintType) -> ScipResult {
    *integrality = if eps_is_int(get_coef_expr_product(scip, expr), 0.0) {
        ImplintType::Strong
    } else {
        ImplintType::None
    };

    let children = &scip.exprs[expr].children;
    let mut i = 0;
    while i < children.len() && *integrality != ImplintType::None {
        let ci = scip.exprs[children[i]].integrality;
        if (ci as i32) < (*integrality as i32) {
            *integrality = ci;
        }
        i += 1;
    }

    Ok(())
}

/// `SCIPincludeExprhdlrProduct`: creates the handler for product
/// expressions and includes it into SCIP.
pub fn include_exprhdlr_product(scip: &mut Scip) -> ScipResult {
    // Allocate expression handler data.
    let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
    let exprhdlrdata = ExprhdlrData {
        conshdlr: if conshdlr == crate::var::NONE { None } else { Some(conshdlr) },
        expandalways: false,
    };

    let exprhdlr = crate::scip_expr::include_exprhdlr(scip, EXPRHDLR_NAME, Some(EXPRHDLR_DESC), EXPRHDLR_PRECEDENCE, eval_product, Some(Box::new(exprhdlrdata)))?;

    let h = &mut scip.exprhdlrs[exprhdlr];
    h.copyhdlr = Some(copyhdlr_product);
    h.freehdlr = Some(freehdlr_product);
    h.copydata = Some(copydata_product);
    h.freedata = Some(freedata_product);
    h.simplify = Some(simplify_product);
    h.compare = Some(compare_product);
    h.print = true;
    h.inteval = Some(inteval_product);
    h.initestimates = Some(initestimates_product);
    h.estimate = Some(estimate_product);
    h.reverseprop = Some(reverseprop_product);
    h.hash = Some(hash_product);
    h.bwdiff = Some(bwdiff_product);
    h.fwdiff = Some(fwdiff_product);
    h.bwfwdiff = Some(bwfwdiff_product);
    h.curvature = Some(curvature_product);
    h.monotonicity = Some(monotonicity_product);
    h.integrality = Some(integrality_product);
    h.getsymdata = true;

    crate::paramset::paramset_add_bool(
        scip,
        &format!("expr/{EXPRHDLR_NAME}/expandalways"),
        "whether to expand products of a sum and several factors in simplify",
        crate::data_target!(bool, exprhdlrs, exprhdlr, data, ExprhdlrData, expandalways),
        false,
        false,
        None,
    )?;

    Ok(())
}

/// `SCIPcreateExprProduct`: creates a product expression.
pub fn create_expr_product(scip: &mut Scip, children: &[ExprId], coefficient: f64, ownercreate: Option<ExprOwnerCreate>, ownercreatedata: usize) -> ScipResult<ExprId> {
    let h = scip.set.exprhdlrproduct.expect("product expression handler");
    expr::expr_create(scip, h, Some(Box::new(ExprData { coefficient })), children, ownercreate, ownercreatedata)
}

/// `SCIPgetCoefExprProduct`: gets the constant coefficient of a product
/// expression.
#[inline]
pub fn get_coef_expr_product(scip: &Scip, expr: ExprId) -> f64 {
    scip.exprs[expr].exprdata.as_ref().unwrap().downcast_ref::<ExprData>().unwrap().coefficient
}

/// C's `MAX(a, b)`.
#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// C's `MIN(a, b)`.
#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}
