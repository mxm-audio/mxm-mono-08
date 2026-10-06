# AGENTS.md — crates/mxm-mono-08-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

The original 1973-card-set Model 208/218 voice as framework-free Rust: complex and modulation
oscillators, envelope, pulser, four held random voltages, five-stage CV/pulse sequencer, weighted CV
and pulse routing, two 208 low-pass gates and a mono post-mix spring.

- **No external-audio path** (preamp, envelope detector, balanced external modulation): the owner's
  ruling, 2026-09-23, a declared deviation. The voice takes no input at all.
- Fidelity is **UNVERIFIED**: this repository has measured no original. The instrument is inspired by
  it, not bound to it; the modulation oscillator's high range is the one deliberate departure.
- Research: `research:instruments/buchla-music-easel.md` and its cited pages.
  [NOTES.md § The voice](NOTES.md#the-voice-its-source-and-its-one-deviation).

# Ownership

Owns `Cargo.toml`, `src/` and `examples/`. It owns DSP state, plain-value controls, realtime
processing, measurements and the render demo. `plugins/mxm-mono-08` owns host events, parameter
ranges/smoothing, the bounded transient Once delivery path, presets, telemetry and the editor.

# Local Contracts

## The network has three signal classes and one fixed order

Audio, continuous/held CV and pulses are distinct. The fixed per-sample order is:

1. accept host touch plus pulser-end pulses queued by the preceding sample;
2. combine enabled pulse routes; advance the sequencer and expose its enabled stage pulse;
3. apply that forward stage pulse to random, envelope and pulser triggers;
4. update performance and held sequencer/random state;
5. run modulation oscillator, envelope and pulser; a pulser end queues its pulse for the next sample;
6. form the weighted destination sums, using the preceding sample for a source not yet produced in
   this order. **The inverter's input is one of those destinations** (D2): its sum is formed and
   complemented before the destinations that read the inverter, so a route through it costs no
   extra sample of delay;
7. run the separate complex-oscillator cores and AM or FM, then Gate 1, Gate 2, the
   opposite-polarity mix and mono spring.

Thus sequencer voltage changes pulser period in the same sample, while every backward connection has
one sample of delay. `routing::Graph` over the shared [`mxm-modulation`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/mxm-modulation/AGENTS.md)
frame, and the event-order tests, are the policy; object call order is not accidental.

## Routes, not a grid

Detail and measurements: [NOTES.md § Routes](NOTES.md#routes-not-a-grid-presence-decides-and-the-frames-unit-is-one).

- Routes (`routing::Routing`) travel **beside** the patch, never inside it. `Graph::set_topology`
  compacts the live list once per processing interval; **an absent pair contributes nothing whatever
  its amount holds**, and the activity predicate must ask presence before depth.
- **Fifteen sources, eleven destinations.** Nothing persisted keys on a source's number (route ids
  are strings). The complex oscillator's audio is produced at the end of the sample, so every route
  from it is backward by one.
- **The tenth destination is the inverter's input** (D2): summed, then complemented
  (`Graph::inverter`). `INVERTER_IS_NOT_ITS_OWN_SOURCE` is refused in the engine and the parameter
  surface (it would be an undamped one-sample feedback oscillator). One route at `+1.0` is
  bit-identical to the retired `inverterinput` selector.
- **The unit is one.** The original unipolar CVs (all but Key) clamp to `0…1` at their write sites;
  the complex audio source peaks at exactly one (`easel_measure`).
- **Each destination scales after the sum**, in declared source order, within this instrument's
  `±4` bound: `a_full_grid_sums_exactly_as_the_dense_grid_did` holds the bit-identity.
- **Pulses are presence and an OR**, with no depth. The OR lives here, not in the shared crate.
- **Every route steps**: a presence change takes effect whole at the next interval, with no
  crossfade. The plugin snaps a newly present route's smoother; this crate has no per-route state.
- **Every source is published unconditionally**, so none is owed a `clear` on a topology change.
  Gating publication later would owe that clear, and the test that proves it.
- **`Routing` carries its own compacted list** (`compact`, once per interval) so the shell advances
  only the smoothers of routes that exist. Do not replace it with a per-sample walk over every grid
  position: that measured slower than the dense grid.

## The modulation standard

Sources and added routes mean what they mean on every instrument (`mxm_modulation::standard`).
Detail: [NOTES.md § The modulation standard](NOTES.md#the-modulation-standard).

- **Key is `(glided note − 60) / 60`** (`KEY_UNIT_SEMITONES`), bipolar about middle C, with glide,
  tuning and bend in it. **Velocity is `v − 1`** of the last press; Pressure, the wheel and the lever
  go through their standard publishers.
- **Reach.** Machine pairs and the complex audio keep each destination's network reach; a
  performance source the 208 did not have takes the standard reach (`takes_standard_reach`) through
  `mxm_modulation::sum_split`, bit-identical while none is live
  (`a_pitch_sum_is_the_networks_until_an_added_pair_is_live`).
- **Offers** (`routing::offer`): one-sided where the destination clamps
  (`the_one_sided_offers_are_the_standards`); every machine pair keeps both halves.
- **Amplitude is the eleventh destination**: `standard::amplitude_factor` on the mix, before the
  `±4` bound and the spring. It cannot open a closed gate. Nothing routes to it at Init.
- `conformance.rs` runs the standard's checks, each falsified once. **Release runs at
  `MIN_SAMPLE_RATE`.**

## Five stages means five stored stages

The sequencer always owns five levels and pulse enables. Active length is clamped to 2–5; shortening
it never erases stages. An advance always changes the held voltage and only independently decides
whether to emit the selected stage pulse. Off holds state. **Chosen unknowns:** power-up is stage 1;
a length reduction wraps on the next advance when the current stage is out of range; coincident
pulse sources coalesce into one edge at a sample boundary; a stage pulse lasts one sample.

## Warts are deliberate

| Established behaviour | Implementation and guarding test |
|---|---|
| Two complex-oscillator cores, not one phase | `oscillator::ComplexOscillator`; `the_complex_cores_are_separate_state` |
| Optical gate memory couples brightness and level | `lpg::LowPassGate`; `combination_mode_loses_brightness_faster_than_vca` |
| Gate 2 reaches the mixer inverted | `voice`; `equal_gate_paths_cancel_in_the_opposite_polarity_mixer` |
| Low-rate monitored modulation-oscillator audio deforms without losing its audio-rate level | route-local low shelf plus AC coupling in `ModulationOscillator::monitored_audio`; `monitored_square_tilts_while_direct_cv_holds` and `monitored_route_preserves_audio_rate_level_while_deforming_low_rate_plateaus` |
| Gate 2 modulation-oscillator audio is AC coupled, direct CV is not | `DcBlocker` only on monitored audio; `direct_cv_keeps_dc_while_the_audio_route_rejects_it` |
| Pulser self mode needs an initiating event | `control::Pulser`; `self_mode_does_not_start_itself` |
| Sequence voltage and pulse enable are independent | `control::Sequencer`; `disabled_pulse_does_not_skip_the_voltage_stage` |
| Sequence voltage shortens the pulser clock that advances it | fixed order in `Voice`; `feedback_rhythm_changes_period_without_a_block_delay` |
| Inverter is full-scale minus the **sum** of its inputs | `routing::complement` and `Graph::inverter`; `the_inverter_is_a_unipolar_complement`, `one_route_at_full_depth_is_exactly_the_retired_selectors_complement` |
| The inverter cannot be its own input | `INVERTER_IS_NOT_ITS_OWN_SOURCE`, cleared in `Graph::set_topology` and minted nowhere; `the_inverter_cannot_be_its_own_input` |

## A memoised coefficient is a cost device, never a numeric one

Transcendental per-sample coefficients (LPG followers and cutoff, `lpg::Pole`, `oscillator::OnePole`,
`ComplexOscillator::process_factor`) are wrapped in `Memo`. A new memoised site must keep:

- **A hit returns what the expression itself produced from that key.** No approximation behind a
  memo (no polynomial, table, or `exp2(v log2 b)` for `powf(b, v)`): output stays bit-identical.
- **Key on everything the result depends on**, `fs` included (`Pole`'s key is `(hz, fs)`).
- **A direction-selected coefficient keeps one slot per direction.**
- A memo is pure derived state; clearing or keeping it across `reset` is unobservable.

Guarded by `lpg::tests::memoisation_is_bit_identical_to_recomputing` and
`tests::a_memo_is_transparent_for_every_key_including_the_hostile_ones`; the bank digests alone
cannot. Why: [NOTES.md § Memoised coefficients](NOTES.md#memoised-coefficients-why-and-where).

## Chosen and derived constants

All implementation values absent from hardware evidence remain replaceable; each, with its
measurement or reason, is in [NOTES.md § Chosen and derived constants](NOTES.md#chosen-and-derived-constants).
`Params::mod_high` multiplies the clamped panel rate by **`MOD_HIGH_RATIO`**, a ratio rather than
the reissue's span, so a semitone on the control stays a semitone in the sound; Low multiplies by
exactly one, so low-range renders stay bit-identical
(`the_high_range_runs_the_modulation_oscillator_at_the_ratio_times_its_panel_rate`).

## Numeric and realtime contract

Full text: [NOTES.md § Numeric and realtime contract](NOTES.md#numeric-and-realtime-contract-in-full).

- `[dependencies]` is exactly `mxm-modulation` (dependency-free at the 1.87 floor). `mxm-measure`,
  `mxm-audio-file`, `mxm-audio-file-decode` and `mxm-modulation`'s `conformance` feature are
  dev-only; this crate's `conformance` feature forwards it for the plugin's tests. None reaches a bundle.
- Per-sample methods allocate and lock nothing; delay memory is sized by `set_sample_rate`.
- Audio is `f32`; recursive coefficients and linear portamento accumulation are `f64`. Every
  external plain value is clamped at its consumer; recursive state flushes before denormals;
  `voice::OUTPUT_BOUND` is the asserted output bound.
- Reset and All Sound Off clear pulses, control state, gates and spring without resizing delay
  memory; an inert voice returns exact zero without advancing cores.
- `set_sample_rate` clamps to `MIN_SAMPLE_RATE` (1 kHz)–768 kHz; a clamp passes NaN on, so the
  plugin refuses a non-finite rate or one below the floor at activation.
- **Live** only when the complete, currently conductive CV/pulse graph reaches an open gate, a
  present carrier and a raised mixer path, judged on effective signed sums and effective positive
  intervals through serial gates; one Random trigger's four voltages are one joint box. A held note
  or an individually positive route alone is not reachability. Stage-derived pulse paths conduct only
  through an enabled stage inside the active length. Parameter-owned held values refresh before the
  inert shortcut. Disconnected and closed-gate loops park after finite settle.
- One-sample pulse telemetry OR-latches until the plugin drains it; the signal pulse is not lengthened.
- The sounding press's identity is exposed only for channel-owned bend/pressure. A targeted choke
  closes the keyed envelope/gates and leaves the spring to decay. Randomness and reset are
  deterministic.

# Work Guidance

- **Do not depend on or extract a sibling DSP crate.** The shared
  [`crates/mxm-modulation`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/mxm-modulation/AGENTS.md) crate owns source-frame semantics that were
  extracted in part from this crate’s former `CvFrame`.
- Facts and our own rewritten prose may cross from research; third-party files and transcriptions may
  not. Cite machine facts with their full `research:` repository-relative path.
- Keep hardware topology distinct from chosen calibration. A self-consistency test is not a hardware
  fidelity claim.

# Verification

```bash
cargo test -p mxm-mono-08-dsp
cargo clippy -p mxm-mono-08-dsp --all-targets
cargo run -p mxm-mono-08-dsp --release --example easel_measure # prints provisional model measurements
cargo run -p mxm-mono-08-dsp --release --example easel_demo    # writes mxm-mono-08-easel-demo.wav
cargo fmt --all -- --check
```

Measurements come from `mxm-measure` (dev-only); every bound and its headroom stays in the test that
argues for it. What the tests cover: [NOTES.md § Measurement](NOTES.md#measurement-and-test-coverage).

# Child DOX Index

No child AGENTS.md files. `src/` and `examples/` are covered here.
