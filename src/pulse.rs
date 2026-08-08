//! ReasoningPulse and CommandChain types.
//!
//! A ReasoningPulse is what the CNS emits when it wants something to happen.
//! It's not a command — it's an intent with context. The NMI dispatcher
//! translates it into a CommandChain of discrete ClawActions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The kind of thing the CNS wants done.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntentType {
    /// Move to a position or state.
    Navigate,
    /// Interact with something in the environment.
    Interact,
    /// Observe and report back.
    Observe,
    /// Equip or configure a capability.
    Equip,
    /// Reflexive response — bypass reasoning, act now.
    Reflex,
    /// Enter a low-power state.
    Rest,
}

/// A constraint on how the intent should be executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    /// Must complete within N milliseconds.
    TimeBudgetMs(u64),
    /// Must not exceed this energy cost.
    EnergyCeiling(f64),
    /// Required precision level (0.0 = sloppy/fast, 1.0 = exact/slow).
    Precision(f64),
    /// Must use these equipment slots.
    RequireSlots(Vec<String>),
    /// Must not enter these states.
    AvoidStates(Vec<String>),
}

/// A pulse from the CNS — high-level intent with energetic and spatial context.
///
/// This is the "thought" that the NMI translates into "muscle."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningPulse {
    /// Unique identifier for this pulse.
    pub pulse_id: Uuid,
    /// What kind of action is intended.
    pub intent_type: IntentType,
    /// Target position in 3D space (x, y, z).
    /// For non-spatial intents, z is typically 0.0.
    pub target_coordinates: [f64; 3],
    /// JEPA-based context shaping — how much the environment matters.
    /// Range: 0.0 (no context) to 1.0 (full environmental awareness).
    pub gravity: f64,
    /// Local energy budget for this pulse.
    /// The NMI uses this to compute tension.
    pub energy_quota: f64,
    /// Constraints on execution.
    pub constraints: Vec<Constraint>,
}

impl ReasoningPulse {
    /// Create a new pulse with the given intent and target.
    pub fn new(intent_type: IntentType, target: [f64; 3]) -> Self {
        Self {
            pulse_id: Uuid::new_v4(),
            intent_type,
            target_coordinates: target,
            gravity: 0.5,
            energy_quota: 1.0,
            constraints: Vec::new(),
        }
    }

    /// Set the gravity (contextual weight) of this pulse.
    pub fn with_gravity(mut self, g: f64) -> Self {
        self.gravity = g.clamp(0.0, 1.0);
        self
    }

    /// Set the energy quota for this pulse.
    pub fn with_energy(mut self, e: f64) -> Self {
        self.energy_quota = e.max(0.0);
        self
    }

    /// Add a constraint.
    pub fn with_constraint(mut self, c: Constraint) -> Self {
        self.constraints.push(c);
        self
    }
}

/// A discrete action that Claw can execute on an equipment slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClawAction {
    /// Equip a capability into a slot.
    Equip(String),
    /// Remove a capability from a slot.
    Unequip(String),
    /// Advance the agent lifecycle by one step.
    Step,
    /// Set the agent to a specific state.
    SetState(String),
}

/// A single command in a chain — an action plus which slot it targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Command {
    /// The action to perform.
    pub action: ClawAction,
    /// Which equipment slot this targets (Head, Torso, Arms, Legs, Special).
    /// None means "whole agent" (e.g., Step).
    pub slot: Option<String>,
}

impl Command {
    pub fn new(action: ClawAction, slot: Option<&str>) -> Self {
        Self {
            action,
            slot: slot.map(|s| s.to_string()),
        }
    }
}

/// A chain of commands derived from a single reasoning pulse.
///
/// The dispatcher decomposes intent into a sequence of deterministic actions.
/// Each command in the chain is executed in order by Claw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandChain {
    /// The pulse that originated this chain.
    pub source_pulse_id: Uuid,
    /// Ordered commands to execute.
    pub commands: Vec<Command>,
    /// Estimated total energy cost (sum of command costs adjusted by tension).
    pub estimated_cost: f64,
}

impl CommandChain {
    pub fn new(pulse_id: Uuid) -> Self {
        Self {
            source_pulse_id: pulse_id,
            commands: Vec::new(),
            estimated_cost: 0.0,
        }
    }

    pub fn push(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }
}
