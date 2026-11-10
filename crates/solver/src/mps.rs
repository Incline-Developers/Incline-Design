//! Free-format MPS, read and written.
//!
//! Names may not contain spaces. The first `N` row is the objective and any
//! other `N` row is dropped; a right-hand side on the objective row is the
//! negated objective offset, as HiGHS reads it.

use std::{collections::HashMap, fmt::Write as _};

use crate::{Problem, Sense};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    ObjSense,
    Rows,
    Columns,
    Rhs,
    Ranges,
    Bounds,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Equal,
    Less,
    Greater,
}

/// Read a free-format MPS program.
pub fn read(text: &str) -> Result<Problem, String> {
    let mut sense = Sense::Minimise;
    let mut section = Section::None;
    let mut objective: Option<String> = None;
    let mut dropped: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut row_ids: HashMap<String, usize> = HashMap::new();
    let mut row_kinds: Vec<RowKind> = Vec::new();
    let mut rhs: Vec<f64> = Vec::new();
    let mut range: Vec<Option<f64>> = Vec::new();
    let mut col_ids: HashMap<String, usize> = HashMap::new();
    let mut cost: Vec<f64> = Vec::new();
    let mut integer: Vec<bool> = Vec::new();
    let mut lower: Vec<f64> = Vec::new();
    let mut upper: Vec<f64> = Vec::new();
    let mut upper_set: Vec<bool> = Vec::new();
    let mut triplets: Vec<(usize, usize, f64)> = Vec::new();
    let mut offset = 0.0;
    let mut in_integer = false;

    let number = |token: &str, line: usize| token.parse::<f64>().map_err(|_| format!("line {line}: '{token}' is not a number"));

    for (line_number, raw) in text.lines().enumerate() {
        let line_number = line_number + 1;
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if !raw.starts_with(char::is_whitespace) {
            section = match tokens[0] {
                "NAME" => Section::None,
                "OBJSENSE" => {
                    if let Some(&word) = tokens.get(1) {
                        sense = if word == "MAX" || word == "MAXIMIZE" { Sense::Maximise } else { Sense::Minimise };
                        Section::None
                    } else {
                        Section::ObjSense
                    }
                }
                "ROWS" => Section::Rows,
                "COLUMNS" => Section::Columns,
                "RHS" => Section::Rhs,
                "RANGES" => Section::Ranges,
                "BOUNDS" => Section::Bounds,
                "ENDATA" => break,
                other => return Err(format!("line {line_number}: section {other} is not read")),
            };
            continue;
        }
        match section {
            Section::None => return Err(format!("line {line_number}: data outside a section")),
            Section::ObjSense => {
                sense = if tokens[0] == "MAX" || tokens[0] == "MAXIMIZE" {
                    Sense::Maximise
                } else {
                    Sense::Minimise
                };
            }
            Section::Rows => {
                let [kind, name] = tokens[..] else {
                    return Err(format!("line {line_number}: a row needs a kind and a name"));
                };
                let kind = match kind {
                    "N" => {
                        if objective.is_none() {
                            objective = Some(name.to_string());
                        } else {
                            dropped.insert(name.to_string());
                        }
                        continue;
                    }
                    "E" => RowKind::Equal,
                    "L" => RowKind::Less,
                    "G" => RowKind::Greater,
                    other => return Err(format!("line {line_number}: row kind {other}")),
                };
                row_ids.insert(name.to_string(), row_kinds.len());
                row_kinds.push(kind);
                rhs.push(0.0);
                range.push(None);
            }
            Section::Columns => {
                if tokens.get(1) == Some(&"'MARKER'") {
                    in_integer = match tokens.get(2) {
                        Some(&"'INTORG'") => true,
                        Some(&"'INTEND'") => false,
                        _ => return Err(format!("line {line_number}: marker")),
                    };
                    continue;
                }
                let name = tokens[0];
                let col = *col_ids.entry(name.to_string()).or_insert_with(|| {
                    cost.push(0.0);
                    integer.push(in_integer);
                    // An integer column without bounds is binary in MPS.
                    lower.push(0.0);
                    upper.push(if in_integer { 1.0 } else { f64::INFINITY });
                    upper_set.push(false);
                    cost.len() - 1
                });
                for pair in tokens[1..].chunks(2) {
                    let [row, value] = pair else {
                        return Err(format!("line {line_number}: a row name without a value"));
                    };
                    let value = number(value, line_number)?;
                    if Some(*row) == objective.as_deref() {
                        cost[col] += value;
                    } else if let Some(&row) = row_ids.get(*row) {
                        triplets.push((row, col, value));
                    } else if !dropped.contains(*row) {
                        return Err(format!("line {line_number}: unknown row {row}"));
                    }
                }
            }
            Section::Rhs | Section::Ranges => {
                // The set name is optional when the line has an even count.
                let pairs = if tokens.len() % 2 == 1 { &tokens[1..] } else { &tokens[..] };
                for pair in pairs.chunks(2) {
                    let [row, value] = pair else { unreachable!() };
                    let value = number(value, line_number)?;
                    if section == Section::Rhs && Some(*row) == objective.as_deref() {
                        offset = -value;
                    } else if let Some(&row) = row_ids.get(*row) {
                        if section == Section::Rhs {
                            rhs[row] = value;
                        } else {
                            range[row] = Some(value);
                        }
                    } else if !dropped.contains(*row) {
                        return Err(format!("line {line_number}: unknown row {row}"));
                    }
                }
            }
            Section::Bounds => {
                let kind = tokens[0];
                let needs_value = !matches!(kind, "FR" | "MI" | "PL" | "BV");
                // KIND [SET] COLUMN [VALUE]
                let (name, value) = match (needs_value, tokens.len()) {
                    (true, 4) => (tokens[2], Some(tokens[3])),
                    (true, 3) => (tokens[1], Some(tokens[2])),
                    (false, 3) => (tokens[2], None),
                    (false, 2) => (tokens[1], None),
                    _ => return Err(format!("line {line_number}: bound")),
                };
                let &col = col_ids.get(name).ok_or_else(|| format!("line {line_number}: unknown column {name}"))?;
                let value = value.map(|value| number(value, line_number)).transpose()?;
                match (kind, value) {
                    ("UP", Some(value)) => {
                        // MPS: a negative upper bound on a column still at
                        // its default lower bound makes that bound -inf.
                        if value < 0.0 && lower[col] == 0.0 {
                            lower[col] = f64::NEG_INFINITY;
                        }
                        upper[col] = value;
                        upper_set[col] = true;
                    }
                    ("LO", Some(value)) => {
                        lower[col] = value;
                        if integer[col] && !upper_set[col] {
                            upper[col] = f64::INFINITY;
                        }
                    }
                    ("FX", Some(value)) => {
                        lower[col] = value;
                        upper[col] = value;
                        upper_set[col] = true;
                    }
                    ("LI", Some(value)) => {
                        integer[col] = true;
                        lower[col] = value;
                        if !upper_set[col] {
                            upper[col] = f64::INFINITY;
                        }
                    }
                    ("UI", Some(value)) => {
                        integer[col] = true;
                        upper[col] = value;
                        upper_set[col] = true;
                    }
                    ("FR", None) => {
                        lower[col] = f64::NEG_INFINITY;
                        upper[col] = f64::INFINITY;
                        upper_set[col] = true;
                    }
                    ("MI", None) => lower[col] = f64::NEG_INFINITY,
                    ("PL", None) => {
                        upper[col] = f64::INFINITY;
                        upper_set[col] = true;
                    }
                    ("BV", None) => {
                        integer[col] = true;
                        lower[col] = 0.0;
                        upper[col] = 1.0;
                        upper_set[col] = true;
                    }
                    _ => return Err(format!("line {line_number}: bound kind {kind}")),
                }
            }
        }
    }

    let mut problem = Problem::new(sense);
    problem.offset = offset;
    for col in 0..cost.len() {
        problem.add_col(cost[col], lower[col], upper[col]);
    }
    if integer.iter().any(|&is| is) {
        problem.integrality = integer;
    }
    triplets.sort_by_key(|&(row, col, _)| (row, col));
    let mut at = 0;
    for (row, &kind) in row_kinds.iter().enumerate() {
        let (row_lower, row_upper) = match (kind, range[row]) {
            (RowKind::Equal, None) => (rhs[row], rhs[row]),
            (RowKind::Less, None) => (f64::NEG_INFINITY, rhs[row]),
            (RowKind::Greater, None) => (rhs[row], f64::INFINITY),
            (RowKind::Equal, Some(r)) if r >= 0.0 => (rhs[row], rhs[row] + r),
            (RowKind::Equal, Some(r)) => (rhs[row] + r, rhs[row]),
            (RowKind::Less, Some(r)) => (rhs[row] - r.abs(), rhs[row]),
            (RowKind::Greater, Some(r)) => (rhs[row], rhs[row] + r.abs()),
        };
        let begin = at;
        while at < triplets.len() && triplets[at].0 == row {
            at += 1;
        }
        // A column repeated within a row is one entry.
        let mut entries: Vec<(usize, f64)> = Vec::with_capacity(at - begin);
        for &(_, col, value) in &triplets[begin..at] {
            match entries.last_mut() {
                Some((last, total)) if *last == col => *total += value,
                _ => entries.push((col, value)),
            }
        }
        problem.add_row(row_lower, row_upper, entries);
    }
    Ok(problem)
}

