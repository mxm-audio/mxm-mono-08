# mxm-mono-08 — UI design brief

Required by mxm-kit's [`MXM_DESIGN_SYSTEM.md`](https://github.com/mxm-audio/mxm-kit/blob/main/docs/MXM_DESIGN_SYSTEM.md) §14 and written before editor work. This brief targets the
original 1973–74 Model 208 sound source with the original Model 218 performance signals. The
specific evidence set is the dated 1973 card set and the late-1973/early-1974 unit described in
`research:instruments/buchla-music-easel.md`; later 208C/218e features are out.

**Instrument:** a monophonic patchable voice with a dual-core complex oscillator, an audio-rate
modulation oscillator, two low-pass gates, interlocking envelope/pulser/random control sources, the
original five-position internal sequencer, and a mono spring reverb. The original's
external-audio processing is not included (the owner, 2026-09-23; see §9).
The interface is not a hardware panel. **Fidelity is UNVERIFIED** because no original unit was
measured for this repository.

**Brief gate:** all ten §14 questions and the audible Init articulation are resolved. The owner chose
the shared collection accent, a keyed envelope opening LPG 1, deterministic sample/event order with
a one-sample delay on backward feedback, and no standalone spring promotion (2026-09-06). The brief
is ready to gate editor work; its recognisability trial and §15 QA still remain release gates.

**Names (the owner, 2026-09-23).** The panel speaks east-coast: the pulser is the **Clock**, a stage
is a **Step**, a pulse is a **Trigger**, and each module's name prefixes its controls — *Complex
timbre*, *Mod depth*, *LPG 1 mix*. `plugins/mxm-mono-08/AGENTS.md` *Names* holds the rules, and
`plugins/mxm-mono-08/NOTES.md` *Names* the rulings and the table from the code's words to the
panel's. Where this brief describes the original hardware, it
keeps the hardware's words.

## Product boundary before the ten questions

- **Keep the five-position sequencer.** The owner requested it, and the evidence makes it part of
  the 208's control architecture rather than a keyboard convenience: its held voltage changes the
  sound, its optional stage pulses articulate internal generators, and its voltage can close the
  pulser-to-sequencer timing loop (`research:instruments/buchla-music-easel.md` §3.6). It is not
  replaced by MXM Player's note sequencer.
- **Keep the spring reverb and no other effect.** It is the original instrument's built-in, mono,
  post-mix effect (`research:instruments/buchla-music-easel.md` §3.10). Chorus, delay, phaser and
  distortion stay out. **By the owner's explicit local ruling (2026-09-06), this spring remains
  inside `mxm-mono-08` only:** this job creates no standalone effect, reserves no effect identity,
  and plans no later wrapper.
- **Keep touch pulse, continuous pressure and linear portamento as performance signals.** A note-on
  supplies touch and pitch; host pressure supplies the continuously changing pressure signal and is
  not replaced by note-on velocity. Remove the physical 29-key limit and the three touch-selectable
  preset-voltage buttons: the former is a keyboard limitation, and the latter duplicate host
  transpose/macros rather than the voice. The routable key voltage, pressure and pulse remain.
- **Drop the external-audio path** (the owner's ruling, 2026-09-23, which reverses this brief's
  earlier *keep*). The preamp, envelope detector, balanced external mode and LPG 2's external
  source went with the auxiliary input they served: Bitwig gives a CLAP instrument its first audio
  configuration, which had no input, so none of it could be reached there. The plugin offers
  stereo and mono out with no input.
- **Replace banana jacks and resistor cards with a software-native routing view and the collection's
  preset system.** The routable, weighted control graph, summed trigger routes, two-stage sequence
  option and voltage-controlled 2–5 stage length remain. Fake jacks, cables, card graphics,
  resistor tolerances and incomplete recall do not. Those are interface and recall limitations,
  not permission to make presets unreliable.

## 1. Primary sound-design task

**Building a responsive or self-running control patch, then striking it through the two low-pass
gates.** The characteristic act is not oscillator → resonant filter. It is assigning envelope,
pulser, sequencer, random, pressure and modulation-oscillator signals to timbre and gate depth so
brightness, loudness and rhythm move together. The editor must make the control network legible
without reproducing the source panel.

## 2. The three to five parameters users reach for most

