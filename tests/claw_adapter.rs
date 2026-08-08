//! Integration tests for ClawNmiAdapter — the full dispatch → execute → telemetry cycle.
//!
//! These tests exercise the complete pipeline:
//!   ReasoningPulse → NmiDispatcher → CommandChain → ClawInstance → TelemetryFrame

use hermes_nmi::{
    ClawAction, ClawNmiAdapter, ConservationBudget, EquipmentSlot, IntentType,
    NmiError, ReasoningPulse,
};
use hermes_nmi::NeuroMuscularInterface;

// ---------------------------------------------------------------------------
// ClawInstance behavior
// ---------------------------------------------------------------------------

#[test]
fn claw_instance_starts_idle() {
    let adapter = ClawNmiAdapter::new();
    let agent = adapter.agent();
    assert_eq!(agent.state, hermes_nmi::AgentState::Idle);
    assert_eq!(agent.step_count, 0);
    assert!(agent.equipment.is_empty());
}

#[test]
fn claw_step_advances_lifecycle() {
    let mut adapter = ClawNmiAdapter::new();
    // Idle → Thinking
    adapter.agent_mut().step().unwrap();
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Thinking);
    // Thinking → Acting
    adapter.agent_mut().step().unwrap();
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Acting);
    // Acting → Idle
    adapter.agent_mut().step().unwrap();
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Idle);
}

#[test]
fn claw_step_increments_count() {
    let mut adapter = ClawNmiAdapter::new();
    for _ in 0..5 {
        adapter.agent_mut().step().unwrap();
    }
    assert_eq!(adapter.agent().step_count, 5);
}

#[test]
fn claw_step_in_error_state_fails() {
    let mut adapter = ClawNmiAdapter::new();
    adapter.agent_mut().set_state("Broken");
    let result = adapter.agent_mut().step();
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Equipment slot management
// ---------------------------------------------------------------------------

#[test]
fn equip_adds_to_equipment() {
    let mut adapter = ClawNmiAdapter::new();
    adapter.agent_mut().equip(EquipmentSlot::Head);
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Head));
}

#[test]
fn unequip_removes_from_equipment() {
    let mut adapter = ClawNmiAdapter::new();
    adapter.agent_mut().equip(EquipmentSlot::Arms);
    adapter.agent_mut().unequip(EquipmentSlot::Arms);
    assert!(!adapter.agent().equipment.contains(&EquipmentSlot::Arms));
}

#[test]
fn equipment_slot_from_name() {
    assert_eq!(EquipmentSlot::from_name("Head"), Some(EquipmentSlot::Head));
    assert_eq!(EquipmentSlot::from_name("Torso"), Some(EquipmentSlot::Torso));
    assert_eq!(EquipmentSlot::from_name("Arms"), Some(EquipmentSlot::Arms));
    assert_eq!(EquipmentSlot::from_name("Legs"), Some(EquipmentSlot::Legs));
    assert_eq!(EquipmentSlot::from_name("Special"), Some(EquipmentSlot::Special));
    assert_eq!(EquipmentSlot::from_name("Unknown"), None);
    assert_eq!(EquipmentSlot::from_name(""), None);
}

#[test]
fn set_state_valid_names() {
    let mut adapter = ClawNmiAdapter::new();
    adapter.agent_mut().set_state("Thinking");
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Thinking);
    adapter.agent_mut().set_state("Acting");
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Acting);
    adapter.agent_mut().set_state("Idle");
    assert_eq!(adapter.agent().state, hermes_nmi::AgentState::Idle);
}

#[test]
fn set_state_invalid_becomes_error() {
    let mut adapter = ClawNmiAdapter::new();
    adapter.agent_mut().set_state("Panicked");
    assert!(matches!(adapter.agent().state, hermes_nmi::AgentState::Error(_)));
}

// ---------------------------------------------------------------------------
// Execute chain
// ---------------------------------------------------------------------------

#[test]
fn execute_empty_chain_errors() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let chain = hermes_nmi::CommandChain::new(pulse.pulse_id); // empty
    let result = adapter.execute_chain(&chain);
    assert!(matches!(result, Err(NmiError::EmptyChain(_))));
}

#[test]
fn execute_single_step() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let mut chain = hermes_nmi::CommandChain::new(pulse.pulse_id);
    chain.push(hermes_nmi::Command::new(ClawAction::Step, None));
    assert!(adapter.execute_chain(&chain).is_ok());
    assert_eq!(adapter.agent().step_count, 1);
}

#[test]
fn execute_equip_command_equips_slot() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0; 3]);
    let mut chain = hermes_nmi::CommandChain::new(pulse.pulse_id);
    chain.push(hermes_nmi::Command::new(
        ClawAction::Equip("sensor".into()),
        Some("Head"),
    ));
    assert!(adapter.execute_chain(&chain).is_ok());
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Head));
}

