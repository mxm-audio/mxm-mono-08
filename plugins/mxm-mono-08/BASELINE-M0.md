# mxm-mono-08 — pre-conversion reference, captured at M0

`plans/plan-mxm-mono-08-modulation.md` (in the private archive) M0. **These figures stop existing once the routing conversion
starts**, which is why they are captured first and committed rather than re-derived.

Produced by `plugins/mxm-mono-08/src/lib.rs`'s `baseline` module, on the tree at `72021b3` with only
the measurement seam and that module added:

```bash
cargo test -p mxm-mono-08 --release --lib baseline::throughput_of_init_and_a_routed_patch -- --ignored --nocapture --test-threads=1
MXM_M0_DUMP=<dir> cargo test -p mxm-mono-08 --release --lib baseline::the_bank_digests -- --ignored --nocapture --test-threads=1
```

Release only, and **run the two separately**: `the_bank_against_the_m0_dump` sorts before
`the_bank_digests`, so a single `baseline` filter would try to read dumps that do not exist yet.

**The module uses only what both revisions have** — `render_block_for_test`, the factory files'
permanent ids and note events — so the same file runs on the converted tree. An id a file does not
carry stays at Init, so the converted tree's larger files load here too. The seam is the loop
`process()` runs, factored out so the two cannot drift:

```rust
fn render_sample(&mut self) -> f32 {
    let patch = self.next_patch();
    self.voice.process(&patch)
}
```

`process()` calls it per sample, and so does `render_block_for_test`. At M0 it took the auxiliary
port's sample as an argument; the external input was dropped on 2026-09-23 (the owner's ruling,
`AGENTS.md`), and the seam lost the argument with it. Factoring it out moved nothing: the plugin's 64 tests pass
unchanged.

## Throughput

Taken with nothing else building — mxm-kit's `docs/code-review-notes.md` §3 is plain that a timing taken
during a build is not a measurement — at 48 000 Hz in 64-sample blocks, one held note, three passes
each:

| Case | Pass 1 | Pass 2 | Pass 3 |
|---|---|---|---|
| Init | 115.103 ns/sample | 117.695 | 114.636 |
| `Random sequence` | 115.835 | 115.718 | 115.948 |

**Read the spread before the numbers**: about 3 ns between passes of Init, which is larger than the
difference between the two cases. The gate compares Init against the converted tree's Init
**measured the same way on the same day**, best of several passes each; a difference inside this
spread is not a finding.

**That the two cases cost the same is the measurement's real content, and it is what the dense grid
predicts**: 108 smoothers advance and 108 products are summed per destination whatever the patch
holds, so a busy patch costs what an empty one costs. `plan-modulation-routing.md` §6.2 names this
instrument the collection's largest saving on exactly that ground. After the conversion, Init should
cost *less* than this and a routed patch should cost more than the converted Init — if the converted
Init does not fall, the claim was wrong and B6 reports it as a number.

`Random sequence` is the routed patch held fixed across the conversion: it is the factory bank's
busiest, driving all four pulse destinations, so the pulser runs, the sequencer steps and the four
held random voltages redraw — and the activity predicate, which re-sums the grid several times per
sample, is doing its full work.

Not comparable with `crates/mxm-mono-08-dsp`'s recorded 105.9 ns/host sample: that is
`easel_measure`'s complex-oscillator path alone, without the routing grid or the shell's patch.
**Not portable across machines.** Other programs were not controlled.

## Factory bank — fifty-one reference digests

**The bank these digests describe was replaced on 2026-09-23, and this table was re-recorded with
it.** The fifty sounds the M0 capture pinned no longer exist: they were fifty names on one patch —
`master` and `mix1` identical in all fifty, two presets rendering byte-identically to one another,
`Sparse steps` silent — and `plugins/mxm-mono-08/src/preset.rs` now states fifty different ones. So
**this half of the file has stopped being a record across the routing conversion**. The throughput
figures above and the eleven inverter-setting digests below still are, because neither depends on the
bank. Nothing running asserts these rows — `the_bank_digests` and `the_bank_against_the_m0_dump` are
both measurements — and what holds the bank honest now is `bank_quality` in `lib.rs`.

