//! Integration tests for Tension and ConservationBudget.
//!
//! These tests focus on edge cases, property invariants, and the
//! interaction between budget depletion and tension — the "fatigue curve"
//! that makes execution fuzzy under energy stress.

use hermes_nmi::{ConservationBudget, Tension};

// ─── ConservationBudget edge cases ──────────────────────────────

#[test]
fn budget_zero_total_has_zero_remaining() {
    let b = ConservationBudget::new(0.0);
    assert_eq!(b.remaining(), 0.0);
    assert_eq!(b.fraction_remaining(), 0.0);
}

#[test]
fn budget_negative_total_clamped_to_zero() {
    // Negative total is nonsensical but shouldn't panic
    let b = ConservationBudget::new(-10.0);
    assert_eq!(b.remaining(), 0.0); // max(−10 − 0, 0) = 0
    assert_eq!(b.fraction_remaining(), 0.0); // total <= 0 → 0.0
}

#[test]
fn budget_exact_spend_consumes_all() {
    let mut b = ConservationBudget::new(5.0);
    assert!(b.spend(5.0));
    assert_eq!(b.remaining(), 0.0);
    assert!((b.fraction_remaining() - 0.0).abs() < 1e-9);
    // Can't spend more
    assert!(!b.spend(0.01));
}

#[test]
fn budget_overspend_returns_false_and_does_not_mutate() {
    let mut b = ConservationBudget::new(10.0);
    assert!(!b.spend(15.0));
    assert_eq!(b.spent, 0.0); // unchanged
    assert_eq!(b.remaining(), 10.0);
}

#[test]
fn budget_zero_spend_is_noop_success() {
    let mut b = ConservationBudget::new(10.0);
    assert!(b.spend(0.0));
    assert_eq!(b.spent, 0.0);
    assert_eq!(b.remaining(), 10.0);
}

#[test]
fn budget_multiple_small_sums_track_correctly() {
    let mut b = ConservationBudget::new(100.0);
    for _ in 0..10 {
        assert!(b.spend(7.0));
    }
    assert_eq!(b.spent, 70.0);
    assert_eq!(b.remaining(), 30.0);
    assert!(!b.spend(31.0));
    assert!(b.spend(30.0));
}

#[test]
fn budget_default_is_one_unit() {
    let b = ConservationBudget::default();
    assert!((b.total - 1.0).abs() < 1e-9);
    assert_eq!(b.remaining(), 1.0);
    assert!((b.fraction_remaining() - 1.0).abs() < 1e-9);
}

#[test]
fn budget_fraction_remaining_clamps_at_one() {
    // If somehow spent < 0 (shouldn't happen via API but defensively)
    let b = ConservationBudget { total: 10.0, spent: 0.0, allocation: 10.0 };
    assert!((b.fraction_remaining() - 1.0).abs() < 1e-9);
}

#[test]
fn budget_fraction_remaining_depleted() {
    let b = ConservationBudget { total: 10.0, spent: 10.0, allocation: 10.0 };
    assert!((b.fraction_remaining() - 0.0).abs() < 1e-9);
}

// ─── Tension level clamping ─────────────────────────────────────

#[test]
fn tension_clamps_above_one() {
    let mut t = Tension::new();
    t.set(5.0);
    assert!((t.level() - 1.0).abs() < 1e-9);
}

#[test]
fn tension_clamps_below_zero() {
    let mut t = Tension::new();
    t.set(-3.0);
    assert!((t.level() - 0.0).abs() < 1e-9);
}

#[test]
fn tension_set_to_exact_boundary_zero() {
    let mut t = Tension::new();
    t.set(0.0);
    assert_eq!(t.level(), 0.0);
}

#[test]
fn tension_set_to_exact_boundary_one() {
    let mut t = Tension::new();
    t.set(1.0);
    assert_eq!(t.level(), 1.0);
}

// ─── Tension × Budget interaction ───────────────────────────────

#[test]
fn tension_zero_gravity_always_zero() {
    let mut t = Tension::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(99.0); // almost depleted
    t.adjust_from_budget(0.0, budget); // gravity = 0
    assert_eq!(t.level(), 0.0);
}

#[test]
fn tension_full_budget_zero_tension() {
    let mut t = Tension::new();
    let budget = ConservationBudget::new(100.0); // nothing spent
    t.adjust_from_budget(0.9, budget); // high gravity but full energy
    assert_eq!(t.level(), 0.0); // energy_factor = 0 → tension = 0
}

#[test]
fn tension_half_budget_moderate_gravity() {
    let mut t = Tension::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(50.0); // 50% remaining
    t.adjust_from_budget(0.6, budget);
    // energy_factor = 0.5, tension = 0.6 * 0.5 = 0.3
    assert!((t.level() - 0.30).abs() < 1e-9);
}

