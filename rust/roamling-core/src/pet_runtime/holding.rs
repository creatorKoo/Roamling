// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Being picked up, carried and put down: the mouse path and the touch path
//! end in the same `begin_catch`.

use super::*;

impl PetRuntime {
    pub fn pointer_down(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        self.touch_down(pointer, now)
    }

    /// Mouse clicks and touch both express direct intent without a preceding
    /// fast approach. The shell handles artwork hit testing; core also rejects
    /// stale or out-of-bounds contacts and duplicate presses while held.
    pub fn touch_down(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        let position = self.movement.position();
        let inside = pointer.x >= position.x - self.object_size.width / 2.0
            && pointer.y >= position.y - self.object_size.height / 2.0
            && pointer.x <= position.x + self.object_size.width / 2.0
            && pointer.y <= position.y + self.object_size.height / 2.0;
        if self.is_hidden
            || !self.are_interactions_enabled
            || self.behavior.state().is_held()
            || !inside
        {
            return self.interaction(now, Some(false), false);
        }
        self.begin_catch(pointer, now)
    }

    fn begin_catch(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        self.crossing_clear = None;
        self.drag_offset = self.movement.position().vector_from(pointer);
        self.click_reaction_until = 0.0;
        self.is_click_reaction_pending = false;
        self.is_dragging = false;
        self.is_evade_transitioning = false;
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::CatchBegan, now);
        self.caught_animation_until = now + self.caught_transition_duration;
        let mut output = self.interaction(now, None, false);
        output.look_direction_degrees = self
            .last_pointer_decision
            .and_then(|decision| decision.look_direction_degrees);
        output
    }

    pub fn pointer_dragged(
        &mut self,
        pointer: WorldPoint,
        distance: f64,
        now: f64,
    ) -> InteractionOutput {
        if !self.behavior.state().is_held() {
            return self.interaction(now, None, false);
        }
        if distance > 4.0 {
            self.is_dragging = true;
            self.behavior.handle(BehaviorInput::DragMoved, now);
        }
        self.movement
            .teleport(pointer.offset(self.drag_offset), true);
        self.interaction(now, None, true)
    }

    pub fn pointer_up(
        &mut self,
        pointer: WorldPoint,
        was_dragged: bool,
        now: f64,
    ) -> InteractionOutput {
        if !self.behavior.state().is_held() {
            return self.interaction(now, Some(false), false);
        }
        if was_dragged || self.is_dragging {
            self.movement
                .teleport(pointer.offset(self.drag_offset), true);
            self.finish_drop(now);
            let mut output = self.interaction(now, Some(false), true);
            output.persist_position = true;
            return output;
        }

        // A click has the same caught -> four-paw scramble response as a drag.
        // Release panel ownership immediately so the reaction never blocks the
        // underlying app, then finish with the normal landing after one loop.
        self.behavior.handle(BehaviorInput::DragMoved, now);
        self.is_click_reaction_pending = true;
        self.click_reaction_until =
            swift_max(now, self.caught_animation_until) + self.dragged_cycle_duration;
        let clamped = self.world.clamp(self.movement.position(), self.object_size);
        self.movement.teleport(clamped, true);
        self.approach_hold_until = 0.0;
        let mut output = self.interaction(now, Some(false), true);
        output.reschedule_after = Some(1.0 / 30.0);
        output
    }

    pub(super) fn finish_drop(&mut self, now: f64) {
        self.is_dragging = false;
        self.is_click_reaction_pending = false;
        self.is_evade_transitioning = false;
        self.caught_animation_until = 0.0;
        self.click_reaction_until = 0.0;
        self.behavior.handle(BehaviorInput::MouseReleased, now);
        let clamped = self.world.clamp(self.movement.position(), self.object_size);
        self.movement.teleport(clamped, true);
        self.approach_hold_until = 0.0;
        // The whole of `Dropped`, so the pet lands and then notices the user a
        // beat later. Reusing the state's own length keeps this from becoming
        // a second number that has to be kept in step with it.
        self.landing_until = now + crate::behavior::timing::DROPPED;
        self.next_wander_at = now + 1.4;
        self.persist_position = true;
    }
}