1. **Complex timbre** — the complex oscillator's harmonic interaction.
2. **Complex wave mix** — sine to the selected Complex wave, distinct from Complex timbre.
3. **LPG 1 level** — brightness and amplitude coupled through the first low-pass gate.
4. **Mod depth** — the amount of AM/FM interaction with the mod oscillator.
5. **Clock period** — the clock/ramp rate behind self-running patches and the internal sequencer.

Complex timbre, Complex wave mix and LPG 1 level are Primary on `Synth`; Mod depth and Clock period
are Standard on their owning cards. Density is handled with the existing Primary, Standard and
Compact tiers, not a new widget size.

## 3. Signal flow that must be visible

```text
host note → Key · Key trigger · Pressure · Velocity · glide ─────────────┐
                                                                         │
envelope · clock · five-step sequencer · Random 1–4 · inverter ──────────┼─→ modulation / trigger routes
                                                                         │
Mod oscillator ──→ AM or FM of Complex oscillator ───────────────────────┘
          │                         │
          │                         └────────────→ LPG 1 ────────────────→ LPG 1 mix (+)
          │                                               └→ LPG 2 input
Mod oscillator audio ── AC-coupled route ─────────────────→ LPG 2 ──────→ LPG 2 mix (−)

LPG 1 mix + opposite-polarity LPG 2 mix → Reverb → Output
```

Five facts must read without a manual:

- the complex oscillator's **Complex wave mix and Complex timbre are separate operations**;
- the examined original's monitored modulation-oscillator output is non-ideal at low rates, while
  the evidence does not place that deformation before the CV/audio split; the direct CV path keeps
  DC and sub-audio content, and the LPG 2 audio path has the independently established AC coupling;
- each low-pass gate couples brightness to level and has VCA / VCF / VCA + VCF modes, with no
  resonance control;
- LPG 1 and LPG 2 reach the output mixer at opposite polarity;
- a step's level and its trigger are independent, and the clock period may close the original
  timing loop.

## 4. Which controls belong in Play view

**No `Play` view.** The host already supplies note input and the performance gestures are not a
small macro layer: pressure and touch pulse are routable sources whose meaning depends on the
patch. The five reached-for controls stay prominent on `Synth`; live pressure is shown in routing
feedback rather than exposed as a second editable parameter.

## 5. Advanced controls and disclosure

- **Depth is edited at its destination, and every route it carries is listed:** a destination card
  shows one row per route — its signed amount and a remove — with the sources it does not yet carry
  offered beneath. Nothing is hidden behind a selection, so there is no summary naming the edges you
  cannot see, because there are none. Trigger destinations show their three enables directly: a
  trigger has no depth, so presence is the whole of it. Adding or removing a route is an ordinary parameter,
  which is what lets a preset restore the whole patch and a host automate the connection itself.
- **`Seq` owns the two-stage and CV-controlled-length reach** as ordinary labelled controls beside
  the documented 3/4/5 choices. They are not hidden behind a fake program card.
- **Calibration stays out of the ordinary parameter surface.** Internal core tracking, optical
  matching and carrier balance are service/model data, not creative controls unless a later plan
  establishes an editable calibration model under `plugins/AGENTS.md`.
- **Not disclosed:** gate modes, oscillator key tracking, Mod type, Envelope mode, the Clock's
  triggers, Clock loop and its momentary Fire once action, step triggers, and LPG 2 input. Each changes what a module does and must remain visible on its
  owning view.

**Once is drawn as a momentary action, not a latched selector value.** One pointer click or keyboard
activation requests one firing. It has no automatable parameter, preset value or saved-state value;
loading a sound cannot start the clock. It uses the bounded transient-action exception in
`plugins/AGENTS.md`; the plan's §5.3 owns its delivery, cancellation, reset and panic semantics.

## 6. Views

**Space-derived pages**, following design-system §3.2, with full category names and no fixed cap.
The following card inventories replace authored view assignments; no card body is re-cut.

