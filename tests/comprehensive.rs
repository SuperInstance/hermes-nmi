//! Comprehensive tests for hermes-nmi modules.
//!
//! These cover the pulse types, dispatcher translation logic,
//! telemetry helpers, and tension/budget edge cases.
//!
//! The neuro-muscular interface is the synapse between thinking and doing.
//! If it mistranslates a Navigate pulse or mishandles tension,
//! the agent twitches instead of acting.

#[cfg(test)]
mod pulse_tests {
    use hermes_nmi::pulse::*;

    #[test]
    fn reasoning_pulse_defaults_are_sensible() {
        let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 2.0, 3.0]);
        assert_eq!(pulse.intent_type, IntentType::Navigate);
        assert_eq!(pulse.target_coordinates, [1.0, 2.0, 3.0]);
        assert!((pulse.gravity - 0.5).abs() < 1e-9);
        assert!((pulse.energy_quota - 1.0).abs() < 1e-9);
        assert!(pulse.constraints.is_empty());
    }

    #[test]
    fn gravity_is_clamped() {
        let pulse = ReasoningPulse::new(IntentType::Observe, [0.0; 3])
            .with_gravity(5.0);
        assert!((pulse.gravity - 1.0).abs() < 1e-9);

        let pulse2 = ReasoningPulse::new(IntentType::Observe, [0.0; 3])
            .with_gravity(-1.0);
        assert!((pulse2.gravity - 0.0).abs() < 1e-9);
    }

    #[test]
    fn energy_is_nonneg() {
        let pulse = ReasoningPulse::new(IntentType::Rest, [0.0; 3])
            .with_energy(-5.0);
        assert!((pulse.energy_quota - 0.0).abs() < 1e-9);
    }

    #[test]
    fn constraints_are_accumulated() {
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
            .with_constraint(Constraint::TimeBudgetMs(100))
            .with_constraint(Constraint::Precision(0.8));
        assert_eq!(pulse.constraints.len(), 2);
    }

    #[test]
    fn command_new_with_slot() {
        let cmd = Command::new(ClawAction::Equip("sensor".into()), Some("Head"));
        assert_eq!(cmd.action, ClawAction::Equip("sensor".into()));
        assert_eq!(cmd.slot.as_deref(), Some("Head"));
    }

    #[test]
    fn command_new_without_slot() {
        let cmd = Command::new(ClawAction::Step, None);
        assert_eq!(cmd.slot, None);
    }

    #[test]
    fn command_chain_starts_empty() {
        let chain = CommandChain::new(uuid::Uuid::new_v4());
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert!((chain.estimated_cost - 0.0).abs() < 1e-9);
    }

    #[test]
    fn command_chain_push_and_len() {
        let mut chain = CommandChain::new(uuid::Uuid::new_v4());
        chain.push(Command::new(ClawAction::Step, None));
        chain.push(Command::new(ClawAction::Step, None));
        chain.push(Command::new(ClawAction::SetState("Idle".into()), None));
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn intent_type_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(IntentType::Navigate);
        set.insert(IntentType::Navigate);
        set.insert(IntentType::Reflex);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn claw_action_equality() {
        assert_eq!(ClawAction::Step, ClawAction::Step);
        assert_ne!(ClawAction::Step, ClawAction::Equip("x".into()));
        assert_eq!(ClawAction::Equip("grasp".into()), ClawAction::Equip("grasp".into()));
    }
}

#[cfg(test)]
mod tension_extra_tests {
    use hermes_nmi::tension::*;

