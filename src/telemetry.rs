//! TelemetryFrame — the sensory feedback that flows from Claw back to the CNS.
//!
//! After every command chain executes, a TelemetryFrame is generated.
//! This is how the "muscles" report back to the "brain" — what happened,
//! what the environment felt like, whether the intent was fulfilled.
//!
//! In biological terms: proprioception and tactile sensation bundled
//! into a single report.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Overall fulfillment status of a dispatched pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    /// The intent was fully achieved.
    Success,
    /// The intent was partially achieved — some commands succeeded.
    PartialSuccess,
    /// The intent could not be achieved.
    Failure,
    /// The agent needs to re-route — the environment has changed.
    ReRoute,
    /// The agent is reflecting before retrying.
    ReThink,
}

/// Sensor data reported from the agent's execution environment.
///
/// This is intentionally flexible — different equipment configurations
/// produce different sensor profiles. The fields are optional because
/// not every action activates every sensor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorPayload {
    /// Current velocity vector (vx, vy, vz) if the agent is moving.
    pub velocity: Option<[f64; 3]>,
    /// Distance to nearest obstacle in meters.
    pub proximity: Option<f64>,
    /// Contact state: what the agent is touching.
    pub contact_state: ContactState,
    /// Environmental resistance or load (0.0 = none, 1.0 = maximum).
    pub resistance: f64,
    /// Positional delta between intended and achieved state.
    /// (dx, dy, dz) — how far off the result was from the target.
    pub positional_delta: [f64; 3],
    /// Additional freeform sensor readings.
    pub extras: serde_json::Value,
}

impl Default for SensorPayload {
    fn default() -> Self {
        Self {
            velocity: None,
            proximity: None,
            contact_state: ContactState::None,
            resistance: 0.0,
            positional_delta: [0.0, 0.0, 0.0],
            extras: serde_json::Value::Null,
        }
    }
}

/// What the agent is in contact with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContactState {
    /// No contact.
    None,
    /// Light, compliant contact.
    Soft,
    /// Firm, resistant contact.
    Hard,
    /// Contact that's actively pushing back.
    Pushing,
}

/// The feedback frame sent from Claw back to the CNS after execution.
///
/// ```text
/// ┌──────────────────────────────────────┐
/// │         TelemetryFrame               │
/// │                                      │
/// │  pulse_id: which pulse this answers  │
/// │  timestamp: when it happened         │
/// │  tension: how tense execution was    │
/// │  state_hash: fingerprint of state    │
/// │  sensor_data: what was felt          │
/// │  fulfillment: did it work?           │
/// └──────────────────────────────────────┘
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryFrame {
    /// Which pulse this telemetry answers.
    pub pulse_id: Uuid,
    /// When the telemetry was generated (Unix epoch ms).
    pub timestamp: u64,
    /// Tension level at time of execution (0.0–1.0).
    /// High tension means the agent was strained.
    pub tension_at_execution: f64,
    /// Hash of the agent's internal state after execution.
    /// Used for change detection without transferring full state.
    pub state_hash: [u8; 32],
    /// Raw sensor readings from execution.
    pub sensor_data: SensorPayload,
    /// Whether the intent was fulfilled.
    pub fulfillment_status: Status,
}

impl TelemetryFrame {
    /// Quick check: did everything work?
    pub fn is_success(&self) -> bool {
        matches!(self.fulfillment_status, Status::Success)
    }

    /// Quick check: did anything go wrong?
    pub fn is_failure(&self) -> bool {
        matches!(self.fulfillment_status, Status::Failure)
    }

    /// Quick check: does the agent need re-routing?
    pub fn needs_reroute(&self) -> bool {
        matches!(self.fulfillment_status, Status::ReRoute)
    }
}
