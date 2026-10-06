# NOTES.md — crates/mxm-mono-08-dsp

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples.
AGENTS.md is the contract; this file is the reference it links to.

## The voice, its source and its one deviation

The original 1973-card-set Model 208/218 voice as framework-free Rust: the separate-core complex
oscillator, modulation oscillator, envelope, pulser, four held random voltages, five-stage
CV/pulse sequencer, weighted CV and pulse routing, two 208 low-pass gates and mono post-mix spring.
**The 208's external-audio path — preamp, envelope detector and balanced external modulation — is
not here** (the owner's ruling, 2026-09-23, a declared deviation): the plugin that hosts this voice
had no reachable input for it, so the voice takes no input at all. Research is
`research:instruments/buchla-music-easel.md` and its cited oscillator, LPG, control-source and
spring pages. Fidelity is **UNVERIFIED** because this repository has measured no original. **The
instrument is inspired by that original, not bound to it** (owner, 2026-09-23: *"we try to catch
the spirit of it as much as possible"*): the modulation oscillator's high range is the one
deliberate departure in the synthesis, and it is marked where it is chosen.

## Routes, not a grid: presence decides, and the frame's unit is one

The weighted network is declared as routes (`routing::Routing`) travelling **beside** the patch, never
inside it: presence and depth change on a parameter event, while `voice::Params` is rebuilt every
sample. `Graph::set_topology` compacts the live list once per processing interval and the per-sample
sums run over it, so **an absent pair contributes nothing whatever its amount holds** — which the
activity predicate must ask too, before it asks depth.

- **Fifteen sources.** The eleven original CV sources come first; velocity, the wheel, the bender's
  signed position and the complex oscillator's audio follow. The envelope detector, once the
  eleventh, left from the middle with the external input: nothing persisted keys on a source's
  number (route ids are strings), and `is_unipolar_cv` compares against `CvSource::Velocity` rather
  than a literal, so the order is free to close up. Display names are the collection's: *Wheel*
  and *Bend* (they were *Mod wheel* and *Pitch bend*; ids unchanged). The last is produced at
  the end of the sample, so **every** route from it is backward by one — including into the complex
  pitch and timbre that make it.
- **Eleven destinations: the tenth is the inverter's input** (D2), **the eleventh the standard
  Amplitude** (below). What the affine inverter reads is summed like anything else and then
  complemented (`Graph::inverter`) rather than scaled, which is why it is a destination and not a
  selector. It is the one destination that does not offer every source:
  `INVERTER_IS_NOT_ITS_OWN_SOURCE` is refused, in the engine as well as in the parameter surface,
  because routed into itself the inverter would read its own previous sample through `complement` —
  a one-sample feedback oscillator inside a control source, which nothing in this network damps. A
  single route present at `+1.0` is bit-identical to the `complement(read(source))` the retired
  `inverterinput` selector computed, since the shared sum is `0.0 + x * 1.0` and both operations are
  exact.
- **The unit is one, and the clamps are this instrument's.** The shared frame bounds what it holds to
  unit magnitude; the original unipolar control voltages — all but Key, now the standard's bipolar
  keyboard voltage — clamp themselves to `0…1` at their write sites, because that is a statement
  about the 208's voltages rather than about frames. The complex audio source needs no scale:
  `easel_measure` sweeps every endpoint, pitch, mix, timbre and modulation setting a player can
  reach and finds a peak of exactly one, with nothing above it.
- **Each destination scales after the sum**, so the sum itself is at unit scale with this
  instrument's own `±4` bound — two multiplies per route, in declared source order, which is the
  sequence this network executed before it had routing.
  `a_full_grid_sums_exactly_as_the_dense_grid_did` holds that bit-identity.
- **Pulses are presence and an OR**, with no depth: a pulse is a one-sample event with no level to
  scale, and the keyboard pulse route also holds the envelope's gate while a key is down, where an
  amount would mean nothing. The OR lives here rather than in the shared crate, which has no boolean
  law and should not gain one for a single consumer.
- **Every route steps** — this instrument's transition class under
  `plans/plan-modulation-routing.md` §4.1, for every destination, the tenth included. A presence
  change takes effect whole at the interval `set_topology` runs in, which is the start of the host's
  next buffer because the plugin's `SAMPLE_ACCURATE_AUTOMATION` is off: adding a route puts its
  whole stored depth into the sum from that interval's first sample, removing one takes it out, and
  nothing crossfades or waits for a zero crossing. The plugin snaps a newly present route's amount
  smoother to that depth (`plugins/mxm-mono-08/AGENTS.md`); this crate has no per-route state to
  fade.
- **Every source is published unconditionally**, so no source is owed a `clear` on a topology change.
  Gating publication later would owe that clear, and the test that proves it.
- **`Routing` carries its own compacted list**, built by `compact` once per interval, beside the
  per-destination lists `Graph` builds for the sums. It exists so the shell can advance only the
  smoothers of routes that exist: a per-sample walk over every grid position testing presence costs
  one branch per position to advance one smoother — 144 of them when that was measured at B2, 159
  since D2 added the tenth destination — and measured *slower* than the dense grid it replaced.
  That is where this conversion's saving actually comes from, and it is easy to lose by writing the
  obvious loop.

## The modulation standard

The collection's performance sources and added routes mean here what they mean on every
instrument (`mxm_modulation::standard`; `plans/plan-modulation-standard.md`).

- **Key is `(glided note − 60) / 60`** (`KEY_UNIT_SEMITONES`), bipolar about middle C, where it was
  `note / 127`. It keeps the glided pitch, tuning and bend in it, as the 208's keyboard voltage did.
  Five octaves of complex pitch per unit now track **one octave per octave** (it was 0.47), mod pitch
  1.6; the frame's unit bound holds Key at one above C9. **Velocity is `v − 1`** of the last press
  (every press is a keyboard pulse); **Pressure, the wheel and the lever** go through their standard
  publishers. The plugin's velocity rests at full before any press.
- **Reach.** The machine's pairs — the original sources into the original destinations — and the
  complex oscillator's audio (a generator: its depth is this network's FM, and a factory sound is
  built on two and a half octaves of it) keep each destination's network reach. **A performance
  source the 208 did not have** (Velocity, the wheel, the lever) takes the standard reach
  (`takes_standard_reach`), which differs only at the pitch destinations: twelve semitones rather
  than five or eight octaves, through `mxm_modulation::sum_split` (`Graph::pitch_octaves`), the
  network's instruction sequence to the bit while none is live
  (`a_pitch_sum_is_the_networks_until_an_added_pair_is_live`).
- **Offers** (`routing::offer`, the standard's criterion plus this instrument's own inverter
  refusal). The LPG levels are the machine's CV amplifiers: Velocity offers only its closing half.
  The clock period and glide speed read `max(Σ, 0)` and the inverter's input clamps to `0…1`, so
  each offers Velocity only its negative half and the wheel only its positive one
  (`the_one_sided_offers_are_the_standards`). Every machine pair keeps both halves.
- **Amplitude is the eleventh destination**: `standard::amplitude_factor` on the mix, before its
  `±4` bound and the spring — it cannot open a closed gate, so activity is unchanged. Nothing routes
  to it at Init.
- `conformance.rs` runs the standard's checks — the declaration, the publishers, the Amplitude
  factor and release — each falsified once. LPG level ← Key is the machine's drone. **Release runs
  at `MIN_SAMPLE_RATE`**: a closed optical gate takes seconds to reach exact zero, every time here is
  in seconds, and at a kilohertz each of the four hundred cases can wait for exact silence and
  `Inert`.

## Memoised coefficients: why and where

Several per-sample coefficients are transcendental in an argument that rarely moves: the
rise/fall follower coefficient `1 - exp(-1/(tau × fs))` in `LowPassGate` and
`ComplexOscillator::process_factor`, the gates' `powf` cutoff, and the `exp` in `lpg::Pole` and
`oscillator::OnePole`. Each is wrapped in `Memo`, which caches one result against the **complete
argument set**. The two low-pass gates alone evaluated eight transcendentals per sample, a quarter
of the whole instrument (`plans/plan-mxm-mono-08-per-sample-cost.md`).

The rules a new memoised site must keep:

- **A hit returns what the expression itself produced from that key.** No approximation may enter
  behind a memo — not a polynomial, not a table, not `exp2(v log2 b)` for `powf(b, v)`. The
  rendered output stays bit-identical, which is why the change was allowed at all.
- **Key on everything the result depends on**, `fs` included. That is what makes a sample-rate
  change an ordinary miss instead of an invalidation path, and why `Pole`'s key is `(hz, fs)` and
  not `hz`.
- **A direction-selected coefficient keeps one slot per direction.** Attack/release alternates every
  sample on rectified audio, and a single slot would miss on all of it and pay a comparison for the
  privilege.
- A memo is pure derived state: clearing or keeping it across `reset` costs one miss and is
  otherwise unobservable, so `ComplexOscillator::reset`'s `*self = Self::new()` needs no care.

`lpg::tests::memoisation_is_bit_identical_to_recomputing` holds it against an unmemoised reference
across six rates, all three gate modes, a rate change and a reset, and
`tests::a_memo_is_transparent_for_every_key_including_the_hostile_ones` holds the mechanism itself.
The bank digests cannot carry this alone: they pin one rate and one set of patches.

## Chosen and derived constants

All implementation values absent from hardware evidence remain replaceable:

- complex-core ratio **1.0007**, cross-mod/fold drive **2.4**, optical index rise/fall **8/45 ms**,
  and the complex cores' smooth phase/timbre interaction at **4× oversampling** are chosen; at
  1.9 kHz under the test's hard timbre/FM setting it measures **−28.06 dB RMS deviation against
  8×**, while 2× measured only −18.50 dB and was rejected; this claim does **not** include the
  host-rate AM or balanced-product stages. Discontinuous complex endpoints and modulation waves use
  PolyBLEP instead. The retained path measured **105.9 ns/host sample** in one Windows release run
  of `easel_measure` (a reproducible cost probe, not a stable cross-machine benchmark);
- modulation-oscillator audio's **4 Hz, −1.94 dB low shelf** is chosen to preserve low-rate square
  deformation without placing the signal behind a low-pass; its **15.9 Hz** high-pass is derived
  from Card 11's 1 µF/10 kΩ. The panel base range is **0.16–50 Hz**; keyboard/routed CV extends it
  into audio rates;
- **the modulation oscillator's high range is a departure, and its ratio is chosen.** The original has
  one range; a range switch arrived with the 2013 reissue, whose high range is 55–1760 Hz
  (`research:instruments/buchla-music-easel.md` §3.2 and §9). The owner wanted the switch
  (2026-09-23). `Params::mod_high` multiplies the clamped panel rate by **`MOD_HIGH_RATIO` = 88**,
  so the control reads **14.08–4400 Hz** high and its 5 Hz default reads **440 Hz** — the owner's
  choice, the same day, *"to make better musical sense"*: a semitone step from the default lands on
  equal temperament at A440. A first cut at 32 (5.12–1600 Hz) put the default at 160 Hz. A ratio
  rather than the reissue's span, because a ratio keeps a semitone on the control a semitone in the
  sound — the plugin's keyboard law and its reading both rest on that — and a remap onto
  55–1760 Hz through the control's skew would not. Low multiplies by exactly one, so every
  low-range render is bit-identical to before the switch existed
  (`the_high_range_runs_the_modulation_oscillator_at_the_ratio_times_its_panel_rate`);
- the 208 LPG optical rise/fall **8/220 ms**, cutoff span **35 Hz–18 kHz**, mode pole relationships
  and divider laws are chosen, not 292 measurements; the topology and asymmetry are established;
- envelope/pulser retrigger starts from the current level; the time controls are supplied in seconds
  and clamp at the documented **2 ms–10 s**; these retrigger and clamp policies are chosen;
- random values are uniform on **0–1**, deterministic from seed `0x0208_1973`; distribution and
  power-up zero are chosen while four fresh independent held draws are established;
- weighted-CV destination scales, last-touch note priority, and linear portamento at **12 semitones
  per control-second** (sped up by its routed CV) are chosen;
- spring transits **31/43 ms**, T60 **2.3 s**, band **180 Hz–3.2 kHz**, eight allpasses at **700 Hz**,
  bounded soft drive and linear additive wet law are chosen within the compatible replacement tank's
  published 1.75–3.0 s class; none is an original-unit measurement;
- spring tail snap waits **two chosen T60 periods (−120 dB from unity), 100 ms and the longest
  transit** after the input falls quiet; activity settle is **250 ms**; both are chosen digital
  housekeeping below audibility.

## Numeric and realtime contract, in full

`[dependencies]` carries exactly one entry, `mxm-modulation`, which is itself dependency-free at this
same floor, so MSRV stays 1.87 and the crate stays genuinely portable; `[dev-dependencies]` carries
`mxm-measure`, `mxm-audio-file` and `mxm-audio-file-decode` — test-only edges for measurements and
for `easel_demo`'s write and read-back, not in the shipped graph — and `mxm-modulation` again with
its `conformance` feature, which this crate's own `conformance` feature also forwards for the
plugin's tests; neither reaches a bundle. Per-sample methods allocate and lock nothing. Delay
memory is sized by `set_sample_rate`, outside processing. Audio is `f32`; recursive coefficients and
linear portamento accumulation are `f64`. Every external plain value is clamped at its consumer;
recursive state flushes before denormals, and `voice::OUTPUT_BOUND` is the asserted output bound.
Reset and All Sound Off clear pulses, control state, gates and spring without resizing delay memory;
an inert voice returns exact zero without advancing cores. `set_sample_rate` clamps the rate to
`MIN_SAMPLE_RATE` (1 kHz)–768 kHz, and a clamp passes NaN on, so the plugin refuses a non-finite
rate or one below the floor at activation. Self-running control state counts as Live
only when its complete, currently conductive CV/pulse graph reaches an open gate, a present carrier
and a raised mixer path; this includes the effective signed sum of held key/sequencer/random/inverter
values and an autonomous modulation oscillator. Parameter-owned held values refresh before the inert
shortcut, so an active-stage edit can wake its route. Autonomous, pulser and pulse-derived
reachability uses effective positive intervals through serial gates rather than checking only source
endpoints or activity flags; the four voltages changed by one Random trigger are evaluated as one
joint box, not four independent routes. A held-note identity or an individually positive route alone
is not reachability. Stage-derived pulse paths conduct only
through an enabled stage inside the active sequence length. Disconnected and closed-gate loops
park after finite settle. One-sample pulse telemetry OR-latches until
the plugin shell drains it at a publication boundary; this does not lengthen the pulse in the signal
graph. The sounding press's identity is exposed only so the plugin can apply channel-owned
bend/pressure; per-note tuning and channel bend hold through release, channel pressure follows a
fallback owner's channel without replacing per-note pressure, and bend enters as a separately
clamped semitone input. A targeted choke closes the keyed envelope/gates immediately while leaving the post-mix
spring to decay. Randomness and reset are deterministic.

## Measurement and test coverage

**The rulers are shared, the thresholds are not.** `mxm-measure` is a `[dev-dependencies]` entry —
zero dependencies at this same 1.87 floor, and **not in the shipped graph**, which is what the
manifest's *no runtime dependencies* comment means. Measurements come from there; every bound and
its headroom stays in the test that argues for it.

The tests cover exact idle/master-zero silence, reset/panic/choke, denormal flushing, rates from
1 kHz through 768 kHz, bounded extreme sweeps, every wart above, oscillator spectral/model-error
comparison, envelope/pulser linearity, all sequence lengths, held random outputs, routing causality,
all LPG modes, spring arrival/tail, pressure and tuning ownership,
memoised-coefficient transparency against an unmemoised reference, and
deterministic whole-voice renders. Linux and macOS remain
unverified because there is no CI; development verification is Windows.