#[test]
fn tension_depleted_budget_max_gravity() {
    let mut t = Tension::new();
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(100.0); // 0% remaining
    t.adjust_from_budget(1.0, budget);
    // energy_factor = 1.0, tension = 1.0 * 1.0 = 1.0
    assert!((t.level() - 1.0).abs() < 1e-9);
    assert!(t.is_critical());
}

// ─── Cost adjustment ────────────────────────────────────────────

#[test]
fn cost_adjustment_at_zero_tension_is_identity() {
    let t = Tension::new();
    assert!((t.adjust_cost(42.0) - 42.0).abs() < 1e-9);
}

#[test]
fn cost_adjustment_at_max_tension_doubles() {
    let mut t = Tension::new();
    t.set(1.0);
    assert!((t.adjust_cost(10.0) - 20.0).abs() < 1e-9);
}

#[test]
fn cost_adjustment_zero_cost_stays_zero() {
    let mut t = Tension::new();
    t.set(0.9);
    assert!((t.adjust_cost(0.0) - 0.0).abs() < 1e-9);
}

#[test]
fn cost_adjustment_negative_cost() {
    // Negative costs are nonsensical but shouldn't panic
    let mut t = Tension::new();
    t.set(0.5);
    let adjusted = t.adjust_cost(-10.0);
    // multiplier = 1.5, adjusted = -15.0
    assert!((adjusted - (-15.0)).abs() < 1e-9);
}

// ─── Fuzziness curve properties ─────────────────────────────────

#[test]
fn fuzziness_at_zero_is_zero() {
    let t = Tension::new();
    assert_eq!(t.fuzziness(), 0.0);
}

#[test]
fn fuzziness_below_half_is_linear_small() {
    // Below 0.5: fuzziness = level * 0.1
    let mut t = Tension::new();
    t.set(0.1);
    assert!((t.fuzziness() - 0.01).abs() < 1e-9);

    t.set(0.3);
    assert!((t.fuzziness() - 0.03).abs() < 1e-9);

    t.set(0.49);
    assert!((t.fuzziness() - 0.049).abs() < 1e-9);
}

#[test]
fn fuzziness_at_half_is_continuous() {
    // At exactly 0.5: low formula gives 0.05, high formula gives 0.05
    let mut t = Tension::new();
    t.set(0.5);
    assert!((t.fuzziness() - 0.05).abs() < 1e-9);
}

#[test]
fn fuzziness_above_half_ramps_sharply() {
    // Above 0.5: fuzziness = 0.05 + (level - 0.5) * 0.9
    let mut t = Tension::new();
    t.set(0.6);
    assert!((t.fuzziness() - 0.14).abs() < 1e-9);

    t.set(0.8);
    assert!((t.fuzziness() - 0.32).abs() < 1e-9);

    t.set(1.0);
    assert!((t.fuzziness() - 0.50).abs() < 1e-9);
}

#[test]
fn fuzziness_monotonically_increasing() {
    // Fuzziness should never decrease as tension increases
    let mut prev = 0.0;
    let mut t = Tension::new();
    for i in 0..=100 {
        let level = i as f64 / 100.0;
        t.set(level);
        let f = t.fuzziness();
        assert!(f >= prev - 1e-9, "Fuzziness decreased at level {}: {} < {}", level, f, prev);
        prev = f;
    }
}

// ─── Critical threshold ─────────────────────────────────────────

#[test]
fn critical_at_exactly_0_8_is_false_strict() {
    // is_critical uses > 0.8, so exactly 0.8 is NOT critical
    let mut t = Tension::new();
    t.set(0.8);
    assert!(!t.is_critical());
}

#[test]
fn critical_above_0_8() {
    let mut t = Tension::new();
    t.set(0.81);
    assert!(t.is_critical());

    t.set(0.99);
    assert!(t.is_critical());

    t.set(1.0);
    assert!(t.is_critical());
}

#[test]
fn critical_below_0_8() {
    let mut t = Tension::new();
    t.set(0.79);
    assert!(!t.is_critical());

    t.set(0.5);
    assert!(!t.is_critical());

    t.set(0.0);
    assert!(!t.is_critical());
}

// ─── Serialization round-trips ──────────────────────────────────

#[test]
fn conservation_budget_serializes_roundtrip() {
    let budget = ConservationBudget::new(42.5);
    let json = serde_json::to_string(&budget).unwrap();
    let deserialized: ConservationBudget = serde_json::from_str(&json).unwrap();
    assert_eq!(budget, deserialized);
}

#[test]
fn conservation_budget_with_spent_serializes_roundtrip() {
    let mut budget = ConservationBudget::new(100.0);
    budget.spend(37.0);
    budget.allocation = 50.0;
    let json = serde_json::to_string(&budget).unwrap();
    let deserialized: ConservationBudget = serde_json::from_str(&json).unwrap();
    assert_eq!(budget, deserialized);
}

// ─── Default impls ──────────────────────────────────────────────

#[test]
fn tension_default_is_zero() {
    let t = Tension::default();
    assert_eq!(t.level(), 0.0);
}
