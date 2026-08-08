# hermes-nmi — Getting Started

> Thought to action. Action to telemetry. Telemetry to thought. The loop closes.

## Prerequisites

- Rust 1.70+ (edition 2021)
- `cargo`

## 1. Add the Dependency

```toml
[dependencies]
hermes-nmi = { path = "../hermes-nmi" }
# or, when published:
# hermes-nmi = "0.1"
tokio = { version = "1", features = ["full"] }
```

## 2. Your First Pulse

Here's a complete example: send a Navigate pulse to an agent, execute it, and read the telemetry.

```rust
use hermes_nmi::{
    ClawNmiAdapter, IntentType, NeuroMuscularInterface, ReasoningPulse,
    ConservationBudget, Status,
};

#[tokio::main]
async fn main() {
    // Create an adapter wrapping a fresh Claw instance
    let mut adapter = ClawNmiAdapter::new();

    // The CNS says: "navigate to (5.0, 3.0, 0.0)"
    let pulse = ReasoningPulse::new(
        IntentType::Navigate,
        [5.0, 3.0, 0.0],
    )
    .with_gravity(0.6)    // moderate environmental complexity
    .with_energy(0.8);    // 80% energy budget

    // Dispatch — translates pulse to chain, executes, returns telemetry
    let telemetry = adapter.dispatch_pulse(pulse).await.unwrap();

    println!("Status: {:?}", telemetry.fulfillment_status);
    println!("Tension: {:.2}", telemetry.tension_at_execution);
    println!("Agent state: {}", telemetry.sensor_data.extras["agent_state"]);
}
```

**Output:**
```
Status: Success
Tension: 0.00
Agent state: "Acting"
```

What happened:
1. `IntentType::Navigate` was translated to: `SetState("Thinking")` → `Step` → `Step`
2. Each command was executed against the `ClawInstance`
3. The lifecycle advanced: Idle → Thinking → Acting → (back to Idle, but we read it mid-flight)
4. A `TelemetryFrame` was returned with `Status::Success`

## 3. Working with Tension

Tension models fatigue. Let's drain energy and see how it affects execution.

```rust
use hermes_nmi::*;

#[tokio::main]
async fn main() {
    let mut adapter = ClawNmiAdapter::new();

    // Simulate low energy: most budget already spent
    let budget = ConservationBudget {
        total: 100.0,
        spent: 85.0,        // only 15% remaining
        allocation: 15.0,
    };

    // High gravity (complex environment) + low energy = high tension
    adapter.adjust_tension(0.9, budget).await;

    // Now dispatch — tension will be > 0.7, so chains get truncated
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 0.0, 0.0])
        .with_gravity(0.9)
        .with_energy(0.15);

    let telemetry = match adapter.dispatch_pulse(pulse).await {
        Ok(t) => {
            println!("✅ Status: {:?}", t.fulfillment_status);
            println!("   Tension at execution: {:.2}", t.tension_at_execution);
            t
        }
        Err(e) => {
            println!("❌ Failed: {e}");
            return;
        }
    };

    // The tension level is available on the dispatcher
    println!("Current tension: {:.2}", adapter.dispatcher().tension_level());
}
```

## 4. Using the Dispatcher Directly

Sometimes you want to inspect the chain before executing:

```rust
use hermes_nmi::*;

fn main() {
    let dispatcher = NmiDispatcher::new();

    let pulse = ReasoningPulse::new(IntentType::Interact, [2.0, 1.0, 0.0])
        .with_constraint(Constraint::EnergyCeiling(0.5))
        .with_constraint(Constraint::Precision(0.9));

    // Translate without executing
    let chain = dispatcher.translate(&pulse);

    println!("Chain has {} commands:", chain.len());
    for (i, cmd) in chain.commands.iter().enumerate() {
        println!("  {}. {:?} → slot: {:?}", i + 1, cmd.action, cmd.slot);
    }
    println!("Estimated cost: {:.3}", chain.estimated_cost);

    // Validate constraints
    match dispatcher.validate(&pulse, &chain) {
        Ok(()) => println!("✅ All constraints satisfied"),
        Err(NmiError::ConstraintViolated(msg)) => println!("⚠️  {msg}"),
        Err(NmiError::EnergyExceeded { needed, available }) =>
            println!("⚠️  Need {needed:.2} but ceiling is {available:.2}"),
        Err(e) => println!("❌ {e}"),
    }
}
```

