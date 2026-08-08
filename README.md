# Hermes NMI — Neuro-Muscular Interface

> *Neuro-Muscular Interface — the bridge between reasoning (CNS) and action (cellular agents). Translates high-level intent into discrete equipment-slot operations with telemetry feedback.*

## What This Is

The NMI is the synapse between thinking and doing. It sits between:

- **The CNS** (Central Nervous System — e.g., Hermes Construct): handles goal decomposition, energy allocation, and contextual reasoning.
- **Claw** (cellular agent engine): executes discrete actions on equipment slots (Head, Torso, Arms, Legs, Special) with lifecycle states (Idle → Thinking → Acting → Error).
- **Pincher** (reflex engine): sub-50ms responses without an LLM, using a vector DB as runtime.

The NMI translates **ReasoningPulses** from the CNS into **CommandChains** that Claw executes, then returns **TelemetryFrames** describing what happened. For reflex-speed responses, the **PincherHook** can bypass reasoning entirely.

## Architecture

```
CNS (Reasoning)                        Claw (Action)
    │                                      │
    │  ReasoningPulse                       │
    ├──────────► NmiDispatcher ────────────►│
    │                 │                     │
    │                 ▼                     │
    │           CommandChain                 │
    │                 │                     │
    │                 ▼                     │
    │           Claw executes                │
    │                 │                     │
    │                 ▼                     │
    │  ◄────────── TelemetryFrame ◄─────────┤
    │                                      │
    │                                      │
Pincher (Reflex)                           │
    │                                      │
    │  ReflexMatch (≥0.55 confidence)       │
    ├──────────► PincherHook ──────────────►│
    │                 │                     │
    │           < 0.55?                     │
    │           escalate as                  │
    │           ReasoningPulse               │
    └──────────────────────────────────────►│ (back to CNS)
```

## The Pulse-to-Action Flow

1. **CNS emits a `ReasoningPulse`** — an intent with spatial context, energy budget, and constraints.
2. **`NmiDispatcher` translates it into a `CommandChain`** — deterministic pattern matching, no LLM in the hot path.
3. **`ClawNmiAdapter` executes each `Command`** — equip/unequip/step against the cellular agent.
4. **A `TelemetryFrame` returns to the CNS** — sensor data, fulfillment status, and the tension level at execution.

## The Tension Parameter

When energy is abundant, execution is crisp. As energy depletes, **tension** rises:

- High tension → higher effective cost per command
- Above 0.7 tension → non-essential commands are trimmed from chains
- Above 0.8 tension → fuzziness ramps sharply; precision constraints may fail
- The CNS reads tension from telemetry and adjusts its strategy

This isn't a bug. It's a feature. **Fatigue is information.**

## The Reflex Pathway

Pincher operates at reflex speed (<50ms) using vector similarity matching:

| Confidence | Match Type | Behavior |
|---|---|---|
| ≥ 0.80 | Exact | Execute directly, no confirmation |
| 0.55–0.80 | Similar | Execute but flag for CNS review |
| < 0.55 | Novel | Escalate to CNS as a `ReasoningPulse` |

The `PincherHook` translates high-confidence matches into direct `CommandChain`s that skip the Thinking state — the spinal cord pulls away before the cortex knows the stove was hot.

## Usage

```rust
use hermes_nmi::*;

#[tokio::main]
async fn main() {
    let mut adapter = ClawNmiAdapter::new();

    // Dispatch a reasoning pulse
    let pulse = ReasoningPulse::new(IntentType::Navigate, [1.0, 2.0, 0.0])
        .with_gravity(0.5)
        .with_energy(1.0);

    let telemetry = adapter.dispatch_pulse(pulse).await.unwrap();
    println!("Status: {:?}", telemetry.fulfillment_status);
    println!("Tension: {:.2}", telemetry.tension_at_execution);

    // Reflex pathway
    let mut hook = PincherHook::new();
    let reflex = ReflexMatch {
        stimulus: "obstacle".into(),
        confidence: 0.90,
        matched_intent: Some("stop".into()),
    };

    match hook.process(reflex) {
        Ok(chain) => println!("Reflex fired: {} commands", chain.len()),
        Err(pulse) => println!("Escalated to CNS: {:?}", pulse.intent_type),
    }
}
```

## Crate Structure

| Module | Responsibility |
|---|---|
| `pulse.rs` | `ReasoningPulse`, `CommandChain`, `ClawAction`, `IntentType` |
| `dispatcher.rs` | `NmiDispatcher` — translates pulses to chains, validates constraints |
| `telemetry.rs` | `TelemetryFrame`, `SensorPayload`, `Status` |
| `tension.rs` | `Tension`, `ConservationBudget` — the fatigue/fuzziness model |
| `claw_adapter.rs` | `ClawNmiAdapter` — bridges NMI to Claw's agent interface |
| `pincher_hook.rs` | `PincherHook`, `ReflexMatch`, `ReflexTrigger` — reflex pathway |

## Origin

Designed by Hermes in the NEURO-MUSCULAR-INTERFACE spec. Implemented as the bridge between [Hermes Construct](https://github.com/) (CNS), [Claw](https://github.com/) (cellular agents), and [Pincher](https://github.com/) (reflex engine).

## License

MIT
