//! Clean Strings in the app: gather the open strings, run the pipeline per
//! layer, write the result as one undo step, ring what is left and report.
//!
//! Every document lookup goes through the active project, never the
//! composite `scene_document`: it omits hidden objects. The planning is
//! pure (objects in, commands, rings and report text out) so it can be
//! tested without an `App`.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::{DVec2, DVec3};

use crate::{
    app::{
        App,
        jobs::{CancelFlag, JobKey},
    },
    i18n::tr,
    model::{
        Command, LayerId, Object, ObjectId, PolyVertex, SceneEntityId,
        progress::Progress,
        string_clean::{self, Change, ChangeKind, JOIN_ON_REQUEST, LayerClean, OddOneOut, Piece, Problem, ProblemKind},
    },
    ui::state::{StringRing, StringRingKind},
    userspace_error, userspace_log, userspace_warn,
};

/// Why a string is left as drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skipped {
    Arcs,
    NotFinite,
}

/// The open strings of one layer a run takes, in object id order, and the
/// ones of that layer left as drawn.
#[derive(Debug)]
pub(crate) struct LayerJob {
    pub(crate) name: String,
    pub(crate) ids: Vec<ObjectId>,
    pub(crate) objects: Vec<Object>,
    pub(crate) skipped: Vec<(ObjectId, Skipped)>,
    /// The number, counting from zero, each string goes by in the report: by
    /// place in `ids` unless the strings were numbered by an earlier run.
    pub(crate) numbers: Vec<usize>,
    /// The string count the layer's header names: what the layer held when
    /// the strings were numbered, which can be more than `ids` holds.
    pub(crate) header_count: usize,
}

impl LayerJob {
    fn strings(&self) -> Vec<Vec<DVec3>> {
        self.objects.iter().map(polyline_positions).collect()
    }
}

fn polyline_positions(object: &Object) -> Vec<DVec3> {
    match object {
        Object::Polyline { verts, .. } => verts.iter().map(|vertex| vertex.pos).collect(),
        _ => Vec::new(),
    }
}

/// Group open polylines by layer, each layer on its own, its strings in
/// object id order. A string holding an arc or a non-finite coordinate is
/// not taken: it is named as left as drawn. Closed polylines and other
/// objects are ignored.
pub(crate) fn group_layers(objects: Vec<Object>, layer_name: impl Fn(LayerId) -> String) -> Vec<LayerJob> {
    let mut by_layer: BTreeMap<u64, LayerJob> = BTreeMap::new();
    let mut objects: Vec<Object> = objects.into_iter().filter(|object| matches!(object, Object::Polyline { closed: false, .. })).collect();
    objects.sort_by_key(|object| object.id().0);
    objects.dedup_by_key(|object| object.id().0);
    for object in objects {
        let Object::Polyline { verts, layer, .. } = &object else {
            continue;
        };
        let job = by_layer.entry(layer.0).or_insert_with(|| LayerJob {
            name: layer_name(*layer),
            ids: Vec::new(),
            objects: Vec::new(),
            skipped: Vec::new(),
            numbers: Vec::new(),
            header_count: 0,
        });
        if verts.iter().any(|vertex| vertex.bulge != 0.0) {
            job.skipped.push((object.id(), Skipped::Arcs));
        } else if verts.iter().any(|vertex| !vertex.pos.is_finite()) {
            job.skipped.push((object.id(), Skipped::NotFinite));
        } else {
            job.numbers.push(job.ids.len());
            job.ids.push(object.id());
            job.objects.push(object);
            job.header_count = job.ids.len();
        }
    }
    by_layer.into_values().collect()
}

/// Number the strings of `job` as the run that ringed them did, by id in
/// `known`; a string `known` lacks keeps its place. The header counts the
/// distinct numbers `known` gives the strings of the layer (`layer_ids`, the
/// ids of every open string it holds), so a subset is never taken for the
/// layer. Nothing changes when none of the job's strings is known.
pub(crate) fn number_as_ringed(job: &mut LayerJob, known: &HashMap<ObjectId, usize>, layer_ids: impl IntoIterator<Item = ObjectId>) {
    if !job.ids.iter().any(|id| known.contains_key(id)) {
        return;
    }
    job.numbers = job.ids.iter().enumerate().map(|(place, id)| known.get(id).copied().unwrap_or(place)).collect();
    let layer_numbers: HashSet<usize> = layer_ids.into_iter().filter_map(|id| known.get(&id).copied()).collect();
    job.header_count = layer_numbers.len().max(job.ids.len());
}

