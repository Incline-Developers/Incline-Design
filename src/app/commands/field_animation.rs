//! The playback deck: a block model played through in the order of a field,
//! its blocks vanishing step by step - for now a schedule's mining, bench by
//! bench.
//!
//! The order goes into a temporary numeric column; each step is shown by
//! that column's colour ramp alone: a transparent band below the steps done,
//! one band per group (period) above, and the ground band last. So a step
//! only restyles the model, through the renderer's usual ramp path - nothing
//! rebuilds per frame, only when the step changes. The block model is shown
//! alone and the scenarios list hides; closing puts both back and removes
//! the column.
//!
//! Meant to become a general command (animate any field by the order of its
//! values).

use std::sync::Arc;

use crate::{
    app::{App, commands::optimization::shell_color, jobs::CancelFlag},
    i18n::tr,
    model::{
        SceneEntityId,
        block_model::{Boundary, ColorTransferFunction, MAX_GRADIENT_ENTRIES},
        npv_schedule_run::{ORDER_AIR, ORDER_GROUND, format_elevation},
        optimization_run::grid::BlockLayout,
    },
    ui::state::{AnimationRestore, AnimationStep, FieldAnimation},
    userspace_log, userspace_warn,
};

/// The temporary column the order is played from. Removed on close.
pub(crate) const ANIMATION_FIELD: &str = "incline_animation_order";
/// Steps per second to start with.
pub(crate) const DEFAULT_SPEED: f64 = 1.0;
const GROUND_COLOR: [f32; 4] = [0.55, 0.55, 0.55, 1.0];
const CLEAR: [f32; 4] = [0.0; 4];

impl<'a> App<'a> {
    /// Open the deck on a schedule's last run: its mining order, at the pit
    /// where the kept case's NPV peaks.
    pub(crate) fn play_npv_schedule_animation(&mut self, parent: u64, id: u64) {
        let state = &self.editor.optimization;
        let Some(scenario) = state.scenarios.iter().find(|scenario| scenario.id == parent) else {
            return;
        };
        let Some(schedule) = scenario.schedules.iter().find(|schedule| schedule.id == id) else {
            return;
        };
        let (Some(order), Some(run)) = (state.schedule_results.get(&id).and_then(|result| result.order.clone()), state.results.get(&parent).cloned()) else {
            userspace_warn!("{}", tr!("sched-anim-no-run", name = schedule.name.clone()));
            return;
        };
        let title = tr!(
            "sched-anim-title",
            name = schedule.name.clone(),
            case = order.case.label(),
            shell = (order.pit + 1).to_string()
        );
        let model_name = scenario.block_model.clone();
        self.close_field_animation();
        let Some(model) = self.block_models.iter_mut().find(|model| model.name == model_name) else {
            userspace_warn!("{}", tr!("sched-anim-no-model", model = model_name));
            return;
        };
        if !model.state.loaded {
            userspace_warn!("{}", tr!("sched-anim-unloaded", model = model.name.clone()));
            return;
        }
        // Each block's step, from its cell on the scenario's run grid.
        let layout = model
            .uniform_grid
            .clone()
            .and_then(|uniform| BlockLayout::new(Arc::clone(&model.blocks), uniform, |local| model.model.local_to_world(local)))
            .filter(|layout| layout.grid == run.grid);
        let Some(block_of_cell) = layout.and_then(|layout| layout.block_of_cell(&CancelFlag::default()).ok()) else {
            userspace_warn!("{}", tr!("sched-run-grid-changed"));
            return;
        };
        let ground = order.steps.len() as f64 + 1.0;
        let values: Vec<f64> = order
            .block_order(&run, &block_of_cell, model.blocks.len())
            .into_iter()
            .map(|step| match step {
                ORDER_AIR => 0.0,
                ORDER_GROUND => ground,
                step => f64::from(step),
            })
            .collect();
        if let Err(error) = model.model.set_numeric_column(ANIMATION_FIELD, &values) {
            userspace_warn!("{}", tr!("sched-anim-failed", error = error.to_string()));
            return;
        }
        let restore = AnimationRestore {
            list_open: self.editor.optimization.list_open,
            active_variable: model.active_color_variable.replace(ANIMATION_FIELD.to_owned()),
            isolated_entity: self.editor.isolated_entity,
        };
        model.clear_active_values_cache();
        let steps = order
            .steps
            .iter()
            .map(|step| AnimationStep {
                label: tr!(
                    "sched-anim-step",
                    pushback = step.pushback.to_string(),
                    elevation = format_elevation(step.elevation),
                    period = step.period.to_string()
                ),
                group: step.period,
                color: shell_color(0, step.period as usize),
            })
            .collect();
        let block_model = model.id;
        self.editor.isolated_entity = Some(SceneEntityId::BlockModel(block_model));
        self.editor.optimization.list_open = false;
        self.editor.field_animation = Some(FieldAnimation {
            block_model,
            title,
            steps,
            playhead: 0.0,
            playing: false,
            speed: self.editor.optimization.animation_speed.unwrap_or(DEFAULT_SPEED),
            keep_outer: true,
            applied: None,
            restore,
        });
        self.apply_field_animation();
        userspace_log!("{}", tr!("sched-anim-opened"));
    }

