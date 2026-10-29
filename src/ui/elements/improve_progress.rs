//! The Improve run's progress card: a floating card that comes up when
//! Improve starts and stays, on any page, until it is put away after the run.
//!
//! It answers what a planner watching a long run wants to know at a glance:
//! how long it has been going and how long it may go on, which stage it is
//! in and what each took, what the best schedule is worth now and how much
//! better that is than the first, and, once something is proved, how far it
//! can still be from the best there is. The value is charted against time as
//! it rises, and the run can be ended at any point keeping the best schedule
//! found - by hand, or on its own once the search stops finding better ones.
//!
//! Folded, it is one line: the spinner, the clock and the value.

use crate::{
    i18n::tr,
    model::schedule::optimisation::progress::{ImproveEnd, ImproveProgress, ImproveStage},
    ui::{
        EditorState,
        fonts::{bold, bold_font},
        state::UiCommand,
        widgets::{
            menu::{DragableMenu, MenuButton, MenuFieldCombo},
            progress::{ring, ring_fill},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

const CARD_WIDTH: f32 = 380.0;
/// The clock's ring, and its size on the folded card's one line.
const RING: f32 = 40.0;
const FOLDED_RING: f32 = 16.0;
const CHART_HEIGHT: f32 = 128.0;
/// The time strip: how the run's time has gone, stage by stage, against its
/// limit.
const STRIP_HEIGHT: f32 = 8.0;
/// Room left of the chart for its value labels, and under it for its time
/// labels.
const AXIS_WIDTH: f32 = 54.0;
const TIME_LABELS: f32 = 14.0;
/// A bound further above the best schedule than this share of the bound is
/// left off the chart: drawn, it would flatten the value into a line along
/// the floor. The gap still says what it is.
const CHARTED_GAP: f64 = 0.25;
/// What the search may be left to go without a better schedule before the
/// run finishes on its own, in minutes.
const STALL_CHOICES: [Option<u32>; 5] = [None, Some(2), Some(5), Some(10), Some(20)];

const STAGE_COLORS: [(ImproveStage, egui::Color32); 5] = [
    (ImproveStage::Capture, egui::Color32::from_rgb(0x8A, 0x93, 0xA6)),
    (ImproveStage::FirstSchedule, egui::Color32::from_rgb(0x4C, 0x9A, 0xE0)),
    (ImproveStage::Search, egui::Color32::from_rgb(0x50, 0xC8, 0x6E)),
    (ImproveStage::WholeHorizon, egui::Color32::from_rgb(0x9B, 0x7F, 0xE0)),
    (ImproveStage::Publish, egui::Color32::from_rgb(0xE0, 0xA8, 0x4C)),
];

pub(crate) fn draw_improve_card(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(progress) = editor.improve_progress.clone() else {
        return;
    };
    let running = progress.ended.is_none();
    if running {
        // The clock moves with nothing else to redraw the window for.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    let title = match progress.ended {
        Some((_, ImproveEnd::Published)) if progress.finishing => tr!("improve-card-title-stopped"),
        Some((_, ImproveEnd::Published)) => tr!("improve-card-title-published"),
        Some((_, ImproveEnd::NotPublished)) => tr!("improve-card-title-not-published"),
        None if progress.finishing => tr!("improve-card-title-finishing"),
        None => tr!("improve-card-title"),
    };
    let content = ui.ctx().content_rect();
    let mut open = true;
    let mut card = DragableMenu::new("improve_progress", title)
        .min_width(CARD_WIDTH)
        .max_width(CARD_WIDTH)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .default_pos(egui::pos2(content.right() - CARD_WIDTH - 40.0, content.top() + 110.0));
    // Only an ended run's card can be put away; a running one folds instead.
    if !running {
        card = card.open(&mut open);
    }
    card.show(ui.ctx(), |ui| {
        if editor.improve_card_collapsed {
            draw_folded(ui, editor, &progress);
        } else {
            draw_full(ui, editor, &progress, commands);
        }
    });
    if !open {
        commands.push(UiCommand::CloseImproveProgress);
    }
}

/// One line: the spinner, the clock, the value and its gain.
fn draw_folded(ui: &mut egui::Ui, editor: &mut EditorState, progress: &ImproveProgress) {
    ui.horizontal(|ui| {
        ring(ui, FOLDED_RING, progress.ended.map(|_| 1.0));
        ui.label(bold(&clock(progress.elapsed_s())));
        if let Some(best) = progress.best() {
            ui.label(money(best.value));
            if let Some(text) = gain_text(progress) {
                ui.label(egui::RichText::new(text).color(ring_fill(ui.visuals())));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| fold_button(ui, editor));
    });
}

fn draw_full(ui: &mut egui::Ui, editor: &mut EditorState, progress: &ImproveProgress, commands: &mut Vec<UiCommand>) {
    let elapsed = progress.elapsed_s();
    ui.horizontal(|ui| {
        ring(ui, RING, progress.ended.map(|_| 1.0));
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(egui::RichText::new(clock(elapsed)).font(bold_font(22.0)));
            let limit = progress.time_limit_s.map_or_else(|| "-".to_owned(), clock);
            ui.label(egui::RichText::new(tr!("improve-card-limit", limit = limit)).weak().small());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            fold_button(ui, editor);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.label(egui::RichText::new(tr!("improve-card-best")).weak().small());
                    match progress.best() {
                        Some(best) => {
                            ui.label(egui::RichText::new(money(best.value)).font(bold_font(18.0)));
                            match gain_text(progress) {
                                Some(text) => ui.label(egui::RichText::new(text).color(ring_fill(ui.visuals()))),
                                None => ui.label(egui::RichText::new(tr!("improve-card-no-gain")).weak().small()),
                            };
                        }
                        None => {
                            ui.label(egui::RichText::new("-").font(bold_font(18.0)));
                        }
                    }
                });
            });
        });
    });
    ui.add_space(8.0);
    draw_stages(ui, progress);
    ui.add_space(8.0);
    draw_chart(ui, progress);
    ui.add_space(4.0);
    draw_standing(ui, progress);
    draw_controls(ui, editor, progress, commands);
}

