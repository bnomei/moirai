# Sea of Grass → Wyrd behavior and continuation parity

**Status:** migration contract; source-audited 2026-07-12
**Scope:** the sixteen behavior names frozen in
[`PHASE_7_INTEGRATIONS.md`](../PHASE_7_INTEGRATIONS.md), plus the continuation state that those
immediate cases do not cover

This document is the deletion gate for the evaluator in
`sea-of-grass/src/wiring.rs`. It records what the current tests actually prove, the replacement
Wyrd fixture shape, and which parts remain Sea of Grass host policy. A similarly named test is not
enough: replacement evidence must reproduce the ordered state/event trace below or explicitly
approve a documented behavior change.

## Source basis

The authoritative local sources are:

- `../../sea-of-grass/src/wiring.rs:312-477`: persisted node, actuator, delay, and step state;
- `../../sea-of-grass/src/wiring.rs:505-715`: graph, target, budget, and stable-topology validation;
- `../../sea-of-grass/src/wiring.rs:729-1024`: sample → edge → delay → combine → apply semantics;
- `../../sea-of-grass/src/wiring.rs:1044-1090`: `PostAction` installation and the explicit
  `before(portal_travel_system)` constraint;
- `../../sea-of-grass/src/wiring.rs:1156-1997`: the sixteen named tests;
- `../../sea-of-grass/src/schedule.rs:221-244`: host lane order and action-step gating;
- `../../sea-of-grass/docs/feat_trigger_effect_wiring.md:20-35`: one stable-id-ordered settle per
  successful action step, boolean level/pulse semantics, DAG-only topology, and stable authored ids;
- `../../sea-of-grass/src/game/save.rs:475-487` and
  `../../sea-of-grass/src/game/level_state.rs:1454-1496`: strict baseline-delta wiring projection;
- `../../wyrd/crates/wyrd-for-games/src/runtime_impl/bind.rs:70-123`: current Wyrd continuation
  fields;
- `../../wyrd/crates/wyrd-for-games/src/runtime_impl/loom.rs:18-61`: current loom order and
  `OnStart` behavior;
- `../../wyrd/crates/wyrd-for-games/src/runtime_impl/loom.rs:112-251`: And, Or, edge, Flag,
  Counter, Timer, and Delay behavior;
- `../../wyrd/crates/wyrd-for-games/src/authoring/error.rs:17-89`: typed cycle and required-input
  diagnostics.

The Wyrd fixture names below are proposed parity fixtures, not claims that those exact fixtures
already exist. Wyrd has matching primitives and nearby tests, but it currently has no public
`RuntimeState` snapshot/restore API. That absence is a hard blocker for deleting the Sea of Grass
runtime.

## Trace model and ownership

One legacy settle with a non-empty graph and one player executes in this order:

1. increment `WiringState.step` with saturation;
2. snapshot each actuator's `prev_level`;
3. sample sensors in authored node order and emit `WiringSensorEdge` for changed levels;
4. collect and remove delayed pulses whose `fire_step <= step`;
5. evaluate actuator inputs in stable topological order, with inbound links sorted by link id;
6. apply actuators in authored actuator order;
7. emit `WorldEdited` during each barrier-cell mutation, then emit one
   `WiringActuatorApplied` for the actuator;
8. clear `pending_use`;
9. after `settle_wiring` returns, apply queued PortalArm levels to matching portal components;
10. only then may portal travel run.

Door opening changes `open_door_cells`; it does not replace the `Door` tile and does not emit
`WorldEdited`. Barrier lowering changes each tile to `Air`, records its previous tile, and emits
`WorldEdited` once per changed cell. `WiringActuatorApplied` is emitted on every settle, even when
the effective level did not change.

Fixture notation:

```text
Sense(x)       = KnotKind::signal_in(SignalDomain::Bool)
Rise           = KnotKind::RisingFromZero
ToggleFlag     = KnotKind::flag(..., enable_toggle = true), wired through toggle
SetFlag        = KnotKind::flag(..., enable_toggle = false), wired through set
And2 / Or2     = explicit Wyrd fan-in knots
Delay(n)       = KnotKind::Delay { ticks: n }
Out(path)      = KnotKind::signal_out(path, SignalDomain::Bool)
```

