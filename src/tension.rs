//! Tension — the fuzziness parameter that bridges energy and action.
//!
//! In biology, muscles don't behave the same way when they're fresh
//! versus when they're fatigued. Tremor increases. Precision drops.
//! Recovery takes longer.
//!
//! The Tension parameter models this. When energy is abundant,
//! tension is low and execution is crisp. As energy depletes,
//! tension rises — commands become lossy, some steps get skipped,
//! and the agent may need to reconsider its approach.
//!
//! This isn't a bug. It's a feature. Fatigue is information.
//! The CNS reads tension from telemetry and adjusts its strategy.

use serde::{Deserialize, Serialize};

/// Conservation budget — how much energy the CNS is willing to spend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConservationBudget {
    /// Total energy available in the system.
    pub total: f64,
    /// How much has been spent so far.
    pub spent: f64,
    /// How much the CNS is willing to spend on the current operation.
    pub allocation: f64,
}

impl ConservationBudget {
    /// Create a new budget with the given total energy.
    pub fn new(total: f64) -> Self {
        Self {
            total,
            spent: 0.0,
            allocation: total,
        }
    }

    /// How much energy remains.
    pub fn remaining(&self) -> f64 {
        (self.total - self.spent).max(0.0)
    }

    /// What fraction of total energy remains.
    pub fn fraction_remaining(&self) -> f64 {
        if self.total > 0.0 {
            (self.remaining() / self.total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Spend some energy.
    pub fn spend(&mut self, amount: f64) -> bool {
        if amount <= self.remaining() {
            self.spent += amount;
            true
        } else {
            false
        }
    }
}

impl Default for ConservationBudget {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// Muscle tension — how strained the agent's execution is.
///
/// Range: 0.0 (fully relaxed, crisp execution) to 1.0 (maximally
/// tense, fuzzy/degraded execution).
///
/// Tension is derived from the conservation budget and gravity:
/// - Low budget remaining → high tension
/// - High gravity (environmental complexity) → higher tension
/// - The two multiply: a complex environment on low battery is worst
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tension {
    /// Current tension level (0.0–1.0).
    level: f64,
}

impl Tension {
    /// Create a new tension at zero (fully relaxed).
    pub fn new() -> Self {
        Self { level: 0.0 }
    }

    /// Current tension level.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Set tension directly (clamped to 0.0–1.0).
    pub fn set(&mut self, level: f64) {
        self.level = level.clamp(0.0, 1.0);
    }

    /// Adjust tension based on CNS guidance.
    ///
    /// Tension = gravity × (1 - fraction_remaining)
    ///
    /// When energy is full and gravity is low: tension ≈ 0.
    /// When energy is depleted and gravity is high: tension ≈ 1.
    pub fn adjust_from_budget(&mut self, gravity: f64, budget: ConservationBudget) {
        let energy_factor = 1.0 - budget.fraction_remaining();
        self.level = (gravity * energy_factor).clamp(0.0, 1.0);
    }

    /// Apply tension to a cost estimate.
    /// Higher tension = higher effective cost (things are harder when strained).
    pub fn adjust_cost(&self, base_cost: f64) -> f64 {
        // Tension multiplier: 1.0 at zero tension, up to 2.0 at max tension
        let multiplier = 1.0 + self.level;
        base_cost * multiplier
    }

    /// How much execution fuzziness tension introduces (0.0–1.0).
    /// This is the probability that any given command might be skipped
    /// or degraded.
    pub fn fuzziness(&self) -> f64 {
        // Fuzziness ramps up sharply above 0.5 tension
        if self.level < 0.5 {
            self.level * 0.1
        } else {
            0.05 + (self.level - 0.5) * 0.9
        }
    }

    /// Is the agent too tense to execute reliably?
    pub fn is_critical(&self) -> bool {
        self.level > 0.8
    }
}

impl Default for Tension {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tension_starts_at_zero() {
        let t = Tension::new();
        assert_eq!(t.level(), 0.0);
        assert!(!t.is_critical());
        assert_eq!(t.fuzziness(), 0.0);
    }

    #[test]
    fn test_tension_adjusts_with_budget() {
        let mut t = Tension::new();
        let mut budget = ConservationBudget::new(100.0);
        budget.spend(80.0); // 20% remaining

        t.adjust_from_budget(0.8, budget); // gravity=0.8, energy_factor=0.8
        assert!((t.level() - 0.64).abs() < 0.001);
    }

    #[test]
    fn test_tension_cost_adjustment() {
        let mut t = Tension::new();
        t.set(0.5);
        assert!((t.adjust_cost(1.0) - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_fuzziness_ramps_up() {
        let mut t = Tension::new();
        t.set(0.3);
        assert!(t.fuzziness() < 0.05); // Low tension = minimal fuzz

        t.set(0.8);
        assert!(t.fuzziness() > 0.3); // High tension = significant fuzz
    }

    #[test]
    fn test_budget_spending() {
        let mut b = ConservationBudget::new(10.0);
        assert!(b.spend(3.0));
        assert_eq!(b.remaining(), 7.0);
        assert!((b.fraction_remaining() - 0.7).abs() < 0.001);
        assert!(!b.spend(8.0)); // Not enough
    }
}
