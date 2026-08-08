//! ClawNmiAdapter — bridges the NMI to Claw's cellular agent interface.
//!
//! Claw speaks in terms of EquipmentSlots and lifecycle States.
//! The NMI speaks in terms of Pulses and CommandChains.
//! This adapter is the translator.
//!
//! ```text
//! ReasoningPulse                     EquipmentSlot
//!     │                                   ▲
//!     ▼                                   │
//! NmiDispatcher ──► CommandChain ──► ClawNmiAdapter
//!                                       │
//!                                  step / equip / unequip
//!                                       │
//!                                       ▼
//!                                  TelemetryFrame
//! ```
//!
//! The adapter owns a simulated ClawInstance (in production, this would
//! be a reference to the actual Claw runtime). Each command in a chain
//! is executed against the instance, and the results are compiled into
//! a TelemetryFrame for the CNS.

use std::collections::HashSet;

use crate::dispatcher::{NmiDispatcher, NmiError};
use crate::pulse::{ClawAction, Command, CommandChain, ReasoningPulse};
use crate::telemetry::{ContactState, SensorPayload, Status, TelemetryFrame};
use crate::tension::ConservationBudget;
use crate::NeuroMuscularInterface;

/// The lifecycle states of a cellular agent.
/// Mirrors Claw's `AgentState` enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentState {
    Idle,
    Thinking,
    Acting,
    Error(String),
}

/// Equipment slots on a cellular agent.
/// Mirrors Claw's `EquipmentSlot` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    Head,
    Torso,
    Arms,
    Legs,
    Special,
}

impl EquipmentSlot {
    /// Parse a slot name string into an EquipmentSlot.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Head" => Some(Self::Head),
            "Torso" => Some(Self::Torso),
            "Arms" => Some(Self::Arms),
            "Legs" => Some(Self::Legs),
            "Special" => Some(Self::Special),
            _ => None,
        }
    }
}

/// A simulated cellular agent instance.
/// In production, this would be a handle to the real Claw runtime.
#[derive(Debug)]
pub struct ClawInstance {
    pub state: AgentState,
    pub equipment: HashSet<EquipmentSlot>,
    pub step_count: u64,
}

impl Default for ClawInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl ClawInstance {
    pub fn new() -> Self {
        Self {
            state: AgentState::Idle,
            equipment: HashSet::new(),
            step_count: 0,
        }
    }

    /// Equip a slot.
    pub fn equip(&mut self, slot: EquipmentSlot) {
        self.equipment.insert(slot);
        self.state = AgentState::Idle;
    }

    /// Unequip a slot.
    pub fn unequip(&mut self, slot: EquipmentSlot) {
        self.equipment.remove(&slot);
        self.state = AgentState::Idle;
    }

    /// Advance the lifecycle by one step.
    /// Returns Err if in an error state.
    pub fn step(&mut self) -> Result<(), String> {
        self.step_count += 1;
        match &self.state {
            AgentState::Idle => {
                self.state = AgentState::Thinking;
                Ok(())
            }
            AgentState::Thinking => {
                self.state = AgentState::Acting;
                Ok(())
            }
            AgentState::Acting => {
                self.state = AgentState::Idle;
                Ok(())
            }
            AgentState::Error(e) => Err(e.clone()),
        }
    }

    /// Set the state directly.
    pub fn set_state(&mut self, state_str: &str) {
        self.state = match state_str {
            "Idle" => AgentState::Idle,
            "Thinking" => AgentState::Thinking,
            "Acting" => AgentState::Acting,
            other => AgentState::Error(other.to_string()),
        };
    }
}

/// The NMI adapter that wraps a Claw instance.
///
/// This implements the NeuroMuscularInterface trait,
/// bridging reasoning pulses to agent actions.
pub struct ClawNmiAdapter {
    /// The dispatcher that translates pulses to chains.
    dispatcher: NmiDispatcher,
    /// The cellular agent being controlled.
    agent: ClawInstance,
}

impl ClawNmiAdapter {
    /// Create a new adapter with a fresh agent.
    pub fn new() -> Self {
        Self {
            dispatcher: NmiDispatcher::new(),
            agent: ClawInstance::new(),
        }
    }