`Sense(use.*)` is a one-frame host interaction level. Occupancy senses are held levels. A Wyrd
fixture owns graph evaluation and stateful knot semantics. Sea of Grass integration owns sampling
`WorldMap`, applying host effects, host ids, schedule position, and the host save envelope. Signal
values here are Wyrd Bool `ZERO`/`ONE`; Moirai `Q16` is not involved.

## Sixteen behavior cases

### 1. `validate_rejects_cycle`

**Exact legacy setup/stimulus.** Nodes are `a: Lever` at `[0,0,0]` and `b: DoorGate` at
`[1,0,0]`, both latching. Level/Pass links are `l1: a → b` and `l2: b → a`. Call
`validate_and_topo` once.

**Ordered expected trace.** Id, link, fan, and dead-end checks pass. Kahn sorting finds no complete
topological order and returns `"wiring graph has a cycle (DAG required)"`. There is no runtime
mutation or event. The current assertion only checks `is_err`, but the replacement must check the
cycle category so an unrelated rejection cannot satisfy it.

**Wyrd fixture mapping.** `sog_cycle_rejected`: construct two port-valid knots, such as two `Not`
knots, with `a.out → b.in` and `b.out → a.in`; assert
`ValidationError::Cycle { at_knot: ... }`. Do not reproduce the legacy actuator-to-sensor link
shape because Wyrd has typed ports.

**Owner.** Pure Wyrd authoring validation.

**Persistence relevance.** None directly. Invalid topology must never produce a snapshot-compatible
runtime.

### 2. `lever_opens_door_latch`

**Exact legacy setup/stimulus.** A Lever tile/node at `[0,1,0]` feeds a latching DoorGate at
`[2,1,0]` through one Level/Pass link. `note_use([0,1,0])` is queued. The test does **not** call
`settle_wiring`: it calls `sample_sensor` directly, manually writes the lever and door runtime bits,
and manually inserts the door cell into `open_door_cells`.

**Ordered expected trace.** `sample_sensor` sees the pending use, toggles
`lever.latched false → true`, and returns `true`. The test assigns `lever.level = true`, then assigns
`door.level = door.latched = true`, then inserts `[2,1,0]` into the open-door overlay. Walkability is
`true` with that overlay and `false` without it. `to_save`/`from_save` preserves the open cell and
lever latch. `step` remains `0`; pending use is not serialized; no sensor, actuator, or world-edit
message is emitted.

**Wyrd fixture mapping.** Split the misleading legacy test into:

```text
Sense(use.lever) → Rise → ToggleFlag(lever)
ToggleFlag(lever).out → SetFlag(door).set → Out(door.open)
```

The pure fixture proves one rising use toggles the lever and permanently sets the door flag. A host
fixture samples the use, applies `door.open = ONE` to the open-door overlay, proves walkability, and
round-trips both Wyrd runtime state and the host overlay.

**Owner.** Wyrd owns the two flags and edge history; Sea of Grass integration owns interaction
sampling, door walkability, effect application, and the save envelope.

**Persistence relevance.** Direct. Preserve both flag states, `prev_in` edge/toggle history, the
driver's next settle tick, and the open-door host state. Do not copy the legacy test's illegal
mid-step save point; target snapshots are allowed only after Apply completes.

### 3. `settle_and_requires_both_levers`

**Exact legacy setup/stimulus.** Lever nodes `a` and `b` at `[0,1,0]` and `[1,1,0]` feed a
latching DoorGate at `[2,1,0]` through two Level/And links. The player is at `[3,1,0]`. Queue use of
`a`, update once, queue use of `b`, and update again. The test redundantly forces `a.latched = true`
before the second update.

**Ordered expected trace.** On settle 1 (`step = 1`), `a` rises and emits
`SensorEdge(a, rose=true)`; `b` remains false; And yields false; the door remains closed and emits
`ActuatorApplied(door, false)`. On settle 2 (`step = 2`), `a` remains true without an edge, `b`
rises and emits `SensorEdge(b, true)`; And yields true; the door sets `level = latched = true`, the
open-door overlay gains `[2,1,0]`, and `ActuatorApplied(door, true)` follows. No `WorldEdited` is
emitted.

**Wyrd fixture mapping.** `sog_two_lever_and_latch`:

```text
Sense(use.a) → Rise → ToggleFlag(a) ┐
                                      And2 → SetFlag(door) → Out(door.open)
Sense(use.b) → Rise → ToggleFlag(b) ┘
```

Drive frames `[(a=1,b=0), (a=0,b=1)]`; expected outputs are `[0,1]`. The host fixture additionally
asserts the ordered sensor/apply trace and door overlay.

**Owner.** Pure Wyrd for edge/toggle/And/latch semantics; host integration for sampling and apply.

**Persistence relevance.** Direct. A save after only `a` is on must restore that partial solution;
the next `b` use must open the door exactly once without retriggering `a`.

### 4. `settle_or_either_lever_opens_door`

**Exact legacy setup/stimulus.** The same two lever and latching-door layout as case 3, but both
links use Or. Queue only `a` and update once.

**Ordered expected trace.** At `step = 1`, `a` toggles/rises and emits
`SensorEdge(a, true)`; `b` stays false; Or yields true; the door latches true, `[2,1,0]` becomes open,
and `ActuatorApplied(door, true)` is emitted. There is no `WorldEdited`.

**Wyrd fixture mapping.** `sog_either_lever_or_latch` replaces `And2` in case 3 with `Or2`. Add
frames for neither, `a` only, `b` only, and both; each non-empty case must set the door flag.

**Owner.** Pure Wyrd plus host apply.

**Persistence relevance.** Direct for the lever/door flags, though the named immediate test itself
does not save.

### 5. `settle_plate_weight_barrel_opens_door`

**Exact legacy setup/stimulus.** The test first writes a PressurePlate at `[1,1,0]` and then replaces
that same tile with `Barrel`; `ContainerSave` is empty. A PressurePlate node still names that cell
and feeds a latching DoorGate at `[3,1,0]` through Level/Pass. The player is at `[4,1,0]`. Update
once.

**Ordered expected trace.** At `step = 1`, host sampling checks player occupancy, saved containers,
then fixture tiles. The `Barrel` tile makes the plate level true and emits
`SensorEdge(plate, true)`. The door sets/latches true, `[3,1,0]` enters `open_door_cells`, then
`ActuatorApplied(door, true)` is emitted.

**Wyrd fixture mapping.** Pure topology is
`Sense(plate.occupied) → SetFlag(door) → Out(door.open)`. The required integration fixture must
derive `plate.occupied = ONE` independently for (a) player, (b) chest/barrel in `ContainerSave`, and
(c) a chest/barrel fixture tile, because Wyrd neither knows `WorldMap` nor decides mass policy.

**Owner.** Host integration is authoritative; the Wyrd graph is trivial.

**Persistence relevance.** Direct for the door latch/overlay and relevant to held-sense/edge
continuation when a save occurs while the plate remains occupied.

### 6. `settle_crystal_lowers_barrier`

**Exact legacy setup/stimulus.** A Crystal tile/node at `[0,1,0]` feeds a non-latching Barrier whose
cells `[2,1,0]` and `[3,1,0]` are Stone. The link is **Level/Pass**, not Pulse. Queue one use of the
crystal and update once.

**Ordered expected trace.** At `step = 1`, use toggles `crystal.latched` true; the sensor rises and
emits `SensorEdge(crystal, true)`. The Level link drives Barrier true. Apply visits the primary cell
then the extra cell: for each, save `Stone` in `barrier_rest`, write `Air`, add the cell to
`lowered_barrier_cells`, and emit `WorldEdited(cell)`. Only after both cell messages does it emit
`ActuatorApplied(barrier, true)`.

**Wyrd fixture mapping.** Correct the Phase 7 shorthand from “pulse edge” to toggled level:

```text
Sense(use.crystal) → Rise → ToggleFlag(crystal) → Out(barrier.lowered)
```

The host fixture asserts the two ordered cell edits, final actuator message, remembered previous
tiles, and idempotence on a later settle.

**Owner.** Wyrd owns the use edge and toggle flag; host integration owns cell mutations and their
ordering.

**Persistence relevance.** Direct. Preserve the crystal flag plus
`lowered_barrier_cells`/`barrier_rest`; otherwise restore cannot raise the barrier to its original
tile later.

### 7. `delayed_pulse_does_not_and_with_live_false`