**Output:**
```
Chain has 3 commands:
  1. Equip("grasp") → slot: Some("Arms")
  2. SetState("Thinking") → slot: None
  3. Step → slot: None
Estimated cost: 0.300
✅ All constraints satisfied
```

## 5. Pincher Reflexes

Reflexes bypass reasoning. Here's how to use the `PincherHook`:

```rust
use hermes_nmi::pincher_hook::{PincherHook, ReflexMatch};

fn main() {
    let mut hook = PincherHook::new();

    // Pincher matched a stimulus with high confidence
    let exact_match = ReflexMatch {
        stimulus: "wall ahead".into(),
        confidence: 0.92,
        matched_intent: Some("stop".into()),
    };

    // High confidence → direct execution (no CNS needed)
    match hook.process(exact_match) {
        Ok(chain) => {
            println!("⚡ Reflex fired! {} commands", chain.len());
            for cmd in &chain.commands {
                println!("   {:?} → {:?}", cmd.action, cmd.slot);
            }
        }
        Err(pulse) => {
            println!("🧠 Escalated to CNS: {:?}", pulse.intent_type);
        }
    }

    // Low confidence → escalation
    let novel_match = ReflexMatch {
        stimulus: "??unknown anomaly??".into(),
        confidence: 0.20,
        matched_intent: None,
    };

    match hook.process(novel_match) {
        Ok(chain) => println!("Reflex fired (unexpected)"),
        Err(pulse) => println!("🧠 Escalated — CNS must reason about this"),
    }

    println!("\nReflexes fired: {}", hook.reflexes_fired());
    println!("Escalations: {}", hook.escalations());
}
```

**Output:**
```
⚡ Reflex fired! 3 commands
   SetState("Acting") → None
   Equip("reflex_module") → Some("Arms")
   Step → None
🧠 Escalated — CNS must reason about this

Reflexes fired: 1
Escalations: 1
```

## 6. Complete Example: Energy Management Loop

This shows the full feedback loop: dispatch pulses, watch energy deplete, observe tension rise, and decide to rest.

```rust
use hermes_nmi::*;

#[tokio::main]
async fn main() {
    let mut adapter = ClawNmiAdapter::new();
    let mut total_budget = ConservationBudget::new(10.0);

    let intents = vec![
        IntentType::Navigate,
        IntentType::Interact,
        IntentType::Observe,
        IntentType::Navigate,
        IntentType::Equip,
    ];

    for (i, intent) in intents.into_iter().enumerate() {
        // Adjust tension based on current energy
        adapter.adjust_tension(0.7, total_budget.clone()).await;

        let pulse = ReasoningPulse::new(intent, [i as f64, 0.0, 0.0])
            .with_energy(total_budget.remaining());

        println!("━━━ Pulse #{}: {:?} ━━━", i + 1, intent);
        println!("   Energy remaining: {:.1}/{:.1}", total_budget.remaining(), total_budget.total);
        println!("   Tension: {:.2}", adapter.dispatcher().tension_level());

        match adapter.dispatch_pulse(pulse).await {
            Ok(telemetry) => {
                println!("   → {:?} | tension was {:.2}",
                    telemetry.fulfillment_status,
                    telemetry.tension_at_execution);

                // Spend energy
                let cost = 2.0;
                total_budget.spend(cost);
            }
            Err(e) => {
                println!("   → FAILED: {e}");
                println!("   → Agent needs to rest.");
                break;
            }
        }
    }

    // After energy is low, rest
    println!("\n━━━ Recovery: Rest ━━━");
    let rest_pulse = ReasoningPulse::new(IntentType::Rest, [0.0, 0.0, 0.0]);
    let _ = adapter.dispatch_pulse(rest_pulse).await;
    println!("   Agent is resting. Slots unequipped.");
}
```

## 7. Running the Tests

```bash
cargo test
```

The crate includes tests for:
- `Tension` adjustment under various budgets
- `ConservationBudget` spending
- `PincherHook` match classification and routing
- `ReflexMatch` thresholds (Exact, Similar, Novel)

## 8. What's Next?

- Read `ARCHITECTURE.md` for the full system design
- Review `src/lib.rs` for the trait definition and module docs
- Check `NEURO-MUSCULAR-INTERFACE.md` (in hermes-construct) for the original spec
- Wire up a real `ClawInstance` (currently simulated)
- Integrate Pincher's vector DB for live reflex matching

---

The cortex is patient. The spinal cord is fast. The space between them is where you decide who you are.
