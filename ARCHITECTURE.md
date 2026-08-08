# hermes-nmi — Architecture

> The synapse between thinking and doing.

## 1. Overview

`hermes-nmi` is the Neuro-Muscular Interface between **Claw** (a cellular agent runtime with equipment slots and lifecycle states) and a **CNS** (Central Nervous System — any reasoning layer that emits intent). It provides:

- **ReasoningPulse** — a typed intent payload from the CNS
- **CommandChain** — a deterministic decomposition of intent into discrete actions
- **Tension** — a parameter that degrades execution fidelity under energy pressure
- **PincherHook** — a reflex pathway for sub-50ms responses that bypass reasoning
- **TelemetryFrame** — sensory feedback from Claw back to the CNS

The crate implements a single trait — `NeuroMuscularInterface` — that defines the boundary contract.

```
┌─────────────────────────────────────────────────────────┐
│                         CNS                              │
│            (Reasoning, Planning, Conservation)           │
└──────────────────┬──────────────────────────▲────────────┘
                   │ ReasoningPulse            │ TelemetryFrame
                   ▼                           │
┌──────────────────────────────────────────────────────────┐
│                    NmiDispatcher                         │
│  pulse ──► translate() ──► CommandChain ──► validate()   │
│                                      │                    │
│                         tension-aware cost estimation    │
└──────────────────┬───────────────────────▲───────────────┘
                   │ CommandChain          │ TelemetryFrame
                   ▼                       │
┌──────────────────────────────────────────────────────────┐
│                    ClawNmiAdapter                         │
│  chain ──► execute_command() ──► AgentState mutation     │
│  equip / unequip / step / set_state                      │
└──────────────────┬──────────────────────────▲────────────┘
                   │                           │
                   ▼                           │
┌──────────────────────────────────────────────────────────┐
│                    ClawInstance                           │
│         (EquipmentSlots, AgentState, step_count)         │
└──────────────────────────────────────────────────────────┘

           ┌──────────────────────────────┐
           │       PincherHook            │
           │  (parallel reflex pathway)   │
           │                              │
           │  Stimulus ──► ReflexMatch    │
           │  confidence ≥ 0.80: direct   │
           │  0.55–0.80: fire + flag      │
           │  < 0.55: escalate to CNS     │
           └──────────────────────────────┘
```

---

## 2. The ReasoningPulse → CommandChain → DeterministicAction Flow

This is the primary data path. Every action the agent takes originates as a pulse and terminates as a state mutation.

### 2.1 ReasoningPulse (Thought)

A `ReasoningPulse` is the CNS saying "I want this to happen." It is **not** a command — it's an intent with context.

```rust
pub struct ReasoningPulse {
    pub pulse_id: Uuid,
    pub intent_type: IntentType,       // Navigate, Interact, Observe, Equip, Reflex, Rest
    pub target_coordinates: [f64; 3],  // spatial target
    pub gravity: f64,                   // JEPA context weight (0.0–1.0)
    pub energy_quota: f64,              // local energy budget
    pub constraints: Vec<Constraint>,   // time, energy, precision limits
}
```

The six `IntentType` variants cover the full behavioral space:

| Intent    | Semantics                          | Typical Chain                      |
|-----------|------------------------------------|------------------------------------|
| Navigate  | Move to a position or state        | Thinking → Step → Step             |
| Interact  | Touch, grasp, manipulate something | Equip(Arms) → Thinking → Step      |
| Observe    | Look and report back               | Equip(Head) → Step                 |
| Equip     | Configure capabilities             | Equip(Torso) + Equip(Legs)         |
| Reflex    | Emergency bypass — act NOW         | Acting → Step (minimal chain)     |
| Rest      | Enter low-power state              | Unequip(all) → Idle                |

### 2.2 CommandChain (Translation)

The `NmiDispatcher::translate()` method converts a pulse into a `CommandChain` via pure pattern matching on `IntentType`. **No LLM, no network calls — deterministic translation only.**

```rust
pub struct CommandChain {
    pub source_pulse_id: Uuid,
    pub commands: Vec<Command>,      // ordered sequence
    pub estimated_cost: f64,         // tension-adjusted
}
```

Each `Command` pairs a `ClawAction` with an optional equipment slot:

```rust
pub enum ClawAction {
    Equip(String),
    Unequip(String),
    Step,
    SetState(String),
}
```

The translation table is static. For example, `IntentType::Interact` always produces:
1. `Equip("grasp")` → slot: Arms
2. `SetState("Thinking")` → slot: None
3. `Step` → slot: None