**Exact legacy setup/stimulus.** A Button at `[0,1,0]` feeds a latching DoorGate at `[2,1,0]` using
one Pulse link with `delay_ticks = 2` and **CombineOp::Pass**. Queue one button use and run three
updates. Despite its name and comment, the fixture has no And fan-in.

**Ordered expected trace.** Settle 1 (`step = 1`) samples button true and emits
`SensorEdge(btn, true)`. The rising pulse is queued as `(fire_step=3, link=l1)`; delayed pulse links
contribute no live false input; the door emits `ActuatorApplied(door, false)`. Settle 2 samples the
released button, emits `SensorEdge(btn, false)`, leaves the queued pulse pending, and again emits
`ActuatorApplied(door, false)`. Settle 3 removes `l1` from the delay queue as due, contributes true,
latches/opens the door, and emits `ActuatorApplied(door, true)`.

**Wyrd fixture mapping.** `sog_delayed_button_latch`:

```text
Sense(use.button) → Rise → Delay(2) → SetFlag(door) → Out(door.open)
```

Drive `[ONE, ZERO, ZERO]`; output must be `[ZERO, ZERO, ONE]`. Add a distinct multi-input fixture
if “delay plus live-false And fan-in” is truly required; this legacy test does not prove it.

**Owner.** Pure Wyrd owns edge, delay, and flag timing; host integration owns the door apply trace.

**Persistence relevance.** Critical. Save after settle 1 or 2 and prove the pulse fires on the same
future settle after restore. The immediate three-frame test cannot substitute for that continuation
test.

### 8. `delayed_pulse_roundtrip`

**Exact legacy setup/stimulus.** Build the same Button → Door Pulse/Pass graph with delay 2, then
manually set `state.step = 5` and append `(7, "l1")` to the pending delay vector. Call `to_save`,
`from_save`, and assert only `step == 5` and the same tuple.

**Ordered expected trace.** Serialization deterministically sorts pending entries by fire step then
link id. Restore revalidates the authored graph, reconstructs node slots, restores step 5, and
recreates `[(7,"l1")]`. No loom/settle, door output, event, or actual continuation is tested.

**Wyrd fixture mapping.** `sog_delay_state_roundtrip` must snapshot a real bound
`Rise → Delay(2) → SetFlag` runtime after the pulse has entered the ring, bind a fresh runtime from
the same Weave, restore, and compare future outputs. Wyrd's state shape is a delay buffer plus
per-knot head/length, not Sea of Grass's pending `(fire_step, link_id)` vector; behavioral
equivalence, not field-shape imitation, is the contract.

**Owner.** Pure Wyrd owns runtime snapshot/restore; Sea of Grass owns serialization of the Wyrd
state inside its level delta and the driver's next settle tick.

**Persistence relevance.** This is the central persistence case. The current Wyrd API cannot yet
pass it.

### 9. `portal_arm_activates_on_settle_before_travel`

**Exact legacy setup/stimulus.** A Lever at `[1,1,0]` feeds PortalArm `arm` at `[2,1,0]` through
Level/Pass; the arm targets portal id `exit`. Queue lever use. Spawn an inactive Passage portal
entity with id `exit`, add only the World and Wiring plugins, and update once.

**Ordered expected trace.** At `step = 1`, lever rises, then arm resolves true. Apply queues
`("exit", true)` and emits `ActuatorApplied(arm, true)`. Pending uses are cleared. After settle
returns, the system finds the portal entity and sets `PortalActive(true)`. The test then observes
that component.

**Evidence limitation.** The test deliberately does **not** install or exercise
`portal_travel_system`; therefore it proves activation, not apply-before-travel behavior. The order
claim currently rests on `.before(portal_travel_system)` in `WiringPlugin`.

**Wyrd fixture mapping.** Pure graph:
`Sense(use.lever) → Rise → ToggleFlag(lever) → Out(portal.exit.active)`. The replacement host test
must install actual portal travel (or an explicit trace sentinel) and assert the ordered trace
`Wyrd sample → loom → apply PortalActive(true) → portal travel observes true` in one action step.

**Owner.** Host integration; Wyrd only produces the level.

**Persistence relevance.** Direct for lever state and host portal-active state. Schedule order
itself is not persisted.

### 10. `plate_graph_validates`

