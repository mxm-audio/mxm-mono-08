# NOTES.md — plugins/mxm-mono-08/

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples.
AGENTS.md is the contract; this file is the reference it links to.

## BASELINE-M0.md

**`BASELINE-M0.md` is the routing conversion's reference**: Init's and the fifty factory sounds'
digests through `render_block_for_test`, the plugin's own per-sample path, and the throughput the
cost gate compares. `lib.rs`'s `#[ignore]`d `baseline` module produces them and runs unchanged on the
converted tree. The seam is a measurement seam, not a second `process()`: `process()` renders every
sample through the same `render_sample`.

**The idle path is measured in the same module, and `BASELINE-M0.md` does not pin it.** The
throughput gate presses a key first, so what this instrument costs with nobody playing was measured
nowhere. `idle_throughput_of_a_parking_and_a_self_running_patch` prints Init, `Random sequence` and
`Mod oscillator drone` with each one's `Activity`: the first two park to the shell's own per-sample
work, while the drone reaches an open gate with no note held and renders the whole voice, which is
correct and not a defect — the case is there so the parked figure is read as a floor rather than as
what every patch costs. It answers the cost half of *doing nothing costs nothing*
(`../AGENTS.md`); the DSP crate's parking tests answer the other half by proving `Inert` is reached
at all.

**Its figures do not all share one provenance, and the file says which is which.** The bank and the
throughput were captured before any of the conversion (`plans/plan-mxm-mono-08-modulation.md` M0).
The eleven *inverter setting* digests were not: they were captured from the post-B4 tree — whose bank
is bit-identical to M0 — because no factory sound moves `inverterinput` off its default, so M0 is
blind to the entire inverter path and could not have recorded them. They exist to hold D2's §12
claim that each of the eleven settings still renders bit-identically once the selector becomes an
*Inverter input* destination.

## Permanent identity and state

- The 380 parameter IDs in `src/params.rs` and `src/routes.rs` are permanent. They cover every
  stateful panel control, all 164 weighted CV routes — **an amount and a presence each**, fifteen
  sources into ten destinations and fourteen into the inverter's input — and all 12 trigger enables
  (`pulse_*`), which are presence only, the one declared exception to decision 1.6 because a trigger
  has no level to scale. The current step, held/random state,
  tails, performance state and Once requests are runtime state, not parameters.
- **Retired ids, never to be reused**: `inverterinput`, an eleven-way source selector, for the
  *Inverter input* destination that replaced it (D2); and, with the external input (below),
  `preampgain`, the twenty `cv_*_follower` / `cv_*_followeron` routes, and `modmode` and
  `gate2source`. Those two lost one option each, and **a stepped id keeps its exact option list**
  (`plans/plan-modulation-routing.md` §5.0) because a stored position would select a different
  option, so `modtype` and `gate2input` are new ids rather than trimmed old ones.
  `the_ids_the_external_input_retired_stay_retired` holds all of them out. Old user presets and host
  automation on them are **not migrated** (D5, X1): a preset naming one reports it as unknown.
  The count ran 340, 369 (D2), 370 (`modhigh`), 349 (the external input), 350 (`pulsersync`),
  380 (the standard Amplitude's `cv_amplitude_*`, 2026-09-27).
- **The clock period has the collection's one tempo sync** (`pulsersync`, 2026-09-25;
  `plans/plan-tempo-sync-controls.md`): the quarter note beside Period on the Clock card, on
  `params::PULSER_SYNC` (1/64 to four bars, the top the longest). `resolve_topology` resolves it
  once a call, into `Configured`, from the tempo `process` stores in `host_tempo`; the
  period-shortening CV applies on top as on a free period, and `Telemetry::tempo` lets the knob read
  its division. The owner left the modulation oscillator's rate out of scope.
