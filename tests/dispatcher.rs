//! Integration tests for NmiDispatcher — the neural layer that translates
//! ReasoningPulses into CommandChains.
//!
//! These tests verify:
//! - Translation for each IntentType produces expected command patterns
//! - Tension affects chain length (trimming under high tension)
//! - Cost estimation scales with tension
//! - Constraint validation catches violations
//! - Telemetry generation works correctly

use hermes_nmi::{
    ClawAction, Constraint, ConservationBudget, IntentType, NmiDispatcher, NmiError,
    ReasoningPulse, Status, SensorPayload,
};

// ---------------------------------------------------------------------------
// Translation: each IntentType produces expected commands
// ---------------------------------------------------------------------------

#[test]
fn translate_navigate_produces_think_step_step() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);

    assert_eq!(chain.len(), 3);
    assert_eq!(chain.commands[0].action, ClawAction::SetState("Thinking".into()));
    assert_eq!(chain.commands[1].action, ClawAction::Step);
    assert_eq!(chain.commands[2].action, ClawAction::Step);
}

#[test]
fn translate_interact_equips_arms() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Interact, [0.0, 1.0, 0.0]);
    let chain = disp.translate(&pulse);

    assert_eq!(chain.len(), 3);
    assert_eq!(chain.commands[0].action, ClawAction::Equip("grasp".into()));
    assert_eq!(chain.commands[0].slot.as_deref(), Some("Arms"));
}

#[test]
fn translate_observe_equips_head_sensor() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 1.0]);
    let chain = disp.translate(&pulse);

    assert_eq!(chain.len(), 2);
    assert_eq!(chain.commands[0].action, ClawAction::Equip("sensor".into()));
    assert_eq!(chain.commands[0].slot.as_deref(), Some("Head"));
}

#[test]
fn translate_equip_equips_torso_and_legs() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Equip, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);

    assert_eq!(chain.len(), 2);
    assert_eq!(chain.commands[0].action, ClawAction::Equip("module".into()));
    assert_eq!(chain.commands[0].slot.as_deref(), Some("Torso"));
    assert_eq!(chain.commands[1].action, ClawAction::Equip("actuator".into()));
    assert_eq!(chain.commands[1].slot.as_deref(), Some("Legs"));
}

#[test]
fn translate_reflex_bypasses_thinking() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);

    assert_eq!(chain.len(), 2);
    // Reflex goes straight to Acting, not Thinking
    assert_eq!(chain.commands[0].action, ClawAction::SetState("Acting".into()));
    assert_eq!(chain.commands[1].action, ClawAction::Step);
}

#[test]
fn translate_rest_unequips_all_and_idles() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);

    // 5 unequips + 1 SetState("Idle") = 6 commands
    assert_eq!(chain.len(), 6);
    // All unequip commands target specific slots
    for cmd in &chain.commands[..5] {
        match &cmd.action {
            ClawAction::Unequip(_) => {
                assert!(cmd.slot.is_some(), "unequip must target a slot");
            }
            _ => panic!("expected Unequip in first 5 commands"),
        }
    }
    // Last command sets Idle
    assert_eq!(chain.commands[5].action, ClawAction::SetState("Idle".into()));
}

// ---------------------------------------------------------------------------
// Tension effects on translation
// ---------------------------------------------------------------------------

#[test]
fn translate_under_low_tension_keeps_all_commands() {
    let disp = NmiDispatcher::new();
    // Fresh dispatcher has zero tension
    assert_eq!(disp.tension_level(), 0.0);

    let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);
    // Rest produces 6 commands — no trimming at low tension
    assert_eq!(chain.len(), 6);
}

#[test]
fn translate_under_high_tension_trims_chain() {
    let mut disp = NmiDispatcher::new();
    // Push tension above 0.7 via budget manipulation
    let budget = ConservationBudget::new(1.0);
    disp.adjust_tension(1.0, budget);
    // After adjust: tension = gravity * (1 - fraction_remaining)
    // budget is fresh so fraction_remaining = 1.0, tension = 0... need to spend
    let mut budget2 = ConservationBudget::new(100.0);
    budget2.spend(100.0); // fully spent → fraction_remaining = 0
    disp.adjust_tension(1.0, budget2);

    assert!(disp.tension_level() > 0.7, "tension should be > 0.7, got {}", disp.tension_level());

    let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);
    // Under high tension, chains > 2 are trimmed to 2
    assert_eq!(chain.len(), 2);
}

#[test]
fn translate_under_high_tension_keeps_short_chains() {
    let mut disp = NmiDispatcher::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(100.0);
    disp.adjust_tension(1.0, budget);

    let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);
    // Reflex chain is 2 commands — at the threshold, not trimmed
    assert_eq!(chain.len(), 2);
}

// ---------------------------------------------------------------------------
// Cost estimation
// ---------------------------------------------------------------------------

