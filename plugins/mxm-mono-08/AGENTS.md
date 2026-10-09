# AGENTS.md — plugins/mxm-mono-08/

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The CLAP shell for `mxm-mono-08`: permanent host identity and parameters, MIDI/performance
translation, state, factory content, bounded realtime processing, telemetry and the production
editor over [`mxm-mono-08-dsp`](../../crates/mxm-mono-08-dsp/AGENTS.md). The history, measurements
and rulings behind each rule below are in [NOTES.md](NOTES.md).

# Ownership

Owns `BASELINE-M0.md`, `Cargo.toml`, `README.md`, `control-map.json`, `presets/` and `src/` (its
licence is the repository's root `LICENSE`). The DSP crate owns synthesis and the hardware-function evidence. `mxm-preset` owns preset
format/library/UI. MXM Player stays generic: its tests may name this product, its production code
may not special-case it.

**`BASELINE-M0.md` is the routing conversion's reference**, produced by `lib.rs`'s `#[ignore]`d
`baseline` module ([NOTES.md § BASELINE-M0.md](NOTES.md#baseline-m0md)).

# Local Contracts

## Permanent identity and state

- Product and bundle name: `mxm-mono-08`; CLAP ID: `dk.mxm.mxm-mono-08`. `plugin_name!` is the only
  product-name literal in Rust; `CLAP_ID` derives from it, never from the Cargo package name. Why
  each rule here: [NOTES.md § Permanent identity and state](NOTES.md#permanent-identity-and-state).
- **The 380 parameter IDs in `src/params.rs` and `src/routes.rs` are permanent**: every stateful
  panel control, all 164 CV routes (an amount and a presence each) and the 12 trigger enables
  (`pulse_*`, presence only: the declared exception to decision 1.6). Step, held/random state,
  tails, performance state and Once requests are runtime state, not parameters.
- **Retired ids, never reused**: `inverterinput`, `preampgain`, the twenty `cv_*_follower` /
  `cv_*_followeron` routes, `modmode`, `gate2source` (`the_ids_the_external_input_retired_stay_retired`).
  **A stepped id keeps its exact option list**; a changed list is a new id. Nothing is migrated (D5, X1).
- **One tempo sync**, on the clock period (`pulsersync`). **No external audio input** (owner, 2026-09-23).
- The preset identity is the only `#[persist]` field outside parameter values. Factory files are
  complete values over every parameter, generated from the readable fifty-sound design.
  **`modhigh` is the one declared legacy default**: a file that does not name it loads in Low.
  **A design names a depth and the generator wires the route that carries it.**
- **Fifty presets have to be fifty sounds** (`bank_quality`): distinct, audible and moving, never
  proven good. **Every design that advances the sequencer routes it somewhere**, in and outside
  `Category::Sequence`. **A gate in VCF mode never closes**: a shaped note uses VCA + VCF or VCA.
- **Parameter text is idempotent through the host's normalised conversion**
  (`params::tests::every_parameter_text_is_idempotent_through_the_hosts_conversion`).

## Names

- Display names only: every id, enum id and Rust identifier stays (`pulser`, `stage`, `pulse`). The
  rules in full, and the table from the code's words to the panel's: [NOTES.md § Names](NOTES.md#names).
- **One name per module, and it prefixes everything the module owns.** A modulation route is
  *‹Destination› from ‹Source›*, a trigger route *‹Module› trigger from ‹Trigger source›*, audio a
  selector. *Pulse* is not in the interface. A knob in hertz is a frequency, its octave destination a pitch.
- **Painted names drop the card's own prefix; canonical names never do** (`sections::panel_label`);
  a canonical name is unique. A shorter name must be the name on every surface.
- Sources are capitalised labels. A selector's cells are the parameter's own option text.

## Init and smoothing

- Init: the Key trigger fires the envelope, which opens LPG 1 at `0.72` (the explicit exception to
  zero CV amounts); one complex oscillator sounds. *Inverter input* from Random 1 at full depth is
  wired too (D2), as translation, not sound.
- The spring, other audio source levels and every other CV route start at zero. Envelope times,
  oscillator/routing configuration, stage values and bend range start useful.
- Values multiplied into or added to audio/CV are smoothed and each smoother advances exactly once
  per rendered sample. Times, selectors, topology switches and pulse enables configure state
  machines and are unsmoothed.
- **The unsmoothed half is read once per non-empty `process()` call** into `Configured`. A new
  unsmoothed parameter belongs there; a smoothed one never does
  (`an_unsmoothed_parameter_edit_reaches_the_sound_at_the_next_call`). Bend range is smoothed.

## Routes: a presence and an amount

- **An amount reaches audio only through a route that exists.** A factory file must carry presences
  ([NOTES.md § A route is a presence](NOTES.md#a-route-is-a-presence-and-an-amount-and-what-that-costs-elsewhere)).
- **Topology resolves once per non-empty `process()` call**
  (`the_topology_resolves_once_per_non_empty_call_whatever_the_event_density`);
  `Routes::topology_from` snaps a newly present route's smoother to its stored depth.
- **The inverter's input is a destination** (D2), never offered to itself (no parameter minted, and
  `Graph::set_topology` clears it). `the_inverter_destination_reaches_every_retired_setting`.
- **The modulation standard**: readings are `mxm_modulation_params::reading`; faders are linear
  except a network pitch pair's; a one-sided offer has only its live half
  (`every_route_parameter_says_what_the_dsp_does`).
- **Three panel switches stay switches** (D7): `complexkeyboard`, `modkeyboard`, `pulserself`.

## The modulation oscillator's high range

`modhigh` is a two-way switch; a third position would be a new id. `modfreq` keeps its stored range
and the voice multiplies it. Its reading follows the switch (an `AtomicBool` from `with_callback`),
so the idempotence walk runs in both ranges; the arrows step semitones and octaves only in High
([NOTES.md § High range](NOTES.md#the-modulation-oscillators-high-range)).
The switch is unsmoothed and lives in `Configured`; flipping it steps the rate, as the hardware
switch does.

## Host events and transient Once

- A bounded press ledger in the DSP chooses the sounding touch; channel updates never overwrite a
  per-note value. **Velocity and CC 1 (Wheel) are routing sources only** and write no parameter.
  Developer CC 119/118/117/116 acts only with `MXM_DEV_CC` set. Zero velocity is Note Off.
- All Notes Off releases every touch and does not cancel an accepted Once act. All Sound Off and
  `reset()` clear presses, control/audio tails, self-running state and every Once request ordered
  before their boundary; the next note restarts from key 69 rather than gliding from stale pitch.
- **Once is a transient act, never a value**: `TransientActions` (64 preallocated atomic entries),
  consumed once, never coalesced, waking the host through `EditorTask::WakeAudio`. No parameter,
  preset, state, control-map role or editor open/close can fire it
  ([NOTES.md § Host events](NOTES.md#host-events-and-transient-once)).

## Audio and activity

- Two audio layouts, stereo out and mono out, **and no input** (`layouts_are_stereo_and_mono_out_with_no_input`).
  Each carries one mono programme duplicated at the host boundary.
- `process()` splits at every host-event offset and at 64-frame internal boundaries. It allocates and
  locks nothing, reads no input, advances every smoother once per sample and publishes telemetry
  only through atomics/preallocated memory.
- `Live`/`Tailing`/`Inert` map to `KeepAlive`/finite `Tail`/`Normal`; the tail bound adds the
  spring's tail. Telemetry is observational. `activate` refuses a non-finite rate or one below
  `MIN_SAMPLE_RATE` ([NOTES.md § Audio and activity](NOTES.md#audio-and-activity)).

## Editor

- Eleven stable paging items (keys 0–10); Parameters is 127. Every card is a `mxm_ui::tree`
  described once in `sections::card`; the plugin states only its own visuals' sizes. mxm-mono-08 is
  the pilot editor (`mxm_ui::pilot`, enabled at the top of `panel`). Rulings and history:
  [NOTES.md § Editor](NOTES.md#editor).
- **Do not hand-edit `REFERENCE`**: change the cards, run `the_opening_size_is_the_budget_hugged`
  and take the number it prints.
- **Output is the app bar's, never a card's.** Theme, zoom, view and layout are not plugin state.
- **Routing is a list, not a picker**, drawn from parameter values alone; **no live bar on a route
  row**. Trigger destinations show their enables, each with an always-allocated flash. A card may
  grow past the window when routes are added; never shrink type. *Sequencer* and *Steps* stay two
  adjacent cards; the step marker shows idle, active and triggering without hue.
- **Every card is drawn at its computed floor**; **no help text on a card**
  (`no_card_prints_help_text`). `proof::every_card_passes_the_tree_checks_in_every_state`.
- Every durable edit uses a bracketed parameter gesture. `Fire once` alone uses
  `TransientActions`; full delivery is reported in the panel. Telemetry is observational, the
  waveform ring is filled only while the editor is open, and the editor asks for a frame every
  50 ms while open.

## The keyboard cursor

The parent's *The keyboard cursor runs in every editor* owns the contract
([NOTES.md § The keyboard cursor](NOTES.md#the-keyboard-cursor)). Locally: `Coverage::Exactly` in two
frames, Init and every route present (`the_keyboard_cursor_reaches_and_operates_every_route_revealed`).
Pitch routes step by semitones and octaves; step faders a semitone coarse and 1 % fine
(`tests/pitch_route_keys.rs`). **MICRO steps the pitch routes a cent and the step faders 0.1 %**
(`mxm_preset::StepLaw`). ← → snap to the next line of the size (2026-10-09): on a pitch route the
same whole semitones and octaves, on a step fader whole semitones coarse and the 1 % and 0.1 % lines.

## Content and scope

- The built-in mono spring is this instrument’s only effect. Do not add
  decay/tone/pre-delay/width/tank controls, another effect, or a standalone derivative of this spring.
- `control-map.json` is conservative: it claims only existing roles with matching meanings. A
  low-pass gate is not called a resonant filter; Complex timbre is not called pulse width; the routing
  and the sequencer remain accessible by parameters until the collection standard has honest roles.
- Product-facing labels, descriptions and preset names use no maker/model names. Internal evidence
  and fidelity decisions remain in the DSP and plan documents (the plans in the private archive).

# Work Guidance

- Keep editor changes within `docs/briefs/mxm-mono-08.md`: preserve `Synth`, `Mod`, `Seq`,
  `Parameters` order and the parameter/telemetry/action ownership boundaries.
- Regenerate presets only through
  `cargo test -p mxm-mono-08 --lib write_the_factory_presets -- --ignored`; the default tests prove
  files match the readable design exactly. A rename changes the filename the generator derives, and
  it **does not delete the old file** — remove the orphan and move `FACTORY_FILES` in the same step.
- **Choose a design's numbers from `the_mapping_table`** (`#[ignore]`d in `preset.rs`), not a hunch.
- Bundle before player tests. A debug bundle is required for `assert_process_allocs`; release alone
  does not prove realtime allocation safety.

# Verification

```bash
cargo test -p mxm-mono-08-dsp
cargo test -p mxm-mono-08 --lib
cargo xtask bundle mxm-mono-08
clap-validator validate target/bundled/mxm-mono-08.clap
cargo test -p mxm-mono-08-host-tests      # behaviour, robustness and golden_audio, through MXM Player
# Debug bundle; real CLAP host, explicitly armed editor-side Once producer:
cargo test -p mxm-mono-08-host-tests --test behaviour editor_once_wakes_a_sleeping_real_host_and_produces_one_burst -- --ignored --nocapture
cargo xtask bundle mxm-mono-08 --release
cargo test -p mxm-player --test t7_editor mxm_mono_08_advertises_a_floating_editor -- --nocapture   # in mxm-player
# In the mxm-player repository, on a Windows desktop; creates a native window twice:
cargo test -p mxm-player --test t7_editor mxm_mono_08s_editor_opens_and_reopens -- --ignored --nocapture
clap-validator validate target/bundled/mxm-mono-08.clap
cargo test -p mxm-mono-08-host-tests      # behaviour, robustness and golden_audio, through MXM Player
cargo clippy -p mxm-mono-08 --all-targets
# Every page, light and dark, for review -> target/layout-tree/mxm-mono-08/<MXM_PICTURES tag>/
MXM_PICTURES=after cargo test -p mxm-mono-08 --lib tree_pictures -- --ignored
cargo fmt --all -- --check
cargo test -p mxm-mono-08 --release --lib baseline -- --ignored --nocapture --test-threads=1   # BASELINE-M0.md and the idle cost; timings count only on a quiet machine
```

The quarter-4K fit check runs by default (headless; not native DPI). Design-system §15 visual review
and real-DAW parenting/resizing stay manual ([NOTES.md § Verification](NOTES.md#verification-the-fit-check-and-the-host-suite)).

# Child DOX Index

No child AGENTS.md files.