- **There is no external audio input** (the owner's ruling, 2026-09-23: *"I don't think the
  external input is an important feature. Let us just drop all of that."*). It replaces this file's
  earlier *"Do not remove that capability"*. The 208's preamp, envelope detector and balanced
  external mode, and the LPG 2 *External* source, are gone, with the two audio layouts that
  offered an auxiliary port. Bitwig gives a CLAP instrument its first layout, which had no input, so
  none of it was reachable there. **This is a declared deviation from the machine**; the brief's §9
  lists it with the other removals. The three factory sounds built on it were replaced (*Ring
  clang*, *Random bleeps*, *Inverter duck*).
- The preset identity is the only `#[persist]` field outside parameter values. Factory files are
  complete values over every parameter and are generated from the readable fifty-sound design.
  **`modhigh` is the one declared legacy default** (`preset.rs`): a file that does not name it was
  written for the one range there was, so it loads in the low range, silently, rather than keeping
  whichever range the instance was in.
  **A design names a depth and the generator wires the route that carries it**: present is Init's
  wiring together with every pair a design overrides to a non-zero amount, because an amount stored
  against an absent pair reaches nothing and the sound would load silent where it used to play.
- **Fifty presets have to be fifty sounds, and a test says so** (`bank_quality` in `lib.rs`,
  2026-09-23). The bank that shipped until then was fifty names on one patch: `master` and `mix1`
  identical in all fifty, `complexfreq` in forty-six, two presets rendering byte-identically,
  `Sparse steps` silent, and two to four routes each out of a hundred and fifty-nine. **Every
  preset test passed on it**, because each asks whether something is *present* — a category, an
  audible path, a pulse enable behind a random route — and none asked whether two sounds *differ*.
  The gate renders all fifty and compares a six-axis fingerprint (level, peak, brightness,
  flatness, attack, movement); the replaced bank scored 0.000 at its closest pair and this one
  0.125, against a floor of 0.10. **It proves distinct, audible and moving — never *good*.** The
  bank was authored against those measurements with no audition, by the owner's choice.
- **The five-stage sequencer is architecture, not a category.** Eighteen of the fifty run it, and
  they run it differently: advanced by the pulser, by the key and by its own stage pulse; its
  voltage reaching complex pitch, timbre, mod pitch, mod index, both gates, pulser period,
  portamento speed and the inverter; its stage pulses striking the envelope, redrawing the random
  voltages and re-firing the pulser. Eight of the eighteen are outside `Category::Sequence` — a
  bell whose stages detune the FM partner, a bass whose stages open LPG 1 — because a run that
  only appears in sounds named after it is the old bank's thinking in smaller form. **Every one
  that advances the sequencer routes it somewhere**; the replaced bank's `Stage length` advanced a
  run that reached no audio at all, which its own `BASELINE-M0.md` entry recorded as the bank's
  weakest.
- **A gate in VCF mode never closes**, so a patch in that mode with a local level sounds forever and
  the envelope shapes only brightness. Designs that want a note with a shape use VCA + VCF or
  VCA. Two of the replaced bank's silences and several of its stuck drones were this.
- **Parameter text is idempotent through the host's normalised conversion** (format, parse, format
  gives the same text). A time chooses `ms` or `s` from its *rounded* milliseconds — chosen from the
  raw value, 0.9995 s printed `1000 ms` and read back `1.00 s` — and a bipolar percentage never
  prints `-0 %`, which read back `0 %` on every routing amount at the `i / 19` grid.
  `params::tests::every_parameter_text_is_idempotent_through_the_hosts_conversion` walks every
  parameter with the unit on, over clap-validator 0.4.1's own grid, the `i / 19` grid and both
  sides of every unit, precision and sign switch; a clean validator run does not prove it.

## Names

The owner's ruling, 2026-09-23: keep the Buchla's function and make its names logical and
coherent, in the words an east-coast synthesist already knows — sequencer and steps rather than
stages, trigger rather than pulse or pulser. *Mod oscillator* and *Complex oscillator* keep theirs,
because they are not ordinary oscillators. **Display names only**: every id, enum id and Rust
identifier stayed, which is why the code still says `pulser`, `stage` and `pulse` (glossary below).

- **One name per module, and it prefixes everything the module owns**: Complex oscillator
  (*Complex frequency, wave, wave mix, timbre, key tracking*), Mod oscillator (*Mod frequency,
  wave, type, depth, key tracking, high range*), LPG 1 and 2 — the low-pass gates, *LPG* on the
  cards and the mixer alike (the owner, 2026-09-24) — (*LPG N level*, *LPG N mode*, *LPG 2 input*),
  Envelope (*attack, hold, decay, mode*), Clock (*Clock period*, *Clock loop*), Sequencer
  (*Sequence length*, *Step N level*, *Step N trigger*), Random (*Random 1–4*), Inverter, the
  keyboard (*Key*) and Glide (*Glide time*, *Glide speed*). The mixer's channels are named for the
  gate they carry: *LPG 1 mix*, *LPG 2 mix*.
- **Three signal classes, three grammars.** Audio is chosen by a selector (*LPG 2 input*). A
  modulation route is *‹Destination› from ‹Source›* (*Complex timbre from Envelope*); a trigger
  route is *‹Module› trigger from ‹Trigger source›* (*Random trigger from Key*), so neither reads as
  the other — *Random from keyboard* read as a random voltage coming from the keyboard. *Pulse* is
  not in the interface.
- **A knob in hertz is a frequency; the destination moving it in octaves is a pitch** — *Complex
  frequency* and *Complex pitch*. Every other destination is the name of the parameter it adds to:
  *LPG 1 level*, *Clock period*, *Mod depth*.