/// What one layer's run gave, as the report reads it.
pub(crate) struct LayerReport<'a> {
    pub(crate) name: &'a str,
    pub(crate) strings: usize,
    /// The number, from zero, each string goes by, by its place in the run's
    /// input; a place past the end goes by itself.
    pub(crate) numbers: &'a [usize],
    pub(crate) skipped: &'a [(ObjectId, Skipped)],
    pub(crate) pieces: &'a [Piece],
    pub(crate) changes: &'a [Change],
    pub(crate) problems: &'a [Problem],
    pub(crate) odd_ones: &'a [OddOneOut],
    /// The strings Build Surface would leave out of a build of the result,
    /// by place in `pieces`.
    pub(crate) left_out: &'a [usize],
    /// Build Surface's checks on the result: `Some(None)` passes,
    /// `Some(Some(_))` refuses, `None` when the run did not run them.
    pub(crate) verdict: Option<&'a Option<String>>,
}

/// The console text of one run: what changed, then what is left.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Report {
    pub(crate) log: Vec<String>,
    pub(crate) warn: Vec<String>,
}

fn at_xyz(at: DVec3) -> [String; 3] {
    [format!("{:.3}", at.x), format!("{:.3}", at.y), format!("{:.2}", at.z)]
}

/// "3", "3 and 7", "3, 5 and 7".
fn number_list(numbers: &[usize]) -> String {
    word_list(&numbers.iter().map(usize::to_string).collect::<Vec<_>>())
}

/// "a", "a and b", "a, b and c".
pub(crate) fn word_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} {} {}", rest.join(", "), tr!("cmd-string-clean-and"), last),
    }
}

/// The strings meeting in one place, numbered from one, each once, lowest
/// first.
fn names_of(sources: impl IntoIterator<Item = usize>) -> String {
    let mut numbers: Vec<usize> = sources.into_iter().map(|source| source + 1).collect();
    numbers.sort_unstable();
    numbers.dedup();
    number_list(&numbers)
}

fn number_of(numbers: &[usize], source: usize) -> usize {
    numbers.get(source).copied().unwrap_or(source)
}

fn change_line(change: &Change, numbers: &[usize]) -> String {
    let number = |source: usize| number_of(numbers, source);
    let string = (number(change.source) + 1).to_string();
    let [x, y, z] = at_xyz(change.at);
    match &change.kind {
        ChangeKind::RepeatsMerged { count } => tr!("cmd-string-clean-repeats-merged", string = string, count = count.to_string(), x = x, y = y, z = z),
        ChangeKind::SpikeDropped => tr!("cmd-string-clean-spike-dropped", string = string, x = x, y = y, z = z),
        ChangeKind::RetraceDropped { vertices } => tr!("cmd-string-clean-retrace-dropped", string = string, count = vertices.to_string(), x = x, y = y, z = z),
        ChangeKind::LoopCut { vertices } => tr!("cmd-string-clean-loop-cut", string = string, count = vertices.to_string(), x = x, y = y, z = z),
        ChangeKind::ZeroDropped => tr!("cmd-string-clean-zero-dropped", string = string, x = x, y = y),
        ChangeKind::HeightDropped { offset } => tr!(
            "cmd-string-clean-height-dropped",
            string = string,
            offset = format!("{:.2}", offset.abs()),
            x = x,
            y = y,
            z = z
        ),
        ChangeKind::SharedCut { kept, length } => tr!(
            "cmd-string-clean-shared-cut",
            string = string,
            length = format!("{length:.2}"),
            kept = (number(*kept) + 1).to_string(),
            x = x,
            y = y,
            z = z
        ),
        ChangeKind::Removed { kept } => tr!("cmd-string-clean-removed", string = string, kept = (number(*kept) + 1).to_string()),
        ChangeKind::Joined { others, miss } => tr!(
            "cmd-string-clean-joined",
            strings = names_of(others.iter().copied().chain([change.source]).map(number)),
            z = z,
            x = x,
            y = y,
            miss = format!("{miss:.2}")
        ),
        ChangeKind::VertexShared { others, miss } => tr!(
            "cmd-string-clean-vertex-shared",
            strings = names_of(others.iter().copied().chain([change.source]).map(number)),
            x = x,
            y = y,
            miss = format!("{miss:.2}")
        ),
    }
}

