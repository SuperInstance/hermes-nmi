//! PincherHook — reflex pathway from Pincher to Claw.
//!
//! Pincher is a reflex engine: sub-50ms responses without an LLM.
//! It uses a vector DB as runtime — Teach → Match → Execute.
//!
//! The PincherHook allows Pincher reflexes to trigger Claw actions
//! directly, bypassing the reasoning pipeline entirely. This is the
//! "spinal cord" of the system: the hand pulls away from the hot stove
//! before the brain knows the stove was hot.
//!
//! ```text
//! Stimulus ──► Pincher matches reflex ──► PincherHook
//!                                                │
//!                                    ┌───────────┴───────────┐
//!                                    │                       │
//!                              confidence ≥ 0.80        confidence < 0.55
//!                                    │                       │
//!                              execute directly         escalate to CNS
//!                              (reflex action)         (ReasoningPulse)
//! ```
//!
//! When confidence is between 0.55 and 0.80, the reflex is "similar"
//! — it fires but flags the telemetry for CNS review.

use serde::{Deserialize, Serialize};

use crate::pulse::{ClawAction, Command, CommandChain, IntentType, ReasoningPulse};
use uuid::Uuid;

/// Pincher's match confidence thresholds.
pub const EXACT_THRESHOLD: f64 = 0.80;
pub const SIMILAR_THRESHOLD: f64 = 0.55;

/// The result of Pincher's reflex matching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflexMatch {
    /// The stimulus that triggered the match.
    pub stimulus: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// The matched reflex's intent (if any).
    pub matched_intent: Option<String>,
}

impl ReflexMatch {
    /// Classify the match quality.
    pub fn match_type(&self) -> MatchType {
        if self.confidence >= EXACT_THRESHOLD {
            MatchType::Exact
        } else if self.confidence >= SIMILAR_THRESHOLD {
            MatchType::Similar
        } else {
            MatchType::Novel
        }
    }

    /// Should this reflex fire automatically?
    pub fn should_auto_fire(&self) -> bool {
        self.confidence >= SIMILAR_THRESHOLD
    }

    /// Should this be escalated to the CNS?
    pub fn should_escalate(&self) -> bool {
        self.confidence < SIMILAR_THRESHOLD
    }
}

/// Classification of match quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatchType {
    /// ≥ 0.80 — execute directly, no confirmation needed.
    Exact,
    /// 0.55–0.80 — execute but flag for review.
    Similar,
    /// < 0.55 — novel situation, escalate to reasoning.
    Novel,
}

/// A trigger that fires when a reflex matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflexTrigger {
    /// The reflex match that triggered this.
    pub match_result: ReflexMatch,
    /// The action to perform.
    pub action: ReflexAction,
    /// Whether this trigger requires CNS confirmation.
    pub requires_confirmation: bool,
}

/// What the reflex does when triggered.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReflexAction {
    /// Execute a specific Claw action immediately.
    Execute(ClawAction),
    /// Equip a slot for defensive posture.
    Defend(String),
    /// Move away from stimulus.
    Withdraw,
    /// Alert the CNS — this needs reasoning.
    Escalate,
}

/// The Pincher hook — bridges reflex matches to Claw actions.
///
/// In the full system, Pincher runs as a separate process with its own
/// SQLite vector store. This hook provides the integration point:
/// it receives reflex matches from Pincher and translates them into
/// either direct Claw commands (high confidence) or ReasoningPulses
/// for the CNS (low confidence).
pub struct PincherHook {
    /// How many reflexes have fired through this hook.
    reflexes_fired: u64,
    /// How many were escalated to the CNS.
    escalations: u64,
}

impl PincherHook {
    pub fn new() -> Self {
        Self {
            reflexes_fired: 0,
            escalations: 0,
        }
    }

    /// Total reflexes that have fired.
    pub fn reflexes_fired(&self) -> u64 {
        self.reflexes_fired
    }

    /// Total escalations to CNS.
    pub fn escalations(&self) -> u64 {
        self.escalations
    }