**Exact legacy setup/stimulus.** Validate the shared graph containing PressurePlate `[1,1,0]`,
latching DoorGate `[2,1,0]`, and one Level/Pass link. The assertion checks only `Ok`, not the returned
order.

**Ordered expected trace.** Validation accepts ids, kinds, single fan-in/out, and acyclicity; stable
topology is plate then door. There is no runtime mutation or event.

**Wyrd fixture mapping.** `sog_plate_door_accepts`: build
`Sense(plate.occupied) → SetFlag(door) → Out(door.open)` and require successful validation/bind. If
the host compiler retains the legacy authored shape, its translation must also prove stable node and
path binding.

**Owner.** Pure Wyrd for Weave validity; host compiler for source-schema translation.

**Persistence relevance.** Indirect. The accepted topology and stable bindings determine the
snapshot fingerprint.

### 11. `combine_mismatch_rejected`

**Exact legacy setup/stimulus.** Lever `a` reaches the door with Combine And while lever `b` reaches
the same door with Combine Or. Both are Level links. Call `validate_and_topo`.

**Ordered expected trace.** While scanning links in authored order, `l1` records And for the door;
`l2` finds Or and returns a diagnostic containing `"mismatched combine"` before topo sorting. No
state or event is produced.

**Wyrd fixture mapping.** Wyrd represents fan-in as an explicit `And` or `Or` knot and permits only
one thread per input, so this mismatch is not representable in a valid native Weave. A Sea of Grass
legacy-authoring compiler must reject mixed per-link combine policies before emitting an explicit
gate. After authored content migrates to native Wyrd, a compile-fail fixture should prove that two
threads to one input yield `ValidationError::FanIn`, while explicit gates remain unambiguous.

**Owner.** Host authoring/import integration, not pure runtime Wyrd.

**Persistence relevance.** None directly; rejected authoring must not receive a topology
fingerprint.

### 12. `dead_actuator_rejected`

**Exact legacy setup/stimulus.** One latching DoorGate node at `[0,0,0]`, no links. Call
`validate_and_topo`.

**Ordered expected trace.** Node/id validation passes; the actuator has fan-in zero and validation
returns a diagnostic containing `"no inbound"`. No runtime mutation/event occurs.

**Wyrd fixture mapping.** `sog_dead_output_rejected`: build a `SignalOut("door.open")` with its
required `in` port unwired and assert
`ValidationError::UnconnectedRequired { knot_id, port }`. If a host actuator mapping is declared
without a matching Wyrd path, the binding resolver must separately reject that mapping.

**Owner.** Pure Wyrd for the unwired output; host integration for unused/missing host bindings.

**Persistence relevance.** None directly.

### 13. `portal_id_missing_rejected`

**Exact legacy setup/stimulus.** Lever `lever` feeds PortalArm `arm` by Level/Pass. The arm carries
the non-empty target string `"missing"`, so `validate_and_topo` succeeds. Then call
`validate_wiring_portal_targets` with an empty portal-id set.

**Ordered expected trace.** Graph validation succeeds first. Host target resolution then returns a
diagnostic containing `"unknown portal"`. No state/event occurs.

**Wyrd fixture mapping.** The Wyrd path `portal.missing.active` is a valid open host string and
cannot prove that a Sea of Grass portal exists. `SogWyrdBinding::bind` must resolve every declared
portal output against the active level's stable portal ids and reject the whole binding atomically.

**Owner.** Host integration only.

**Persistence relevance.** Indirect. Restoring against a level whose stable target manifest changed
must fail before applying runtime or host state.

### 14. `tile_walkable_for_save_honors_open_doors`

**Exact legacy setup/stimulus.** Start with default `WiringSave`, add `[3,1,0]` to
`open_door_cells`, then query a Door at `[3,1,0]` and another Door at `[4,1,0]`.

**Ordered expected trace.** The listed door is walkable. The unlisted door falls through to normal
tile walkability and is not walkable. The save and world are not mutated; there are no events.

**Wyrd fixture mapping.** None in pure Wyrd. The Sea of Grass host save-view helper must continue to
consult restored open-door effect state while validating player/portal placement. A Wyrd Bool output
does not replace this host-world rule.

**Owner.** Host integration only.

**Persistence relevance.** Direct. Open-door host state must be available during save validation,
including before a Wyrd runtime is installed into Moirai.