fn problem_line(problem: &Problem, pieces: &[Piece], numbers: &[usize]) -> String {
    problem_line_named(problem, &|piece| pieces.get(piece).map(|piece| number_of(numbers, piece.source)))
}

/// One problem's console line, each string named by `name` from its place
/// in the strings checked (counting from zero, shown from one); a string
/// `name` cannot place is left out.
pub(crate) fn problem_line_named(problem: &Problem, name: &dyn Fn(usize) -> Option<usize>) -> String {
    let strings = names_of(problem.sides.iter().filter_map(|side| name(side.piece)));
    let [x, y, _] = at_xyz(problem.at);
    match &problem.kind {
        ProblemKind::TooShort => tr!("cmd-string-clean-hand-too-short", string = strings, x = x, y = y),
        ProblemKind::EndsWhereItStarts => tr!("cmd-string-clean-hand-ends-where-it-starts", string = strings, x = x, y = y),
        ProblemKind::TurnsBack => tr!("cmd-string-clean-hand-turns-back", string = strings, x = x, y = y),
        ProblemKind::CrossesItself => tr!("cmd-string-clean-hand-crosses-itself", string = strings, x = x, y = y),
        ProblemKind::PointsDisagree { miss } => tr!("cmd-string-clean-hand-points-disagree", string = strings, miss = format!("{miss:.2}"), x = x, y = y),
        ProblemKind::Along => tr!("cmd-string-clean-hand-along", strings = strings, x = x, y = y),
        ProblemKind::Crossing { miss } if *miss <= JOIN_ON_REQUEST => tr!("cmd-string-clean-join-all-crossing", strings = strings, miss = format!("{miss:.2}"), x = x, y = y),
        ProblemKind::Crossing { miss } => tr!("cmd-string-clean-hand-crossing", strings = strings, miss = format!("{miss:.2}"), x = x, y = y),
        ProblemKind::NearMiss { miss } => tr!("cmd-string-clean-hand-near-miss", strings = strings, miss = format!("{miss:.2}"), x = x, y = y),
        ProblemKind::BuildRefuses(refusal) => tr!("cmd-string-clean-hand-build-refuses", refusal = refusal.clone()),
    }
}

/// The line for a string on one side of every string it misses by more
/// than [`JOIN_ON_REQUEST`], the string named `name` (from zero, shown from
/// one). "Every string it crosses" only when every crossing it has is one
/// of those misses.
pub(crate) fn odd_one_out_line(odd: &OddOneOut, name: usize) -> String {
    let string = (name + 1).to_string();
    let (low, high) = (format!("{:.2}", odd.low), format!("{:.2}", odd.high));
    let (count, total) = (odd.count.to_string(), odd.total.to_string());
    let limit = JOIN_ON_REQUEST.to_string();
    match (odd.count == odd.total, odd.below) {
        (true, true) => tr!("cmd-string-clean-odd-below-every", string = string, low = low, high = high, count = count, total = total),
        (true, false) => tr!("cmd-string-clean-odd-above-every", string = string, low = low, high = high, count = count, total = total),
        (false, true) => tr!(
            "cmd-string-clean-odd-below-misses",
            string = string,
            low = low,
            high = high,
            limit = limit,
            count = count,
            total = total
        ),
        (false, false) => tr!(
            "cmd-string-clean-odd-above-misses",
            string = string,
            low = low,
            high = high,
            limit = limit,
            count = count,
            total = total
        ),
    }
}

/// The header of a ring's canvas menu for a problem of two strings or
/// more: the strings and, where it measures one, the miss, as its console
/// line gives them. `None` for a problem of one string.
pub(crate) fn ring_title(problem: &Problem, name: &dyn Fn(usize) -> Option<usize>) -> Option<String> {
    if problem.sides.len() < 2 {
        return None;
    }
    let strings = names_of(problem.sides.iter().filter_map(|side| name(side.piece)));
    Some(match problem.kind {
        ProblemKind::Crossing { miss } | ProblemKind::NearMiss { miss } => tr!("cmd-string-clean-ring-title-miss", strings = strings, miss = format!("{miss:.2}")),
        _ => tr!("cmd-string-clean-ring-title", strings = strings),
    })
}

