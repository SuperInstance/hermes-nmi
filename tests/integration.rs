//! Integration tests for the hermes-nmi crate.
//!
//! These tests exercise the full pulse → dispatch → execute → telemetry
//! pipeline, as well as the Pincher reflex hook integration.

use hermes_nmi::*;

#[tokio::test]
async fn test_full_pulse_pipeline() {
    let mut adapter = ClawNmiAdapter::new();

    // Dispatch a Navigate pulse
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 2.0, 0.0])
        .with_gravity(0.5)
        .with_energy(1.0);

    let telemetry = adapter.dispatch_pulse(pulse).await;

    assert!(telemetry.is_ok());
    let frame = telemetry.unwrap();
    assert_eq!(frame.fulfillment_status, Status::Success);
    assert!(frame.tension_at_execution < 0.1); // Should be relaxed at start
}

#[tokio::test]
async fn test_observe_pulse_equips_head() {
    let mut adapter = ClawNmiAdapter::new();

    let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 0.0]);
    let result = adapter.dispatch_pulse(pulse).await;

    assert!(result.is_ok());
    assert!(adapter.agent().equipment.contains(&claw_adapter::EquipmentSlot::Head));
}

#[tokio::test]
async fn test_interact_pulse_equips_arms() {
    let mut adapter = ClawNmiAdapter::new();

    let pulse = ReasoningPulse::new(IntentType::Interact, [5.0, 3.0, 0.0]);
    let result = adapter.dispatch_pulse(pulse).await;

    assert!(result.is_ok());
    assert!(adapter.agent().equipment.contains(&claw_adapter::EquipmentSlot::Arms));
}

#[tokio::test]
async fn test_rest_pulse_unequips_all() {
    let mut adapter = ClawNmiAdapter::new();

    // First equip some things
    let equip_pulse = ReasoningPulse::new(IntentType::Equip, [0.0, 0.0, 0.0]);
    adapter.dispatch_pulse(equip_pulse).await.unwrap();
    assert!(!adapter.agent().equipment.is_empty());

    // Now rest
    let rest_pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    adapter.dispatch_pulse(rest_pulse).await.unwrap();
    assert!(adapter.agent().equipment.is_empty());
    assert_eq!(adapter.agent().state, claw_adapter::AgentState::Idle);
}

#[tokio::test]
async fn test_tension_rises_with_low_budget() {
    let mut adapter = ClawNmiAdapter::new();

    // Drain the budget significantly
    let low_budget = tension::ConservationBudget {
        total: 100.0,
        spent: 90.0,
        allocation: 10.0,
    };
    adapter.adjust_tension(0.9, low_budget).await;

    // Tension should be high
    assert!(adapter.dispatcher().tension_level() > 0.5);
}

#[tokio::test]
async fn test_energy_ceiling_constraint_rejects() {
    let mut adapter = ClawNmiAdapter::new();

    // Drain budget to create tension
    let low_budget = tension::ConservationBudget {
        total: 100.0,
        spent: 95.0,
        allocation: 5.0,
    };
    adapter.adjust_tension(0.9, low_budget).await;

    // This pulse has a very tight energy ceiling that tension should violate
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 0.0, 0.0])
        .with_constraint(Constraint::EnergyCeiling(0.001));

    let result = adapter.dispatch_pulse(pulse).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_dispatches_consume_energy() {
    let mut adapter = ClawNmiAdapter::new();

    let initial_consumed = adapter.dispatcher().energy_consumed();

    for _ in 0..5 {
        let pulse = ReasoningPulse::new(IntentType::Observe, [0.0, 0.0, 0.0]);
        let _ = adapter.dispatch_pulse(pulse).await;
    }

    assert!(adapter.dispatcher().energy_consumed() > initial_consumed);
}

#[test]
fn test_dispatcher_translates_all_intents() {
    let dispatcher = NmiDispatcher::new();

    for intent in [
        IntentType::Navigate,
        IntentType::Interact,
        IntentType::Observe,
        IntentType::Equip,
        IntentType::Reflex,
        IntentType::Rest,
    ] {
        let pulse = ReasoningPulse::new(intent, [0.0, 0.0, 0.0]);
        let chain = dispatcher.translate(&pulse);
        assert!(!chain.is_empty(), "Chain for {:?} was empty", pulse.intent_type);
    }
}

#[test]
fn test_command_chain_preserves_pulse_id() {
    let dispatcher = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 0.0, 0.0]);
    let chain = dispatcher.translate(&pulse);
    assert_eq!(chain.source_pulse_id, pulse.pulse_id);
}

#[test]
fn test_tension_trims_under_strain() {
    let mut dispatcher = NmiDispatcher::new();
    let low_budget = tension::ConservationBudget {
        total: 100.0,
        spent: 95.0,
        allocation: 5.0,
    };
    dispatcher.adjust_tension(0.95, low_budget);

    // Under high tension, chains should be trimmed
    let pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    let chain = dispatcher.translate(&pulse);
    assert!(chain.len() <= 2, "Chain wasn't trimmed under high tension");
}

#[test]
fn test_reflex_bypasses_thinking() {
    let dispatcher = NmiDispatcher::new();
    let pulse = ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0]);
    let chain = dispatcher.translate(&pulse);

    // Reflex should go straight to Acting, not Thinking
    assert!(chain.commands.iter().any(|c| {
        matches!(
            &c.action,
            ClawAction::SetState(ref s) if s == "Acting"
        )
    }));
}

#[test]
fn test_pincher_hook_exact_match() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "obstacle".into(),
        confidence: 0.90,
        matched_intent: Some("stop".into()),
    };

    let result = hook.process(m);
    assert!(result.is_ok());
    let chain = result.unwrap();
    assert!(!chain.is_empty());
    assert_eq!(hook.reflexes_fired(), 1);
}

#[test]
fn test_pincher_hook_escalates_novel() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "quantum anomaly".into(),
        confidence: 0.20,
        matched_intent: None,
    };

    let result = hook.process(m);
    assert!(result.is_err()); // Escalation
    let pulse = result.unwrap_err();
    assert_eq!(pulse.intent_type, IntentType::Reflex);
    assert_eq!(hook.escalations(), 1);
}

#[test]
fn test_telemetry_frame_checks() {
    let frame_success = TelemetryFrame {
        pulse_id: uuid::Uuid::new_v4(),
        timestamp: 0,
        tension_at_execution: 0.0,
        state_hash: [0u8; 32],
        sensor_data: SensorPayload::default(),
        fulfillment_status: Status::Success,
    };
    assert!(frame_success.is_success());
    assert!(!frame_success.is_failure());

    let frame_fail = TelemetryFrame {
        fulfillment_status: Status::Failure,
        ..frame_success
    };
    assert!(frame_fail.is_failure());
}