- **Painted names drop the card's own prefix; canonical names never do** (design system §7.1; the
  owner, 2026-09-24: *"It is inside the Complex oscillator card, so it is self evident"*). Every
  control on a module's card paints the short form — knobs, selectors, pictures and switches alike
  (`sections::panel_label`): *Frequency*, *Wave mix*, *Attack*, *Mode*, *Period*, *Loop*, *Length*,
  *Level*; a stack paints `routing::DESTINATION_PANEL_NAMES` — *Pitch*, *Timbre*, *Level* — and a
  trigger switch *Trigger from Key*, Random's too. The mixer's knobs paint *LPG 1* and *LPG 2*. A
  short name may repeat across cards; a canonical name may not — it is what the host, the tooltip
  and a screen reader read — and `routing`'s tests hold source and destination names unique.
  *Glide time*, *Glide speed* and *Inverter input* keep their words, because the Voice card is not
  Glide's and the *Random and inverter* card holds two modules.
- **Sources are capitalised labels**, as every other instrument's are but `mxm-para-07`'s.
- **A shorter name is allowed where a card cannot hold the long one, if it is the name everywhere**
  (the owner, 2026-09-23). Never shorten one surface and not the others.
- **A selector's cells are the parameter's own option text** (`sections::selector`), so an option has
  one spelling. The options: Mod type *AM / FM*; LPG 2 input *Mod oscillator / LPG 1*; gate mode
  *VCA / VCF / VCA + VCF*; Envelope mode *One-shot / Sustained*; Sequence length *2 steps … 5 steps*.

| In the code | Displayed |
|---|---|
| `pulser`, `pulserperiod`, `pulsersync`, `pulserself`; `PulseDestination::Pulser` | Clock, *Clock period*, *Clock period sync*, *Clock loop* |
| `CvSource::Pulser` | *Clock ramp* |
| `PulseSource::{Keyboard, PulserEnd, SequenceStage}`, `pulse_*` | triggers from *Key*, *Clock*, *Step* |
| `stage`, `seqNlevel`, `seqNpulse` | step, *Step N level*, *Step N trigger* |
| `duration`, `envmode` `transient` | *Envelope hold*, *One-shot* |
| `portamento`, `CvDestination::PortamentoSpeed` | *Glide time*, *Glide speed* |
| `modindex`, `modtype` `am`/`fm`; `gate2input` (field `gate2_source`) | *Mod depth*, *Mod type* AM/FM; *LPG 2 input* |
| `complexfreq`, `complexendpoint`, `wavemix` | *Complex frequency*, *Complex wave*, *Complex wave mix* |
| gate mode `vca`/`lowpass`/`combination`; `CvDestination::Gate1` | VCA/VCF/VCA + VCF; *LPG 1 level* |
| `mix1`, `mix2`; `CvSource::Wheel`, `Bend` | *LPG 1 mix*, *LPG 2 mix*; *Mod wheel*, *Pitch bend* |

## Init and smoothing

- Init is the approved keyed articulation: the Key trigger fires the envelope, the envelope opens
  LPG 1 at `0.72` (*LPG 1 level from Envelope*), LPG 1's own level is closed, LPG 1 mix and
  Output are useful, and one complex oscillator sounds. That non-zero route is the explicit
  exception to zero modulation/CV amounts.
- **Init wires a second pair, and it is translation rather than sound**: *Inverter input* from
  Random 1 at full depth, which is the retired `inverterinput` selector's own shipped default
  expressed as a route (D2). Leaving it absent would make a fresh instance's inverter a constant
  `complement(0.0)` and silently change every patch that routes the inverter. It reaches audio only
  where a patch routes the inverter somewhere, which Init does not.
- **The unsmoothed half of the patch is read once per non-empty `process()` call**, not once per
  sample and no longer once per interval. It lives in `Configured`, refreshed in `resolve_topology`
  beside the topology, because `SAMPLE_ACCURATE_AUTOMATION` is off — the wrapper applies every
  parameter change before the call, so no selector, switch or state-machine time can move inside
  one. It is a cost device and changes no value (`plans/plan-mxm-mono-08-per-sample-cost.md`); a
  new unsmoothed parameter belongs in it, and a **smoothed** one never does, because its smoother
  owes one advance per sample. The factory digests cannot police this — every one of them fixes its
  patch before the first sample — so `an_unsmoothed_parameter_edit_reaches_the_sound_at_the_next_call`
  moves one mid-render.
- **Bend range is one of the smoothed values.** The channel stores the bender's position and the
  range scales it into pitch every sample, so an unsmoothed range edit under a held bend is a pitch
  step (`docs/code-review-notes.md` §2). `a_range_edit_under_a_held_bend_ramps_rather_than_steps`
  holds the ramp.

## A route is a presence and an amount, and what that costs elsewhere