/// The miss of a crossing problem, for ordering the report.
fn crossing_miss(problem: &Problem) -> Option<f64> {
    match problem.kind {
        ProblemKind::Crossing { miss } => Some(miss),
        _ => None,
    }
}

fn skipped_line(id: ObjectId, why: Skipped) -> String {
    match why {
        Skipped::Arcs => tr!("cmd-string-clean-left-arcs", string = id.0.to_string()),
        Skipped::NotFinite => tr!("cmd-string-clean-left-not-finite", string = id.0.to_string()),
    }
}

/// The lines of one run, nothing capped. The changes come first, each layer
/// under a line naming it. The strings left follow: crossings with the
/// biggest miss first, then the rest, again by layer. A line per layer on
/// whether Build Surface's checks pass ends the report, after the last
/// message that exists.
pub(crate) fn build_report(layers: &[LayerReport]) -> Report {
    let mut report = Report::default();
    let mut verdicts: Vec<String> = Vec::new();
    for layer in layers {
        let header = tr!("cmd-string-clean-layer", layer = layer.name.to_string(), strings = layer.strings.to_string());
        let mut lines: Vec<String> = Vec::new();
        let mut repeated: HashSet<String> = HashSet::new();
        for change in layer.changes {
            let line = change_line(change, layer.numbers);
            let repeatable = matches!(change.kind, ChangeKind::Joined { .. } | ChangeKind::VertexShared { .. });
            if !repeatable || repeated.insert(line.clone()) {
                lines.push(line);
            }
        }
        if !lines.is_empty() {
            report.log.push(header.clone());
            report.log.extend(lines);
        }

        let mut left: Vec<String> = layer.skipped.iter().map(|&(id, why)| skipped_line(id, why)).collect();
        let mut crossings: Vec<&Problem> = layer.problems.iter().filter(|problem| crossing_miss(problem).is_some()).collect();
        crossings.sort_by(|a, b| crossing_miss(b).unwrap_or(0.0).total_cmp(&crossing_miss(a).unwrap_or(0.0)));
        left.extend(crossings.into_iter().map(|problem| problem_line(problem, layer.pieces, layer.numbers)));
        left.extend(
            layer
                .problems
                .iter()
                .filter(|problem| crossing_miss(problem).is_none())
                .map(|problem| problem_line(problem, layer.pieces, layer.numbers)),
        );
        left.extend(
            layer
                .odd_ones
                .iter()
                .filter_map(|odd| layer.pieces.get(odd.piece).map(|piece| odd_one_out_line(odd, number_of(layer.numbers, piece.source)))),
        );
        if !left.is_empty() {
            report.warn.push(header);
            report.warn.extend(left);
        }

        match layer.verdict {
            Some(None) => verdicts.push(tr!("cmd-string-clean-checks-pass", layer = layer.name.to_string())),
            Some(Some(_)) => verdicts.push(tr!(
                "cmd-string-clean-checks-refuse",
                layer = layer.name.to_string(),
                count = layer.problems.len().to_string()
            )),
            None => {}
        }
        if !layer.left_out.is_empty() {
            verdicts.push(tr!(
                "cmd-string-clean-build-would-leave-out",
                strings = names_of(
                    layer
                        .left_out
                        .iter()
                        .filter_map(|&piece| layer.pieces.get(piece).map(|piece| number_of(layer.numbers, piece.source)))
                ),
                layer = layer.name.to_string()
            ));
        }
    }
    if report.log.is_empty() {
        report.log.push(tr!("cmd-string-clean-nothing-to-clean"));
    }
    let any_refuses = layers.iter().any(|layer| matches!(layer.verdict, Some(Some(_))));
    if report.warn.is_empty() && !any_refuses {
        report.log.extend(verdicts);
    } else {
        report.warn.extend(verdicts);
    }
    report
}

/// What a run comes to, decided without the `App`.
#[derive(Debug)]
pub(crate) struct RunPlan {
    /// One batch element each; empty when nothing changed.
    pub(crate) commands: Vec<Command>,
    /// Every string the run took, whether it changed or not.
    pub(crate) taken: Vec<ObjectId>,
    /// One ring per problem that has a place.
    pub(crate) rings: Vec<StringRing>,
    pub(crate) report: Report,
    /// The number each string the run took, and each fresh piece, goes by in
    /// its report; `None` for a run that leaves the numbering as it was.
    pub(crate) numbers: Option<Vec<(ObjectId, usize)>>,
}

