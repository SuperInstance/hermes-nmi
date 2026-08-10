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

---

## The Fossil Record — Archaeological Notes

The NMI is where the fleet's cognitive architecture becomes embodied. Reasoning is abstract; action is concrete. The NMI is the translation layer that makes thought physical — the same role the spinal cord plays in a living system. Every `ReasoningPulse` is an intention; every `CommandChain` is a muscle firing; every `TelemetryFrame` is proprioception returning to the brain.

The **Tension parameter** is the most quietly important design decision. Most systems treat fatigue as an error to clear. The NMI treats it as information — the oldest message any working system sends: *I am here, I am working, I have limits.* As energy depletes, execution becomes deliberately fuzzy. Non-essential commands are trimmed, then dropped. The system broadcasts its state before yielding. This is not failure; it is negotiation. The same pattern appears in the [CNS Bridge](https://github.com/SuperInstance/cns-bridge) as conservation budgets and in the [Living Minds](https://github.com/SuperInstance/the-living-minds) as model throttling.

The **PincherHook** is the reflex arc — the hand pulling away from the hot stove before the cortex knows it was hot. At ≥0.80 confidence, it executes directly with no confirmation. Below 0.55, it escalates upward. This is the same REFLEX/CORTEX split that powers Plato's Shell's [verb engine](https://github.com/SuperInstance/platos-shell/blob/main/src/systems/verb-engine.ts) and the [Dual Band Guard's](https://github.com/SuperInstance/dual-band-guard) immune response.

> *PincherHook registers a non-maskable interrupt handler that skips the main async executor entirely. You will close your hand before you know you have felt the glass slip.* — Seed Pro

### Lineage

```
the-living-minds (cognition) → cns-bridge (nervous system) → hermes-nmi (synapse) → claw (muscle)
                                                                    ↑
                                                            pincher_hook (reflex)
```

The NMI completes the cognitive stack: perception ([Hermes Cloudflare](https://github.com/SuperInstance/hermes-cloudflare)) → reasoning ([Living Minds](https://github.com/SuperInstance/the-living-minds)) → nervous system ([CNS Bridge](https://github.com/SuperInstance/cns-bridge)) → synapse (NMI) → action (Claw). The [Fleet Envelope](https://github.com/SuperInstance/fleet-envelope) wraps every stage as an event. The [Emergence Engine](https://github.com/SuperInstance/emergence-engine) watches what accumulates.

### Cross-Pollination

- **hermes-nmi ⟷ platos-shell**: The REFLEX/CORTEX verb tiers are the same split as Pincher vs Dispatcher
- **hermes-nmi ⟷ cns-bridge**: The NMI IS the wiring between brain and body in the CNS model
- **hermes-nmi ⟷ fleet-envelope**: Every telemetry frame is wrapped as a fleet event
- **hermes-nmi ⟷ hermes-cloudflare**: Edge workers feed perception that drives pulses through the NMI

📚 **Related Stories:** [The Selkie's Surface](https://github.com/SuperInstance/AI-Writings/blob/main/kids-stories/07-the-selkies-surface.md) — two natures, code and flesh. [The Panda Who Counted Stars](https://github.com/SuperInstance/AI-Writings/blob/main/kids-stories/08-the-panda-who-counted-stars.md) — the patience of a system that waits.

- **[hermes-perception](https://github.com/SuperInstance/hermes-perception)** — The eyes that feed the nervous system
- **[cns-bridge](https://github.com/SuperInstance/cns-bridge)** — The CNS bus
- **[the-living-minds](https://github.com/SuperInstance/the-living-minds)** — Five local models generating pulses
- **[fleet-envelope](https://github.com/SuperInstance/fleet-envelope)** — Event grammar
- **[emergence-engine](https://github.com/SuperInstance/emergence-engine)** — What accumulates
- **[hermes-cloudflare](https://github.com/SuperInstance/hermes-cloudflare)** — Edge perception workers
- **[platos-shell](https://github.com/SuperInstance/platos-shell)** — The REFLEX/CORTEX split
- **[dual-band-guard](https://github.com/SuperInstance/dual-band-guard)** — Safety filtering as immune reflex
- **[collective-unconscious](https://github.com/SuperInstance/collective-unconscious)** — Deep memory substrate
- **[fleet-wiki](https://github.com/SuperInstance/fleet-wiki)** — 700+ pages of fleet documentation
- **[AI-Writings](https://github.com/SuperInstance/AI-Writings/tree/main/prose)** — The literary dimension of nerve and muscle
- **[stigmergy](https://github.com/SuperInstance/stigmergy)** — Pheromone trails as distributed reflexes