**Tension affects the chain.** When `tension.level() > 0.7` and the chain has more than 2 commands, it's truncated to 2 (preserve the first action and the final step). This models how a fatigued system skips non-essential steps.

### 2.3 DeterministicAction (Execution)

The `ClawNmiAdapter::execute_chain()` method applies each command in sequence to a `ClawInstance`:

```
ClawInstance {
    state: AgentState,           // Idle → Thinking → Acting → Idle
    equipment: HashSet<Slot>,    // Head, Torso, Arms, Legs, Special
    step_count: u64,             // increments each Step
}
```

The lifecycle follows a strict state machine:

```
        Step           Step          Step
  ┌──────────┐    ┌──────────┐    ┌─────────┐
  │   Idle   │───►│ Thinking │───►│ Acting  │
  │          │◄───│          │◄───│         │
  └──────────┘    └──────────┘    └─────────┘
                        │               │
                        ▼               ▼
                   Error(String)   Error(String)
                   (any invalid state transition)
```

If any command fails (e.g., stepping from an Error state), execution halts and the error propagates as `NmiError::AgentError`.

### 2.4 Full Sequence

```
 CNS          NmiDispatcher       ClawNmiAdapter       ClawInstance
  │                  │                   │                   │
  │ ReasoningPulse   │                   │                   │
  │─────────────────►│                   │                   │
  │                  │                   │                   │
  │                  │ translate(pulse)  │                   │
  │                  │──► CommandChain   │                   │
  │                  │                   │                   │
  │                  │ validate(pulse,   │                   │
  │                  │         chain)    │                   │
  │                  │                   │                   │
  │                  │   CommandChain    │                   │
  │                  │──────────────────►│                   │
  │                  │                   │                   │
  │                  │                   │ execute_command() │
  │                  │                   │──────────────────►│
  │                  │                   │                   │
  │                  │                   │    (state mutated)│
  │                  │                   │◄──────────────────┤
  │                  │                   │                   │
  │                  │      build_telemetry()                │
  │                  │◄──────────────────┤                   │
  │                  │                   │                   │
  │ TelemetryFrame   │                   │                   │
  │◄─────────────────┤                   │                   │
  │                  │                   │                   │
```

---

## 3. The Tension Parameter

Tension is the bridge between energy and action quality. It models how muscles behave under fatigue: tremor increases, precision drops, recovery takes longer.

### 3.1 Formula

```
Tension = gravity × (1 − fraction_remaining)

where:
  gravity            = CNS-provided context weight (0.0–1.0)
  fraction_remaining = (total_energy − spent_energy) / total_energy
```

| Energy Remaining | Gravity | Tension | Behavior                           |
|-----------------|---------|---------|------------------------------------|
| 100%            | any     | 0.0     | Crisp, deterministic execution     |
| 50%             | 0.8     | 0.4     | Slight cost increase, minimal fuzz |
| 20%             | 0.8     | 0.64    | Notable degradation, chains trim   |
| 5%              | 1.0     | 0.95    | Critical — most commands skipped   |

### 3.2 Effects on Execution

Tension affects three things:

1. **Cost Inflation** — `adjust_cost(base) = base × (1 + tension)`. At max tension, everything costs twice as much.

2. **Chain Truncation** — When `tension > 0.7` and chain length > 2, only the first 2 commands survive. The system sheds non-essential steps.

3. **Fuzziness** — The probability that a command is skipped or degraded:
   ```
   tension < 0.5:  fuzziness = tension × 0.1
   tension ≥ 0.5:  fuzziness = 0.05 + (tension − 0.5) × 0.9
   ```
   Fuzziness ramps sharply past 50% tension.

4. **Constraint Validation** — High tension blocks high-precision constraints. If `precision > 0.8` and `tension > 0.6`, the pulse is rejected with `ConstraintViolated`.

### 3.3 Philosophy

> Tension isn't a bug. It's a feature. Fatigue is information.
> The CNS reads tension from telemetry and adjusts its strategy.

Tension makes the system **honest about its limits**. Rather than pretending to execute perfectly on empty batteries, it degrades gracefully and reports the degradation through telemetry. The CNS can then decide: rest, re-allocate energy, or accept lower fidelity.

---

## 4. Pincher Reflexes — The Spinal Cord

### 4.1 What Is Pincher?

Pincher is a reflex engine that operates **outside the reasoning pipeline**. It uses a vector database as runtime memory: Teach → Match → Execute. Response time is sub-50ms because there's no LLM in the loop.

### 4.2 The PincherHook

The `PincherHook` integrates Pincher into the NMI. It receives `ReflexMatch` results and routes them:

```
                        ReflexMatch
                             │
                    ┌────────┴────────┐
                    │                 │
            confidence              confidence
            ≥ 0.80                  < 0.55
                    │                 │
                    ▼                 ▼
           ┌─────────────┐   ┌──────────────┐
           │ DIRECT FIRE │   │  ESCALATE    │
           │             │   │              │
           │ Build chain │   │ ReasoningPulse│
           │ → execute   │   │ → CNS handles │
           └─────────────┘   └──────────────┘
                    │
                    │     confidence 0.55–0.80
                    │           │
                    │           ▼
                    │   ┌──────────────┐
                    │   │ FIRE + FLAG  │
                    │   │              │
                    │   │ Execute but  │
                    │   │ mark for CNS │
                    │   │ review       │
                    │   └──────────────┘
```

### 4.3 Confidence Thresholds

| Confidence | MatchType | Action                                     |
|-----------|-----------|--------------------------------------------|
| ≥ 0.80    | Exact     | Execute directly, no confirmation needed   |
| 0.55–0.80 | Similar   | Execute but flag for CNS review            |
| < 0.55    | Novel     | Escalate to CNS as a ReasoningPulse(Reflex)|

### 4.4 Reflex Command Chains

Reflex chains are intentionally minimal — usually 2–3 commands:

1. `SetState("Acting")` — skip Thinking entirely
2. `Equip("reflex_module")` → slot: Arms
3. `Step` — complete the reflex

Estimated cost: 0.15 (reflexes are cheap).

### 4.5 Escalation

When Pincher encounters a novel stimulus (confidence < 0.55), it creates a `ReasoningPulse` with `IntentType::Reflex` and routes it to the CNS. The CNS then decides what to do — it can teach Pincher a new reflex for next time, or handle the situation through normal reasoning.

This is the **learning loop**: novel stimuli that get resolved successfully become new reflexes. Over time, the system gets faster.

---

## 5. TelemetryFrame — The Feedback Loop

### 5.1 Structure

After every command chain execution, a `TelemetryFrame` is generated:

```rust
pub struct TelemetryFrame {
    pub pulse_id: Uuid,               // which pulse this answers
    pub timestamp: u64,                // when (epoch ms)
    pub tension_at_execution: f64,     // how strained execution was
    pub state_hash: [u8; 32],          // fingerprint of agent state
    pub sensor_data: SensorPayload,    // what was felt
    pub fulfillment_status: Status,    // did it work?
}
```

### 5.2 SensorPayload

```rust
pub struct SensorPayload {
    pub velocity: Option<[f64; 3]>,     // current speed vector
    pub proximity: Option<f64>,          // distance to nearest obstacle
    pub contact_state: ContactState,     // None, Soft, Hard, Pushing
    pub resistance: f64,                 // environmental load (0.0–1.0)
    pub positional_delta: [f64; 3],      // intended vs. achieved position
    pub extras: serde_json::Value,       // freeform additional readings
}
```

### 5.3 Fulfillment Status

| Status          | Meaning                                    |
|----------------|--------------------------------------------|
| Success        | Intent fully achieved                      |
| PartialSuccess | Some commands succeeded, some failed       |
| Failure        | Intent could not be achieved               |
| ReRoute        | Environment changed — need new plan        |
| ReThink        | Agent is reflecting before retrying        |

### 5.4 The Closed Loop

```
┌─────────────────────────────────────────────────┐
│                                                 │
│    CNS ──► Pulse ──► Dispatcher ──► Claw        │
│                                      │          │
│                                      ▼          │
│    CNS ◄── Telemetry ◄── Adapter ◄───┘          │
│                                                 │
│    CNS reads tension, status, sensor data       │
│    Adjusts strategy, re-allocates energy        │
│    Sends next pulse with updated context        │
│                                                 │
└─────────────────────────────────────────────────┘
```

The CNS uses telemetry to:
- **Detect failures** — if status is `Failure` or `ReRoute`, reformulate the plan
- **Read tension** — high tension means energy is low; consider `IntentType::Rest`
- **Track state changes** — the `state_hash` enables change detection without full state sync
- **Learn from sensor data** — positional delta reveals accuracy; contact state reveals environment

---

## 6. Built vs. Specified: Gap Analysis

### 6.1 What Was Specified

The original `NEURO-MUSCULAR-INTERFACE.md` defined:
1. ✅ `NeuroMuscularInterface` trait with `dispatch_pulse` and `adjust_tension`
2. ✅ `ReasoningPulse` with `pulse_id`, `intent_type`, `target_coordinates`, `gravity`, `energy_quota`, `constraints`
3. ✅ `TelemetryFrame` with `timestamp`, `state_hash`, `sensor_data`, `fulfillment_status`
4. ✅ `NmiDispatcher` translating pulses to command chains
5. ✅ `ClawNmiAdapter` consuming pulses against a Claw instance