/// The id of every piece of a layer: the first piece of a source keeps the
/// source's id, further pieces take a fresh one each.
fn piece_ids(pieces: &[Piece], ids: &[ObjectId], alloc: &mut impl FnMut() -> ObjectId) -> Vec<ObjectId> {
    let mut seen: HashSet<usize> = HashSet::new();
    pieces.iter().map(|piece| if seen.insert(piece.source) { ids[piece.source] } else { alloc() }).collect()
}

fn with_positions(object: &Object, verts: &[DVec3]) -> Object {
    let mut result = object.clone();
    if let Object::Polyline { verts: held, .. } = &mut result {
        *held = verts.iter().map(|&pos| PolyVertex { pos, bulge: 0.0 }).collect();
    }
    result
}

/// The batch elements turning each source into its pieces: the first piece
/// replaces the source when its vertices changed, further pieces are copies
/// of the source (same layer, colour, fill and weight) with fresh ids, and a
/// source with no pieces is deleted.
fn layer_commands(job: &LayerJob, pieces: &[Piece], ids: &[ObjectId]) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut handled: HashSet<usize> = HashSet::new();
    for (index, piece) in pieces.iter().enumerate() {
        let source = &job.objects[piece.source];
        if handled.insert(piece.source) {
            let before_positions = polyline_positions(source);
            if before_positions != piece.verts {
                commands.push(Command::Replace {
                    before: source.clone(),
                    after: with_positions(source, &piece.verts),
                });
            }
        } else {
            let copy = source.with_id_and_layer(ids[index], source.layer());
            commands.push(Command::AddObject(with_positions(&copy, &piece.verts)));
        }
    }
    for (index, source) in job.objects.iter().enumerate() {
        if !handled.contains(&index) {
            commands.push(Command::DeleteObject {
                object: source.clone(),
                index: None,
            });
        }
    }
    commands
}

/// One ring per problem that has a place; the build's own refusal of what
/// nothing else names has none and is only reported. `name` numbers the
/// strings for a ring's menu header as the report does.
fn rings_from<'a>(problems: &'a [Problem], ids: &'a [ObjectId], name: &'a dyn Fn(usize) -> Option<usize>) -> impl Iterator<Item = StringRing> + 'a {
    problems.iter().filter(|problem| problem.at.is_finite()).map(move |problem| StringRing {
        at: problem.at,
        kind: StringRingKind::Left(problem.kind.clone()),
        sides: problem.sides.iter().filter_map(|side| ids.get(side.piece).map(|&id| (id, side.vertex))).collect(),
        title: ring_title(problem, name),
    })
}

/// One layer's join: its strings and the plan positions of its rings.
pub(crate) type JoinJob = (LayerJob, Vec<DVec2>);

fn no_layer_report<'a>(job: &'a LayerJob) -> LayerReport<'a> {
    LayerReport {
        name: &job.name,
        strings: 0,
        numbers: &job.numbers,
        skipped: &job.skipped,
        pieces: &[],
        changes: &[],
        problems: &[],
        odd_ones: &[],
        left_out: &[],
        verdict: None,
    }
}

/// The heavy half of Clean Strings: each layer with strings cleaned, `None`
/// for one with none. `done` hears the layers finished so far and the total.
/// `Err` once `cancelled` says so, with nothing kept.
pub(crate) fn compute_clean(jobs: &[LayerJob], cancelled: &dyn Fn() -> bool, done: &dyn Fn(usize, usize)) -> Result<Vec<Option<LayerClean>>, string_clean::Cancelled> {
    let mut cleaned = Vec::with_capacity(jobs.len());
    for job in jobs {
        cleaned.push((!job.ids.is_empty()).then(|| string_clean::clean_layer(&job.strings(), cancelled)).transpose()?);
        done(cleaned.len(), jobs.len());
    }
    Ok(cleaned)
}

