//! NmiDispatcher — translates ReasoningPulses into CommandChains.
//!
//! The dispatcher is the neural layer of the interface. It receives
//! high-level intent from the CNS and decomposes it into a sequence
//! of discrete equipment-slot operations that Claw can execute.
//!
//! The translation logic is deterministic — no LLM in the hot path.
//! Pattern matching on IntentType produces known command sequences.
//! When tension is high (low energy), the dispatcher may simplify
//! or skip optional commands.

use crate::pulse::{ClawAction, Command, CommandChain, Constraint, IntentType, ReasoningPulse};
use crate::tension::{ConservationBudget, Tension};
use crate::telemetry::{SensorPayload, Status, TelemetryFrame};
use uuid::Uuid;

/// Errors that can occur during dispatch or execution.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NmiError {
    /// The agent was in an error state and could not execute.
    #[error("agent in error state: {0}")]
    AgentError(String),
    /// The pulse exceeded its energy budget.
    #[error("energy budget exceeded: needed {needed}, had {available}")]
    EnergyExceeded { needed: f64, available: f64 },
    /// The command chain was empty (nothing to do).
    #[error("empty command chain for pulse {0}")]
    EmptyChain(Uuid),
    /// A constraint was violated.
    #[error("constraint violated: {0}")]
    ConstraintViolated(String),
}

/// The dispatcher that sits between CNS reasoning and Claw execution.
///
/// It holds the current tension state and uses it to modulate
/// how pulses are translated into commands.
pub struct NmiDispatcher {
    /// Current muscle tension — affects execution fuzziness.
    tension: Tension,
    /// Total energy consumed so far.
    energy_consumed: f64,
}

impl NmiDispatcher {
    /// Create a new dispatcher with zero tension (full energy).
    pub fn new() -> Self {
        Self {
            tension: Tension::new(),
            energy_consumed: 0.0,
        }
    }

    /// Get current tension level (0.0 = relaxed, 1.0 = maximally tense).
    pub fn tension_level(&self) -> f64 {
        self.tension.level()
    }

    /// Get total energy consumed across all dispatches.
    pub fn energy_consumed(&self) -> f64 {
        self.energy_consumed
    }

    /// Translate a reasoning pulse into a command chain.
    ///
    /// This is pure pattern matching — no LLM, no network calls.
    /// The intent type determines the command sequence; constraints
    /// and tension may trim or simplify the chain.
    pub fn translate(&self, pulse: &ReasoningPulse) -> CommandChain {
        let mut chain = CommandChain::new(pulse.pulse_id);

        match pulse.intent_type {
            IntentType::Navigate => {
                // Navigation: step through thinking → acting cycle
                chain.push(Command::new(ClawAction::SetState("Thinking".into()), None));
                chain.push(Command::new(ClawAction::Step, None));
                chain.push(Command::new(ClawAction::Step, None));
            }
            IntentType::Interact => {
                // Interaction: equip arms, think, act
                chain.push(Command::new(ClawAction::Equip("grasp".into()), Some("Arms")));
                chain.push(Command::new(ClawAction::SetState("Thinking".into()), None));
                chain.push(Command::new(ClawAction::Step, None));
            }
            IntentType::Observe => {
                // Observation: equip head sensor, step once
                chain.push(Command::new(ClawAction::Equip("sensor".into()), Some("Head")));
                chain.push(Command::new(ClawAction::Step, None));
            }
            IntentType::Equip => {
                // Configuration: equip specified slots
                chain.push(Command::new(ClawAction::Equip("module".into()), Some("Torso")));
                chain.push(Command::new(ClawAction::Equip("actuator".into()), Some("Legs")));
            }
            IntentType::Reflex => {
                // Reflex: bypass thinking, act immediately
                chain.push(Command::new(ClawAction::SetState("Acting".into()), None));
                chain.push(Command::new(ClawAction::Step, None));
            }
            IntentType::Rest => {
                // Rest: unequip everything, go idle
                chain.push(Command::new(ClawAction::Unequip("all".into()), Some("Head")));
                chain.push(Command::new(ClawAction::Unequip("all".into()), Some("Torso")));
                chain.push(Command::new(ClawAction::Unequip("all".into()), Some("Arms")));
                chain.push(Command::new(ClawAction::Unequip("all".into()), Some("Legs")));
                chain.push(Command::new(ClawAction::Unequip("all".into()), Some("Special")));
                chain.push(Command::new(ClawAction::SetState("Idle".into()), None));
            }
        }

        // Under high tension, trim non-essential commands (keep first + last)
        if self.tension.level() > 0.7 && chain.len() > 2 {
            let essential_count = 2.min(chain.len());
            chain.commands.truncate(essential_count);
        }

        // Estimate cost: base cost per command, inflated by tension
        let base_cost = chain.len() as f64 * 0.1;
        chain.estimated_cost = self.tension.adjust_cost(base_cost);

        chain
    }

    /// Consume energy from a dispatch and update tension accordingly.
    pub fn consume_energy(&mut self, cost: f64) {
        self.energy_consumed += cost;
        // As energy is consumed, tension naturally rises
        // (in a real system, this would be driven by the budget)
    }

    /// Adjust tension from CNS guidance.
    pub fn adjust_tension(&mut self, gravity: f64, budget: ConservationBudget) {
        self.tension.adjust_from_budget(gravity, budget);
    }

    /// Create a telemetry frame from execution results.
    pub fn build_telemetry(
        &self,
        pulse_id: Uuid,
        status: Status,
        sensor_data: SensorPayload,
    ) -> TelemetryFrame {
        TelemetryFrame {
            pulse_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            tension_at_execution: self.tension.level(),
            state_hash: self.hash_state(),
            sensor_data,
            fulfillment_status: status,
        }
    }

    /// Simple state hash for telemetry (deterministic but not cryptographic).
    fn hash_state(&self) -> [u8; 32] {
        let mut hash = [0u8; 32];
        let consumed_bytes = self.energy_consumed.to_le_bytes();
        let tension_bytes = self.tension.level().to_le_bytes();
        for i in 0..8 {
            hash[i] = consumed_bytes[i] ^ tension_bytes[i];
        }
        hash
    }

    /// Check whether a pulse violates its constraints given current state.
    pub fn validate(&self, pulse: &ReasoningPulse, chain: &CommandChain) -> Result<(), NmiError> {
        for constraint in &pulse.constraints {
            match constraint {
                Constraint::TimeBudgetMs(ms) => {
                    // Estimate execution time: ~1ms per command at zero tension
                    let estimated_ms = chain.len() as u64 * (1 + (self.tension.level() * 10.0) as u64);
                    if estimated_ms > *ms {
                        return Err(NmiError::ConstraintViolated(format!(
                            "estimated {}ms exceeds budget of {}ms",
                            estimated_ms, ms
                        )));
                    }
                }
                Constraint::EnergyCeiling(ceiling) => {
                    if chain.estimated_cost > *ceiling {
                        return Err(NmiError::EnergyExceeded {
                            needed: chain.estimated_cost,
                            available: *ceiling,
                        });
                    }
                }
                Constraint::Precision(p) => {
                    // High precision requirement conflicts with high tension
                    if *p > 0.8 && self.tension.level() > 0.6 {
                        return Err(NmiError::ConstraintViolated(format!(
                            "precision {:.2} unreachable at tension {:.2}",
                            p,
                            self.tension.level()
                        )));
                    }
                }
                _ => {} // Other constraints checked at runtime
            }
        }
        Ok(())
    }
}

impl Default for NmiDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