### 15. `expand_structure_wiring_prefixes_ids`

**Exact legacy setup/stimulus.** A structure-local Lever uses marker `lever` at offset `[0,0,0]`; a
DoorGate uses marker `door` at `[2,0,0]`; link `l1` connects them. Expand instance `inst_a` at origin
`[10,20,0]` with identity rotation.

**Ordered expected trace.** Marker resolution produces nodes `inst_a/lever` at `[10,20,0]` and
`inst_a/door` at `[12,20,0]`; the link id becomes `inst_a/l1` and endpoints become
`inst_a/lever → inst_a/door`. The expanded graph validates. Portal target ids, if any, are
intentionally host-level ids and are not instance-prefixed by this function.

**Wyrd fixture mapping.** A Sea of Grass structure compiler must deterministically namespace Wyrd
knot ids and binding paths/keys per structure instance, then emit a native Weave or Pattern. Test
identity rotation plus at least one non-identity rotation, extra cells, missing markers, duplicate
local ids, and two instances of the same template.

**Owner.** Host authoring integration. Wyrd validates the compiled result but does not know world
markers or placement transforms.

**Persistence relevance.** High but indirect. Namespacing and full topology identity must be stable
across runs so a snapshot cannot drift onto a different structure instance.

### 16. `e2e_app_lever_opens_door_and_saves`

**Exact legacy setup/stimulus.** Build a complete Bevy app with configured game sets, WorldPlugin,
and WiringPlugin. Install Lever `[1,1,0]`, Door `[3,1,0]`, dirt path `[2,1,0]`, one Level/Pass
latching graph, a queued lever use, and a player on the dirt. Run one update, query runtime/world
state, then `to_save`/`from_save`.

**Ordered expected trace.** In `PostAction` at `step = 1`, lever rises and emits
`SensorEdge(lever, true)`; the door sets/latches true, gains `[3,1,0]` in the open overlay, and emits
`ActuatorApplied(door, true)`. Host walkability reports the still-`Door` tile open. Save projection
contains the authored graph, sorted node state, open door, and step; restore rebuilds/validates the
graph and retains the open door and lever latch. The current test does not consume the messages,
does not continue execution after restore, and does not assert door latch or restored step.

**Wyrd fixture mapping.** `sog_moirai_wyrd_lever_door_save_restore` is the full downstream gate:
install the Wyrd-owned atomic Moirai driver as a resource, run one host action step, assert the
ordered observation trace, capture Wyrd `RuntimeState` plus last-completed/next settle ticks and Sea
of Grass effect state, bind a fresh driver, restore atomically, rebuild the host world, and continue through
at least an off frame and another lever use. Compare against an uninterrupted control run.

**Owner.** Sea of Grass host integration, using Wyrd state/persistence and Moirai scheduling.

**Persistence relevance.** Critical end-to-end gate. A serialization-only roundtrip is insufficient;
continued behavior must match.

## Continuation matrix

### Required snapshot contract

Current Wyrd `Runtime` stores continuation in `sense_values`, `prev_in`, `prev_dec`, `counter`,
`flag`, `timer_left`, `on_start_done`, `delay_buf`, `delay_head`, `tick`, and `rng`. Some arrays also
carry multiplexed state: for example Random stores its last sample in `counter`, and several edge,
toggle, threshold, and gate behaviors share `prev_in`. `begin_frame` clears the outbox but does not
clear `sense_values`.

`RuntimeState` therefore needs, at minimum:

- a format version;
- the full runtime layout/topology fingerprint, including knot ids/kinds/constants/ports/threads
  and dense state layout—not merely the SignalIn/SignalOut manifest;
- the numeric-path tag;
- held sense values and all prior-edge/decrement values;
- counter/Random sample, flag/threshold, timer, and OnStart state;
- delay storage, per-knot lengths/offsets/heads, or an equally complete validated encoding;
- RNG stream state plus an optional begun-frame/runtime tick and continuation cursor;
- lengths and invariants sufficient to validate the entire payload before mutation.

The ephemeral outbox, runtime-owner-specific handles, and borrowed binding data are excluded. A
driver snapshot is legal only after Apply completes and before its next `begin_frame`. The adapter
envelope stores both `last_completed_tick: Option<SettleTick>` and `next_settle_tick`; Sea of Grass
stores host domain state such as open doors, lowered barriers/rest tiles, and portal activity.