/// The light half of Clean Strings: commands, rings and report from what
/// [`compute_clean`] gave for `jobs`. Fresh ids come from `alloc`.
pub(crate) fn assemble_clean(jobs: &[LayerJob], cleaned: &[Option<LayerClean>], alloc: &mut impl FnMut() -> ObjectId) -> RunPlan {
    let mut plan = RunPlan {
        commands: Vec::new(),
        taken: Vec::new(),
        rings: Vec::new(),
        report: Report::default(),
        numbers: Some(Vec::new()),
    };
    let mut reports = Vec::new();
    for (job, clean) in jobs.iter().zip(cleaned) {
        let Some(clean) = clean else {
            reports.push(no_layer_report(job));
            continue;
        };
        let ids = piece_ids(&clean.pieces, &job.ids, alloc);
        plan.commands.extend(layer_commands(job, &clean.pieces, &ids));
        plan.taken.extend(job.ids.iter().copied());
        if let Some(numbers) = plan.numbers.as_mut() {
            numbers.extend(job.ids.iter().zip(&job.numbers).map(|(&id, &number)| (id, number)));
            numbers.extend(clean.pieces.iter().zip(&ids).map(|(piece, &id)| (id, number_of(&job.numbers, piece.source))));
        }
        let name = |piece: usize| clean.pieces.get(piece).map(|piece| number_of(&job.numbers, piece.source));
        plan.rings.extend(rings_from(&clean.problems, &ids, &name));
        reports.push(LayerReport {
            name: &job.name,
            strings: job.header_count,
            numbers: &job.numbers,
            skipped: &job.skipped,
            pieces: &clean.pieces,
            changes: &clean.changes,
            problems: &clean.problems,
            odd_ones: &clean.odd_ones,
            left_out: &clean.left_out,
            verdict: Some(&clean.refusal),
        });
    }
    plan.report = build_report(&reports);
    plan
}

/// The heavy half of Join at halfway: each layer with strings joined at the
/// positions of its rings, `None` for one with none. `done` and `Err` as for
/// [`compute_clean`].
pub(crate) fn compute_join(jobs: &[JoinJob], cancelled: &dyn Fn() -> bool, done: &dyn Fn(usize, usize)) -> Result<Vec<Option<string_clean::LayerJoin>>, string_clean::Cancelled> {
    let mut joined = Vec::with_capacity(jobs.len());
    for (job, at) in jobs {
        joined.push((!job.ids.is_empty()).then(|| string_clean::join_layer(&job.strings(), at, cancelled)).transpose()?);
        done(joined.len(), jobs.len());
    }
    Ok(joined)
}

/// The light half of Join at halfway, from what [`compute_join`] gave. No
/// string is added or removed.
pub(crate) fn assemble_join(jobs: &[JoinJob], joined: &[Option<string_clean::LayerJoin>]) -> RunPlan {
    let mut plan = RunPlan {
        commands: Vec::new(),
        taken: Vec::new(),
        rings: Vec::new(),
        report: Report::default(),
        numbers: None,
    };
    let mut reports = Vec::new();
    for ((job, _), join) in jobs.iter().zip(joined) {
        let Some(join) = join else {
            reports.push(no_layer_report(job));
            continue;
        };
        plan.commands.extend(layer_commands(job, &join.pieces, &job.ids));
        plan.taken.extend(job.ids.iter().copied());
        let name = |piece: usize| join.pieces.get(piece).map(|piece| number_of(&job.numbers, piece.source));
        plan.rings.extend(rings_from(&join.problems, &job.ids, &name));
        reports.push(LayerReport {
            name: &job.name,
            strings: job.header_count,
            numbers: &job.numbers,
            skipped: &job.skipped,
            pieces: &join.pieces,
            changes: &join.changes,
            problems: &join.problems,
            odd_ones: &join.odd_ones,
            left_out: &join.left_out,
            verdict: None,
        });
    }
    plan.report = build_report(&reports);
    plan
}

/// Drop every ring whose strings are all among `taken`, keeping the rest
/// (rings on strings the run did not look at), then add `fresh`.
pub(crate) fn replace_rings(rings: &mut Vec<StringRing>, taken: &[ObjectId], fresh: Vec<StringRing>) {
    rings.retain(|ring| ring.sides.is_empty() || !ring.sides.iter().all(|(id, _)| taken.contains(id)));
    rings.extend(fresh);
}

/// After the rings are replaced: their old projections go, so the next frame
/// knows the rings are new, and a canvas menu open on one of the old rings
/// is closed, its index now naming another ring or none. A run can land
/// while that menu is open.
pub(crate) fn forget_ring_menu(editor: &mut crate::ui::state::EditorState) {
    editor.string_rings_screen_px.clear();
    if editor.canvas_context_menu_ring.take().is_some() {
        editor.canvas_context_menu_open = false;
        editor.canvas_context_menu_px = None;
    }
}

