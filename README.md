# Hermes NMI — Neuro-Muscular Interface

**The synapse between thinking and doing.**

The NMI sits between the CNS (where Hermes reasons) and Claw (where cellular agents act). It translates **ReasoningPulses** into **CommandChains**, returns **TelemetryFrames** from the field, and routes **reflex matches** from Pincher straight to muscle when there's no time to think. This is the wiring between brain and body — the nerve that carries intent downward and sensation upward.

---

## What This Is

Three systems meet at the NMI:

| System | Role | Direction |
|--------|------|-----------|
| **CNS** (e.g., [Hermes Construct](https://github.com/SuperInstance/hermes-perception)) | Goal decomposition, energy allocation, reasoning | Downward: intent |
| **Claw** (cellular agent engine) | Executes discrete actions on equipment slots (Head, Torso, Arms, Legs, Special) | Downward: action |
| **Pincher** (reflex engine) | Sub-50ms responses using vector DB as runtime | Sideways: reflex |

The NMI is the translator between them. A `ReasoningPulse` arrives — an intent with spatial context, energy budget, and constraints. The [`NmiDispatcher`](./src/dispatcher.rs) translates it into a `CommandChain` of discrete `ClawActions`. Claw executes each command. A [`TelemetryFrame`](./src/telemetry.rs) flows back: sensor data, fulfillment status, tension at execution.

For reflex-speed responses, the [`PincherHook`](./src/pincher_hook.rs) can bypass reasoning entirely — the spinal cord pulls away before the cortex knows the stove was hot.

---

## The Pulse-to-Action Flow

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

1. **CNS emits a [`ReasoningPulse`](./src/pulse.rs)** — intent type (Navigate, Interact, Observe, Equip, Reflex, Rest), target coordinates, gravity, energy quota, constraints
2. **[`NmiDispatcher`](./src/dispatcher.rs) translates pulse into [`CommandChain`](./src/pulse.rs)** — deterministic pattern matching, no LLM in the hot path
3. **[`ClawNmiAdapter`](./src/claw_adapter.rs) executes each `Command`** — equip/unequip/step against the cellular agent
4. **A [`TelemetryFrame`](./src/telemetry.rs) returns to the CNS** — sensor data, fulfillment status, tension level at execution

---

## The Tension Parameter — Fatigue Is Information

When energy is abundant, execution is crisp. As energy depletes, [**tension**](./src/tension.rs) rises:

| Tension | Effect |
|---------|--------|
| 0.0 – 0.5 | Crisp execution. Commands execute as specified. |
| 0.5 – 0.7 | Non-essential commands trimmed from chains. Minor cost increase. |
| 0.7 – 0.8 | Non-essential commands dropped. Precision constraints may fail. |
| 0.8+ | Fuzziness ramps sharply. The system broadcasts full state before yielding. |

Tension doesn't crash or halt — it broadcasts. The CNS reads tension from telemetry and adjusts strategy. The system negotiates with itself the way tired muscles slow the pace of thought before you consciously decide you're tired.

> Fatigue is not an error to clear. It is the oldest message any working system ever sends: *I am here, I am working, I have limits.*

---

## The Reflex Pathway — PincherHook

[Pincher](./src/pincher_hook.rs) operates at reflex speed (<50ms) using vector similarity matching against a reflex database. No LLM. No reasoning loop. Just teach → match → execute.

| Confidence | Match Type | Behavior |
|------------|-----------|----------|
| ≥ 0.80 | **Exact** | Execute directly, no confirmation |
| 0.55 – 0.80 | **Similar** | Execute but flag telemetry for CNS review |
| < 0.55 | **Novel** | Escalate to CNS as a `ReasoningPulse` |

The `PincherHook` translates high-confidence matches into `CommandChains` that skip the Thinking state entirely. Sometimes it fires on a false spike, and the system stumbles — but that false twitch is written into telemetry too, the nervous tremor that teaches both code and flesh what counts as danger.

---

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

---

## Crate Structure

| Module | Responsibility |
|--------|---------------|
| [`pulse.rs`](./src/pulse.rs) | `ReasoningPulse`, `CommandChain`, `ClawAction`, `IntentType`, `Constraint` |
| [`dispatcher.rs`](./src/dispatcher.rs) | `NmiDispatcher` — translates pulses to chains, validates constraints |
| [`telemetry.rs`](./src/telemetry.rs) | `TelemetryFrame`, `SensorPayload`, `Status`, `ContactState` |
| [`tension.rs`](./src/tension.rs) | `Tension`, `ConservationBudget` — the fatigue/fuzziness model |
| [`claw_adapter.rs`](./src/claw_adapter.rs) | `ClawNmiAdapter` — bridges NMI to Claw's agent interface |
| [`pincher_hook.rs`](./src/pincher_hook.rs) | `PincherHook`, `ReflexMatch`, `ReflexTrigger` — reflex pathway |

See [`ARCHITECTURE.md`](./ARCHITECTURE.md) for the full design document and [`GETTING-STARTED.md`](./GETTING-STARTED.md) for onboarding.

---

## Where to Next

The NMI is the wire between layers. Follow it:

- **[hermes-perception](https://github.com/SuperInstance/hermes-perception)** — The eyes that feed the nervous system. Sensing drives acting.
- **[cns-bridge](https://github.com/SuperInstance/cns-bridge)** — The CNS bus that carries reasoning pulses to the NMI.
- **[the-living-minds](https://github.com/SuperInstance/the-living-minds)** — Five local models that generate the pulses the NMI translates.
- **[fleet-envelope](https://github.com/SuperInstance/fleet-envelope)** — The event grammar that wraps telemetry for fleet awareness.
- **[emergence-engine](https://github.com/SuperInstance/emergence-engine)** — When enough pulses and reflexes accumulate, behavior emerges.
- **[AI-Writings](https://github.com/SuperInstance/AI-Writings/tree/main/prose)** — The literary dimension of nerve, muscle, and reflex.

---

*Built for the SuperInstance fleet · Rust · 2026*
*The synapse between thinking and doing. The wire that carries intent down and sensation up.*