FNV-1a over the raw sample bits, the digest the player's golden tests use. **Every sound plays one
score**, fixed forever: one press; a lower press taking ownership under last-touch priority; the
first press returning when the lower one releases, which glides and retriggers nothing; both
releases; and the spring's tail — 2 040 blocks, 130 560 samples. It is the shape of the host
golden's score (`apps/mxm-player/tests/t4_golden_audio_mono_08.rs`) so the two measure the same
gestures. No parameter is written by the score: every sound supplies its own patch, and Init's one
wired route already sounds. The last column is how many of the file's parameters applied: all 349.
Re-measured on 2026-09-23 after the modulation high range (`modhigh`) joined every file, off: all
fifty-one digests and peaks are unchanged, and only this column moved, from 369.

**The score drove the external port until 2026-09-23**, with 173 Hz at 0.25, because with silence
there the old bank's external sounds rendered byte-identically to one another and the envelope
follower read zero. **The port, the follower and the sounds built on them were then dropped** (the
owner's ruling, `AGENTS.md`), and the table was re-measured without the tone: `init` and all
forty-seven surviving sounds kept their digests bit for bit, which is the control — the tone only
ever reached audio where a patch routed it. `balanced-edge` (`9d5116e3ae3f7533`),
`external-follower` (`e17c1745e461ff41`) and `external-spring` (`68cdbb7084578a04`) were replaced by
`ring-clang`, `random-bleeps` and `inverter-duck`, whose rows are new; the Applied column went from
370 to 349, the parameters the external input retired.

**The sample-by-sample dump comparison belonged to the old bank** and is not re-run here: it proved
the routing conversion moved 0 of 51 renders, which is a finished argument about the conversion
rather than about these sounds. `MXM_M0_DUMP=<dir>` still makes `the_bank_digests` write every render
as raw `f32`, and the dumps stay local to the machine that took them.

| Sound | Digest | Peak | Applied |
|---|---|---|---|
| *(Init)* | `35fb8426dfd7d9bf` | 0.3293 | 0 |
| `struck-bell` | `f61ff1197dce12e8` | 0.6840 | 349 |
| `wood-block` | `bdec318f05824e1e` | 0.1028 | 349 |
| `metal-ring` | `1c987971ebe16884` | 0.3001 | 349 |
| `gong-wash` | `95300a62e4bbfe01` | 0.3181 | 349 |
| `tin-cascade` | `fdeea94e63a081a6` | 0.5695 | 349 |
| `iron-pulse` | `dbe06bf42e6e31e9` | 0.3345 | 349 |
| `hollow-key` | `9d0430c4f07176cc` | 0.1956 | 349 |
| `glass-keys` | `866eb1653f1cdd8f` | 0.2793 | 349 |
| `soft-strike` | `554ad74b94cc2cb6` | 0.2896 | 349 |
| `reed-key` | `c078450c25af001f` | 0.2766 | 349 |
| `clear-pluck` | `b6ef5b50b06cfb01` | 0.2515 | 349 |
| `wire-pluck` | `a04e32034fdebcb2` | 0.2098 | 349 |
| `gut-pluck` | `99d256edd97aba6f` | 0.2640 | 349 |
| `harp-ping` | `810dd431fdb49262` | 0.3150 | 349 |
| `muted-pick` | `fbfe53081bf76835` | 0.2938 | 349 |
| `round-bass` | `e75902a79b8f444f` | 0.3810 | 349 |
| `tight-bass` | `d250e4dd0da69b74` | 0.3958 | 349 |
| `folded-bass` | `4b934b3b7cfa63c2` | 0.3229 | 349 |
| `growl-bass` | `27994494bc840507` | 0.5906 | 349 |
| `pressure-bloom` | `5eab33d079d7faa4` | 0.1258 | 349 |
| `long-spring` | `11fed162aa83049c` | 0.1674 | 349 |
| `breathing-air` | `6dbcb194225282a5` | 0.6803 | 349 |
| `wide-swell` | `06107b3737f57ecd` | 0.2222 | 349 |
| `mod-motion` | `4d440e7cebd03baf` | 0.2794 | 349 |
| `narrow-lead` | `008bcde120ddee8c` | 0.0870 | 349 |
| `singing-lead` | `35636714567c305e` | 0.1626 | 349 |
| `pulser-drone` | `674af984adde5f14` | 0.4297 | 349 |
| `mod-oscillator-drone` | `f58b980fd42d4e4d` | 0.6901 | 349 |
| `low-pulse-drone` | `f53f671d72999911` | 0.6102 | 349 |
| `random-hold` | `f72240ecad8ebcda` | 0.3854 | 349 |
| `inverted-motion` | `bfb20c2a86f7e652` | 0.2829 | 349 |
| `beating-pair` | `5d62c0153ec1ba96` | 0.2535 | 349 |
| `rain-on-tin` | `26d15d4318b1f7f1` | 0.1446 | 349 |
| `two-step-climb` | `50a41fb55bad902b` | 0.3679 | 349 |
| `three-step-fall` | `715007326b8345f7` | 0.6281 | 349 |
| `four-step-arch` | `316903bd1eef5073` | 0.3210 | 349 |
| `five-step-rise` | `0facf034cbfe2483` | 0.4260 | 349 |
| `sparse-steps` | `50dcb32f1c2a1957` | 0.3564 | 349 |
| `pulse-staircase` | `3132866e3fcb406b` | 0.3749 | 349 |
| `stage-tempo` | `9fbb89f702c905d7` | 0.3408 | 349 |
| `random-sequence` | `512c0ed4cea75a3f` | 0.2162 | 349 |
| `gliding-sequence` | `30b65d7aded81272` | 0.2412 | 349 |
| `stage-timbre` | `b412f9d85ef61f1c` | 0.2628 | 349 |
| `pressure-bend` | `8184ff0ce4034339` | 0.2503 | 349 |
| `wheel-opening` | `6487ad1684afed8d` | 0.2610 | 349 |
| `feedback-sheen` | `75271c1381527075` | 0.2528 | 349 |
| `ring-clang` | `7d6eb9bf27eb3639` | 0.3041 | 349 |
| `random-bleeps` | `200cd15af33b0096` | 0.4878 | 349 |
| `inverter-duck` | `4f88edde49f21491` | 0.5006 | 349 |
| `patch-laboratory` | `854e604a73a572f6` | 0.2590 | 349 |

**Every peak is below unity**, the highest being `mod-oscillator-drone`'s 0.6901. (The replaced
`external-spring` peaked at 1.1055 under the external tone; nothing in the bank does now.)
Recorded so a later reader can tell a conversion that changes level from one that does not; whether
the level is right is a question for the owner's listening pass.