    /// Create an adapter wrapping an existing agent instance.
    pub fn with_agent(agent: ClawInstance) -> Self {
        Self {
            dispatcher: NmiDispatcher::new(),
            agent,
        }
    }

    /// Access the underlying agent (read-only).
    pub fn agent(&self) -> &ClawInstance {
        &self.agent
    }

    /// Access the underlying agent (mutable).
    pub fn agent_mut(&mut self) -> &mut ClawInstance {
        &mut self.agent
    }

    /// Access the dispatcher.
    pub fn dispatcher(&self) -> &NmiDispatcher {
        &self.dispatcher
    }

    /// Execute a command chain against the agent.
    ///
    /// Each command is applied in sequence. If any command fails
    /// (agent in error state), execution stops and returns the error.
    pub fn execute_chain(&mut self, chain: &CommandChain) -> Result<(), NmiError> {
        if chain.is_empty() {
            return Err(NmiError::EmptyChain(chain.source_pulse_id));
        }

        for cmd in &chain.commands {
            self.execute_command(cmd)?;
        }

        Ok(())
    }

    /// Execute a single command against the agent.
    fn execute_command(&mut self, cmd: &Command) -> Result<(), NmiError> {
        match &cmd.action {
            ClawAction::Equip(_) => {
                if let Some(ref slot_name) = cmd.slot {
                    if let Some(slot) = EquipmentSlot::from_name(slot_name) {
                        self.agent.equip(slot);
                    }
                }
                Ok(())
            }
            ClawAction::Unequip(_) => {
                if let Some(ref slot_name) = cmd.slot {
                    if let Some(slot) = EquipmentSlot::from_name(slot_name) {
                        self.agent.unequip(slot);
                    } else if slot_name == "all" {
                        self.agent.equipment.clear();
                    }
                }
                Ok(())
            }
            ClawAction::Step => {
                self.agent.step().map_err(NmiError::AgentError)
            }
            ClawAction::SetState(state_str) => {
                self.agent.set_state(state_str);
                if let AgentState::Error(ref e) = self.agent.state {
                    return Err(NmiError::AgentError(e.clone()));
                }
                Ok(())
            }
        }
    }

    /// Build telemetry from current agent state.
    fn build_telemetry(&self, pulse: &ReasoningPulse, success: bool) -> TelemetryFrame {
        let status = if success {
            Status::Success
        } else {
            Status::Failure
        };

        let contact_state = if self.agent.equipment.contains(&EquipmentSlot::Arms) {
            ContactState::Soft
        } else {
            ContactState::None
        };

        let sensor_data = SensorPayload {
            velocity: None,
            proximity: None,
            contact_state,
            resistance: self.dispatcher.tension_level(),
            positional_delta: [0.0, 0.0, 0.0],
            extras: serde_json::json!({
                "agent_state": format!("{:?}", self.agent.state),
                "equipped_slots": self.agent.equipment.len(),
                "step_count": self.agent.step_count,
            }),
        };

        self.dispatcher.build_telemetry(pulse.pulse_id, status, sensor_data)
    }
}

impl Default for ClawNmiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl NeuroMuscularInterface for ClawNmiAdapter {
    async fn dispatch_pulse(&mut self, pulse: ReasoningPulse) -> Result<TelemetryFrame, NmiError> {
        // Translate the pulse into a command chain
        let chain = self.dispatcher.translate(&pulse);

        // Validate constraints
        self.dispatcher.validate(&pulse, &chain)?;

        // Execute
        let result = self.execute_chain(&chain);

        // Consume energy
        self.dispatcher.consume_energy(chain.estimated_cost);

        // Build telemetry
        let success = result.is_ok();
        let telemetry = self.build_telemetry(&pulse, success);

        // If execution failed, annotate the telemetry
        if let Err(e) = result {
            return Err(e);
        }

        Ok(telemetry)
    }

    async fn adjust_tension(&mut self, gravity: f64, budget: ConservationBudget) {
        self.dispatcher.adjust_tension(gravity, budget);
    }
}