/// Write `problem` as free-format MPS, columns named `C<j>` and rows `R<i>`.
pub fn write(problem: &Problem) -> String {
    let mut out = String::new();
    let num = |value: f64| format!("{value:e}");
    out.push_str("NAME solver\n");
    if problem.sense == Sense::Maximise {
        out.push_str("OBJSENSE\n    MAX\n");
    }
    out.push_str("ROWS\n N  OBJ\n");
    for row in 0..problem.num_rows() {
        let kind = match (problem.row_lower[row].is_finite(), problem.row_upper[row].is_finite()) {
            _ if problem.row_lower[row] == problem.row_upper[row] => 'E',
            (true, _) => 'G',
            (false, true) => 'L',
            // A free row: written as N would drop it, so as L with an
            // infinite right-hand side it is not; give it a huge bound.
            (false, false) => 'L',
        };
        let _ = writeln!(out, " {kind}  R{row}");
    }
    let (start, index, value) = problem.col_wise();
    let integrality = |col: usize| problem.integrality.get(col).copied().unwrap_or(false);
    out.push_str("COLUMNS\n");
    let mut in_integer = false;
    for col in 0..problem.num_cols() {
        if integrality(col) != in_integer {
            in_integer = integrality(col);
            let marker = if in_integer { "'INTORG'" } else { "'INTEND'" };
            let _ = writeln!(out, "    MARKER    'MARKER'    {marker}");
        }
        if problem.col_cost[col] != 0.0 || start[col] == start[col + 1] {
            let _ = writeln!(out, "    C{col}  OBJ  {}", num(problem.col_cost[col]));
        }
        for at in start[col]..start[col + 1] {
            let _ = writeln!(out, "    C{col}  R{}  {}", index[at], num(value[at]));
        }
    }
    if in_integer {
        out.push_str("    MARKER    'MARKER'    'INTEND'\n");
    }
    out.push_str("RHS\n");
    if problem.offset != 0.0 {
        let _ = writeln!(out, "    RHS  OBJ  {}", num(-problem.offset));
    }
    for row in 0..problem.num_rows() {
        let (lower, upper) = (problem.row_lower[row], problem.row_upper[row]);
        let value = if lower.is_finite() {
            lower
        } else if upper.is_finite() {
            upper
        } else {
            1e30
        };
        if value != 0.0 {
            let _ = writeln!(out, "    RHS  R{row}  {}", num(value));
        }
    }
    out.push_str("RANGES\n");
    for row in 0..problem.num_rows() {
        let (lower, upper) = (problem.row_lower[row], problem.row_upper[row]);
        if lower.is_finite() && upper.is_finite() && lower != upper {
            let _ = writeln!(out, "    RNG  R{row}  {}", num(upper - lower));
        }
    }
    out.push_str("BOUNDS\n");
    for col in 0..problem.num_cols() {
        let (lower, upper) = (problem.col_lower[col], problem.col_upper[col]);
        if integrality(col) && lower == 0.0 && upper == 1.0 {
            let _ = writeln!(out, " BV BND  C{col}");
            continue;
        }
        if lower == upper {
            let _ = writeln!(out, " FX BND  C{col}  {}", num(lower));
            continue;
        }
        match (lower.is_finite(), upper.is_finite()) {
            (false, false) => {
                let _ = writeln!(out, " FR BND  C{col}");
            }
            (false, true) => {
                let _ = writeln!(out, " MI BND  C{col}");
                let _ = writeln!(out, " UP BND  C{col}  {}", num(upper));
            }
            (true, _) => {
                if lower != 0.0 || integrality(col) {
                    let _ = writeln!(out, " LO BND  C{col}  {}", num(lower));
                }
                if upper.is_finite() {
                    let _ = writeln!(out, " UP BND  C{col}  {}", num(upper));
                } else if integrality(col) {
                    let _ = writeln!(out, " PL BND  C{col}");
                }
            }
        }
    }
    out.push_str("ENDATA\n");
    out
}