/// The fold toggle: the console's chevron, pointing down while unfolded.
fn fold_button(ui: &mut egui::Ui, editor: &mut EditorState) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    let mut chevron = egui::Image::new(crate::ui::themed_icon!(ui, "console_chevron.svg")).fit_to_exact_size(egui::vec2(10.0, 10.0));
    if !editor.improve_card_collapsed {
        chevron = chevron.rotate(std::f32::consts::FRAC_PI_2, egui::Vec2::splat(0.5));
    }
    if response.hovered() {
        ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, ui.visuals().widgets.hovered.bg_fill);
    }
    chevron.paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::vec2(10.0, 10.0)));
    let response = response.on_hover_text(if editor.improve_card_collapsed {
        tr!("improve-card-expand")
    } else {
        tr!("improve-card-collapse")
    });
    if response.clicked() {
        editor.improve_card_collapsed = !editor.improve_card_collapsed;
    }
}

/// The run's time as one strip against its limit, a band per stage, and
/// under it each stage reached and how long it took.
fn draw_stages(ui: &mut egui::Ui, progress: &ImproveProgress) {
    let durations = progress.stage_durations();
    let elapsed = progress.elapsed_s();
    // A run over its limit - publishing takes what it takes - fills the strip.
    let span = progress.time_limit_s.map_or(elapsed, |limit| limit.max(elapsed)).max(1e-6);
    let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), STRIP_HEIGHT), egui::Sense::hover());
    let painter = ui.painter_at(strip);
    let radius = STRIP_HEIGHT / 2.0;
    painter.rect_filled(strip, radius, ui.visuals().extreme_bg_color);
    let mut from = 0.0;
    for (stage, took) in &durations {
        let left = strip.left() + strip.width() * (from / span) as f32;
        let right = strip.left() + strip.width() * ((from + took) / span) as f32;
        if right > left {
            painter.rect_filled(egui::Rect::from_x_y_ranges(left..=right, strip.y_range()), 0.0, stage_color(*stage));
        }
        from += took;
    }
    // Rounded ends, drawn over the bands' square ones.
    painter.rect_stroke(strip, radius, egui::Stroke::new(1.0, ui.visuals().window_fill), egui::StrokeKind::Outside);
    ui.add_space(4.0);
    let current = progress.ended.is_none().then(|| progress.stage());
    // One label per stage, its dot included, so the row wraps between
    // stages and never through one.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let small = egui::TextStyle::Small.resolve(ui.style());
        for (stage, took) in durations {
            let mut job = egui::text::LayoutJob::default();
            job.append("● ", 0.0, egui::TextFormat::simple(small.clone(), stage_color(stage)));
            let (font, color) = if current == Some(stage) {
                (bold_font(small.size), ui.visuals().strong_text_color())
            } else {
                (small.clone(), ui.visuals().weak_text_color())
            };
            job.append(&format!("{} {}", stage_name(stage), duration(took)), 0.0, egui::TextFormat::simple(font, color));
            ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
        }
    });
}