#[test]
fn cost_scales_with_tension() {
    let disp_low = NmiDispatcher::new();

    let mut budget = ConservationBudget::new(100.0);
    budget.spend(100.0);
    let mut disp_high = NmiDispatcher::new();
    disp_high.adjust_tension(1.0, budget);

    let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
    let chain_low = disp_low.translate(&pulse);
    let chain_high = disp_high.translate(&pulse);

    assert!(
        chain_high.estimated_cost > chain_low.estimated_cost,
        "high tension cost ({}) should exceed low tension cost ({})",
        chain_high.estimated_cost,
        chain_low.estimated_cost
    );
}

#[test]
fn cost_at_zero_tension_is_base() {
    let disp = NmiDispatcher::new();
    assert_eq!(disp.tension_level(), 0.0);

    let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
    let chain = disp.translate(&pulse);
    // 2 commands × 0.1 base = 0.2, tension multiplier = 1.0
    assert!((chain.estimated_cost - 0.2).abs() < 0.001);
}

// ---------------------------------------------------------------------------
// Constraint validation
// ---------------------------------------------------------------------------

#[test]
fn validate_passes_with_no_constraints() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let chain = disp.translate(&pulse);
    assert!(disp.validate(&pulse, &chain).is_ok());
}

#[test]
fn validate_time_budget_violation() {
    let disp = NmiDispatcher::new();
    // Navigate produces 3 commands, estimated time ~3ms at zero tension
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(Constraint::TimeBudgetMs(1)); // 1ms budget
    let chain = disp.translate(&pulse);
    let result = disp.validate(&pulse, &chain);
    assert!(matches!(result, Err(NmiError::ConstraintViolated(_))));
}

#[test]
fn validate_time_budget_satisfied() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(Constraint::TimeBudgetMs(100)); // generous
    let chain = disp.translate(&pulse);
    assert!(disp.validate(&pulse, &chain).is_ok());
}

#[test]
fn validate_energy_ceiling_violation() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(Constraint::EnergyCeiling(0.01)); // too low
    let chain = disp.translate(&pulse);
    // Cost is ~0.3 (3 commands × 0.1), ceiling is 0.01
    let result = disp.validate(&pulse, &chain);
    assert!(matches!(result, Err(NmiError::EnergyExceeded { .. })));
}

#[test]
fn validate_precision_conflict_with_tension() {
    let mut disp = NmiDispatcher::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(100.0);
    disp.adjust_tension(1.0, budget);
    // tension = 1.0, precision required > 0.8 → conflict
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(Constraint::Precision(0.9));
    let chain = disp.translate(&pulse);
    let result = disp.validate(&pulse, &chain);
    assert!(matches!(result, Err(NmiError::ConstraintViolated(_))));
}

#[test]
fn validate_precision_ok_at_low_tension() {
    let disp = NmiDispatcher::new();
    assert_eq!(disp.tension_level(), 0.0);
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(Constraint::Precision(0.95));
    let chain = disp.translate(&pulse);
    assert!(disp.validate(&pulse, &chain).is_ok());
}

// ---------------------------------------------------------------------------
// Energy tracking
// ---------------------------------------------------------------------------

#[test]
fn energy_consumed_starts_zero() {
    let disp = NmiDispatcher::new();
    assert!((disp.energy_consumed() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn energy_consumed_accumulates() {
    let mut disp = NmiDispatcher::new();
    disp.consume_energy(0.5);
    assert!((disp.energy_consumed() - 0.5).abs() < f64::EPSILON);
    disp.consume_energy(0.3);
    assert!((disp.energy_consumed() - 0.8).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Telemetry building
// ---------------------------------------------------------------------------

#[test]
fn build_telemetry_carries_tension() {
    let mut disp = NmiDispatcher::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(50.0);
    disp.adjust_tension(0.8, budget);

    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let telemetry = disp.build_telemetry(
        pulse.pulse_id,
        Status::Success,
        SensorPayload::default(),
    );

    assert_eq!(telemetry.pulse_id, pulse.pulse_id);
    assert!((telemetry.tension_at_execution - disp.tension_level()).abs() < 0.001);
    assert_eq!(telemetry.fulfillment_status, Status::Success);
    assert!(telemetry.is_success());
}

#[test]
fn build_telemetry_timestamp_nonzero() {
    let disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let telemetry = disp.build_telemetry(
        pulse.pulse_id,
        Status::Success,
        SensorPayload::default(),
    );
    assert!(telemetry.timestamp > 0);
}

#[test]
fn state_hash_changes_with_energy() {
    let mut disp = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);

    let t1 = disp.build_telemetry(pulse.pulse_id, Status::Success, SensorPayload::default());
    disp.consume_energy(5.0);
    let t2 = disp.build_telemetry(pulse.pulse_id, Status::Success, SensorPayload::default());

    assert_ne!(t1.state_hash, t2.state_hash);
}
