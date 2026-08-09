//! Integration tests for PincherHook — the reflex pathway.
//!
//! These tests exercise the full flow from stimulus → match → process,
//! including the trigger creation paths and threshold boundary behavior.

use hermes_nmi::{
    ClawAction, MatchType, PincherHook,
    ReflexAction, ReflexMatch, ReflexTrigger,
    EXACT_THRESHOLD, SIMILAR_THRESHOLD,
};

// ─── ReflexMatch classification at boundaries ───────────────────

#[test]
fn match_type_at_exact_threshold() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: EXACT_THRESHOLD, // exactly 0.80
        matched_intent: None,
    };
    assert_eq!(m.match_type(), MatchType::Exact);
}

#[test]
fn match_type_just_below_exact_threshold() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: EXACT_THRESHOLD - 0.001,
        matched_intent: None,
    };
    assert_eq!(m.match_type(), MatchType::Similar);
}

#[test]
fn match_type_at_similar_threshold() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: SIMILAR_THRESHOLD, // exactly 0.55
        matched_intent: None,
    };
    assert_eq!(m.match_type(), MatchType::Similar);
}

#[test]
fn match_type_just_below_similar_threshold() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: SIMILAR_THRESHOLD - 0.001,
        matched_intent: None,
    };
    assert_eq!(m.match_type(), MatchType::Novel);
}

#[test]
fn match_type_at_zero_confidence() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: 0.0,
        matched_intent: None,
    };
    assert_eq!(m.match_type(), MatchType::Novel);
    assert!(m.should_escalate());
    assert!(!m.should_auto_fire());
}

#[test]
fn match_type_at_perfect_confidence() {
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: 1.0,
        matched_intent: Some("known".into()),
    };
    assert_eq!(m.match_type(), MatchType::Exact);
    assert!(m.should_auto_fire());
    assert!(!m.should_escalate());
}

// ─── Hook processing: exact matches ─────────────────────────────

#[test]
fn hook_exact_match_returns_command_chain() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "wall ahead".into(),
        confidence: 0.95,
        matched_intent: Some("stop".into()),
    };
    let result = hook.process(m);
    assert!(result.is_ok());
    let chain = result.unwrap();
    assert!(!chain.commands.is_empty());
    // First command should be SetState("Acting")
    match &chain.commands[0].action {
        ClawAction::SetState(s) => assert_eq!(s, "Acting"),
        other => panic!("Expected SetState, got {:?}", other),
    }
}

#[test]
fn hook_exact_match_increments_fired_counter() {
    let mut hook = PincherHook::new();
    for _ in 0..5 {
        let m = ReflexMatch {
            stimulus: "x".into(),
            confidence: 0.90,
            matched_intent: None,
        };
        hook.process(m);
    }
    assert_eq!(hook.reflexes_fired(), 5);
    assert_eq!(hook.escalations(), 0);
}

#[test]
fn hook_reflex_chain_has_estimated_cost() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: 0.88,
        matched_intent: None,
    };
    let chain = hook.process(m).unwrap();
    assert!(chain.estimated_cost > 0.0);
}

#[test]
fn hook_reflex_chain_includes_step() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "x".into(),
        confidence: 0.85,
        matched_intent: None,
    };
    let chain = hook.process(m).unwrap();
    let has_step = chain.commands.iter().any(|c| matches!(c.action, ClawAction::Step));
    assert!(has_step, "Reflex chain should include a Step command");
}

// ─── Hook processing: similar matches ───────────────────────────

#[test]
fn hook_similar_match_auto_fires() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "something".into(),
        confidence: 0.60,
        matched_intent: Some("maybe".into()),
    };
    // Similar matches (0.55–0.80) still fire, not escalate
    let result = hook.process(m);
    assert!(result.is_ok());
    assert_eq!(hook.reflexes_fired(), 1);
    assert_eq!(hook.escalations(), 0);
}

// ─── Hook processing: novel matches ─────────────────────────────

#[test]
fn hook_novel_match_escalates_with_reasoning_pulse() {
    let mut hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "????".into(),
        confidence: 0.10,
        matched_intent: None,
    };
    let result = hook.process(m);
    assert!(result.is_err());
    let pulse = result.unwrap_err();
    // The pulse should have Reflex intent
    assert_eq!(pulse.intent_type, hermes_nmi::IntentType::Reflex);
}

#[test]
fn hook_novel_match_increments_escalation_counter() {
    let mut hook = PincherHook::new();
    for _ in 0..3 {
        let m = ReflexMatch {
            stimulus: "???".into(),
            confidence: 0.20,
            matched_intent: None,
        };
        let _ = hook.process(m);
    }
    assert_eq!(hook.reflexes_fired(), 0);
    assert_eq!(hook.escalations(), 3);
}

// ─── Trigger creation ───────────────────────────────────────────