- **An amount reaches audio only through a route that exists.** Init wires LPG 1 from the envelope
  and nothing else, so a stored depth on any other pair is silent until its presence is set. This is
  the instrument's central routing fact and the one that breaks things quietly: a factory file that
  carried only amounts loaded every sound unwired until the files were regenerated with presences.
- **A route steps in and out, and arrives at its stored depth.** Topology is resolved **once per
  non-empty `process()` call** — on the first rendered interval, so an empty callback still resolves
  nothing and the first interval's events are still drained first. It was once per interval, which
  meant once per host event offset at 423 ns each; a densely automated callback reached 3.3% of a
  core on rebuilds alone (`plans/handover-mxm-mono-08-event-density.md`). Nothing inside a call can
  move it: the wrapper applies parameters before `process()` and `handle_event` writes none.
  `the_topology_resolves_once_per_non_empty_call_whatever_the_event_density` counts resolutions,
  because resolving unchanged parameters twice is identical by construction and no render
  comparison could catch a regression. `Routes::topology_from` snaps each newly present route's
  smoother to its stored depth, because an absent route's smoother is never advanced while its
  amount stays editable. The step itself is the DSP's declared transition class
  (`crates/mxm-mono-08-dsp/AGENTS.md`). `a_route_steps_out_and_back_in_at_its_stored_depth` holds
  both halves: a removed pair leaves the live list at the next interval, and one edited while absent
  and re-added reads its stored depth on its first sample.
- **Old user presets are not migrated** (plan D5). The preset format has no hook for this, so a
  pre-conversion user preset loads onto Init's presences and every route but LPG 1 ← Envelope is
  silent. Named rather than handled: this instrument is unreleased and its factory bank is
  regenerated. A governing decision on preset migration would change this.
- **No `filter_state` migration is built** (collection-wide X1, which supersedes this plan's D4). An
  old host state keeps every parameter that still exists and loses what any retired id held.
- **The inverter's input is a destination, not a selector** (D2, delivered). `inverterinput` is
  retired and `cv_inverter_*` mints thirty ids in its place, so there are ten CV destinations. The
  tenth carries **fourteen** sources, not fifteen: the inverter is not offered to itself, and that
  refusal is stated twice — no parameter is minted for the pair, and `Graph::set_topology` clears it
  so no caller can wire it by accident either.
- **A retired id has to keep its sounds reachable, and this one is measured.** One route present at
  +100 % is bit-identical to what the selector computed, and
  `the_inverter_destination_reaches_every_retired_setting` renders all eleven of its settings
  against digests captured before the conversion (`BASELINE-M0.md`). A host *automation lane* on the
  retired id cannot be carried; X1 governs, and nothing is migrated.
- **The collection's modulation standard** (`plans/plan-modulation-standard.md`,
  `crates/mxm-mono-08-dsp/AGENTS.md`). Readings are the shared `mxm_modulation_params::reading`:
  pitch in semitones (it read octaves), the controls, gates, inverter and Amplitude in percent, Key
  per octave; the clock, glide and sequence keep their units. **Faders are linear**, except a
  network pitch pair's — five or eight octaves a unit — which keeps the square law; **a one-sided
  offer has only its live half** (LPG ← Velocity 0…+100 %; clock period, glide speed and inverter
  input ← Velocity −100…0 and ← Wheel 0…+100 %). **Amplitude** (`cv_amplitude_*`) is the standard
  factor on the mix, its stack on the Mixer card. The factory designs were re-expressed for the
  linear faders at the same plain amounts **to the bit** (a square-law position's `skewed`
  proportion is the linear position), the two Timbre ← Velocity designs moved their timbre base up
  by the amount (`v − 1`), and *Narrow lead*'s Complex pitch ← Bend rescaled to the standard's
  twelve semitones; the fifty-one-sound bank capture is identical, though its score plays full
  velocity and no lever, so it cannot see those three. The player golden's score sets its route at
  0.68 where it set 0.8, the same plain amount, and holds its digest. **The inverter's Key setting
  moved**: Key's new meaning is no longer the retired selector's `1 − note / 127`, and its digest was
  re-pinned (`INVERTER_DIGESTS`); the other nine hold. `every_route_parameter_says_what_the_dsp_does`
  holds every pair's registration, travel, unit and reading to the DSP's declaration.
- **Three panel switches stay switches** (D7). `complexkeyboard` and `modkeyboard` add 1 V/oct key
  tracking at a scale no Key route reproduces, and `pulserself` (*Clock loop*) duplicates the
  *Clock trigger from Clock* pair. The redundancy is recorded here rather than tidied away, because
  removing it would change what a stored patch means.

## The modulation oscillator's high range