    #[test]
    fn budget_new_has_full_energy() {
        let b = ConservationBudget::new(100.0);
        assert_eq!(b.total, 100.0);
        assert_eq!(b.spent, 0.0);
        assert_eq!(b.allocation, 100.0);
        assert_eq!(b.remaining(), 100.0);
        assert!((b.fraction_remaining() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn budget_zero_total_has_zero_remaining() {
        let mut b = ConservationBudget::new(0.0);
        assert_eq!(b.remaining(), 0.0);
        assert_eq!(b.fraction_remaining(), 0.0);
        assert!(!b.spend(1.0));
    }

    #[test]
    fn budget_overspend_returns_false() {
        let mut b = ConservationBudget::new(10.0);
        assert!(b.spend(5.0));
        assert!(!b.spend(10.0));
        assert_eq!(b.remaining(), 5.0);
    }

    #[test]
    fn budget_spend_exact_amount() {
        let mut b = ConservationBudget::new(10.0);
        assert!(b.spend(10.0));
        assert_eq!(b.remaining(), 0.0);
        assert!((b.fraction_remaining() - 0.0).abs() < 1e-9);
        assert!(!b.spend(0.01));
    }

    #[test]
    fn tension_set_clamps_high() {
        let mut t = Tension::new();
        t.set(5.0);
        assert!((t.level() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn tension_set_clamps_low() {
        let mut t = Tension::new();
        t.set(-3.0);
        assert!((t.level() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn tension_cost_at_max_doubles() {
        let mut t = Tension::new();
        t.set(1.0);
        assert!((t.adjust_cost(1.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn tension_cost_at_zero_is_identity() {
        let t = Tension::new();
        assert!((t.adjust_cost(42.0) - 42.0).abs() < 1e-9);
    }

    #[test]
    fn tension_fuzziness_at_boundary_0_5() {
        let mut t = Tension::new();
        t.set(0.5);
        // Below 0.5: fuzziness = level * 0.1
        // At exactly 0.5: formula uses first branch (0.5 * 0.1 = 0.05)
        assert!((t.fuzziness() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn tension_fuzziness_just_above_0_5() {
        let mut t = Tension::new();
        t.set(0.5 + f64::EPSILON);
        // Above 0.5: fuzziness = 0.05 + (level - 0.5) * 0.9
        // ≈ 0.05 at the boundary
        let expected = 0.05 + (t.level() - 0.5) * 0.9;
        assert!((t.fuzziness() - expected).abs() < 1e-9);
    }

    #[test]
    fn tension_critical_threshold() {
        let mut t = Tension::new();
        t.set(0.79);
        assert!(!t.is_critical());
        t.set(0.81);
        assert!(t.is_critical());
        t.set(0.8);
        assert!(!t.is_critical()); // Exactly 0.8 is NOT critical (> 0.8 is)
    }

    #[test]
    fn tension_adjust_from_budget_full_energy_is_zero() {
        let mut t = Tension::new();
        let budget = ConservationBudget::new(100.0); // nothing spent
        t.adjust_from_budget(0.9, budget);
        assert!((t.level() - 0.0).abs() < 1e-9); // fraction_remaining=1.0, energy_factor=0
    }

    #[test]
    fn tension_adjust_from_budget_depleted_is_gravity() {
        let mut t = Tension::new();
        let mut budget = ConservationBudget::new(100.0);
        budget.spend(100.0); // 0% remaining
        t.adjust_from_budget(0.7, budget);
        // energy_factor = 1.0, tension = gravity * 1.0 = 0.7
        assert!((t.level() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn tension_adjust_from_budget_clamps_to_one() {
        let mut t = Tension::new();
        let mut budget = ConservationBudget::new(10.0);
        budget.spend(10.0); // 0% remaining
        t.adjust_from_budget(1.5, budget); // gravity > 1 (shouldn't be, but test)
        // energy_factor = 1.0, tension = 1.5 * 1.0 = 1.5 → clamped to 1.0
        assert!((t.level() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn conservation_budget_default() {
        let b = ConservationBudget::default();
        assert!((b.total - 1.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use hermes_nmi::dispatcher::*;
    use hermes_nmi::pulse::*;
    use hermes_nmi::tension::*;

    #[test]
    fn dispatcher_starts_with_zero_tension() {
        let d = NmiDispatcher::new();
        assert!((d.tension_level() - 0.0).abs() < 1e-9);
        assert!((d.energy_consumed() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn navigate_produces_think_step_step() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Navigate, [5.0, 0.0, 0.0]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.len(), 3);
        // First command should set Thinking state
        assert_eq!(chain.commands[0].action, ClawAction::SetState("Thinking".into()));
        // Then two steps
        assert_eq!(chain.commands[1].action, ClawAction::Step);
        assert_eq!(chain.commands[2].action, ClawAction::Step);
    }

    #[test]
    fn interact_produces_equip_think_step() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Interact, [1.0, 1.0, 0.0]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.commands[0].action, ClawAction::Equip("grasp".into()));
        assert_eq!(chain.commands[0].slot.as_deref(), Some("Arms"));
    }

    #[test]
    fn observe_produces_sensor_step() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 0.0]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.commands[0].action, ClawAction::Equip("sensor".into()));
        assert_eq!(chain.commands[0].slot.as_deref(), Some("Head"));
    }

    #[test]
    fn equip_produces_torso_legs() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Equip, [0.0, 0.0, 0.0]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.commands[0].slot.as_deref(), Some("Torso"));
        assert_eq!(chain.commands[1].slot.as_deref(), Some("Legs"));
    }

    #[test]
    fn reflex_bypasses_thinking() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.len(), 2);
        // Reflex sets Acting directly, no Thinking state
        assert_eq!(chain.commands[0].action, ClawAction::SetState("Acting".into()));
        assert_eq!(chain.commands[1].action, ClawAction::Step);
    }

    #[test]
    fn rest_unequips_all_slots() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
        let chain = d.translate(&pulse);
        // 5 unequips + 1 SetState(Idle) = 6
        assert_eq!(chain.len(), 6);
        for i in 0..5 {
            assert!(matches!(chain.commands[i].action, ClawAction::Unequip(_)));
        }
        assert_eq!(chain.commands[5].action, ClawAction::SetState("Idle".into()));
    }

    #[test]
    fn chain_source_pulse_id_matches() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
        let chain = d.translate(&pulse);
        assert_eq!(chain.source_pulse_id, pulse.pulse_id);
    }

    #[test]
    fn chain_cost_is_nonzero_for_nonempty() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
        let chain = d.translate(&pulse);
        assert!(chain.estimated_cost > 0.0);
    }

    #[test]
    fn chain_cost_increases_with_tension() {
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);

        let d_relaxed = NmiDispatcher::new();
        let chain_relaxed = d_relaxed.translate(&pulse);

        let mut d_tense = NmiDispatcher::new();
        d_tense.adjust_tension(1.0, {
            let mut b = ConservationBudget::new(100.0);
            b.spend(100.0);
            b
        });

        // Under high tension (> 0.7), the chain is truncated to 2 commands
        // But cost per command is higher due to tension multiplier
        let chain_tense = d_tense.translate(&pulse);

        // Tense chain should be shorter (trimmed)
        assert!(chain_tense.len() <= chain_relaxed.len());
        // But cost per command is higher
        if chain_tense.len() > 0 {
            let cost_per_tense = chain_tense.estimated_cost / chain_tense.len() as f64;
            let cost_per_relaxed = chain_relaxed.estimated_cost / chain_relaxed.len() as f64;
            assert!(cost_per_tense > cost_per_relaxed);
        }
    }

    #[test]
    fn high_tension_trims_long_chains() {
        let mut d = NmiDispatcher::new();
        // Set tension above 0.7
        d.adjust_tension(1.0, {
            let mut b = ConservationBudget::new(100.0);
            b.spend(100.0);
            b
        });

        let pulse = ReasoningPulse::new(IntentType::Rest, [0.0; 3]);
        let chain = d.translate(&pulse);
        // Rest normally produces 6 commands; with high tension, trimmed to 2
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn consume_energy_accumulates() {
        let mut d = NmiDispatcher::new();
        d.consume_energy(5.0);
        assert!((d.energy_consumed() - 5.0).abs() < 1e-9);
        d.consume_energy(3.0);
        assert!((d.energy_consumed() - 8.0).abs() < 1e-9);
    }

    #[test]
    fn validate_passes_for_simple_pulse() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
        let chain = d.translate(&pulse);
        assert!(d.validate(&pulse, &chain).is_ok());
    }

    #[test]
    fn validate_fails_on_energy_exceed() {
        let d = NmiDispatcher::new();
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
            .with_constraint(Constraint::EnergyCeiling(0.001));
        let chain = d.translate(&pulse);
        let result = d.validate(&pulse, &chain);
        assert!(matches!(result, Err(NmiError::EnergyExceeded { .. })));
    }

    #[test]
    fn validate_fails_on_precision_at_high_tension() {
        let mut d = NmiDispatcher::new();
        d.adjust_tension(1.0, {
            let mut b = ConservationBudget::new(100.0);
            b.spend(100.0);
            b
        });
        let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
            .with_constraint(Constraint::Precision(0.9));
        let chain = d.translate(&pulse);
        let result = d.validate(&pulse, &chain);
        assert!(matches!(result, Err(NmiError::ConstraintViolated(_))));
    }

    #[test]
    fn nmi_error_display_works() {
        let e = NmiError::AgentError("crashed".into());
        assert!(format!("{e}").contains("crashed"));

        let e2 = NmiError::EmptyChain(uuid::Uuid::new_v4());
        assert!(format!("{e2}").contains("empty command chain"));
    }
}

#[cfg(test)]
mod telemetry_tests {
    use hermes_nmi::telemetry::*;
    use uuid::Uuid;

    #[test]
    fn status_equality() {
        assert_eq!(Status::Success, Status::Success);
        assert_ne!(Status::Success, Status::Failure);
    }

    #[test]
    fn sensor_payload_defaults_are_empty() {
        let s = SensorPayload::default();
        assert_eq!(s.velocity, None);
        assert_eq!(s.proximity, None);
        assert_eq!(s.contact_state, ContactState::None);
        assert!((s.resistance - 0.0).abs() < 1e-9);
        assert_eq!(s.positional_delta, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn telemetry_is_success_checker() {
        let frame = TelemetryFrame {
            pulse_id: Uuid::new_v4(),
            timestamp: 1000,
            tension_at_execution: 0.0,
            state_hash: [0u8; 32],
            sensor_data: SensorPayload::default(),
            fulfillment_status: Status::Success,
        };
        assert!(frame.is_success());
        assert!(!frame.is_failure());
        assert!(!frame.needs_reroute());
    }

    #[test]
    fn telemetry_is_failure_checker() {
        let frame = TelemetryFrame {
            pulse_id: Uuid::new_v4(),
            timestamp: 1000,
            tension_at_execution: 0.5,
            state_hash: [0u8; 32],
            sensor_data: SensorPayload::default(),
            fulfillment_status: Status::Failure,
        };
        assert!(!frame.is_success());
        assert!(frame.is_failure());
        assert!(!frame.needs_reroute());
    }

    #[test]
    fn telemetry_needs_reroute_checker() {
        let frame = TelemetryFrame {
            pulse_id: Uuid::new_v4(),
            timestamp: 1000,
            tension_at_execution: 0.3,
            state_hash: [0u8; 32],
            sensor_data: SensorPayload::default(),
            fulfillment_status: Status::ReRoute,
        };
        assert!(frame.needs_reroute());
        assert!(!frame.is_success());
    }

    #[test]
    fn contact_state_variants() {
        assert_ne!(ContactState::None, ContactState::Soft);
        assert_ne!(ContactState::Soft, ContactState::Hard);
        assert_ne!(ContactState::Hard, ContactState::Pushing);
    }

    #[test]
    fn all_status_variants_exist() {
        let statuses = vec![Status::Success, Status::PartialSuccess, Status::Failure, Status::ReRoute, Status::ReThink];
        // Ensure all 5 are distinct
        let mut sorted = statuses;
        sorted.sort_by_key(|s| format!("{s:?}"));
        sorted.dedup();
        assert_eq!(sorted.len(), 5);
    }
}

#[cfg(test)]
mod property_tests {
    use hermes_nmi::pulse::*;
    use hermes_nmi::tension::*;
    use hermes_nmi::dispatcher::*;

    #[test]
    fn tension_cost_is_monotonically_increasing_with_level() {
        let base = 10.0_f64;
        let mut prev = 0.0_f64;
        for i in 0..=100 {
            let mut t = Tension::new();
            t.set(i as f64 / 100.0);
            let cost = t.adjust_cost(base);
            assert!(cost >= prev, "cost decreased at level {}: {} < {}", i, cost, prev);
            prev = cost;
        }
    }

    #[test]
    fn budget_remaining_never_negative() {
        let mut b = ConservationBudget::new(5.0);
        b.spend(3.0);
        assert!(b.remaining() >= 0.0);
        b.spend(2.0);
        assert!(b.remaining() >= 0.0);
        // Can't spend more
        assert!(!b.spend(0.01));
        assert!(b.remaining() >= 0.0);
    }

    #[test]
    fn fraction_remaining_in_unit_interval() {
        for initial in [0.01, 0.1, 1.0, 10.0, 100.0, 1000.0] {
            let mut b = ConservationBudget::new(initial);
            for _ in 0..10 {
                b.spend(initial * 0.1);
                let f = b.fraction_remaining();
                assert!((0.0..=1.0).contains(&f), "fraction {} out of range", f);
            }
        }
    }

    #[test]
    fn translate_preserves_pulse_id() {
        let d = NmiDispatcher::new();
        for intent in [
            IntentType::Navigate, IntentType::Interact, IntentType::Observe,
            IntentType::Equip, IntentType::Reflex, IntentType::Rest,
        ] {
            let pulse = ReasoningPulse::new(intent.clone(), [0.0; 3]);
            let chain = d.translate(&pulse);
            assert_eq!(chain.source_pulse_id, pulse.pulse_id,
                "pulse_id mismatch for {:?}", intent);
        }
    }

    #[test]
    fn translate_never_produces_empty_chain() {
        let d = NmiDispatcher::new();
        for intent in [
            IntentType::Navigate, IntentType::Interact, IntentType::Observe,
            IntentType::Equip, IntentType::Reflex, IntentType::Rest,
        ] {
            let pulse = ReasoningPulse::new(intent.clone(), [0.0; 3]);
            let chain = d.translate(&pulse);
            assert!(!chain.is_empty(),
                "empty chain for {:?}", intent);
        }
    }

    #[test]
    fn tension_fuzziness_in_unit_interval() {
        for i in 0..=1000 {
            let mut t = Tension::new();
            t.set(i as f64 / 1000.0);
            let f = t.fuzziness();
            assert!((0.0..=1.0).contains(&f), "fuzziness {} out of range at level {}", f, t.level());
        }
    }
}