- **Performance / Generators / Tone** — one **Voice** card (Performance) containing the performance
  controls, the two oscillator branches, LPG 1, LPG 2, then one **Mixer and
  reverb** card containing LPG 1 mix, LPG 2 mix and Reverb in signal order. **Output is not on
  a card**: the instrument's output level is an inline slider in the app bar beside the meter
  (design system §3.1; the owner's ruling of 2026-09-18 for every instrument). Glide speed,
  oscillator and gate destinations list the routes
  they carry, one row each, and a card grows as routes are added — the window is resized rather than
  the type shrunk.
- **Modulators** — three cards: **Envelope**, **Clock** and **Random and inverter**.
  Envelope and Clock show their three trigger enables directly, and Clock also lists the routes
  into its period. **Random and inverter holds the two utilities that are each too small to be a
  card**: Random's three trigger enables and the Inverter input's route stack — it takes a sum and
  complements it, where it once offered a menu of one source. Each is a nested group,
  named by its own controls rather than by a sub-heading, because a card whose body is one caption
  and one switch block is a border around nothing. The former wall of nine twelve-slider route cards
  and four trigger cards is deliberately absent.
- **Sequencers** — two cards, **Sequencer** and **Steps**, kept side by side as a preferred group.
  Steps is the run and nothing else: five step columns, each a position marker, its held level as a
  full-height vertical fader and its own trigger enable. Sequencer is what drives the run — the
  length selector, the routes into length, the enables of the triggers that advance it. **The faders are a main
  feature of the instrument and the card is sized around them** (the owner, 2026-09-23); sharing
  their card with the length and route controls is what made them short, and splitting the two is
  what lets them be tall. Five separate step cards, which came before either, made the reader
  assemble the sequencer the panel should have shown them. There is no transport, reverse, pendulum,
  skip, per-step division, quantizer or more-than-five mode.
- **Parameters** — the separate developer testing surface, CC 119 value 127, never a tab.

Oscillators are Generators; the low-pass gates and Mixer and reverb are Tone. The embedded spring does
not force a second primary category or a separate Effects card. Oscillator/gate pairs,
Envelope/Clock and Sequencer/Steps remain preferred groups while they fit. No bar for one page; controller pages
remain unchanged. No fake program-card view.

## 7. Identity accent

**The shared collection accent, not an instrument-specific identity hue:** dark `#4CC9D8`, light
`#247F91`. This is the owner's choice (2026-09-06); `mxm-mono-08` does not add an Easel-specific
colour or consume another place on the identity wheel.

Measured with the WCAG formula used by `mxm_ui::theme::contrast`:

| | vs `surface-1` | vs `surface-2` |
|---|---:|---:|
| Dark `#4CC9D8` | **8.84 : 1** | **8.09 : 1** |
| Light `#247F91` | **4.65 : 1** | **3.89 : 1** |

The shared token clears 4.5:1 for text on `surface-1` and 3:1 for boundaries on both surfaces. In
light mode it is therefore not used as text on `surface-2`; shared controls use it there only for
selection and control boundaries. Fixed modulation and status colours keep their own meanings. A
single restrained shared accent also avoids reproducing the source instrument's multi-colour control
arrangement.

## 8. Live visualizations

1. **Complex-wave display** on `Synth`: the actual output shape after Complex wave mix and Complex
   timbre, so
   the two operations cannot be mistaken for one generic fold control.
2. **Dual gate response**: one compact trace per low-pass gate showing the current control level and
   the coupled brightness/level decay. It explains why VCA + VCF mode rings like a struck object.
3. **Routing activity at each destination:** the selected assignment shows source identity and live
   amount by shape plus colour, while an "Also from" line names every other non-zero route that
   remains in force and is absent when there is none. Each destination is one nested group, so
   which source belongs to which amount is visible without counting rows. Triggers flash as events while held/continuous CVs show position.
   Telemetry is observational only.
4. **Step position and trigger state** on `Seq`: the current step, its held level and whether its
   trigger fired must remain distinguishable in all overlapping states.
5. **Output level and latched clip** in the app bar beside the Output control, measured after it.

Telemetry follows the collection contract: atomics or a bounded lock-free snapshot, written at
block/event boundaries, droppable by the UI, and never read by DSP. Peak is max-combined and reset
on read; clip remains latched until acknowledged.

## 9. What is removed from the source hardware layout, and why

**Kept:** module membership, signal classes, the normal audio routes, the weighted control/pulse
graph, five sequencer stages, and signal-flow order. **Removed:** appearance and physical operating
constraints.

| Removed | Why |
|---|---|
| Panel geometry, coloured caps, typography, case, fake sliders and lamps | Design-system §2 forbids a hardware replica and §5.3 forbids signature colour arrangements |
| Banana jacks, stacked plugs and drawn cables | Target-local source pickers expose each destination's independent amounts accessibly without imitating hardware |
| Resistor-card graphics, local/remote/both ritual, tolerance and incomplete recall | The graph reach remains; collection presets must restore their declared parameter values reliably |
| The 29-key limit and three preset-voltage touch buttons | Keyboard/front-controller limitations already supplied more generally by the host; pressure, pulse, pitch and portamento remain because they articulate the voice |
| Duplicate mono output jack and powered headset/monitor path | The DAW owns monitoring and output routing; the plugin emits one mono programme in host-compatible mono/stereo layouts |
| Later arpeggiator, fourth preset, touch strip, MIDI extras | They were not on the original target revision |
| Any resonant VCF, audio noise source, oscillator sync/PWM/sub, stereo reverb or extra effect | None existed on the target |
| The preamp, envelope detector, balanced external mode and LPG 2's external source | **On the target, and removed by the owner's ruling (2026-09-23)**: a DAW gives the plugin its first audio configuration, which had no input, so the path was unreachable where the instrument is played. A declared deviation, not an omission |

**One later feature is in by the owner's choice** (2026-09-23): the modulation oscillator's
high-range switch, which arrived with the 2013 reissue. The instrument is inspired by the original
and catches its spirit rather than reproducing one revision, and the owner wanted the switch. It is
two-way — Low and High — in the Mod oscillator card's knob row, beside Mod frequency and Mod
depth; it is *Mod high range*.

**Established warts to preserve and label in DSP:** separate complex-oscillator cores; optical gate
memory and brightness/level coupling; opposite output polarity; observed low-rate non-ideality on
the examined original's monitored modulation-oscillator output; the separate AC-coupled LPG 2 audio
route; the pulser requiring an initiating event in self mode; independent sequencer voltage/pulse
state; direct sequencer → pulser-period → sequencer feedback; and affine `full scale − input`
inversion. Any provisional deformation stage is confined to the observed audio/monitor path and its
placement is labelled chosen; the direct CV output is not required to inherit it. The direct-CV
versus LPG 2 comparison proves only the independently established AC-coupling distinction.
Unit-dependent oscillator spread, carrier bleed, LED feedthrough, dark reverb, noise and crosstalk
stay measurement questions rather than invented age.

## 10. Minimum size and 200% scale

**Resizable: the editor's `REFERENCE` and `MINIMUM`, derived and held by its tests.** All eleven
§6 items participate in
width- and height-derived paging. Each card's floor is measured and is also its ceiling, so a
card is drawn exactly as wide as its content. Groups split only when width or height requires it.
`editing_cards_fit_the_quarter_4k_content_budget_at_one_times_scale` checks every derived page in
both themes at opening size, the quarter-4K content size and the minimum; the former ignored whole-Synth
density assertion is replaced, not waived. Component geometry and gesture proofs remain.

Zoom is independently chosen at **75–200%**. Keep the physical window fixed for the §15 DPI/zoom
gate; only indivisible overflow scrolls. Native-window, real-DAW and owner inspection remain open.

## Init patch

**Keyed articulation, chosen by the owner (2026-09-06):** the complex oscillator follows its normal
route through LPG 1 in VCA + VCF mode; the envelope takes the Key trigger and opens
LPG 1 at a useful non-zero CV depth. LPG 1's local level stays closed, so Init is silent until a
note and does not drone.

That LPG 1 depth is the instrument's one explicit deviation from the shared zero-amount rule,
necessary for the required plain source to sound. Its value is chosen during implementation by the
smallest musically useful keyed response, not guessed in this brief. Every other modulation/CV route
depth and Reverb begin at zero; the necessary dry-channel and master levels begin useful.
Configuration defaults select useful modes. The modulation oscillator is not forced into an invented
near-unison detune: it is a control/audio modulator rather than a second peer oscillator in Init.

## Recognisability trial

**Trial patch:** complex oscillator through LPG 1 in VCA + VCF mode; a short envelope strike;
mod oscillator applying light FM; the clock advancing the sequencer; the sequencer shortening the
clock period; selected step triggers striking the envelope; light reverb.

Run first on a wireframe, then on the finished editor, with someone who knows an original Easel or
its documented workflow but without showing source imagery:

1. identify why brightness and level decay together;
2. distinguish Complex wave mix from Complex timbre;
3. find the source that advances the sequencer and explain why a disabled step trigger does not skip
   its level;
4. make the loop speed vary by stage;
5. route Pressure to Complex timbre;
6. return to a keyed, non-self-running patch and reduce Reverb.

Each task must be found within ten seconds and at most one wrong view. Gate: five of six. Results are
unrun and must never be reported as passed until observed.