/// The vertex of `verts` at `at`, within the build's merge distance in plan,
/// counting from zero.
pub(crate) fn vertex_at(verts: &[PolyVertex], at: DVec3) -> Option<usize> {
    verts
        .iter()
        .position(|vertex| vertex.pos.truncate().distance(at.truncate()) <= crate::model::rbf::MERGE_DISTANCE)
}

impl<'a> App<'a> {
    /// Clean the selected open polylines of the active project.
    pub(crate) fn clean_selected_strings(&mut self) {
        let ids: Vec<ObjectId> = self
            .editor
            .selected_handles
            .iter()
            .filter_map(|handle| match handle {
                SceneEntityId::Object(id) => Some(*id),
                _ => None,
            })
            .collect();
        self.clean_strings(&ids);
    }

    fn gather_layers(&self, ids: &[ObjectId]) -> Vec<LayerJob> {
        let document = self.active_document();
        let objects: Vec<Object> = ids.iter().filter_map(|&id| document.get_object(id).cloned()).collect();
        group_layers(objects, |layer| {
            document.layer(layer).map_or_else(|| tr!("cmd-object-edit-unassigned"), |layer| layer.name.clone())
        })
    }

    /// The key a run's job carries: this project at this document revision.
    fn string_clean_key(&self) -> Option<JobKey> {
        let project = self.workspace.active_project()?;
        Some(JobKey::StringClean {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        })
    }

    /// Cancel any run still going, a Clean or a Join, so the newest wins, and
    /// say that this one starts.
    fn begin_run(&mut self, label: &str, strings: usize, layers: usize) {
        self.cancel_jobs(|key| matches!(key, JobKey::StringClean { .. }));
        userspace_log!(
            "{}",
            tr!(
                "cmd-string-clean-run-started",
                label = label.to_string(),
                strings = strings.to_string(),
                layers = layers.to_string()
            )
        );
    }

    /// Clean the open polylines in `ids` of the active project, layer by
    /// layer, on a worker; the batch lands as one undo step when it is done.
    pub(crate) fn clean_strings(&mut self, ids: &[ObjectId]) {
        let Some(key) = self.string_clean_key() else {
            return;
        };
        let jobs = self.gather_layers(ids);
        if jobs.is_empty() {
            userspace_warn!("{}", tr!("cmd-string-clean-nothing-to-clean"));
            return;
        }
        let JobKey::StringClean { runtime_id, .. } = key else {
            return;
        };
        let label = tr!("cmd-string-clean-cleaning-strings");
        self.begin_run(&label, jobs.iter().map(|job| job.ids.len()).sum(), jobs.len());
        let compute = move |cancel: &CancelFlag, progress: &Progress| {
            let cleaned = compute_clean(&jobs, &|| cancel.is_cancelled(), &|done, total| progress.set_items(done as u64, total as u64))
                .map_err(|_| anyhow::anyhow!("{}", tr!("common-cancelled")))?;
            Ok((jobs, cleaned))
        };
        let run_label = label.clone();
        let apply = move |app: &mut App, result: anyhow::Result<(Vec<LayerJob>, Vec<Option<LayerClean>>)>| {
            let (jobs, cleaned) = match result {
                Ok(done) => done,
                Err(error) => {
                    userspace_error!("{}: {error:#}", run_label);
                    return;
                }
            };
            let Some(project) = app.workspace.active_project_mut().filter(|project| project.runtime_id == runtime_id) else {
                return;
            };
            let document = &mut project.project.document;
            let plan = assemble_clean(&jobs, &cleaned, &mut || document.allocate_object_id());
            app.finish_run(plan, &run_label);
        };
        self.spawn_job_reporting_progress(label, vec![key], compute, apply);
    }

