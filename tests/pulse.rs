//! Integration tests for hermes-nmi pulse types.
//!
//! Tests ReasoningPulse construction, Constraint variants, CommandChain,
//! and ClawAction types — the vocabulary of the neuro-muscular interface.

use hermes_nmi::{
    ClawAction, Command, CommandChain, Constraint, IntentType, ReasoningPulse,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ReasoningPulse construction
// ---------------------------------------------------------------------------

#[test]
fn pulse_new_has_defaults() {
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 2.0, 3.0]);
    assert_eq!(pulse.intent_type, IntentType::Navigate);
    assert_eq!(pulse.target_coordinates, [1.0, 2.0, 3.0]);
    assert!((pulse.gravity - 0.5).abs() < f64::EPSILON);
    assert!((pulse.energy_quota - 1.0).abs() < f64::EPSILON);
    assert!(pulse.constraints.is_empty());
}

#[test]
fn pulse_with_gravity_clamps_high() {
    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 0.0])
        .with_gravity(5.0);
    assert!((pulse.gravity - 1.0).abs() < f64::EPSILON);
}

#[test]
fn pulse_with_gravity_clamps_low() {
    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 0.0])
        .with_gravity(-1.0);
    assert!(pulse.gravity.abs() < f64::EPSILON);
}

#[test]
fn pulse_with_energy_clamps_negative() {
    let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0])
        .with_energy(-10.0);
    assert!(pulse.energy_quota.abs() < f64::EPSILON);
}

#[test]
fn pulse_with_constraint_adds() {
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0, 0.0, 0.0])
        .with_constraint(Constraint::TimeBudgetMs(100))
        .with_constraint(Constraint::Precision(0.9));
    assert_eq!(pulse.constraints.len(), 2);
}

#[test]
fn pulse_ids_are_unique() {
    let p1 = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let p2 = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    assert_ne!(p1.pulse_id, p2.pulse_id);
}

// ---------------------------------------------------------------------------
// IntentType variants
// ---------------------------------------------------------------------------

#[test]
fn intent_type_equality() {
    assert_eq!(IntentType::Navigate, IntentType::Navigate);
    assert_ne!(IntentType::Navigate, IntentType::Interact);
    assert_ne!(IntentType::Reflex, IntentType::Rest);
}

#[test]
fn intent_type_all_variants() {
    let variants = [
        IntentType::Navigate,
        IntentType::Interact,
        IntentType::Observe,
        IntentType::Equip,
        IntentType::Reflex,
        IntentType::Rest,
    ];
    // All variants should be distinct
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j], "variants {} and {} are equal", i, j);
        }
    }
}

// ---------------------------------------------------------------------------
// Constraint variants
// ---------------------------------------------------------------------------

#[test]
fn constraint_time_budget() {
    let c = Constraint::TimeBudgetMs(500);
    assert_eq!(format!("{:?}", c), "TimeBudgetMs(500)");
}

#[test]
fn constraint_energy_ceiling() {
    let c = Constraint::EnergyCeiling(0.8);
    if let Constraint::EnergyCeiling(e) = c {
        assert!((e - 0.8).abs() < f64::EPSILON);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn constraint_precision_range() {
    for p in [0.0, 0.5, 1.0] {
        let c = Constraint::Precision(p);
        if let Constraint::Precision(val) = c {
            assert!((val - p).abs() < f64::EPSILON);
        }
    }
}

#[test]
fn constraint_require_slots() {
    let c = Constraint::RequireSlots(vec!["Head".into(), "Arms".into()]);
    if let Constraint::RequireSlots(slots) = c {
        assert_eq!(slots.len(), 2);
    }
}

#[test]
fn constraint_avoid_states() {
    let c = Constraint::AvoidStates(vec!["Error".into()]);
    if let Constraint::AvoidStates(states) = c {
        assert_eq!(states, vec!["Error"]);
    }
}

// ---------------------------------------------------------------------------
// Command and ClawAction
// ---------------------------------------------------------------------------

#[test]
fn command_new_with_slot() {
    let cmd = Command::new(ClawAction::Equip("sensor".into()), Some("Head"));
    assert_eq!(cmd.action, ClawAction::Equip("sensor".into()));
    assert_eq!(cmd.slot.as_deref(), Some("Head"));
}

#[test]
fn command_new_without_slot() {
    let cmd = Command::new(ClawAction::Step, None);
    assert!(cmd.slot.is_none());
}

#[test]
fn claw_action_equality() {
    assert_eq!(
        ClawAction::Equip("x".into()),
        ClawAction::Equip("x".into())
    );
    assert_ne!(
        ClawAction::Equip("x".into()),
        ClawAction::Equip("y".into())
    );
    assert_eq!(ClawAction::Step, ClawAction::Step);
    assert_ne!(ClawAction::Step, ClawAction::SetState("Idle".into()));
}

#[test]
fn claw_action_all_variants() {
    let _ = ClawAction::Equip("test".into());
    let _ = ClawAction::Unequip("test".into());
    let _ = ClawAction::Step;
    let _ = ClawAction::SetState("Thinking".into());
}

// ---------------------------------------------------------------------------
// CommandChain
// ---------------------------------------------------------------------------

#[test]
fn chain_new_is_empty() {
    let id = Uuid::new_v4();
    let chain = CommandChain::new(id);
    assert!(chain.is_empty());
    assert_eq!(chain.len(), 0);
    assert_eq!(chain.source_pulse_id, id);
    assert!((chain.estimated_cost - 0.0).abs() < f64::EPSILON);
}

#[test]
fn chain_push_grows() {
    let id = Uuid::new_v4();
    let mut chain = CommandChain::new(id);
    chain.push(Command::new(ClawAction::Step, None));
    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());

    chain.push(Command::new(ClawAction::Step, None));
    assert_eq!(chain.len(), 2);
}

#[test]
fn chain_preserves_order() {
    let id = Uuid::new_v4();
    let mut chain = CommandChain::new(id);
    chain.push(Command::new(ClawAction::SetState("Thinking".into()), None));
    chain.push(Command::new(ClawAction::Step, None));
    chain.push(Command::new(ClawAction::SetState("Acting".into()), None));

    assert_eq!(chain.commands[0].action, ClawAction::SetState("Thinking".into()));
    assert_eq!(chain.commands[1].action, ClawAction::Step);
    assert_eq!(chain.commands[2].action, ClawAction::SetState("Acting".into()));
}

#[test]
fn chain_estimated_cost_independent() {
    let id = Uuid::new_v4();
    let mut chain = CommandChain::new(id);
    chain.estimated_cost = 0.42;
    chain.push(Command::new(ClawAction::Step, None));
    // Cost is not auto-updated by push
    assert!((chain.estimated_cost - 0.42).abs() < f64::EPSILON);
}