    /// Show the deck's step: the model's ramp on the order column.
    pub(crate) fn apply_field_animation(&mut self) {
        let Some(animation) = self.editor.field_animation.as_mut() else {
            return;
        };
        let Some(model) = self.block_models.iter_mut().find(|model| model.id == animation.block_model) else {
            // The model went away: nothing to play or put back on it.
            self.editor.isolated_entity = animation.restore.isolated_entity;
            self.editor.field_animation = None;
            self.invalidate_geometry();
            return;
        };
        let done = animation.done();
        model
            .color_transfers
            .insert(ANIMATION_FIELD.to_owned(), animation_ramp(&animation.steps, done, animation.keep_outer));
        animation.applied = Some((done, animation.keep_outer));
        self.invalidate_geometry();
    }

    /// Close the deck: the column goes, and the model's colouring, the
    /// visibility and the scenarios list come back as they were.
    pub(crate) fn close_field_animation(&mut self) {
        let Some(animation) = self.editor.field_animation.take() else {
            return;
        };
        if let Some(model) = self.block_models.iter_mut().find(|model| model.id == animation.block_model) {
            model.model.remove_column(ANIMATION_FIELD);
            model.color_transfers.remove(ANIMATION_FIELD);
            model.active_color_variable = animation.restore.active_variable.clone();
            model.clear_active_values_cache();
        }
        self.editor.isolated_entity = animation.restore.isolated_entity;
        self.editor.optimization.list_open = animation.restore.list_open;
        self.invalidate_geometry();
        self.invalidate_overlay();
        // The speed is the project's, for the next deck of any schedule.
        let state = &mut self.editor.optimization;
        if state.loaded_from_file && state.animation_speed.unwrap_or(DEFAULT_SPEED) != animation.speed {
            state.animation_speed = Some(animation.speed);
            if let Err(error) = self.write_optimization_scenarios() {
                userspace_warn!("{error:#}");
            }
        }
    }
}

/// The ramp showing `done` of `steps` gone: below the first boundary (steps
/// done, and air at 0) is clear; then one band per group still to come, in
/// its colour; then the ground (`steps + 1`), grey or clear.
fn animation_ramp(steps: &[AnimationStep], done: usize, keep_outer: bool) -> ColorTransferFunction {
    let count = steps.len();
    let ground = if keep_outer { GROUND_COLOR } else { CLEAR };
    let boundary = |value: f64| Boundary { id: 0, value, inclusive: false };
    let mut boundaries = vec![boundary(done as f64 + 0.5)];
    let mut gradient = vec![CLEAR];
    if done < count {
        gradient.push(steps[done].color);
        // Where each later group starts; thinned evenly to what the ramp holds
        // (the cut and the ground take a boundary each).
        let starts: Vec<usize> = (done + 1..count).filter(|&step| steps[step].group != steps[step - 1].group).collect();
        let room = MAX_GRADIENT_ENTRIES - 3;
        let every = starts.len().div_ceil(room).max(1);
        for &start in starts.iter().step_by(every) {
            boundaries.push(boundary(start as f64 + 0.5));
            gradient.push(steps[start].color);
        }
        boundaries.push(boundary(count as f64 + 0.5));
    }
    gradient.push(ground);
    for (index, boundary) in boundaries.iter_mut().enumerate() {
        boundary.id = index as u64 + 1;
    }
    ColorTransferFunction::Discrete { boundaries, gradient }
}
