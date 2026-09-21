//! The Schedule workspace's setup: the project's loader classes and the
//! machines assigned to them.
//!
//! Unlike the Solids config lists beside it, these edits *are* undoable: one
//! committed cell edit, addition or deletion becomes one
//! [`Command::SetSchedulePlan`] and therefore one Ctrl-Z. The plan is small
//! enough for a snapshot either side, and a snapshot is what guarantees undo
//! puts the same ids back - which every Gantt row is keyed by.
//!
//! Every handler here validates through [`SchedulePlan`] rather than trusting
//! the UI: a command naming a class that has since been deleted, or carrying
//! a rate typed into a stale field, is refused and reported.

use crate::{
    i18n::tr,
    model::{
        Command, ReserveFieldId,
        schedule::{BarId, LoaderAgentId, LoaderClassId, SchedulePlan, ScheduleResult},
    },
    ui::state::ScheduleEdit,
    userspace_warn,
};

impl crate::app::App<'_> {
    /// Apply one fleet edit to the project it was drawn from.
    ///
    /// `session` is the runtime id the UI read the plan under. Commands are
    /// queued during a frame and handled after it, so the project can be
    /// closed, reverted or replaced in between - and because a fresh project
    /// numbers its first class `0` too, the ids inside the edit would land on
    /// unrelated equipment rather than failing to resolve. The token is what
    /// makes that case detectable; stage 2's asynchronous picks and
    /// calculations need the same guarantee over much longer gaps.
    pub(crate) fn apply_schedule_edit(&mut self, session: u32, edit: ScheduleEdit) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        if project.runtime_id != session {
            userspace_warn!("{}", tr!("schedule-stale-edit"));
            return;
        }
        match edit {
            ScheduleEdit::SetName(name) => self.set_schedule_name(name),
            ScheduleEdit::AddClass { name, rate_tph } => self.add_loader_class(name, rate_tph),
            ScheduleEdit::RenameClass { class, name } => self.rename_loader_class(class, name),
            ScheduleEdit::SetClassRate { class, rate_tph } => self.set_loader_class_rate(class, rate_tph),
            ScheduleEdit::DeleteClass(class) => self.delete_loader_class(class),
            ScheduleEdit::AddAgent { name, class } => self.add_loader_agent(name, class),
            ScheduleEdit::RenameAgent { agent, name } => self.rename_loader_agent(agent, name),
            ScheduleEdit::SetAgentClass { agent, class } => self.set_loader_agent_class(agent, class),
            ScheduleEdit::DeleteAgent(agent) => self.delete_loader_agent(agent),
            ScheduleEdit::SetCalendarCells { edits } => self.set_calendar_cells(edits),
            ScheduleEdit::SetTonnageField(field) => self.set_tonnage_field(field),
            ScheduleEdit::SetBarHeight(height) => self.set_bar_height(height),
            ScheduleEdit::AddBar { name, agent, priority, window } => self.add_bar(name, agent, priority, window),
            ScheduleEdit::RenameBar { bar, name } => self.rename_bar(bar, name),
            ScheduleEdit::DeleteBar(bar) => self.delete_bar(bar),
            ScheduleEdit::CopyBar(bar) => self.copy_bar(bar),
            ScheduleEdit::SetBarAgent { bar, agent } => self.set_bar_agent(bar, agent),
            ScheduleEdit::SetBarPriority { bar, priority } => self.set_bar_priority(bar, priority),
            ScheduleEdit::SetBarWindow { bar, window } => self.set_bar_window(bar, window),
            ScheduleEdit::SetBarPlacement {
                bar,
                agent,
                priority,
                window,
                insert_lane,
            } => self.set_bar_placement(bar, agent, priority, window, insert_lane),
            ScheduleEdit::AddMember { bar, position, pick } => self.add_bar_member(bar, position, pick),
            ScheduleEdit::RemoveMember { bar, position } => self.remove_bar_member(bar, position),
            ScheduleEdit::MoveMember { bar, from, to } => self.move_bar_member(bar, from, to),
            ScheduleEdit::SetBarMembers { bar, expected, members } => self.set_bar_members(bar, &expected, &members),
            ScheduleEdit::SetRoutingEnabled(enabled) => self.set_routing_enabled(enabled),
            ScheduleEdit::AddDestination { name, kind } => self.add_destination(name, kind),
            ScheduleEdit::RenameDestination { destination, name } => self.edit_routing(|routing| routing.rename_standalone(destination, &name)),
            ScheduleEdit::DeleteDestination(destination) => self.delete_destination(destination),
            ScheduleEdit::SetDestinationCapacity { destination, capacity_t } => self.set_destination_capacity(destination, capacity_t),
            ScheduleEdit::SetCrusherCells { edits } => self.edit_routing(|routing| routing.set_crusher_cells(&edits)),
            ScheduleEdit::AddRule { name, destinations } => self.add_destination_rule(name, destinations),
            ScheduleEdit::DuplicateRule(rule) => self.duplicate_destination_rule(rule),
            ScheduleEdit::DeleteRule(rule) => self.delete_destination_rule(rule),
            ScheduleEdit::RenameRule { rule, name } => self.edit_routing(|routing| routing.rename_rule(rule, &name)),
            ScheduleEdit::SetRuleEnabled { rule, enabled } => self.edit_routing(|routing| routing.set_rule_enabled(rule, enabled)),
            ScheduleEdit::SetRuleDestinations { rule, destinations } => self.edit_routing(|routing| routing.set_rule_destinations(rule, destinations)),
            ScheduleEdit::SetRuleLoaders { rule, loaders } => self.edit_routing(|routing| routing.set_rule_loaders(rule, loaders)),
            ScheduleEdit::SetRuleSources { rule, sources } => self.edit_routing(|routing| routing.set_rule_sources(rule, sources)),
            ScheduleEdit::SetRuleConditions { rule, conditions } => self.edit_routing(|routing| routing.set_rule_conditions(rule, conditions)),
            ScheduleEdit::MoveRule { rule, later } => self.edit_routing(|routing| routing.move_rule(rule, later)),
            ScheduleEdit::SetDestinationDistance { destination, distance_km } => self.edit_routing(|routing| routing.set_distance_km(destination, distance_km)),
            ScheduleEdit::AddTruckClass { name } => self.add_truck_class(name),
            ScheduleEdit::DuplicateTruckClass(class) => self.duplicate_truck_class(class),
            ScheduleEdit::DeleteTruckClass(class) => self.delete_truck_class(class),
            ScheduleEdit::RenameTruckClass { class, name } => self.edit_trucks(|trucks| trucks.rename_class(class, &name)),
            ScheduleEdit::SetTruckClassPayload { class, payload_t } => self.edit_trucks(|trucks| trucks.set_class_payload(class, payload_t)),
            ScheduleEdit::SetTruckClassSpeeds { class, loaded_kph, unloaded_kph } => self.edit_trucks(|trucks| trucks.set_class_speeds(class, loaded_kph, unloaded_kph)),
            ScheduleEdit::SetTruckCells { edits } => self.edit_trucks(|trucks| trucks.set_truck_cells(&edits)),
            ScheduleEdit::AddTruckingRule { name, classes } => self.add_trucking_rule(name, classes),
            ScheduleEdit::DuplicateTruckingRule(rule) => self.duplicate_trucking_rule(rule),
            ScheduleEdit::DeleteTruckingRule(rule) => self.delete_trucking_rule(rule),
            ScheduleEdit::RenameTruckingRule { rule, name } => self.edit_trucks(|trucks| trucks.rename_rule(rule, &name)),
            ScheduleEdit::SetTruckingRuleEnabled { rule, enabled } => self.edit_trucks(|trucks| trucks.set_rule_enabled(rule, enabled)),
            ScheduleEdit::SetTruckingRuleLoaders { rule, loaders } => self.edit_trucks(|trucks| trucks.set_rule_loaders(rule, loaders)),
            ScheduleEdit::SetTruckingRuleSources { rule, sources } => self.edit_trucks(|trucks| trucks.set_rule_sources(rule, sources)),
            ScheduleEdit::SetTruckingRuleDestinations { rule, destinations } => self.edit_trucks(|trucks| trucks.set_rule_destinations(rule, destinations)),
            ScheduleEdit::SetTruckingRuleClasses { rule, classes } => self.edit_trucks(|trucks| trucks.set_rule_classes(rule, classes)),
            ScheduleEdit::SetCurrency(currency) => self.edit_schedule(|plan| plan.set_currency(&currency)),
            ScheduleEdit::AddCashflowRule { name } => self.add_cashflow_rule(name),
            ScheduleEdit::DuplicateCashflowRule(rule) => self.duplicate_cashflow_rule(rule),
            ScheduleEdit::DeleteCashflowRule(rule) => self.delete_cashflow_rule(rule),
            ScheduleEdit::RenameCashflowRule { rule, name } => self.edit_cashflow(|cashflow| cashflow.rename_rule(rule, &name)),
            ScheduleEdit::SetCashflowRuleEnabled { rule, enabled } => self.edit_cashflow(|cashflow| cashflow.set_rule_enabled(rule, enabled)),
            ScheduleEdit::SetCashflowRuleActivity { rule, activity } => self.edit_cashflow(|cashflow| cashflow.set_rule_activity(rule, activity)),
            ScheduleEdit::SetCashflowRuleLoaders { rule, loaders } => self.edit_cashflow(|cashflow| cashflow.set_rule_loaders(rule, loaders)),
            ScheduleEdit::SetCashflowRuleSources { rule, sources } => self.edit_cashflow(|cashflow| cashflow.set_rule_sources(rule, sources)),
            ScheduleEdit::SetCashflowRuleDestinations { rule, destinations } => self.edit_cashflow(|cashflow| cashflow.set_rule_destinations(rule, destinations)),
            ScheduleEdit::SetCashflowRuleConditions { rule, conditions } => self.edit_cashflow(|cashflow| cashflow.set_rule_conditions(rule, conditions)),
            ScheduleEdit::SetCashflowRuleValue { rule, value_per_tonne } => self.edit_cashflow(|cashflow| cashflow.set_rule_value(rule, value_per_tonne)),
            ScheduleEdit::SetClassReclaimRate { class, rate_tph } => self.edit_schedule(|plan| plan.set_class_reclaim_rate(class, rate_tph)),
            ScheduleEdit::SetReclaimOrder { destination, order } => self.edit_stockpile_routing(destination, |routing| routing.set_reclaim_order(destination, order)),
            ScheduleEdit::AddOpeningLot { destination, name, tonnes_t } => self.add_opening_lot(destination, name, tonnes_t),
            ScheduleEdit::DuplicateOpeningLot { destination, lot } => self.duplicate_opening_lot(destination, lot),
            ScheduleEdit::DeleteOpeningLot { destination, lot } => self.delete_opening_lot(destination, lot),
            ScheduleEdit::RenameOpeningLot { destination, lot, name } => self.edit_stockpile_routing(destination, |routing| routing.rename_opening_lot(destination, lot, &name)),
            ScheduleEdit::MoveOpeningLot { destination, lot, newer } => self.edit_stockpile_routing(destination, |routing| routing.move_opening_lot(destination, lot, newer)),
            ScheduleEdit::AddOpeningPortion { destination, lot, tonnes_t } => {
                self.edit_stockpile_routing(destination, |routing| routing.add_opening_portion(destination, lot, tonnes_t).map(|_| ()))
            }
            ScheduleEdit::DeleteOpeningPortion { destination, lot, portion } => {
                self.edit_stockpile_routing(destination, |routing| routing.remove_opening_portion(destination, lot, portion))
            }
            ScheduleEdit::SetOpeningPortionTonnes {
                destination,
                lot,
                portion,
                tonnes_t,
            } => self.edit_stockpile_routing(destination, |routing| routing.set_opening_portion_tonnes(destination, lot, portion, tonnes_t)),
            ScheduleEdit::SetOpeningPortionValue {
                destination,
                lot,
                portion,
                field,
                value,
            } => self.edit_stockpile_routing(destination, |routing| routing.set_opening_portion_value(destination, lot, portion, field, value)),
            ScheduleEdit::AddReclaimBar {
                name,
                agent,
                priority,
                window,
                sources,
                maximum_t,
            } => self.add_reclaim_bar(name, agent, priority, window, sources, maximum_t),
            ScheduleEdit::SetReclaimSources { bar, sources } => self.set_reclaim_sources(bar, sources),
            ScheduleEdit::SetReclaimMaximum { bar, maximum_t } => self.edit_schedule(|plan| plan.set_reclaim_maximum(bar, maximum_t)),
            ScheduleEdit::SetExperimentHorizon { end_day, interval_h } => self.edit_schedule(|plan| {
                // One edit, both fields: a horizon and the resolution it is
                // split at are typed together and are one undo step.
                plan.experiment_mut().set_planning_end_day(end_day)?;
                plan.experiment_mut().set_interval_h(interval_h)
            }),
            ScheduleEdit::SetExperimentSolveLimits { seconds, relative_gap } => self.edit_schedule(|plan| {
                plan.experiment_mut().set_solve_seconds(seconds)?;
                plan.experiment_mut().set_relative_gap(relative_gap)
            }),
            ScheduleEdit::SetExperimentGradeUnit { field, unit } => self.edit_schedule(|plan| plan.experiment_mut().set_grade_unit(field, unit)),
            ScheduleEdit::SetStockpileRepresentation { destination, representation } => {
                self.edit_stockpile_routing_plan(destination, move |plan| plan.experiment_mut().set_representation(destination, representation))
            }
            ScheduleEdit::SetStockpileChunks { destination, capacities } => {
                self.edit_stockpile_routing_plan(destination, move |plan| plan.experiment_mut().set_receiving_chunks(destination, capacities))
            }
        }
    }

    /// Apply one routing edit as a single undo step.
    ///
    /// The same whole-plan snapshot every other edit here takes: routing lives
    /// in the plan, so one committed rule edit is one Ctrl-Z and the ids an
    /// undo puts back are the ids the rules still name.
    fn edit_routing(&mut self, edit: impl FnOnce(&mut crate::model::schedule::RoutingConfig) -> ScheduleResult) {
        self.edit_schedule(|plan| edit(plan.routing_mut()));
    }

    /// Apply an inventory edit only while the destination resolves as a
    /// stockpile in the current document. Saved references are preserved when
    /// their target later disappears; this guards new authored mutations.
    fn edit_stockpile_routing(&mut self, destination: crate::model::schedule::DestinationId, edit: impl FnOnce(&mut crate::model::schedule::RoutingConfig) -> ScheduleResult) {
        if !self.destination_is_stockpile(destination) {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::NotAStockpile.message());
            return;
        }
        self.edit_routing(edit);
    }

    /// The same stockpile guard, for a setting that lives on the plan rather
    /// than inside the routing configuration.
    fn edit_stockpile_routing_plan(&mut self, destination: crate::model::schedule::DestinationId, edit: impl FnOnce(&mut SchedulePlan) -> ScheduleResult) {
        if !self.destination_is_stockpile(destination) {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::NotAStockpile.message());
            return;
        }
        self.edit_schedule(edit);
    }

    fn destination_is_stockpile(&self, destination: crate::model::schedule::DestinationId) -> bool {
        self.workspace.active_document().is_some_and(|document| {
            crate::model::schedule::destinations::resolve(destination, document.solids(), document.schedule().routing())
                .is_ok_and(|entry| entry.kind == crate::model::schedule::DestinationKind::Stockpile)
        })
    }

    fn edit_trucks(&mut self, edit: impl FnOnce(&mut crate::model::schedule::TruckFleetConfig) -> ScheduleResult) {
        self.edit_schedule(|plan| edit(plan.trucks_mut()));
    }

    fn edit_cashflow(&mut self, edit: impl FnOnce(&mut crate::model::schedule::CashflowConfig) -> ScheduleResult) {
        self.edit_schedule(|plan| edit(plan.cashflow_mut()));
    }

    fn add_cashflow_rule(&mut self, name: String) {
        let mut added = None;
        self.edit_cashflow(|cashflow| {
            added = Some(cashflow.add_rule(&name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_cashflow_rule = Some(id);
            self.editor.schedule_cashflow_draft = None;
        }
    }

    fn duplicate_cashflow_rule(&mut self, rule: crate::model::schedule::CashflowRuleId) {
        let Some(document) = self.workspace.active_document() else { return };
        let cashflow = document.schedule().cashflow();
        let Some(source) = cashflow.rule(rule) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownCashflowRule.message());
            return;
        };
        let name = crate::model::schedule::suggested_name(&source.name, cashflow.rules.iter().map(|rule| rule.name.clone()));
        let mut added = None;
        self.edit_cashflow(|cashflow| {
            added = Some(cashflow.duplicate_rule(rule, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_cashflow_rule = Some(id);
            self.editor.schedule_cashflow_draft = None;
        }
    }

    fn delete_cashflow_rule(&mut self, rule: crate::model::schedule::CashflowRuleId) {
        let mut removed = false;
        self.edit_cashflow(|cashflow| {
            cashflow.remove_rule(rule)?;
            removed = true;
            Ok(())
        });
        if removed && self.editor.schedule_selected_cashflow_rule == Some(rule) {
            self.editor.schedule_selected_cashflow_rule = None;
            self.editor.schedule_cashflow_draft = None;
        }
    }

    fn add_truck_class(&mut self, name: String) {
        let mut added = None;
        self.edit_trucks(|trucks| {
            added = Some(trucks.add_class(&name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_truck_class = Some(id);
            self.editor.schedule_truck_class_draft = None;
        }
    }

    fn duplicate_truck_class(&mut self, class: crate::model::schedule::TruckClassId) {
        let Some(document) = self.workspace.active_document() else { return };
        let trucks = document.schedule().trucks();
        let Some(source) = trucks.class(class) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownTruckClass.message());
            return;
        };
        let name = crate::model::schedule::suggested_name(&source.name, trucks.classes.iter().map(|class| class.name.clone()));
        let mut added = None;
        self.edit_trucks(|trucks| {
            added = Some(trucks.duplicate_class(class, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_truck_class = Some(id);
            self.editor.schedule_truck_class_draft = None;
        }
    }

    fn delete_truck_class(&mut self, class: crate::model::schedule::TruckClassId) {
        let mut removed = false;
        self.edit_trucks(|trucks| {
            trucks.remove_class(class)?;
            removed = true;
            Ok(())
        });
        if removed && self.editor.schedule_selected_truck_class == Some(class) {
            self.editor.schedule_selected_truck_class = None;
            self.editor.schedule_truck_class_draft = None;
        }
    }

    fn add_trucking_rule(&mut self, name: String, classes: Vec<crate::model::schedule::TruckClassId>) {
        let mut added = None;
        self.edit_trucks(|trucks| {
            added = Some(trucks.add_rule(&name, classes.clone())?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_truck_rule = Some(id);
            self.editor.schedule_truck_rule_draft = None;
        }
    }

    fn duplicate_trucking_rule(&mut self, rule: crate::model::schedule::TruckingRuleId) {
        let Some(document) = self.workspace.active_document() else { return };
        let trucks = document.schedule().trucks();
        let Some(source) = trucks.rule(rule) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownTruckingRule.message());
            return;
        };
        let name = crate::model::schedule::suggested_name(&source.name, trucks.rules.iter().map(|rule| rule.name.clone()));
        let mut added = None;
        self.edit_trucks(|trucks| {
            added = Some(trucks.duplicate_rule(rule, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_truck_rule = Some(id);
            self.editor.schedule_truck_rule_draft = None;
        }
    }

    fn delete_trucking_rule(&mut self, rule: crate::model::schedule::TruckingRuleId) {
        let mut removed = false;
        self.edit_trucks(|trucks| {
            trucks.remove_rule(rule)?;
            removed = true;
            Ok(())
        });
        if removed && self.editor.schedule_selected_truck_rule == Some(rule) {
            self.editor.schedule_selected_truck_rule = None;
            self.editor.schedule_truck_rule_draft = None;
        }
    }

    fn set_routing_enabled(&mut self, enabled: bool) {
        self.edit_schedule(|plan| {
            plan.routing_mut().set_enabled(enabled);
            Ok(())
        });
    }

    fn add_destination(&mut self, name: String, kind: crate::model::schedule::DestinationKind) {
        let mut added = None;
        self.edit_routing(|routing| {
            added = Some(routing.add_standalone(&name, kind)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_destination = Some(crate::model::schedule::DestinationId::Standalone(id));
        }
    }

    /// Delete a standalone destination, and take the selection off it.
    ///
    /// Refused while a rule still delivers to it, which the config reports with
    /// those rules named: deletion never edits the rule list on the user's
    /// behalf.
    fn delete_destination(&mut self, destination: crate::model::schedule::StandaloneDestinationId) {
        let mut removed = false;
        self.edit_routing(|routing| {
            routing.remove_standalone(destination)?;
            removed = true;
            Ok(())
        });
        if removed && self.editor.schedule_selected_destination == Some(crate::model::schedule::DestinationId::Standalone(destination)) {
            self.editor.schedule_selected_destination = None;
        }
    }

    fn set_destination_capacity(&mut self, destination: crate::model::schedule::DestinationId, capacity_t: Option<f64>) {
        self.edit_routing(|routing| match destination {
            crate::model::schedule::DestinationId::Solid(solid) => routing.set_solid_capacity(solid, capacity_t),
            crate::model::schedule::DestinationId::Standalone(id) => routing.set_standalone_capacity(id, capacity_t),
        });
    }

    fn add_destination_rule(&mut self, name: String, destinations: Vec<crate::model::schedule::DestinationId>) {
        let mut added = None;
        self.edit_routing(|routing| {
            added = Some(routing.add_rule(&name, destinations.clone())?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_rule = Some(id);
        }
    }

    fn duplicate_destination_rule(&mut self, rule: crate::model::schedule::RuleId) {
        let Some(document) = self.workspace.active_document() else { return };
        let routing = document.schedule().routing();
        let Some(source) = routing.rule(rule) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownRule.message());
            return;
        };
        let name = crate::model::schedule::suggested_name(&source.name, routing.rules.iter().map(|rule| rule.name.clone()));
        let mut added = None;
        self.edit_routing(|routing| {
            added = Some(routing.duplicate_rule(rule, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_rule = Some(id);
        }
    }

    fn delete_destination_rule(&mut self, rule: crate::model::schedule::RuleId) {
        let mut removed = false;
        self.edit_routing(|routing| {
            routing.remove_rule(rule)?;
            removed = true;
            Ok(())
        });
        if removed && self.editor.schedule_selected_rule == Some(rule) {
            self.editor.schedule_selected_rule = None;
        }
    }

    /// Apply one edit to the active project's schedule, as a single undo step.
    ///
    /// An edit the plan refuses is reported and dropped; one that changes
    /// nothing - a property field handing back the value it already holds -
    /// is dropped silently, so viewing or re-committing cannot dirty the
    /// project.
    fn edit_schedule(&mut self, edit: impl FnOnce(&mut SchedulePlan) -> ScheduleResult) {
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let before = document.schedule().clone();
        let mut after = before.clone();
        if let Err(error) = edit(&mut after) {
            userspace_warn!("{}", error.message());
            return;
        }
        if after == before {
            return;
        }
        self.execute_edit(Command::SetSchedulePlan {
            before: Box::new(before),
            after: Box::new(after),
        });
    }

    fn set_schedule_name(&mut self, name: String) {
        self.edit_schedule(|plan| {
            plan.set_name(&name);
            Ok(())
        });
    }

    fn add_loader_class(&mut self, name: String, rate_tph: f64) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_class(&name, rate_tph)?);
            Ok(())
        });
        // Selecting what was just added is editor state, not project state,
        // so it is set here rather than inside the undoable edit.
        if let Some(id) = added {
            self.editor.schedule_selected_class = Some(id);
        }
    }

    fn rename_loader_class(&mut self, id: LoaderClassId, name: String) {
        self.edit_schedule(|plan| plan.rename_class(id, &name));
    }

    fn set_loader_class_rate(&mut self, id: LoaderClassId, rate_tph: f64) {
        self.edit_schedule(|plan| plan.set_class_rate(id, rate_tph));
    }

    fn delete_loader_class(&mut self, id: LoaderClassId) {
        self.edit_schedule(|plan| plan.remove_class(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().class(id).is_none()) {
            self.editor.schedule_selected_class = None;
            self.editor.schedule_class_draft = None;
        }
    }

    fn add_loader_agent(&mut self, name: String, class_id: LoaderClassId) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_agent(&name, class_id)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_agent = Some(id);
        }
    }

    fn rename_loader_agent(&mut self, id: LoaderAgentId, name: String) {
        self.edit_schedule(|plan| plan.rename_agent(id, &name));
    }

    fn set_loader_agent_class(&mut self, id: LoaderAgentId, class_id: LoaderClassId) {
        self.edit_schedule(|plan| plan.set_agent_class(id, class_id));
    }

    fn delete_loader_agent(&mut self, id: LoaderAgentId) {
        self.edit_schedule(|plan| plan.remove_agent(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().agent(id).is_none()) {
            self.editor.schedule_selected_agent = None;
            self.editor.schedule_agent_draft = None;
        }
    }

    fn set_calendar_cells(&mut self, edits: Vec<crate::model::schedule::CalendarCellEdit>) {
        self.edit_schedule(|plan| plan.set_calendar_cells(&edits));
    }

    /// Nominate the field read as tonnes.
    ///
    /// Deliberately not validated here against the project's Field List: a
    /// field can be deleted afterwards, so the choice has to be checked every
    /// time it is read anyway. Checking it in one place - the readiness
    /// report - is what keeps a stale choice explained rather than silently
    /// dropped. See [`crate::app::commands::schedule_readiness`].
    fn set_tonnage_field(&mut self, field: Option<ReserveFieldId>) {
        self.edit_schedule(|plan| {
            plan.set_tonnage_field(field);
            Ok(())
        });
    }

    fn set_bar_height(&mut self, height: f32) {
        self.edit_schedule(|plan| plan.set_bar_height(height));
    }

    fn add_bar(&mut self, name: String, agent: Option<LoaderAgentId>, priority: u32, window: crate::model::schedule::WorkWindow) {
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_bar(&name, agent, priority, window)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_bar = Some(id);
            self.editor.schedule_selected_member = None;
        }
    }

    /// Add a bar permitted to reclaim from a set of stockpiles, and select it.
    ///
    /// No dig-block picking: a reclaim bar names piles and a window, so
    /// creating it never puts the viewport into a picking mode.
    fn add_reclaim_bar(
        &mut self,
        name: String,
        agent: Option<LoaderAgentId>,
        priority: u32,
        window: crate::model::schedule::WorkWindow,
        sources: Vec<crate::model::schedule::DestinationId>,
        maximum_t: Option<f64>,
    ) {
        if !self.reclaim_sources_are_stockpiles(&sources) {
            return;
        }
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.add_reclaim_bar(&name, agent, priority, window, sources, maximum_t)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_bar = Some(id);
            self.editor.schedule_selected_member = None;
        }
    }

    /// Replace a reclaim bar's permitted stockpiles in one edit.
    ///
    /// A selection identical to the one the bar already holds is not an edit
    /// and is dropped before the undo history sees it, so reopening the popup
    /// and closing it again leaves nothing behind.
    fn set_reclaim_sources(&mut self, bar: BarId, sources: Vec<crate::model::schedule::DestinationId>) {
        if !self.reclaim_sources_are_stockpiles(&sources) {
            return;
        }
        // `edit_schedule` already drops an edit that changed nothing, so a
        // selection equal to the one held never reaches the undo history.
        self.edit_schedule(|plan| plan.set_reclaim_sources(bar, sources).map(|_| ()));
    }

    /// Whether every *newly authored* source is a stockpile today.
    ///
    /// Checked against the document because a solid-backed stockpile's kind is
    /// the solid's. An entry the bar already holds that has since stopped
    /// being a stockpile remains visible in the draft and readiness report;
    /// applying a changed source set requires repairing that entry explicitly.
    fn reclaim_sources_are_stockpiles(&mut self, sources: &[crate::model::schedule::DestinationId]) -> bool {
        if sources.is_empty() {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::EmptyRuleSelection.message());
            return false;
        }
        if !sources.iter().all(|source| self.destination_is_stockpile(*source)) {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::NotAStockpile.message());
            return false;
        }
        true
    }

    fn add_opening_lot(&mut self, destination: crate::model::schedule::DestinationId, name: String, tonnes_t: f64) {
        let mut added = None;
        self.edit_stockpile_routing(destination, |routing| {
            added = Some(routing.add_opening_lot(destination, &name, tonnes_t)?);
            Ok(())
        });
        if let Some(lot) = added {
            self.editor.schedule_selected_lot = Some(lot);
        }
    }

    /// Copy a lot into an independent one below it. The name is suggested here
    /// rather than in the model, for the same reason a copied bar's is: the
    /// model refuses a duplicate outright, and choosing one for the user is a
    /// decision that belongs where the user can see it.
    fn duplicate_opening_lot(&mut self, destination: crate::model::schedule::DestinationId, lot: crate::model::schedule::OpeningLotId) {
        if !self.destination_is_stockpile(destination) {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::NotAStockpile.message());
            return;
        }
        let Some(inventory) = self
            .workspace
            .active_document()
            .and_then(|document| document.schedule().routing().inventory(destination).cloned())
        else {
            return;
        };
        let Some(source) = inventory.lot(lot) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownLot.message());
            return;
        };
        let name = crate::model::schedule::suggested_name(&source.name.clone(), inventory.lots.iter().map(|lot| lot.name.clone()));
        let mut added = None;
        self.edit_stockpile_routing(destination, |routing| {
            added = Some(routing.duplicate_opening_lot(destination, lot, &name)?);
            Ok(())
        });
        if let Some(lot) = added {
            self.editor.schedule_selected_lot = Some(lot);
        }
    }

    fn delete_opening_lot(&mut self, destination: crate::model::schedule::DestinationId, lot: crate::model::schedule::OpeningLotId) {
        self.edit_stockpile_routing(destination, |routing| routing.remove_opening_lot(destination, lot));
        let gone = self
            .workspace
            .active_document()
            .is_none_or(|document| document.schedule().routing().inventory(destination).is_none_or(|inventory| inventory.lot(lot).is_none()));
        if gone && self.editor.schedule_selected_lot == Some(lot) {
            self.editor.schedule_selected_lot = None;
        }
    }

    fn rename_bar(&mut self, id: BarId, name: String) {
        self.edit_schedule(|plan| plan.rename_bar(id, &name));
    }

    fn delete_bar(&mut self, id: BarId) {
        self.edit_schedule(|plan| plan.remove_bar(id));
        if self.workspace.active_document().is_none_or(|document| document.schedule().bar(id).is_none()) {
            self.editor.schedule_selected_bar = None;
            self.editor.schedule_selected_member = None;
        }
    }

    /// Copy a bar into an independent one, and select the copy.
    ///
    /// The name is suggested here rather than in the plan, because a suggested
    /// name is a UI convenience: the plan refuses a duplicate outright, and
    /// choosing one for the user is the kind of decision that belongs where
    /// the user can see it. The copy is otherwise a clone - same machine,
    /// same lane, same earliest start and the same ground - which makes it
    /// two bars holding one piece of ground, reported as a conflict rather
    /// than quietly resolved.
    fn copy_bar(&mut self, id: BarId) {
        let Some(plan) = self.workspace.active_document().map(|document| document.schedule()) else {
            return;
        };
        let Some(source) = plan.bar(id) else {
            userspace_warn!("{}", crate::model::schedule::ScheduleError::UnknownBar.message());
            return;
        };
        let name = if source.has_custom_name() {
            let base = tr!("schedule-bar-copy-name", name = source.name().to_owned());
            crate::model::schedule::suggested_name(&base, plan.bars().iter().filter(|bar| bar.has_custom_name()).map(|bar| bar.name().to_owned()))
        } else {
            String::new()
        };
        let mut added = None;
        self.edit_schedule(|plan| {
            added = Some(plan.copy_bar(id, &name)?);
            Ok(())
        });
        if let Some(id) = added {
            self.editor.schedule_selected_bar = Some(id);
            self.editor.schedule_selected_member = None;
        }
    }

    fn set_bar_agent(&mut self, id: BarId, agent: Option<crate::model::schedule::LoaderAgentId>) {
        self.edit_schedule(|plan| plan.set_bar_agent(id, agent));
    }

    fn set_bar_priority(&mut self, id: BarId, priority: u32) {
        self.edit_schedule(|plan| plan.set_bar_priority(id, priority));
    }

    fn set_bar_window(&mut self, id: BarId, window: crate::model::schedule::WorkWindow) {
        self.edit_schedule(|plan| plan.set_bar_window(id, window));
    }

    fn set_bar_placement(&mut self, id: BarId, agent: Option<LoaderAgentId>, priority: u32, window: crate::model::schedule::WorkWindow, insert_lane: bool) {
        self.edit_schedule(|plan| plan.set_bar_placement(id, agent, priority, window, insert_lane));
    }

    fn add_bar_member(&mut self, id: BarId, position: usize, pick: crate::model::schedule::DigBlockPick) {
        // The pick boundary: a reference exists only through a block of the
        // current run, checked here, so nothing downstream ever has to trust
        // a hand-built reference or refresh provenance itself.
        let block = match self.add_dig_block(&pick) {
            Ok(block) => block,
            Err(reason) => {
                userspace_warn!("{}", reason);
                return;
            }
        };
        self.edit_schedule(|plan| plan.insert_bar_member(id, position, block));
    }

    fn remove_bar_member(&mut self, id: BarId, position: usize) {
        self.edit_schedule(|plan| plan.remove_bar_member(id, position));
        // The row under the one that went is the one now at this position;
        // past the end, nothing is selected rather than the wrong thing.
        let length = self
            .workspace
            .active_document()
            .and_then(|document| document.schedule().bar(id))
            .map_or(0, |bar| bar.members().len());
        if self.editor.schedule_selected_member.is_some_and(|selected| selected >= length) {
            self.editor.schedule_selected_member = length.checked_sub(1);
        }
    }

    fn move_bar_member(&mut self, id: BarId, from: usize, to: usize) {
        self.edit_schedule(|plan| plan.move_bar_member(id, from, to));
        if self.editor.schedule_selected_member == Some(from) {
            self.editor.schedule_selected_member = Some(to);
        }
    }

    /// Apply a whole drafted dig order to its bar, as one undo step.
    ///
    /// The sequence editor is a floating window, so it is open across frames
    /// in which anything can happen. Four things are checked here rather than
    /// assumed, and each of them refuses the edit outright: applying half of
    /// a stale draft would be worse than applying none of it.
    ///
    /// - The **project**, by `apply_schedule_edit`'s session token before this
    ///   is reached at all.
    /// - The **bar**, which an undo or a deletion can have taken away.
    /// - The **target order**, against the list the draft was opened from: an
    ///   undo or an edit from elsewhere makes the draft an older version of
    ///   the same order, and writing it back would silently revert the newer
    ///   one.
    /// - Every **new pick**, against the snapshot that is current now - not
    ///   the one it was picked from. A rerun between the pick and Apply
    ///   produces different ground under the same id, and
    ///   [`crate::app::App::add_dig_block`] is the only door a reference
    ///   enters through.
    fn set_bar_members(&mut self, id: BarId, expected: &[crate::model::schedule::DigBlockRef], members: &[crate::ui::state::DraftMember]) {
        let current = self
            .workspace
            .active_document()
            .and_then(|document| document.schedule().bar(id))
            .map(|bar| bar.members().to_vec());
        let resolved = match resolve_drafted_order(current.as_deref(), expected, members, |pick| self.add_dig_block(pick)) {
            Ok(resolved) => resolved,
            Err(refusal) => {
                userspace_warn!("{}", refusal.message());
                // A bar that is gone cannot be edited back into existence, so
                // the window goes with it. Every other refusal leaves the
                // draft on screen, because the draft is where it is fixed.
                if refusal == SequenceRefusal::BarGone {
                    self.editor.close_sequence_editor();
                }
                return;
            }
        };
        let mut committed = false;
        self.edit_schedule(|plan| {
            plan.set_bar_members(id, resolved)?;
            committed = true;
            Ok(())
        });
        // Closed only when the plan accepted the order. A refusal - duplicate
        // ground, say - leaves the window open on the draft that caused it,
        // which is the only place it can be fixed.
        if committed {
            self.editor.close_sequence_editor();
        }
    }
}

/// Why a dig order drafted in the sequence editor was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SequenceRefusal {
    /// The bar is no longer in the schedule - deleted, or undone, while the
    /// window was open.
    BarGone,
    /// The bar's dig order is no longer the one the draft was opened from.
    TargetChanged,
    /// One member picked in the editing session did not survive
    /// revalidation, carrying the pick boundary's own reason.
    Pick(String),
}

impl SequenceRefusal {
    fn message(&self) -> String {
        match self {
            Self::BarGone => crate::model::schedule::ScheduleError::UnknownBar.message(),
            Self::TargetChanged => tr!("sequence-target-changed"),
            Self::Pick(reason) => reason.clone(),
        }
    }
}

/// Turn a drafted member list into the dig order it would replace, or refuse.
///
/// The whole of the decision, kept pure so the three refusals can be checked
/// without a running application: `current` is what the bar holds now
/// (`None` when it is gone), `expected` is what the draft was opened from,
/// and `resolve_pick` is the generation-checked pick boundary -
/// [`crate::app::App::add_dig_block`] in the application, something that
/// refuses on demand in a test.
///
/// Two properties this has to keep. It resolves the *whole* list before
/// returning anything, so a draft carrying one stale pick is refused entire
/// rather than leaving the bar holding the first half of an editing session.
/// And it copies held references through untouched: a member already in the
/// bar keeps the provenance it was captured with, because refreshing a stamp
/// that could no longer be re-derived is exactly the silent repair the
/// identity layer exists to prevent.
fn resolve_drafted_order(
    current: Option<&[crate::model::schedule::DigBlockRef]>,
    expected: &[crate::model::schedule::DigBlockRef],
    members: &[crate::ui::state::DraftMember],
    resolve_pick: impl Fn(&crate::model::schedule::DigBlockPick) -> std::result::Result<crate::model::schedule::DigBlockRef, String>,
) -> std::result::Result<Vec<crate::model::schedule::DigBlockRef>, SequenceRefusal> {
    use crate::ui::state::DraftMember;

    let Some(current) = current else {
        return Err(SequenceRefusal::BarGone);
    };
    if current != expected {
        return Err(SequenceRefusal::TargetChanged);
    }
    let mut resolved = Vec::with_capacity(members.len());
    for member in members {
        match member {
            DraftMember::Held(reference) => resolved.push(*reference),
            DraftMember::Picked(pick) => resolved.push(resolve_pick(pick).map_err(SequenceRefusal::Pick)?),
        }
    }
    Ok(resolved)
}
