//! Integration tests for telemetry types — the feedback channel from
//! Claw back to the CNS.
//!
//! Tests TelemetryFrame, SensorPayload, Status, and ContactState.

use hermes_nmi::{ContactState, SensorPayload, Status, TelemetryFrame};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Status variants
// ---------------------------------------------------------------------------

#[test]
fn status_all_variants_distinct() {
    let variants = [
        Status::Success,
        Status::PartialSuccess,
        Status::Failure,
        Status::ReRoute,
        Status::ReThink,
    ];
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j]);
        }
    }
}

#[test]
fn status_equality_and_matching() {
    assert_eq!(Status::Success, Status::Success);
    assert_ne!(Status::Success, Status::Failure);
    assert_ne!(Status::ReRoute, Status::ReThink);
    assert_ne!(Status::PartialSuccess, Status::Failure);
}

// ---------------------------------------------------------------------------
// ContactState
// ---------------------------------------------------------------------------

#[test]
fn contact_state_variants() {
    let states = [ContactState::None, ContactState::Soft, ContactState::Hard, ContactState::Pushing];
    for i in 0..states.len() {
        for j in (i + 1)..states.len() {
            assert_ne!(states[i], states[j]);
        }
    }
}

// ---------------------------------------------------------------------------
// SensorPayload defaults
// ---------------------------------------------------------------------------

#[test]
fn sensor_payload_default_is_empty() {
    let payload = SensorPayload::default();
    assert!(payload.velocity.is_none());
    assert!(payload.proximity.is_none());
    assert_eq!(payload.contact_state, ContactState::None);
    assert!((payload.resistance - 0.0).abs() < f64::EPSILON);
    assert_eq!(payload.positional_delta, [0.0, 0.0, 0.0]);
    assert!(payload.extras.is_null());
}

#[test]
fn sensor_payload_with_velocity() {
    let mut payload = SensorPayload::default();
    payload.velocity = Some([1.0, 0.0, -0.5]);
    assert_eq!(payload.velocity.unwrap(), [1.0, 0.0, -0.5]);
}

#[test]
fn sensor_payload_with_proximity() {
    let mut payload = SensorPayload::default();
    payload.proximity = Some(3.14);
    assert!((payload.proximity.unwrap() - 3.14).abs() < f64::EPSILON);
}

#[test]
fn sensor_payload_extras_json() {
    let mut payload = SensorPayload::default();
    payload.extras = serde_json::json!({"temperature": 42.0, "pressure": 1013});
    assert!(payload.extras.is_object());
    assert_eq!(payload.extras["temperature"], 42.0);
}

// ---------------------------------------------------------------------------
// TelemetryFrame construction
// ---------------------------------------------------------------------------

#[test]
fn telemetry_frame_is_success_checks_status() {
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
fn telemetry_frame_is_failure_checks_status() {
    let frame = TelemetryFrame {
        pulse_id: Uuid::new_v4(),
        timestamp: 1000,
        tension_at_execution: 0.5,
        state_hash: [0u8; 32],
        sensor_data: SensorPayload::default(),
        fulfillment_status: Status::Failure,
    };
    assert!(frame.is_failure());
    assert!(!frame.is_success());
}

#[test]
fn telemetry_frame_needs_reroute() {
    let frame = TelemetryFrame {
        pulse_id: Uuid::new_v4(),
        timestamp: 1000,
        tension_at_execution: 0.0,
        state_hash: [0u8; 32],
        sensor_data: SensorPayload::default(),
        fulfillment_status: Status::ReRoute,
    };
    assert!(frame.needs_reroute());
}

#[test]
fn telemetry_frame_rethink_is_neither_success_nor_failure() {
    let frame = TelemetryFrame {
        pulse_id: Uuid::new_v4(),
        timestamp: 1000,
        tension_at_execution: 0.0,
        state_hash: [0u8; 32],
        sensor_data: SensorPayload::default(),
        fulfillment_status: Status::ReThink,
    };
    assert!(!frame.is_success());
    assert!(!frame.is_failure());
    assert!(!frame.needs_reroute());
}

// ---------------------------------------------------------------------------
// TelemetryFrame serialization (serde)
// ---------------------------------------------------------------------------

#[test]
fn telemetry_frame_serializes_to_json() {
    let frame = TelemetryFrame {
        pulse_id: Uuid::nil(),
        timestamp: 42,
        tension_at_execution: 0.7,
        state_hash: [1u8; 32],
        sensor_data: SensorPayload::default(),
        fulfillment_status: Status::Success,
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("pulse_id"));
    assert!(json.contains("timestamp"));
    assert!(json.contains("Success"));
}

#[test]
fn sensor_payload_serializes_to_json() {
    let payload = SensorPayload::default();
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains("resistance"));
    assert!(json.contains("positional_delta"));
}

// ---------------------------------------------------------------------------
// ReflexMatch and PincherHook integration
// ---------------------------------------------------------------------------

#[test]
fn reflex_match_thresholds_are_consistent() {
    // Exact threshold should be above similar threshold
    assert!(
        hermes_nmi::EXACT_THRESHOLD > hermes_nmi::SIMILAR_THRESHOLD,
        "exact threshold must be above similar threshold"
    );
    assert!((hermes_nmi::EXACT_THRESHOLD - 0.80).abs() < f64::EPSILON);
    assert!((hermes_nmi::SIMILAR_THRESHOLD - 0.55).abs() < f64::EPSILON);
}