#[test]
fn trigger_for_exact_match_is_execute_no_confirmation() {
    let hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "known".into(),
        confidence: 0.90,
        matched_intent: Some("act".into()),
    };
    let trigger = hook.create_trigger(m);
    assert!(matches!(trigger.action, ReflexAction::Execute(_)));
    assert!(!trigger.requires_confirmation);
}

#[test]
fn trigger_for_similar_match_is_defend_with_confirmation() {
    let hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "fuzzy".into(),
        confidence: 0.65,
        matched_intent: Some("maybe".into()),
    };
    let trigger = hook.create_trigger(m);
    assert!(matches!(trigger.action, ReflexAction::Defend(_)));
    assert!(trigger.requires_confirmation);
}

#[test]
fn trigger_for_novel_match_is_escalate() {
    let hook = PincherHook::new();
    let m = ReflexMatch {
        stimulus: "unknown".into(),
        confidence: 0.30,
        matched_intent: None,
    };
    let trigger = hook.create_trigger(m);
    assert!(matches!(trigger.action, ReflexAction::Escalate));
}

// ─── Mixed processing: counters track independently ─────────────

#[test]
fn hook_mixed_matches_track_counters_correctly() {
    let mut hook = PincherHook::new();

    // 3 exact fires
    for _ in 0..3 {
        let _ = hook.process(ReflexMatch {
            stimulus: "a".into(),
            confidence: 0.90,
            matched_intent: None,
        });
    }

    // 2 similar fires
    for _ in 0..2 {
        let _ = hook.process(ReflexMatch {
            stimulus: "b".into(),
            confidence: 0.60,
            matched_intent: None,
        });
    }

    // 1 novel escalation
    let _ = hook.process(ReflexMatch {
        stimulus: "c".into(),
        confidence: 0.10,
        matched_intent: None,
    });

    assert_eq!(hook.reflexes_fired(), 5); // exact + similar
    assert_eq!(hook.escalations(), 1);
}

#[test]
fn hook_default_equals_new() {
    let h1 = PincherHook::new();
    let h2 = PincherHook::default();
    assert_eq!(h1.reflexes_fired(), h2.reflexes_fired());
    assert_eq!(h1.escalations(), h2.escalations());
}

// ─── ReflexMatch serialization ──────────────────────────────────

#[test]
fn reflex_match_serializes_roundtrip() {
    let m = ReflexMatch {
        stimulus: "obstacle".into(),
        confidence: 0.77,
        matched_intent: Some("dodge".into()),
    };
    let json = serde_json::to_string(&m).unwrap();
    let back: ReflexMatch = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
}

#[test]
fn reflex_match_with_none_intent_serializes() {
    let m = ReflexMatch {
        stimulus: "???".into(),
        confidence: 0.1,
        matched_intent: None,
    };
    let json = serde_json::to_string(&m).unwrap();
    let back: ReflexMatch = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
}

// ─── MatchType serialization ────────────────────────────────────

#[test]
fn match_type_serializes_roundtrip() {
    for variant in [MatchType::Exact, MatchType::Similar, MatchType::Novel] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: MatchType = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
}

// ─── ReflexAction serialization ─────────────────────────────────

#[test]
fn reflex_action_execute_serializes_roundtrip() {
    let action = ReflexAction::Execute(ClawAction::Step);
    let json = serde_json::to_string(&action).unwrap();
    let back: ReflexAction = serde_json::from_str(&json).unwrap();
    assert_eq!(action, back);
}

#[test]
fn reflex_action_defend_serializes_roundtrip() {
    let action = ReflexAction::Defend("shield".into());
    let json = serde_json::to_string(&action).unwrap();
    let back: ReflexAction = serde_json::from_str(&json).unwrap();
    assert_eq!(action, back);
}

#[test]
fn reflex_action_withdraw_serializes() {
    let json = serde_json::to_string(&ReflexAction::Withdraw).unwrap();
    let back: ReflexAction = serde_json::from_str(&json).unwrap();
    assert_eq!(ReflexAction::Withdraw, back);
}

#[test]
fn reflex_action_escalate_serializes() {
    let json = serde_json::to_string(&ReflexAction::Escalate).unwrap();
    let back: ReflexAction = serde_json::from_str(&json).unwrap();
    assert_eq!(ReflexAction::Escalate, back);
}

// ─── ReflexTrigger serialization ────────────────────────────────

#[test]
fn reflex_trigger_serializes_roundtrip() {
    let trigger = ReflexTrigger {
        match_result: ReflexMatch {
            stimulus: "fire".into(),
            confidence: 0.85,
            matched_intent: Some("run".into()),
        },
        action: ReflexAction::Withdraw,
        requires_confirmation: false,
    };
    let json = serde_json::to_string(&trigger).unwrap();
    let back: ReflexTrigger = serde_json::from_str(&json).unwrap();
    assert_eq!(trigger, back);
}