The owner's ruling, 2026-09-23: *"We are mainly just inspired by it, but we try to catch the spirit
of it as much as possible. But I want that switch."* The original has one range; the switch is the
2013 reissue's (`crates/mxm-mono-08-dsp/AGENTS.md` has the evidence and the ratio). Here it is a
two-way `modhigh` switch — Low and High, as the owner described it; the reissue's third, reversed
position is not offered, and a switch cannot grow one, so adding it would be a new id.

- **The frequency keeps its stored range; the voice multiplies it.** `modfreq` still stores
  0.16–50 Hz, so every preset, the mapping table and the pitch routes keep their meaning, and
  `lfo1.rate` in the control map still moves the same control in either range.
- **The frequency's reading follows the switch** — `440.0 Hz` where Low reads `5.00 Hz`, the
  default in each, which is why the ratio is 88 (owner, 2026-09-23) — on the knob and in the host,
  and typed text is taken in the range showing. **It is the one reading here that depends on
  another parameter**: the switch's `with_callback` keeps an `AtomicBool` the frequency's formatter
  and parser read. The idempotence walk runs in both ranges for that reason.
- **The arrows step it in semitones and octaves only in High** (`sections::bound`): a ratio of the
  stored value is the same ratio of the reading, so `StepLaw::Hertz` on the stored value is exact.
  In Low it is a rate and keeps its own step, the owner's earlier exclusion.

## Host events and transient Once

- A bounded press ledger in the DSP chooses the sounding touch. Per-note tuning and pressure match
  channel/key/voice ID; tuning and channel bend hold through release, while per-note pressure ends
  with the touch. Channel pressure and bend follow the sounding touch's channel, including a
  same- or different-channel held-note fallback; channel updates do not overwrite a per-note value.
  **Velocity is a routing source and nothing else**: the last press's note-on velocity is held
  through its release — every press is a keyboard pulse — and the DSP publishes it as the
  standard's `v − 1`; it rests at full before any press and after a reset, so a Velocity route does
  nothing until a key is struck softer than full. It writes no parameter, and the machine still has
  no velocity sensitivity of its own. Zero velocity remains Note Off. Choke ends the named voice
  immediately while preserving the post-mix spring tail. No 29-key MIDI limit is imposed.
- **CC 1 is the Wheel routing source**, held per channel and read by whichever routes name it. It
  writes no parameter, which is exactly what the control map's CC 1 reservation allows.
  Developer CC 119/118/117/116 is active only when `MXM_DEV_CC` is set and never writes parameters;
  118 is consumed and does nothing, because this brief has no disclosure, and 116 applies a theme by
  index (0 light, 1 dark, 2 system) without saving it.
- **Once is a transient act, never a value.** `TransientActions` is a preallocated 64-entry atomic
  counter path. Each accepted activation is consumed once at the next process/event boundary;
  requests do not coalesce, overflow increments the rejection count, and sequentially consistent
  counters give cancellation and concurrent submission one total order. Every accepted editor act
  also schedules `EditorTask::WakeAudio`; the vendored CLAP wrapper turns that non-parameter task
  into `host.request_process`, so an inert host resumes. No parameter ID, preset, CLAP state,
  control-map role or editor open/close can fire it. The real-host test starts a child process with
  an immutable `MXM_MONO_08_TEST_ONCE_DIR`; one debug instance atomically claims `ready`, then an
  atomically created `fire` file submits through this exact editor path after the host sleeps. The
  worker is single-act and timeout-bounded; no process mutates its environment after startup.

## Audio and activity

- DSP `Live`, `Tailing`, `Inert` maps to `KeepAlive`, finite `Tail`, `Normal`. Activity follows the
  complete currently conductive graph: effective signed CV sums, refreshed held
  key/sequencer/random/inverter values, and enabled stage pulses inside the active sequence length.
  Holding a disconnected note or naming a positive route that another route cancels is not itself
  Live. A disconnected loop parks after finite settle; autonomous, pulser and pulse-derived sources
  remain Live only when some point in their effective range can open an audible path, including an
  interior overlap between serial gates and the joint box of all four simultaneously refreshed
  Random voltages. The conservative tail bound includes all three possible
  envelope segments and gate settling, then **adds** the spring's tail because its last excitation
  can arrive only after the dry interval. Dry Init reaches exact silence; spring-enabled patches keep
  a finite host tail. Waveform-ring writes are parked while no
  editor is open.
- Telemetry publishes max-combined peak, latched clip, sample rate, activity, complex-oscillator
  samples, all fifteen live CV source levels, all three trigger-source states, the envelope level,
  both gate levels, pulser period, stage/value/pulse, Once fired/cancelled/rejected counts and gated
  developer requests. While the editor is open, one-sample pulses OR-latch across the DSP segment
  and publish under the current monotonic editor generation, then drain once for the whole rendered
  Mod/Seq view. Generation-tagged latches make close/reopen atomic with publication: a delayed old
  event cannot become a stale flash or erase a new event. It is observational only.