**The replaced bank's weakest entry was `stage-length`.** Its digest did not move when the external
port was driven, and its design routed Random 1 to sequence length while nothing carries the sequencer's
voltage into audio, so the bank does not discriminate it on its design's own merits. Its render
differs from Init's, and what accounts for that difference is not established here.

## The player golden

`apps/mxm-player/tests/t4_golden_audio_mono_08.rs`, **pinned at `19a90ffe21920f05`** — a pin that
predates this plan, taken with the non-editor shell. *Since the split (2026-10-06):* that test is
this repository's `plugins/mxm-mono-08/host-tests/tests/golden_audio.rs`, same pin, compared on
Windows only. At M0 it **holds with the seam in place**,
against a release bundle built from this tree: `the_fixed_real_host_score_has_not_moved` and
`the_reference_is_sensitive_to_the_complex_oscillator` both pass, and the debug bundle's behaviour
suite passes beside them. The bundles were confirmed present, because that test returns early — and
still reports success — when no bundle exists, so a green line alone proves nothing.

Plan §6 is what this pin is for: the conversion's score gains one presence write for *Timbre from
sequencer*, whose pair is absent at the new Init, and the amount's 10 ms ramp still completes in the
four silent blocks before the first note. **The digest is therefore expected to hold**; if it moves,
that is a defect to find — signed-zero summation, or snap timing — not a digest to re-pin.