    /// Join at the halfway height the rings at `indices`, grouped by layer,
    /// on a worker; the batch lands as one undo step when it is done. The
    /// strings go by the numbers the run that ringed them gave them.
    pub(crate) fn join_at_halfway(&mut self, indices: &[usize]) {
        let Some(key) = self.string_clean_key() else {
            return;
        };
        let rings: Vec<&StringRing> = indices.iter().filter_map(|&index| self.editor.string_rings.get(index)).collect();
        let ids: Vec<ObjectId> = rings.iter().flat_map(|ring| ring.sides.iter().map(|&(id, _)| id)).collect();
        let jobs = self.gather_layers(&ids);
        let document = self.active_document();
        let known = &self.editor.string_numbers;
        let layer_of = |id: ObjectId| document.get_object(id).map(|object| object.layer().0);
        let with_at: Vec<JoinJob> = jobs
            .into_iter()
            .map(|mut job| {
                let layer = job.ids.first().or(job.skipped.first().map(|(id, _)| id)).and_then(|&id| layer_of(id));
                let at = rings
                    .iter()
                    .filter(|ring| ring.sides.first().and_then(|&(id, _)| layer_of(id)) == layer)
                    .map(|ring| ring.at.truncate())
                    .collect();
                number_as_ringed(&mut job, known, known.keys().copied().filter(|&id| layer_of(id) == layer));
                (job, at)
            })
            .collect();
        if with_at.is_empty() {
            return;
        }
        let JobKey::StringClean { runtime_id, .. } = key else {
            return;
        };
        let label = tr!("cmd-string-clean-joining-strings");
        self.begin_run(&label, with_at.iter().map(|(job, _)| job.ids.len()).sum(), with_at.len());
        let compute = move |cancel: &CancelFlag, progress: &Progress| {
            let joined = compute_join(&with_at, &|| cancel.is_cancelled(), &|done, total| progress.set_items(done as u64, total as u64))
                .map_err(|_| anyhow::anyhow!("{}", tr!("common-cancelled")))?;
            Ok((with_at, joined))
        };
        let run_label = label.clone();
        let apply = move |app: &mut App, result: anyhow::Result<(Vec<JoinJob>, Vec<Option<string_clean::LayerJoin>>)>| {
            let (with_at, joined) = match result {
                Ok(done) => done,
                Err(error) => {
                    userspace_error!("{}: {error:#}", run_label);
                    return;
                }
            };
            if app.workspace.active_project().is_none_or(|project| project.runtime_id != runtime_id) {
                return;
            }
            app.finish_run(assemble_join(&with_at, &joined), &run_label);
        };
        self.spawn_job_reporting_progress(label, vec![key], compute, apply);
    }

    /// Write a run's batch as one undo step, replace the rings of the
    /// strings it took with the ones it leaves, and say what happened: the
    /// report, then one line on what the run came to.
    fn finish_run(&mut self, plan: RunPlan, label: &str) {
        if !plan.report.log.is_empty() {
            userspace_log!("{}", plan.report.log.join("\n"));
        }
        if !plan.report.warn.is_empty() {
            userspace_warn!("{}", plan.report.warn.join("\n"));
        }
        userspace_log!(
            "{}",
            tr!(
                "cmd-string-clean-run-finished",
                label = label.to_string(),
                edits = plan.commands.len().to_string(),
                rings = plan.rings.len().to_string()
            )
        );
        if let Some(numbers) = plan.numbers {
            for id in &plan.taken {
                self.editor.string_numbers.remove(id);
            }
            self.editor.string_numbers.extend(numbers);
        }
        replace_rings(&mut self.editor.string_rings, &plan.taken, plan.rings);
        forget_ring_menu(&mut self.editor);
        if !plan.commands.is_empty() {
            self.execute_edit(Command::Batch(plan.commands));
        }
        self.invalidate_geometry();
        self.invalidate_overlay();
        self.redraw_requested = true;
    }

    /// Delete the vertex a ring names through the canvas Delete Vertex's own
    /// path, one undo step, and update the rings; said and left alone when
    /// the string is at its fewest vertices.
    pub(crate) fn delete_ring_vertex(&mut self, id: ObjectId, vertex: usize) {
        if self.delete_polyline_vertex(id, vertex) {
            self.editor.forget_deleted_ring_vertex(id, vertex);
            self.redraw_requested = true;
            return;
        }
        if let Some(Object::Polyline { verts, closed, .. }) = self.active_document().get_object(id) {
            let fewest = if *closed { 3 } else { crate::model::control_checks::MINIMUM_CONTROL_VERTICES };
            if verts.len() <= fewest {
                userspace_warn!("{}", tr!("object-edit-object-needs-least-required-vertices", required = fewest.to_string()));
            }
        }
    }
}