/// The best schedule's value against time, a step at each better one, with
/// the bound above it once one is proved and close enough to read beside it.
fn draw_chart(ui: &mut egui::Ui, progress: &ImproveProgress) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), CHART_HEIGHT), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + AXIS_WIDTH, rect.top() + 4.0),
        egui::pos2(rect.right() - 4.0, rect.bottom() - TIME_LABELS),
    );
    painter.rect_filled(plot, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    let weak = visuals.weak_text_color();
    let small = egui::TextStyle::Small.resolve(ui.style());
    let Some(first) = progress.first_value() else {
        painter.text(plot.center(), egui::Align2::CENTER_CENTER, tr!("improve-card-chart-empty"), small, weak);
        return;
    };
    let elapsed = progress.elapsed_s();
    let span = progress.time_limit_s.map_or(elapsed, |limit| limit.max(elapsed)).max(1.0);
    let best = progress.best().map_or(first, |best| best.value);
    let bound = progress.best().and_then(|best| best.bound).filter(|_| progress.gap().is_some_and(|gap| gap <= CHARTED_GAP));
    // The value's own range, opened out a little either side; never flat.
    let top = bound.unwrap_or(best).max(best);
    let pad = ((top - first).abs() * 0.12).max(first.abs() * 1e-4).max(1.0);
    let (low, high) = (first - pad, top + pad);
    let x_of = |at_s: f64| plot.left() + plot.width() * (at_s / span).clamp(0.0, 1.0) as f32;
    let y_of = |value: f64| plot.bottom() - plot.height() * ((value - low) / (high - low)).clamp(0.0, 1.0) as f32;

    // Gridlines and the value scale at the first schedule and the top; the
    // top's label alone while the two would overlap.
    let label_room = small.size * 2.0;
    for value in [first, top] {
        let y = y_of(value);
        painter.line_segment([egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)], egui::Stroke::new(1.0, weak.gamma_multiply(0.25)));
        if value == top || y - y_of(top) >= label_room {
            painter.text(egui::pos2(plot.left() - 6.0, y), egui::Align2::RIGHT_CENTER, money(value), small.clone(), weak);
        }
    }
    painter.text(egui::pos2(plot.left(), plot.bottom() + 2.0), egui::Align2::LEFT_TOP, clock(0.0), small.clone(), weak);
    painter.text(egui::pos2(plot.right(), plot.bottom() + 2.0), egui::Align2::RIGHT_TOP, clock(span), small.clone(), weak);

    if let Some(bound) = bound {
        let y = y_of(bound);
        let mut x = plot.left();
        while x < plot.right() {
            let end = (x + 5.0).min(plot.right());
            painter.line_segment([egui::pos2(x, y), egui::pos2(end, y)], egui::Stroke::new(1.0, weak));
            x += 9.0;
        }
        painter.text(
            egui::pos2(plot.right() - 4.0, y - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            tr!("improve-card-chart-bound"),
            small.clone(),
            weak,
        );
    }

    // The value as steps: it holds until a better schedule replaces it, and
    // runs on to now.
    let green = ring_fill(visuals);
    let mut line: Vec<egui::Pos2> = Vec::with_capacity(progress.points.len() * 2 + 1);
    for point in &progress.points {
        let (x, y) = (x_of(point.at_s), y_of(point.value));
        if let Some(last) = line.last().copied() {
            line.push(egui::pos2(x, last.y));
        }
        line.push(egui::pos2(x, y));
    }
    if let Some(last) = line.last().copied() {
        line.push(egui::pos2(x_of(elapsed), last.y));
    }
    // The area under it, a column per step: the whole is not convex.
    for pair in line.windows(2).filter(|pair| pair[1].x > pair[0].x) {
        let column = egui::Rect::from_min_max(egui::pos2(pair[0].x, pair[0].y), egui::pos2(pair[1].x, plot.bottom()));
        painter.rect_filled(column, 0.0, green.gamma_multiply(0.18));
    }
    painter.add(egui::Shape::line(line, egui::Stroke::new(2.0, green)));
    // A dot at each better schedule.
    let mut held = f64::NEG_INFINITY;
    for point in &progress.points {
        if point.value > held {
            if held.is_finite() {
                painter.circle_filled(egui::pos2(x_of(point.at_s), y_of(point.value)), 2.5, green);
            }
            held = point.value;
        }
    }
    // Now.
    if progress.ended.is_none() {
        let x = x_of(elapsed);
        painter.line_segment([egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())], egui::Stroke::new(1.0, weak.gamma_multiply(0.6)));
    }

    // What the run held at the time under the pointer.
    if let Some(pointer) = response.hover_pos().filter(|pointer| plot.contains(*pointer)) {
        let at_s = f64::from((pointer.x - plot.left()) / plot.width()) * span;
        if let Some(point) = progress.points.iter().take_while(|point| point.at_s <= at_s).last() {
            response.on_hover_text(format!("{}  {}", clock(at_s), money(point.value)));
        }
    }
}