No human listening is claimed, here or there.

## After B1 — 2026-09-16

The same `baseline` module on the converted tree: the weighted grid declared as routes on
`mxm-modulation`, four sources appended, and the activity predicate re-expressed over live routes.
**B1 mints no parameter ids**: every pair the instrument already had is present, which is the dense
grid expressed as routing, and the four new sources are published but unread. The host surface is
still 160 parameters.

### Factory bank

**51 renders against the M0 dumps, sample by sample: every one bit-identical — moved 0 of 51, worst
|diff| `0.000e0`.** Not "small", not "rounding": equal bit for bit, Init and all fifty factory
sounds, external port and all.

That is what the arithmetic predicted and it is worth writing down, because it is the whole argument
for the conversion being safe: `Compacted` keeps sources in index order, so the sum runs in the same
sequence as `CvSource::ALL` did; `amount × source` is the same product as `source × amount`; and the
shared sum's unit scale is an exact multiply by one, with each destination's own scale still applied
afterwards. `a_full_grid_sums_exactly_as_the_dense_grid_did` holds that reasoning at the unit level,
and these 51 renders hold it through the whole instrument.

### Throughput — **a regression, not the predicted saving**

The same two cases, measured the same way on the same day with nothing else building, three passes:

| Case | M0 | Converted |
|---|---|---|
| Init | 115.103, 117.695, 114.636 ns/sample | 120.950, 121.862, 121.677 |
| `Random sequence` | 115.835, 115.718, 115.948 | 120.973, 121.840, 120.588 |

**Best of three, Init 114.6 → 121.0 and `Random sequence` 115.7 → 120.6: about 5 ns/sample slower,
4–6 %.** M0's own spread within Init was about 3 ns, so this sits outside it and is a real
difference rather than noise. The two cases still cost the same as each other, which is the dense
grid's signature and is expected while every pair is present.

**Why, and what it means for `plan-modulation-routing.md` §6.2's claim** that this instrument is the
collection's largest saving: nothing has been saved *yet*, because B1 changed the shape without
changing the work. All 108 pairs are present, so the same 108 products are summed — now through an
indirection into the live list — and all 108 amount smoothers still advance every sample. The saving
the plan predicts is a **B2** effect: once presences exist, Init's live lists collapse from 108
routes to one, absent pairs stop being summed, and their smoothers stop advancing.

So this figure is the honest B1 number and not the verdict on §11. `plan-mxm-mono-08-modulation.md`
§11 says an Init regression argues for optimisation before B6; this is that regression, recorded
with its measurement, and B2 is where it is answered or where the claim is withdrawn.

## After B2 — 2026-09-16

Presences and the 36 new amounts exist as permanent ids, the surface is **340**, and the fifty
factory files are regenerated under §6's rule: present is Init's wiring together with every pair a
design overrides to a non-zero amount.

### Factory bank

**51 renders against the M0 dumps: bit-identical again — moved 0 of 51, worst |diff| `0.000e0`.**

That number was earned twice over, and the intermediate failure is worth recording because it is the
trap this phase sets. Before regeneration the same comparison reported **22 of 51 moved, worst
|diff| 1.191** — sounds rendering completely differently, not rounding. The cause was not the
conversion: the shipped files stored 160 parameters, so every new presence loaded at its Init
default of *absent*, and a design that set `cv_timbre_pressure` kept its depth against a route that
no longer existed. A presence that defaults to absent silently unwires every stored amount, and
regenerating the files is not tidying-up after B2 — it is the half of B2 that keeps the sounds.

### Throughput — **the saving, at last**

Three passes each, same machine, same day, nothing else building:

| Case | M0 | B1 | B2 |
|---|---|---|---|
| Init | 115.103, 117.695, 114.636 ns/sample | 120.950, 121.862, 121.677 | 120.424, 100.806, 93.996 |
| `Random sequence` | 115.835, 115.718, 115.948 | 120.973, 121.840, 120.588 | 96.692, 101.232, 97.683 |