- `activate` refuses a non-finite host rate or one below `mxm_mono_08_dsp::MIN_SAMPLE_RATE`, 1 kHz,
  before anything changes. The DSP clamps its rate to that floor, but a clamp passes NaN on and
  the gates' clamp then panicked on the audio thread; below the floor the host's rate and the
  DSP's disagreed. `activation_refuses_a_non_finite_rate_and_any_below_the_floor` and
  `the_rate_floor_activates_and_plays_at_every_parameter_extreme` hold both.

## Editor

- Eleven stable paging items, opening at the quarter-4K budget hugged (`REFERENCE`), minimum one
  widest card plus its gutters (`MINIMUM`, held by
  `the_one_card_minimum_holds_the_widest_declared_floor_plus_gutters`). Voice (key 0)
  is Performance; oscillators (1–2) are Generators; the low-pass gates and Mixer and reverb (3–5)
  are Tone; Envelope, Clock and Random and inverter (6–8) are Modulators; Sequencer (9) and Steps
  (10) are Sequencers. Mixed-purpose Random/inverter and Mixer/reverb bodies remain indivisible
  under those primary categories.
  Oscillator/gate pairs, Envelope/Clock and Sequencer/Steps are the preferred
  groups while width and height permit. Full category names, no fixed tab count, no bar for one page.
  CC 119 uses the parent's category addresses; Parameters is 127, separate and absent from tabs.
  `editing_cards_fit_the_quarter_4k_content_budget_at_one_times_scale` checks every derived page;
  native/DPI inspection is still open.
- **Every card is a `mxm_ui::tree`** (`plans/plan-layout-tree.md`; `crates/ui/AGENTS.md`, *A
  card body as data*). `sections::card` describes a card's body once — knob columns, selectors,
  toggles, route stacks, the trigger groups, the displays — and that one description is measured
  for the card's floor and height and drawn leaf by leaf through the same bindings
  (`sections::paint`). The paged view is `paging::editor::show`. The plugin states only its
  own visuals' sizes (`visuals::WAVEFORM_HEIGHT`, `GATE_HEIGHT`, `PULSE_FLASH`, `MARKER`,
  `source_levels_size`); a route stack states its own (`stack_size`). **mxm-mono-08 is the pilot
  editor** (`mxm_ui::pilot`): the pilot is enabled at the top of `panel`, before any card is built,
  because a piloted rule may change a card's size. It piloted the route layout and the slider rule,
  which every editor has had since 2026-09-24, and nothing is piloted now.
- **The card count and the opening size move together, and the opening size is derived.** Merging
  Random, the inverter and the envelope detector into one card and the five steps into one lowered the
  tallest card, which is what `the_opening_size_is_the_budget_hugged` measures: the panel opens with
  every card but Mixer and reverb on one page. Do not hand-edit `REFERENCE` — change the cards, run
  that test, and take the number it prints. The Steps card's height feeds the same loop: with the
  length and route controls still in it, a full-height fader made that card too tall for any page it
  could share, the opening page lost it, and the window derived from what was left collapsed — which
  then cut the sequencer off. Splitting the card is what fixed the window, not shortening the faders.
- The app bar owns presets, **Output** (`master`), the post-master meter and latched clip
  acknowledgement, fixed 75–200% user zoom, and the shared `mxm_ui::shell::editor_theme_control`,
  which switches immediately and remembers the choice in the collection's
  `<config>/mxm/editor.json`. Opening theme comes from `mxm_ui::theme::preference`. None of theme,
  zoom, view or layout is plugin or preset state.
- **Output is the app bar's, never a card's** (owner, 2026-09-18: every instrument's master volume
  is in the app bar; design system §3.1). It is an inline slider beside the meter
  (`Bound::slider_inline`, reserving the widest reading so the bar does not move under a drag),
  drawn inside `mxm_ui::navigation::bar_card` under key `MASTER_CARD` (64, outside the paging keys
  0–16), and the cursor reaches it through `paged_with_bar`. The app bar is on every view, so the
  developer Parameters list is the one surface that draws Output twice.
- The musician surface follows the voice and oscillator sources through both low-pass gates,
  opposite-polarity mixing and the spring reverb. The first card, `Voice`, holds the glide and bend
  controls. **LPG 1 mix, LPG 2 mix and Reverb share the final `Mixer and reverb` card**, in that
  order, with the standard Amplitude's route stack under them — it acts on the mix before the
  spring; there is no separate reverb card, and Output is in the app bar.
  **Routing is a list, not a picker.** Each CV destination draws
  `mxm_modulation_params::ui::stack`: one row per route the patch actually carries — its signed
  depth and a remove — with the sources it does not yet carry offered beneath. A destination with
  nothing routed draws no border at all, and the rows come from **parameter values alone**, so a
  preset determines the whole panel and there is no editor-side selection to restore, invalidate or
  get out of step. Adding or removing a route is one parameter write; removing leaves the depth
  behind, so putting the source back restores it.