### 6.2 What Was Built (Beyond Spec)

The implementation **exceeded** the spec in several areas:

| Feature | Spec | Built |
|---------|------|-------|
| `Tension` parameter | Mentioned as "muscle tension" concept | Full module with formula, fuzziness, cost adjustment, critical threshold |
| `ConservationBudget` | Mentioned as parameter to `adjust_tension` | Complete struct with `total`, `spent`, `allocation`, `spend()`, `fraction_remaining()` |
| `CommandChain` | Not in spec (only trait method) | Full type with `source_pulse_id`, `commands`, `estimated_cost`, truncation under tension |
| `ClawAction` enum | Not in spec | `Equip`, `Unequip`, `Step`, `SetState` — concrete action vocabulary |
| `PincherHook` | Not in spec | Complete reflex pathway with confidence thresholds, escalation, match classification |
| `ReflexMatch` / `ReflexTrigger` | Not in spec | Full reflex matching with Exact/Similar/Novel classification |
| `Constraint` validation | Listed in spec but no validation logic | `validate()` method checks TimeBudget, EnergyCeiling, Precision against tension |
| `EquipmentSlot` model | Not in spec | Head, Torso, Arms, Legs, Special — maps to Claw's agent model |
| `AgentState` lifecycle | Not in spec | Idle → Thinking → Acting → Idle state machine |
| `ContactState` in telemetry | Not in spec | None, Soft, Hard, Pushing |
| Status enum | Only Success/Failure/Re-routing in spec | Added `PartialSuccess` and `ReThink` |

### 6.3 What's Missing (Spec Gap)

| Spec Item | Status | Notes |
|-----------|--------|-------|
| Protobuf serialization | ❌ Not implemented | Spec mentioned "JSON/Protobuf"; only JSON (serde) is supported |
| Real Claw runtime binding | ❌ Simulated only | `ClawInstance` is a simulated agent, not wired to actual Claw |
| Spline-Observer (Phase 04) | ❌ Not started | Sensory harvest pipeline to `ai-writings` |
| JEPA gravity implementation | ⚠️ Partial | `gravity` is stored and affects tension, but no actual JEPA model integration |
| async dispatch with tokio | ⚠️ Partial | Trait is async, but execution is synchronous (no I/O) |
| `target_coordinates` usage | ⚠️ Partial | Stored in pulse but not used by translate() — chains don't reference spatial data |

### 6.4 Summary

The built crate is a **faithful and expanded** implementation of the spec. It covers all four specified phases (trait definition, adapter, client, telemetry) and adds significant depth in tension modeling, reflex pathways, and equipment slot mechanics. The main gaps are integration-level: no real Claw binding, no protobuf, no JEPA model — all expected for a v0.1.0.

---

## 7. Module Map

```
hermes-nmi/
├── src/
│   ├── lib.rs            # Trait definition, re-exports, crate docs
│   ├── pulse.rs          # ReasoningPulse, IntentType, Constraint, CommandChain, Command, ClawAction
│   ├── dispatcher.rs     # NmiDispatcher (translate, validate, build_telemetry), NmiError
│   ├── tension.rs        # Tension, ConservationBudget
│   ├── telemetry.rs      # TelemetryFrame, SensorPayload, Status, ContactState
│   ├── claw_adapter.rs   # ClawNmiAdapter, ClawInstance, AgentState, EquipmentSlot
│   └── pincher_hook.rs   # PincherHook, ReflexMatch, ReflexTrigger, ReflexAction, MatchType
├── Cargo.toml
├── ARCHITECTURE.md       # ← you are here
└── GETTING-STARTED.md    # worked example
```

---

## 8. Design Principles

1. **No LLM in the hot path.** Translation from pulse to chain is pure pattern matching. The CNS does the thinking; the NMI does the doing.

2. **Tension is a feature, not a bug.** Graceful degradation under energy pressure is the whole point. A system that pretends to be crisp on empty batteries is lying.

3. **Reflexes bypass reasoning.** The spinal cord doesn't ask the cortex for permission to pull away from a hot stove. Neither does PincherHook.

4. **Everything reports back.** Every execution produces a TelemetryFrame. No silent failures.

5. **The trait is the contract.** `NeuroMuscularInterface` is the only thing the CNS needs to know about. Everything else is implementation detail.

---

## 9. License

MIT. See `Cargo.toml`.