Restore validates the runtime state and envelope as one unit. A fresh state is exactly
`last_completed_tick = None`, `next_settle_tick = 0`, and no begun-frame tick. After successful Apply
at `n`, RuntimeState's frame tick and `last_completed_tick` both equal `n`, while
`next_settle_tick == n.checked_add(1)`. An individually valid RuntimeState and individually valid
next tick with the wrong cross-field relationship is corruption, not a state to normalize. Reject it
before mutating the fresh runtime, driver, or host effects. Faulted/mid-step drivers are not
snapshot-capable.

For every positive row below, run an uninterrupted control and a split run:

```text
bind A → drive prefix → Apply → snapshot
bind fresh B from the same topology → restore → drive suffix
```

Compare every suffix settle's SettleTick, ordered SignalOut samples, ordered emits/payloads/drop
count, host apply trace, and final snapshot. Restore failures must leave runtime B and the host
envelope bit-for-bit unchanged.

| Case | Exact prefix / snapshot point | Exact suffix and expected continuation | State proved | Owner |
| --- | --- | --- | --- | --- |
| Latched flag | `Sense(use)=ONE` at settle 0 drives `Rise → SetFlag`; Apply sees output ONE; snapshot after Apply | drive ZERO for two settles; output remains ONE with no new rise; then exercise an explicit reset if the fixture has one | `flag`, `prev_in`, held output semantics, next settle tick | Pure Wyrd + driver envelope |
| Mid-delay pulse | `Sense(use)=[ONE,ZERO]` through `Rise → Delay(3)` at settles 0 and 1; snapshot after settle 1 Apply while the pulse is inside the ring | drive ZERO at settles 2 and 3; output is ZERO then ONE at exactly settle 3, and ZERO after | full `delay_buf`, length/offset/head, edge state, tick | Pure Wyrd |
| Held sense and rising edge | write Sense ONE at settle 0, observe one rise, snapshot; deliberately do not call `set_sense` for settle 1 | restored held Sense remains ONE and Rise outputs ZERO; write ZERO, then ONE, and observe exactly one new rise | `sense_values` plus `prev_in`; proves restore does not invent a release/rise | Pure Wyrd + binding policy |
| PulseHold timer | start `Timer(PulseHold,3)` at settle 0, release at settle 1, snapshot after settle 1 while one active tick remains | output is active for the same remaining settle count and then false; it must not restart | `timer_left`, timer edge `prev_in`, tick | Pure Wyrd |
| FedCountdown timer | feed `Timer(FedCountdown,4)` for two settles and snapshot before completion | continued feed completes after exactly two more feeds; a dropped feed resets exactly as uninterrupted | `timer_left`, feed edge/history | Pure Wyrd |
| `OnStart` | `OnStart → EmitCommand("boot")`; settle 0 emits once; snapshot after Apply | fresh restored runtime at settle 1 emits nothing; later settles also emit nothing | `on_start_done`; outbox is not restored/replayed | Pure Wyrd |
| RNG stream | seeded ungated Random runs two settles; snapshot after second Apply | next N samples exactly equal uninterrupted values in order | `rng`, last sample storage, numeric representation | Pure Wyrd |
| Gated RNG | rising gate samples once, keep gate held ONE, snapshot | held gate does not resample; after ZERO then ONE, exactly one next stream value appears | `rng`, cached sample in `counter`, gate `prev_in`, held sense | Pure Wyrd |
| Driver tick | snapshot any successful case after SettleTick `n` Apply | restored driver's next call uses `n+1`; any sample/loom/apply error leaves next tick unchanged and sticky-faults driver plus App | RuntimeState frame tick plus adapter last/next ticks and fault envelope | Wyrd-owned Moirai adapter |
| Driver-envelope mismatch | start from a valid fresh or post-Apply snapshot; independently alter last-completed, next-settle, or RuntimeState frame tick to another in-range value | reject before Runtime/driver/host mutation; never infer, clamp, or skip/repeat a settle | cross-field phase/tick invariant | Wyrd-owned Moirai adapter |
| Topology mismatch | snapshot weave A; bind weave B with the **same external endpoint manifest** but a changed internal constant, knot kind, thread, delay length, or stable id | restore returns `TopologyMismatch`; no field in B or host state changes | full topology/layout fingerprint, not endpoint-only identity | Pure Wyrd |
| Structure-instance mismatch | snapshot compiled `inst_a/...`; attempt restore into otherwise identical `inst_b/...` | reject atomically before binding/application | stable host ids included in topology/binding identity | Host compiler + Wyrd restore |
| Numeric mismatch | serialize a snapshot from the f32 path and attempt restore in i32-Q16 (and reverse) using cross-feature fixtures | return `NumericPathMismatch`; no numeric conversion or partial restore | numeric-path tag | Pure Wyrd, feature-matrix test |
| Version corruption | mutate format version to unknown older/newer values | return `UnsupportedVersion` before reading mutable payload | format-version gate | Pure Wyrd |
| Length/index corruption | truncate/extend a state vector; corrupt delay offset/length/head; use an impossible knot/state count | return a precise corruption/invariant error; runtime remains unchanged | complete preflight validation | Pure Wyrd |
| RNG/state corruption | encode forbidden RNG zero if the implementation requires non-zero; corrupt Bool/Count/Level domain values or a cursor | reject before mutation rather than normalizing silently | semantic invariant validation | Pure Wyrd |