#[test]
fn execute_unequip_all_clears_equipment() {
    let mut adapter = ClawNmiAdapter::new();
    // First equip multiple slots
    let pulse = ReasoningPulse::new(IntentType::Equip, [0.0; 3]);
    let mut chain = hermes_nmi::CommandChain::new(pulse.pulse_id);
    chain.push(hermes_nmi::Command::new(
        ClawAction::Equip("x".into()),
        Some("Head"),
    ));
    chain.push(hermes_nmi::Command::new(
        ClawAction::Equip("x".into()),
        Some("Torso"),
    ));
    adapter.execute_chain(&chain).unwrap();
    assert_eq!(adapter.agent().equipment.len(), 2);

    // Now unequip each slot individually (the dispatch pattern for Rest)
    let mut clear_chain = hermes_nmi::CommandChain::new(pulse.pulse_id);
    for slot in ["Head", "Torso", "Arms", "Legs", "Special"] {
        clear_chain.push(hermes_nmi::Command::new(
            ClawAction::Unequip("all".into()),
            Some(slot),
        ));
    }
    adapter.execute_chain(&clear_chain).unwrap();
    assert!(adapter.agent().equipment.is_empty());
}

#[test]
fn execute_set_state_error_stops_chain() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    let mut chain = hermes_nmi::CommandChain::new(pulse.pulse_id);
    chain.push(hermes_nmi::Command::new(
        ClawAction::SetState("Broken".into()),
        None,
    ));
    chain.push(hermes_nmi::Command::new(ClawAction::Step, None));
    let result = adapter.execute_chain(&chain);
    assert!(result.is_err());
    // Second command should NOT have executed
    assert_eq!(adapter.agent().step_count, 0);
}

// ---------------------------------------------------------------------------
// Full dispatch cycle (async)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_navigate_succeeds() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 0.0, 0.0]);
    let result = adapter.dispatch_pulse(pulse).await;
    assert!(result.is_ok());
    let telemetry = result.unwrap();
    assert!(telemetry.is_success());
    // Navigate: Think + Step + Step → ends at Idle after cycling
}

#[tokio::test]
async fn dispatch_interact_equips_arms() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Interact, [0.0, 1.0, 0.0]);
    let telemetry = adapter.dispatch_pulse(pulse).await.unwrap();
    assert!(telemetry.is_success());
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Arms));
}

#[tokio::test]
async fn dispatch_observe_equips_head() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 1.0]);
    adapter.dispatch_pulse(pulse).await.unwrap();
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Head));
}

#[tokio::test]
async fn dispatch_rest_clears_equipment() {
    let mut adapter = ClawNmiAdapter::new();

    // First equip some slots
    let equip_pulse = ReasoningPulse::new(IntentType::Equip, [0.0; 3]);
    adapter.dispatch_pulse(equip_pulse).await.unwrap();
    assert!(!adapter.agent().equipment.is_empty());

    // Now rest — should clear everything
    let rest_pulse = ReasoningPulse::new(IntentType::Rest, [0.0; 3]);
    adapter.dispatch_pulse(rest_pulse).await.unwrap();
    assert!(adapter.agent().equipment.is_empty());
}

#[tokio::test]
async fn dispatch_constraint_violation_returns_error() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3])
        .with_constraint(hermes_nmi::Constraint::EnergyCeiling(0.001));
    let result = adapter.dispatch_pulse(pulse).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn dispatch_telemetry_carries_sensor_data() {
    let mut adapter = ClawNmiAdapter::new();
    let pulse = ReasoningPulse::new(IntentType::Interact, [0.0; 3]);
    let telemetry = adapter.dispatch_pulse(pulse).await.unwrap();

    // Interact equips Arms → contact state should reflect that
    assert_eq!(telemetry.sensor_data.contact_state, hermes_nmi::ContactState::Soft);
}

#[tokio::test]
async fn dispatch_multiple_pulses_cumulative() {
    let mut adapter = ClawNmiAdapter::new();

    // Dispatch observe (equips Head)
    let p1 = ReasoningPulse::new(IntentType::Observe, [0.0; 3]);
    adapter.dispatch_pulse(p1).await.unwrap();
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Head));

    // Dispatch interact (equips Arms)
    let p2 = ReasoningPulse::new(IntentType::Interact, [0.0; 3]);
    adapter.dispatch_pulse(p2).await.unwrap();
    assert!(adapter.agent().equipment.contains(&EquipmentSlot::Arms));

    // Both should be equipped
    assert_eq!(adapter.agent().equipment.len(), 2);
}

#[tokio::test]
async fn dispatch_tension_adjustment() {
    let mut adapter = ClawNmiAdapter::new();

    // Adjust tension to high
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(100.0);
    adapter.adjust_tension(1.0, budget).await;

    assert!(adapter.dispatcher().tension_level() > 0.5);
}

// ---------------------------------------------------------------------------
// Agent state after dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_step_count_increases_with_dispatch() {
    let mut adapter = ClawNmiAdapter::new();
    let initial_steps = adapter.agent().step_count;

    let pulse = ReasoningPulse::new(IntentType::Navigate, [0.0; 3]);
    adapter.dispatch_pulse(pulse).await.unwrap();

    // Navigate produces 2 Step commands
    assert!(adapter.agent().step_count > initial_steps);
}