    /// Process a reflex match and produce either a direct command chain
    /// or an escalation pulse.
    ///
    /// Returns `Ok(chain)` for direct execution, or `Err(pulse)` for
    /// CNS escalation.
    pub fn process(&mut self, match_result: ReflexMatch) -> Result<CommandChain, ReasoningPulse> {
        if match_result.should_escalate() {
            // Novel — needs reasoning
            self.escalations += 1;
            return Err(self.escalate(match_result));
        }

        // Direct reflex — build command chain
        self.reflexes_fired += 1;
        let chain = self.build_reflex_chain(&match_result);
        Ok(chain)
    }

    /// Build a command chain from a reflex match.
    ///
    /// Reflexes are intentionally simple — usually 1–2 commands.
    /// They bypass the thinking state entirely.
    fn build_reflex_chain(&self, _match_result: &ReflexMatch) -> CommandChain {
        let pulse_id = Uuid::new_v4();
        let mut chain = CommandChain::new(pulse_id);

        // Reflexes skip straight to acting
        chain.push(Command::new(
            ClawAction::SetState("Acting".into()),
            None,
        ));

        // The actual reflexive action
        // In a real system, this would be derived from the matched reflex's
        // stored action. Here we use a reasonable default.
        chain.push(Command::new(
            ClawAction::Equip("reflex_module".into()),
            Some("Arms"),
        ));

        // Step to complete the reflex
        chain.push(Command::new(ClawAction::Step, None));

        chain.estimated_cost = 0.15; // Reflexes are cheap

        chain
    }

    /// Escalate a low-confidence match to the CNS as a reasoning pulse.
    fn escalate(&self, match_result: ReflexMatch) -> ReasoningPulse {
        ReasoningPulse::new(IntentType::Reflex, [0.0, 0.0, 0.0])
            .with_gravity(match_result.confidence)
            .with_energy(0.5)
            .with_constraint(crate::pulse::Constraint::Precision(match_result.confidence))
    }

    /// Create a trigger for a matched reflex.
    pub fn create_trigger(&self, match_result: ReflexMatch) -> ReflexTrigger {
        let action = if match_result.should_escalate() {
            ReflexAction::Escalate
        } else if match_result.confidence >= EXACT_THRESHOLD {
            ReflexAction::Execute(ClawAction::Step)
        } else {
            // Similar match — defensive posture
            ReflexAction::Defend("cautious".into())
        };

        ReflexTrigger {
            requires_confirmation: matches!(match_result.match_type(), MatchType::Similar),
            match_result,
            action,
        }
    }
}

impl Default for PincherHook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match_auto_fires() {
        let m = ReflexMatch {
            stimulus: "obstacle ahead".into(),
            confidence: 0.92,
            matched_intent: Some("stop".into()),
        };
        assert!(m.should_auto_fire());
        assert!(!m.should_escalate());
        assert_eq!(m.match_type(), MatchType::Exact);
    }

    #[test]
    fn test_novel_match_escalates() {
        let m = ReflexMatch {
            stimulus: "unknown anomaly".into(),
            confidence: 0.30,
            matched_intent: None,
        };
        assert!(!m.should_auto_fire());
        assert!(m.should_escalate());
        assert_eq!(m.match_type(), MatchType::Novel);
    }

    #[test]
    fn test_similar_match_fires_with_flag() {
        let m = ReflexMatch {
            stimulus: "partial obstacle".into(),
            confidence: 0.65,
            matched_intent: Some("slow_down".into()),
        };
        assert!(m.should_auto_fire());
        assert!(!m.should_escalate());
        assert_eq!(m.match_type(), MatchType::Similar);
    }

    #[test]
    fn test_hook_processes_exact_match() {
        let mut hook = PincherHook::new();
        let m = ReflexMatch {
            stimulus: "wall".into(),
            confidence: 0.90,
            matched_intent: Some("stop".into()),
        };
        let result = hook.process(m);
        assert!(result.is_ok());
        assert_eq!(hook.reflexes_fired(), 1);
        assert_eq!(hook.escalations(), 0);
    }

    #[test]
    fn test_hook_escalates_novel() {
        let mut hook = PincherHook::new();
        let m = ReflexMatch {
            stimulus: "???".into(),
            confidence: 0.20,
            matched_intent: None,
        };
        let result = hook.process(m);
        assert!(result.is_err()); // Escalation = Err(pulse)
        assert_eq!(hook.reflexes_fired(), 0);
        assert_eq!(hook.escalations(), 1);
    }
}