## Save-schema mapping

Sea of Grass's strict baseline-delta save currently keeps the authored graph in the resolved level
baseline and stores only dynamic wiring state in `WiringDeltaSave`: node runtimes, open doors,
lowered barriers, original barrier tiles, step, and delayed pulses. That ownership split should
remain:

```text
authored baseline
  Wyrd Weave / Recipe identity
  stable structure and portal binding manifest

level delta
  versioned Wyrd RuntimeState
  WyrdDriver last_completed_tick, next_settle_tick, and snapshot-valid phase
  Sea of Grass host effects (open doors, lowered/rest barriers, portal state as applicable)
  Sea of Grass binding/domain state not owned by Wyrd
```

The current full `WiringState::from_save` silently ignores unknown saved node ids, while the newer
strict delta path explicitly rejects unknown nodes before calling it. Neither path proves full
topology identity or validates every delayed link reference. The Wyrd replacement must not preserve
that tolerance: validate version, topology, numeric path, state lengths, delayed storage, and host
targets completely, then apply runtime and host state as one transaction.

## Migration corrections and open decisions

These are evidence gaps, not optional polish:

1. Rename or split `lever_opens_door_latch`; it currently manufactures the result instead of
   settling the graph.
2. Keep the corrected Phase 7 crystal contract as “use edge → toggled Flag level → barrier,” never
   the old “pulse edge” shorthand.
3. Keep the delayed fixture source-faithful: the existing test uses one Pass link and does not cover
   And fan-in. Add an actual delayed-plus-live And fixture only if that interaction is a migration
   requirement.
4. Replace the portal ordering test with one that actually runs travel and records Apply before the
   travel observation.
5. Replace `delayed_pulse_roundtrip`'s hand-authored tuple with uninterrupted-vs-restored execution.
6. Assert message/observation ordering in host replacements; most current tests inspect only final
   state even though sensor and actuator messages are documented replay/debug output.
7. Decide explicitly whether `WiringActuatorApplied` remains every-settle output or becomes
   change-only output. Wyrd `SignalOut` naturally yields one sample per loom; preserving every-settle
   application is the least surprising migration.
8. Preserve action-step gating: render-only/environment-only Moirai updates must not call the Wyrd
   driver or advance SettleTick.

## Deletion gate

The old evaluator can be deleted only when:

- all sixteen cases above have named, owned replacement fixtures;
- every pure Wyrd fixture passes on every supported Wyrd numeric path;
- every host-owned fixture passes through the Wyrd-owned atomic Moirai driver;
- the actual portal-travel trace proves Apply-before-travel;
- uninterrupted and restore-continuation traces pass every positive continuation row;
- all mismatch/corruption rows reject atomically;
- mixed RuntimeState/last-completed/next-settle snapshots reject without skipping or repeating time;
- Sea of Grass strict save validation can inspect restored host walkability before installation;
- two structure instances cannot exchange snapshots accidentally;
- skipped non-action updates do not advance the driver;
- the Bevy wiring evaluator is removed only after the path-dependency migration, full suite, and
  save/load continuation suite are green.