Best of three: **Init 114.6 → 94.0, about 18 % faster; `Random sequence` 115.7 → 96.7, about 16 %.**
`plan-modulation-routing.md` §6.2's claim that this instrument is the collection's largest saving is
supported, where B1's figure contradicted it.

**Read the spread before celebrating.** B2's passes range over 26 ns on Init (94.0 to 120.4) against
M0's 3 ns, and the widest reading is the first pass, which looks like warm-up the 64-block warm-up
loop did not cover. The direction is far larger than that noise and the mechanism is understood —
Init's live list is one route where the dense grid summed 108 terms and advanced 108 smoothers every
sample, whatever the patch held — but the *figure* deserves re-taking at B6 beside the owner's pass
rather than being quoted to three decimals from this run.

**What made the difference, and it was a defect of mine, not of the design**: the first B2 `advance`
walked all 144 grid positions every sample testing presence, which cost 144 branches to advance one
smoother and measured *slower* than B1. Walking the compacted live list is what the conversion was
for. Clippy flagged that loop as `needless_range_loop` and I nearly dismissed it as style.

## Before D2 — the inverter's eleven settings, 2026-09-16

`inverterinput` is an eleven-way source selector that D2 retires, replacing it with an *Inverter
input* destination carrying fifteen routes. The plan's §12 requires that **each of the eleven
settings still renders bit-identically after the conversion** — and nothing in the fifty-one digests
above can show that: not one factory sound moves this selector off its default, so the entire
inverter path is invisible to the bank. These eleven digests are that missing reference, captured
from the post-B4 tree, whose bank is bit-identical to M0. The chain is M0 → B1/B2's bit-identical
bank → this capture → D2.

`baseline::the_inverter_selector_digests`, release, one score per setting.

**The patch matters more than the digests.** The first capture was taken against Init and was
worthless: seven of the eleven settings hashed alike, because Init starts no pulser, steps no
sequencer, redraws no random voltage and sends no aftertouch — so all seven of those sources sat at
zero, the inverter read a constant one, and Gate 1 clipped every difference away before it reached
audio. That is the same blind-reference failure the external input caused in M0 itself, where six
sounds came out byte-identical. The reference therefore drives pulses into the pulser, the sequencer
and the random box, turns `pulserself` on so the pulser keeps cycling rather than firing once, sends
channel pressure, and routes the inverter into **complex pitch** — exponential and unclamped, where
timbre, the modulation index and both gates clamp to `0…1` and would hide the very thing being
measured. The test asserts that all eleven digests differ, so a patch that stops separating them
fails rather than passing quietly.

| `inverterinput` | Digest |
|---|---|
| `Key` | `0176ac0f304311bd` — **re-pinned 2026-09-27 to `f302cab0186b53ac`** by the modulation standard's Key (`(note − 60) / 60` where it was `note / 127`); the retired `1 − note / 127` is no longer a route, and the test holds the inverter reading the standard Key |
| `Pressure` | `536e52748f6b84e9` |
| `Mod oscillator` | `a22761f89e51711e` |
| `Envelope` | `1ca031c4d7a84ccb` |
| `Pulser` | `7cf74acf8f6b950d` |
| `Sequence` | `371e7c00d03b65c2` |
| `Random 1` | `eaebe5f4148806d9` |
| `Random 2` | `6063a3fdf132b96a` |
| `Random 3` | `97f3bc4b8dac7dbc` |
| `Random 4` | `d9ee6dfe5a92632e` |
| `Envelope follower` | `6ad2bbe82af5569e` — **retired 2026-09-23** with the external input it followed; no route reproduces it, and `the_inverter_destination_reaches_every_retired_setting` holds the other ten |

## After D2 — 2026-09-16

**`inverterinput` is retired and the inverter's input is the tenth destination.** One permanent id
out, thirty in; the surface is **369**. The tenth destination carries fifteen sources, not sixteen:
the inverter is not offered to itself, and that refusal is stated in the parameter surface (no pair
is minted) and again in `Graph::set_topology` (a hand-wired pair is cleared).