/// Where the run stands: the gap to the bound, and when the last better
/// schedule came.
fn draw_standing(ui: &mut egui::Ui, progress: &ImproveProgress) {
    ui.horizontal(|ui| {
        match progress.gap() {
            Some(gap) => ui.label(tr!("improve-card-gap", gap = percent(gap))).on_hover_text(tr!("improve-card-gap-note")),
            None if progress.ended.is_some() => ui
                .label(egui::RichText::new(tr!("improve-card-no-bound-ended")).weak())
                .on_hover_text(tr!("improve-card-gap-note")),
            None => ui
                .label(egui::RichText::new(tr!("improve-card-no-bound")).weak())
                .on_hover_text(tr!("improve-card-gap-note")),
        };
        if let Some(at_s) = progress.improved_at_s.filter(|_| progress.ended.is_none()) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(tr!("improve-card-last-gain", ago = duration(progress.elapsed_s() - at_s)))
                        .weak()
                        .small(),
                );
            });
        }
    });
}

fn draw_controls(ui: &mut egui::Ui, editor: &mut EditorState, progress: &ImproveProgress, commands: &mut Vec<UiCommand>) {
    match progress.ended {
        Some((_, ImproveEnd::Published)) => {
            if let Some(best) = progress.best() {
                ui.add_space(4.0);
                ui.label(tr!("improve-card-published", value = money(best.value)));
            }
        }
        Some((_, ImproveEnd::NotPublished)) => {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr!("improve-card-not-published")).weak());
        }
        None => {
            ui.add_space(6.0);
            let stall_text = |minutes: Option<u32>| {
                minutes.map_or_else(
                    || tr!("improve-card-stall-never"),
                    |minutes| tr!("improve-card-stall-minutes", minutes = minutes.to_string()),
                )
            };
            let selected = stall_text(editor.improve_stop_after_stall_min);
            MenuFieldCombo::new(
                "improve_stop_after_stall",
                tr!("improve-card-stall"),
                &mut editor.improve_stop_after_stall_min,
                selected,
                STALL_CHOICES.map(|choice| (choice, egui::WidgetText::from(stall_text(choice)))),
            )
            .help_text(tr!("improve-card-stall-note"))
            .show(ui);
            crate::ui::widgets::menu::menu_actions(ui, |ui| {
                let finishing = progress.finishing;
                if ui
                    .add(MenuButton::new(tr!("improve-card-stop")).primary().enabled(!finishing))
                    .on_hover_text(tr!("improve-card-stop-note"))
                    .clicked()
                {
                    commands.push(UiCommand::FinishImprove);
                }
                if ui
                    .add(MenuButton::new(tr!("improve-card-discard")))
                    .on_hover_text(tr!("improve-card-discard-note"))
                    .clicked()
                {
                    commands.push(UiCommand::CancelScheduleCalculation);
                }
            });
        }
    }
}

fn stage_color(stage: ImproveStage) -> egui::Color32 {
    STAGE_COLORS.iter().find(|(of, _)| *of == stage).map_or(egui::Color32::GRAY, |(_, color)| *color)
}

fn stage_name(stage: ImproveStage) -> String {
    match stage {
        ImproveStage::Capture => tr!("improve-stage-capture"),
        ImproveStage::FirstSchedule => tr!("improve-stage-first"),
        ImproveStage::Search => tr!("improve-stage-search"),
        ImproveStage::WholeHorizon => tr!("improve-stage-whole"),
        ImproveStage::Publish => tr!("improve-stage-publish"),
    }
}

/// How far the best schedule is above the first, in money and as a share.
fn gain_text(progress: &ImproveProgress) -> Option<String> {
    let (first, best) = (progress.first_value()?, progress.best()?.value);
    (best > first).then(|| tr!("improve-card-gain", gain = money(best - first), percent = percent((best - first) / first.abs().max(1e-9))))
}

/// A clock reading: m:ss, or h:mm:ss past the hour.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// A stage's length: tenths of a second while short, then a clock reading.
fn duration(seconds: f64) -> String {
    if seconds < 60.0 { format!("{seconds:.1} s") } else { clock(seconds) }
}

/// A share as a percentage, to two significant figures below 10% - a
/// search's gains on a large schedule are often a few thousandths of one.
fn percent(share: f64) -> String {
    let share = share * 100.0;
    let decimals = if share >= 10.0 {
        1
    } else if share > 0.0 {
        (1 - share.log10().floor() as i32).clamp(2, 4) as usize
    } else {
        2
    };
    format!("{share:.decimals$}%")
}

/// Money to two decimals of its unit, which is fine enough to see a search's
/// gains on a schedule worth hundreds of millions.
fn money(value: f64) -> String {
    let symbol = tr!("common-currency-symbol");
    let size = value.abs();
    let (scaled, suffix) = if size >= 1e9 {
        (value / 1e9, "B")
    } else if size >= 1e6 {
        (value / 1e6, "M")
    } else if size >= 1e3 {
        (value / 1e3, "k")
    } else {
        (value, "")
    };
    let sign = if scaled < 0.0 { "-" } else { "" };
    format!("{sign}{symbol}{:.2}{suffix}", scaled.abs())
}
