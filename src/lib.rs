//! # Hermes NMI — Neuro-Muscular Interface
//!
//! The synapse between thinking and doing.
//!
//! Bridges high-level reasoning pulses from a CNS (Central Nervous System)
//! to discrete equipment-slot operations on a cellular agent (Claw),
//! with reflex hooks for sub-50ms response via Pincher.
//!
//! ## Architecture
//!
//! ```text
//! CNS (Reasoning)                  Claw (Action)
//!     │                                 │
//!     │  ReasoningPulse                  │
//!     ├─────────────► NmiDispatcher ─────►│
//!     │                 │                 │
//!     │                 ▼                 │
//!     │           CommandChain             │
//!     │                 │                 │
//!     │                 ▼                 │
//!     │           Claw executes            │
//!     │                 │                 │
//!     │                 ▼                 │
//!     │  ◄────────── TelemetryFrame ◄─────┤
//!     │                                   │
//! ```
//!
//! ## The Tension Parameter
//!
//! When energy is abundant, execution is crisp and deterministic.
//! As energy depletes, tension rises — introducing controlled fuzziness
//! into command execution, mirroring how muscles tremble under fatigue.

pub mod pulse;
pub mod dispatcher;
pub mod telemetry;
pub mod tension;
pub mod claw_adapter;
pub mod pincher_hook;

pub use pulse::{ReasoningPulse, IntentType, Constraint, CommandChain, Command, ClawAction};
pub use dispatcher::{NmiDispatcher, NmiError};
pub use telemetry::{TelemetryFrame, SensorPayload, Status};
pub use tension::{Tension, ConservationBudget};
pub use claw_adapter::ClawNmiAdapter;
pub use pincher_hook::{PincherHook, ReflexTrigger, ReflexMatch, EXACT_THRESHOLD, SIMILAR_THRESHOLD, MatchType, ReflexAction};
pub use claw_adapter::{ClawInstance, AgentState, EquipmentSlot};
pub use telemetry::ContactState;

/// The core trait defining the neuro-muscular boundary.
///
/// Implementors sit between a reasoning system (CNS) and a cellular
/// agent (Claw), translating intent into action and sensation into feedback.
#[async_trait::async_trait]
pub trait NeuroMuscularInterface: Send {
    /// Receive a high-level intent from the CNS.
    /// Returns a telemetry frame describing what happened.
    async fn dispatch_pulse(&mut self, pulse: ReasoningPulse) -> Result<TelemetryFrame, NmiError>;

    /// Adjust muscle tension based on CNS energy guidance.
    /// High gravity + low budget = more tension = fuzzier execution.
    async fn adjust_tension(&mut self, gravity: f64, budget: ConservationBudget);
}