### Factory bank

**All fifty-one renders are bit-identical to M0** — every digest in the table above, unmoved,
including `inverted-motion`, the one factory sound whose audio actually exercises the inverter path.
The `Applied` column moves 160 → 369, which is the whole surface being carried by every file.

This is the claim that mattered, because D2 changed more of the stored state than any step before it:
every one of the fifty files was regenerated, and Init itself gained a second wired pair.

### The eleven settings

`the_inverter_destination_reaches_every_retired_setting` renders all eleven settings the retired
selector could hold and compares them against the digests recorded above. It passes **in debug
against digests captured in release**, so the two profiles agree bit for bit here. It is a standing
assertion rather than an `#[ignore]`d measurement: eleven renders is a cost the suite can carry, and
the reachability of a retired control is exactly the thing that should fail loudly.

### Init gained a second wired pair, and it is translation rather than sound

*Inverter input* from Random 1 at full depth. The retired selector shipped set to Random 1, so that
is what a fresh instance's inverter has always read; leaving the pair absent would have made it a
constant `complement(0.0)`. It reaches audio only where a patch routes the inverter somewhere, which
Init does not — which is why Init's own digest is unmoved.

### Throughput — B5's cost re-measure, and the first quiet reading since M0

| Case | M0 | B1 | B2 | D2 |
|---|---|---|---|---|
| Init | 115.103, 117.695, 114.636 ns/sample | 120.950, 121.862, 121.677 | 120.424, 100.806, 93.996 | **87.489, 87.682, 87.300** |
| `Random sequence` | 115.835, 115.718, 115.948 | 120.973, 121.840, 120.588 | 96.692, 101.232, 97.683 | **88.048, 88.773, 88.854** |

**Read the spread first, because it is the point.** D2's three passes range over **0.38 ns** on Init
and **0.81 ns** on `Random sequence`, against B2's 26 ns and M0's 3 ns. B2's row said its own figure
deserved re-taking for exactly this reason, and this is that re-take: the numbers are tight enough to
mean something.

Against M0: **Init 114.6 → 87.3, about 24 % faster; `Random sequence` 115.7 → 88.0, about 24 %.**
`plan-modulation-routing.md` §6.2's claim that this instrument is the collection's largest saving is
supported, and better supported than at B2.

**This is not D2 making the instrument faster, and it must not be read that way.** D2 *added* a
destination and gave Init a second live route; it can only have cost a little. The distance from
B2's 94.0/96.7 to these figures is B2's noise resolving, not a gain. The saving is the same one B2
identified — Init's live list is two routes where the dense grid summed 108 terms and advanced 108
smoothers every sample whatever the patch held — now measured on a quiet machine.

**One caveat that cannot be removed.** M0's figures were taken on a different day. The pre-conversion
code no longer exists in the tree, so the comparison cannot be re-run head to head without checking
out `7845eea`'s parent; M0's own 3 ns spread is the reason it is still worth quoting.

### Two defects this step produced, both caught by tests rather than by reading

- **The preset generator mints a presence by appending `on` to an amount id.** `Inverted motion`
  needs to *clear* Init's inverter pair, which means naming a presence directly — and the generator
  reached for `cv_inverter_random1onon`, which does not exist. It now writes presences only for
  amount ids.
- **`routed_factory_sources_are_actually_animated` decided from an id's spelling.** An override
  merely mentioning a random source counted as routing one, so clearing a random pair demanded a
  random pulse. It now asks about presence and depth, which also fixes the pre-existing case of an
  amount overridden to exactly zero.

## What this does not establish

- **Nothing about how it sounds.** A digest proves *unchanged*; it cannot prove *good*.
- **Nothing about the host path, except the golden.** The bank is the plugin library, in process,
  and the score drives an external port that MXM Player's chosen layout does not even offer.
- **Nothing about the editor**, whose floors and coverage the conversion also moves (plan B4).