- **No live bar on a route row** — the owner's ruling, 2026-09-16. `visuals::source_levels` already
  shows every source's current value on the Modulators cards and each row shows its own depth
  moving, so a per-route meter repeated what the panel says elsewhere and spent a row of height
  doing it. The bar and the drag hazard it carried retired together with the picker.
- **Trigger destinations show their three enables directly** (plan D6), each with its flash: a
  trigger has no depth, so presence is the whole of it and three switches are fewer controls than a
  picker plus the one switch it revealed. The flash stays because a trigger is a one-sample event,
  visible nowhere else, and it is always allocated so enabling the switch moves nothing.
- Each destination is preceded by `SPACE_3`, which its own `SPACE_2` rhythm must stay under —
  design system §4.1. **A card may grow past the window when routes are added, and the player
  resizes** (the owner's ruling, 2026-09-16); design system §4.2 still forbids shrinking type to
  avoid that. Modulators holds three producer/processor cards — Envelope, Clock and **Random and
  inverter** — rather than nine route cards plus four trigger cards; oscillator, gate and Glide
  speed destinations stay on their owning cards, and Sequence length and the Sequencer's triggers
  live on the Sequencer card. LPG 2 input is the two-way Mod oscillator / LPG 1 switch.
- **Random and inverter is one card because neither part is one.** Random is three trigger
  switches and the inverter one route stack — a card each meant two borders around one cluster of
  controls apiece. They are nested groups, told apart by their own control names: §3.3 gives a
  group no header, so none was invented, and the card's title names both.
- **The sequencer is two cards and the run is one of them**, where it was five step cards plus a
  header. `Steps` holds five step columns and nothing else — a position marker, a full-height
  vertical fader and its own trigger switch each — because everything else that shared the card
  competed with the faders for height, and **the faders are a main feature rather than a detail**
  (the owner, 2026-09-23). `Sequencer` holds what drives the run: length, the routes into it and the
  triggers that advance it. Keep them adjacent; the wrap that separates them puts the run on its own
  page and undoes the split. The five steps stay separately editable,
  and every readout uses its own stored level while telemetry supplies only active/fired state.
  **The marker distinguishes idle, active and triggering without hue**: empty outline, solid fill,
  and a filled dot inside the solid fill. A column says only its step number, and its trigger
  switch is a **picture of one trigger pulse** (`Wave::Trigger`, `binding::toggle_picture`) rather
  than the word *Trigger*, which made the run much wider than it had to be (the owner,
  2026-09-24). `Step 3 level` and `Step 3 trigger` remain the canonical names in AccessKit, the
  tooltip and host automation; `mxm_ui::control::toggle_wave` and `ParamView::labelled` are what
  allow that.
- **Every card is drawn at its floor, and the floor is computed** (the owner, 2026-09-24: *"so
  everything pack tight"*). A card's ceiling is its floor, so no card grows past what its content
  needs; the displays, meters and live readings take the width the card has rather than setting
  it. **No help text on a card** (the owner, 2026-09-27; design system §7.6): the only captions are
  the clock's live period and the Once counts, which `no_card_prints_help_text` holds. What the
  eight card captions explained is the tooltip of the control it is about, written for the player —
  LPG Level (brighter and louder together), Random's triggers (four new levels each), the step
  triggers (a step without one still plays its level); Voice's note on sources, the inverter's
  formula, frequency ranges and polarities went (2026-09-27).
  `page_items` computes each floor every frame from the card's tree — its narrowest plus the card's
  chrome — so there is no number to re-measure after a content change. What sets a floor is what
  cannot shrink: a route slider's minimum track with its source and its widest reading (every route
  revealed), selector cells, knob rows. A knob row is `ui.columns` capped at
  `Σ max(diameter + SPACE_5, KNOB_COLUMN_MIN)` as data — the collection's `mxm_ui::tree::knob_row`
  since every editor took mono-08's column (2026-09-24): its columns grow to that cap and shrink
  below it only as far as the widest knob allows.
  `proof::every_card_passes_the_tree_checks_in_every_state` runs the shared checks
  (`mxm_plugin_test::tree_checks`) over every card at Init, with every route revealed at full
  negative depth, and with the Once queue full.
- **The Envelope card's source meters are one row per source**, the name unwrapped in caption type
  and the bar beside it. As three columns under a hugging card, a column was narrower than *Mod
  oscillator* and the names broke mid-word and re-wrapped as the bars moved (the owner: *"text is
  dancing"*).
- **Nothing is drawn to learn a size**, so there is no measuring pass to guard: pulse sources and
  sequence firing are still read once, before the frame, because reading them is destructive. The
  trees are rebuilt every frame, so a route added, a preset loaded or the queue-full line appearing
  re-plans the pages at once. Leaf widget ids are salted with their parameter ids and survive
  re-paging.
- The complete `Parameters` view lays out in one, two or three columns and scrolls. The in-process
  proof covers private defaults, every parameter in AccessKit, essential visual labels, exact
  begin/set/end gestures for every control kind across derived pages and diagnostics, reset, text
  cancellation, every CV source choice beneath every destination in both themes at narrow/default
  widths, the two mix knobs and Reverb together on one card with Output drawn once in the app bar,
  above every card and inside the window, and editable there as one gesture,
  external updates, both themes, narrow/default/wide geometry, honest floors, clipping and overlap,
  aligned rows, stable order, parallel groups and every card held at its floor. Player coverage
  proves the bundle advertises a floating editor and deliberately opens, closes and reopens its
  native window.

## The keyboard cursor

The parent's *The keyboard cursor runs in every editor* owns the contract. One thing is local.

**The coverage check is `Coverage::Exactly`, and it runs in two frames** — the init patch, and every
route present (`the_keyboard_cursor_reaches_and_operates_every_route_revealed`), the same shape
`mxm-mono-03` uses. Two frames because **an absent route is not drawn**: a single frame at the init
patch would reach one route of a hundred and sixty-four. The expected set is built from the patch
rather than from the declared surface — the panel's own ids, plus the amount and presence of each
live pair — so `Exactly` stays strict in both frames instead of being loosened to accommodate the
routes that are not there. `master` is one of the panel's own ids and registers from its bar card on
every page; the app bar is drawn before the cards, so a cursor that has never landed starts on
Output.

**The pitch routes step by semitones and octaves** (owner, 2026-09-23: *"A lot of the pitch
sliders in the mono 08 does not jump octaves and semitones with the keys. Even though they use
those as display values."*). Complex pitch and Mod pitch read in semitones, so `sections::routes`
draws them through `mxm_modulation_params::ui::stack_with_law` with `StepLaw::Interval` at
`routes::reach` — the reading's own reach, a network pair's or the standard's, Key's per octave: a
press lands on the next whole semitone (left/right) or octave (up/down). The other destinations keep
their own step, as does every other instrument's route. The bend reach is `Semitones`, Complex pitch
`Hertz`, and the modulation frequency `Hertz` in its high range only, declared in `sections::bound`.

**The step faders step a semitone coarse and 1 % fine** (owner, the same day: *"so when this is
set to 1 octave, there is a 1:1 on the pitches"*, and then that an octave coarse step is not
meaningful when the whole range is one octave). A step's level is a voltage, not a pitch, and its
reading stays a percentage; but a pitch route reading `+12.00 st` makes the whole fader one octave,
because the sum scales linearly into octaves. So `StepLaw::Voltage { octaves_per_unit: 1.0, fine:
0.01 }` takes a coarse press (up/down) to the next twelfth — twelve semitones span the fader — and a
fine press (left/right) exactly 1 %. **Coarse always lands on a whole semitone**: 2 % up and then
coarse up is one semitone, not one and 2 %. Only a coarse press lands on the grid: the stored level
stays continuous, and fine, a drag or a host reaches anything between, which is what keeps the
sequencer a voltage source rather than a quantiser. `tests/pitch_route_keys.rs` drives both through
the shipped panel.

## The mapping table

**Choose a design's numbers from `the_mapping_table`, not from a hunch.** It is `#[ignore]`d in
`preset.rs` and prints what every normalised value means in Hz, ms and mode names.
`docs/adding-an-instrument.md` has always said every instrument has this; mono-08 did not, and
fifty designs were written blind as a result.

## Verification: the fit check and the host suite

**The quarter-4K fit check runs by default.**
`editing_cards_fit_the_quarter_4k_content_budget_at_one_times_scale` renders the real panel through
`paging_checks::verify` at `REFERENCE`, at the quarter-4K content size and at `MINIMUM`, and
requires every derived page to fit; it passed on 2026-09-15. Routing panels still require
`2 * SPACE_3` vertical padding, and their always-allocated live bars and pulse flashes cannot
collapse at rest, so added rows make a card taller. This is a headless proof, not native DPI or the
owner's look, and design system §4.2 still forbids shrinking typography or pointer floors to make a
page fit.

The host suite covers the real bundle, its 350-parameter surface, keyed sound, parameter delivery,
five-stage sequencing, event-offset splitting, activity/tails, debug allocation assertions, hostile
rates and legal callback sizes, and the pinned real-host render. Design-system §15 visual review,
real-DAW parenting/resizing, Linux and macOS remain manual because this repository has no CI.
