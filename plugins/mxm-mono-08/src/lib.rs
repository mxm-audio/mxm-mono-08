//! mxm-mono-08 — a patchable monophonic voice with a five-stage CV/pulse sequencer.
//!
//! The framework-free signal network is `mxm-mono-08-dsp`; this crate owns permanent identity,
//! host parameters, note/performance translation, state, presets, realtime processing, telemetry
//! and the dynamically paged production editor.

macro_rules! plugin_name {
    () => {
        "mxm-mono-08"
    };
}
pub const NAME: &str = plugin_name!();
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

pub mod actions;
pub mod editor;
pub mod params;
pub mod preset;
pub mod routes;
pub mod telemetry;

use actions::TransientActions;
use mxm_mono_08_dsp::routing::Routing;
use mxm_mono_08_dsp::spring::Spring;
use mxm_mono_08_dsp::voice::{Activity, NoteId, Params as VoiceParams, Voice};
use nice_plug::prelude::*;
use params::MxmMono08Params;
use std::sync::Arc;

const MAX_BLOCK_SIZE: usize = 64;
const NUM_CHANNELS: usize = 16;
const DEV_VIEW_CC: u8 = 119;
const DEV_DISCLOSURE_CC: u8 = 118;
const DEV_BROWSER_CC: u8 = 117;
const DEV_THEME_CC: u8 = 116;
const DEV_CC_ENV: &str = "MXM_DEV_CC";

#[derive(Debug, Clone, Copy)]
pub enum EditorTask {
    /// Carries no value; scheduling it asks the CLAP host to resume a sleeping processor.
    WakeAudio,
}

pub struct MxmMono08 {
    params: Arc<MxmMono08Params>,
    voice: Voice,
    /// Which routes exist and how deep each one is. **It travels beside the patch, not inside it**:
    /// presence changes only on a parameter event, while the amounts are smoothed signals refreshed
    /// every sample.
    routing: Routing,
    bend: [f32; NUM_CHANNELS],
    /// CC 1 per channel — a routing source and nothing else; it writes no parameter.
    wheel: [f32; NUM_CHANNELS],
    /// The last press's note-on velocity, held through its release: every press is a keyboard
    /// pulse, so it is the press that last could trigger the envelope. **Full before any press and
    /// after a reset**, so the standard Velocity (`v − 1`) rests at zero.
    velocity: f32,
    channel_pressure: [f32; NUM_CHANNELS],
    sample_rate: f32,
    telemetry: Arc<telemetry::Telemetry>,
    actions: Arc<TransientActions>,
    dev_cc: bool,
    /// The unsmoothed half of the patch, read **once per non-empty `process()` call** rather than
    /// once per sample. Selectors, switches and state-machine times configure behaviour and are not
    /// smoothed (`plugins/AGENTS.md`, *Smooth signals, not coefficients*);
    /// `SAMPLE_ACCURATE_AUTOMATION` is off, so the wrapper applies every parameter change before
    /// the call and none of them can move inside it. Refreshed beside the topology, which moves on
    /// the same boundary for the same reason.
    configured: Configured,
    /// The host's tempo this call, which the clock period's sync reads when the topology resolves.
    host_tempo: Option<f64>,
    /// Test-only: how many times `resolve_topology` has run. The hoist below is invisible to any
    /// render comparison — resolving unchanged parameters twice is identical by construction — so
    /// the regression oracle counts resolutions instead of comparing samples.
    #[cfg(test)]
    topology_resolutions: usize,
}

/// The interval-constant fields of [`VoiceParams`]. Same values, read about twenty times less often.
#[derive(Debug, Clone, Copy, Default)]
struct Configured {
    complex_endpoint: mxm_mono_08_dsp::oscillator::ComplexEndpoint,
    complex_keyboard: bool,
    mod_wave: mxm_mono_08_dsp::oscillator::ModWave,
    mod_keyboard: bool,
    mod_high: bool,
    modulation_mode: mxm_mono_08_dsp::voice::ModulationMode,
    gate1_mode: mxm_mono_08_dsp::lpg::GateMode,
    gate2_mode: mxm_mono_08_dsp::lpg::GateMode,
    gate2_source: mxm_mono_08_dsp::voice::Gate2Source,
    attack_s: f32,
    duration_s: f32,
    decay_s: f32,
    envelope_mode: mxm_mono_08_dsp::control::EnvelopeMode,
    pulser_period_s: f32,
    pulser_self: bool,
    sequence_length: usize,
    sequence_pulses: [bool; 5],
    portamento_s: f32,
}
impl Default for MxmMono08 {
    fn default() -> Self {
        Self {
            params: Arc::new(MxmMono08Params::default()),
            voice: Voice::new(),
            routing: Routing::new(),
            bend: [0.0; NUM_CHANNELS],
            wheel: [0.0; NUM_CHANNELS],
            velocity: 1.0,
            channel_pressure: [0.0; NUM_CHANNELS],
            sample_rate: 48_000.0,
            telemetry: telemetry::Telemetry::shared(),
            actions: TransientActions::shared(),
            dev_cc: std::env::var_os(DEV_CC_ENV).is_some(),
            configured: Configured::default(),
            host_tempo: None,
            #[cfg(test)]
            topology_resolutions: 0,
        }
    }
}

impl MxmMono08 {
    /// Producer cloned by the editor. It exposes only the transient Once act, never DSP
    /// state or a value that could be persisted.
    pub fn transient_actions(&self) -> Arc<TransientActions> {
        self.actions.clone()
    }

    /// Rebuilds which routes exist, **once per non-empty `process()` call**, and hands the topology
    /// to the voice so it can compact its live lists. It also refreshes [`Configured`], which is
    /// interval-constant for the same reason the topology is.
    ///
    /// Every newly present route's smoother is snapped to its stored depth on the way through, so a
    /// pair re-added after being edited while absent arrives where the player put it rather than
    /// ramping in from wherever the last live sample left it.
    fn resolve_topology(&mut self) {
        #[cfg(test)]
        {
            self.topology_resolutions += 1;
        }
        self.routing = self.params.routes.topology_from(&self.routing);
        self.voice.set_topology(&self.routing);
        let p = &self.params;
        self.configured = Configured {
            complex_endpoint: p.complex_endpoint.value().into(),
            complex_keyboard: p.complex_keyboard.value(),
            mod_wave: p.mod_wave.value().into(),
            mod_keyboard: p.mod_keyboard.value(),
            mod_high: p.mod_high.value(),
            modulation_mode: p.modulation_mode.value().into(),
            gate1_mode: p.gate1_mode.value().into(),
            gate2_mode: p.gate2_mode.value().into(),
            gate2_source: p.gate2_source.value().into(),
            attack_s: p.attack.value(),
            duration_s: p.duration.value(),
            decay_s: p.decay.value(),
            envelope_mode: p.envelope_mode.value().into(),
            // Synced, the clock runs at its division (`plans/plan-tempo-sync-controls.md`); the
            // period-shortening CV applies on top as ever.
            pulser_period_s: p
                .synced_pulser_period(self.host_tempo)
                .unwrap_or_else(|| p.pulser_period.value()),
            pulser_self: p.pulser_self.value(),
            sequence_length: p.sequence_length.value().stages(),
            sequence_pulses: [
                p.sequence_1_pulse.value(),
                p.sequence_2_pulse.value(),
                p.sequence_3_pulse.value(),
                p.sequence_4_pulse.value(),
                p.sequence_5_pulse.value(),
            ],
            portamento_s: p.portamento.value(),
        };
    }

    #[inline]
    fn next_patch(&self) -> VoiceParams {
        let p = &self.params;
        let c = &self.configured;
        let owner_channel = self.voice.owner().channel as usize % NUM_CHANNELS;
        VoiceParams {
            complex_hz: p.complex_frequency.smoothed.next(),
            complex_endpoint: c.complex_endpoint,
            wave_mix: p.wave_mix.smoothed.next(),
            timbre: p.timbre.smoothed.next(),
            complex_keyboard: c.complex_keyboard,
            mod_hz: p.mod_frequency.smoothed.next(),
            mod_wave: c.mod_wave,
            mod_keyboard: c.mod_keyboard,
            mod_high: c.mod_high,
            modulation_mode: c.modulation_mode,
            modulation_index: p.modulation_index.smoothed.next(),
            gate1_mode: c.gate1_mode,
            gate1_level: p.gate1_level.smoothed.next(),
            gate2_mode: c.gate2_mode,
            gate2_level: p.gate2_level.smoothed.next(),
            gate2_source: c.gate2_source,
            mix1: p.mix1.smoothed.next(),
            mix2: p.mix2.smoothed.next(),
            reverb: p.reverb.smoothed.next(),
            master: p.master.smoothed.next(),
            attack_s: c.attack_s,
            duration_s: c.duration_s,
            decay_s: c.decay_s,
            envelope_mode: c.envelope_mode,
            pulser_period_s: c.pulser_period_s,
            pulser_self: c.pulser_self,
            sequence_length: c.sequence_length,
            sequence_levels: [
                p.sequence_1_level.smoothed.next(),
                p.sequence_2_level.smoothed.next(),
                p.sequence_3_level.smoothed.next(),
                p.sequence_4_level.smoothed.next(),
                p.sequence_5_level.smoothed.next(),
            ],
            sequence_pulses: c.sequence_pulses,
            portamento_s: c.portamento_s,
            // The stored controller position times the smoothed range, so a range edit under a
            // held bend ramps instead of stepping the pitch.
            bend_semitones: self.bend[owner_channel] * p.bend_range.smoothed.next(),
            // The performance sources, as values rather than as parameters: the bender's raw
            // position beside the semitones it also reaches through, the wheel, and the velocity of
            // the last press.
            velocity: self.velocity,
            wheel: self.wheel[owner_channel],
            bend: self.bend[owner_channel],
        }
    }

    /// One sample through the plugin's own path: the patch from the smoothers, then the voice.
    ///
    /// **What `process()` renders per sample, and all the measurement seam renders**, so the two
    /// cannot drift and [`Self::render_block_for_test`] measures the path a host hears.
    #[inline]
    fn render_sample(&mut self) -> f32 {
        // Only live routes' smoothers advance. The dense grid advanced all 108 every sample
        // whatever the patch held, which is the cost this conversion set out to remove.
        self.params.routes.advance(&mut self.routing);
        let patch = self.next_patch();
        self.voice.process(&patch, &self.routing)
    }

    /// Renders one block through the plugin's own per-sample path, for measurement.
    ///
    /// **A measurement seam, not a second `process()`.** It calls what `process()` calls —
    /// [`Self::render_sample`] per sample — and omits the wrapper's event handling, telemetry and
    /// buffer plumbing, which are not where the routing conversion's work lands
    /// (`plans/plan-mxm-mono-08-modulation.md` §11). `mxm-mono-01`, `mxm-mono-02`, `mxm-poly-06` and `mxm-mono-03` carry the
    /// same seam for the same reason.
    pub fn render_block_for_test(&mut self, out: &mut [f32]) {
        self.resolve_topology();
        for sample in out.iter_mut() {
            *sample = self.render_sample();
        }
    }

    /// Consume accepted editor acts at one deterministic event boundary. Voice::once keeps a
    /// second bounded count so several acts accepted before this process call still fire once each.
    fn service_transient_actions(&mut self) {
        while self.actions.take_once() {
            let accepted = self.voice.once();
            debug_assert!(
                accepted,
                "the shell queue is smaller than the DSP pending count"
            );
        }
        self.telemetry.publish_once(
            self.actions.fired(),
            self.actions.cancelled(),
            self.actions.rejected(),
        );
    }

    fn cancel_transient_actions(&mut self) {
        self.actions.cancel_pending();
        self.telemetry.publish_once(
            self.actions.fired(),
            self.actions.cancelled(),
            self.actions.rejected(),
        );
    }

    fn sync_owner_channel_pressure(&mut self) {
        let owner = self.voice.owner();
        self.voice
            .set_channel_pressure(self.channel_pressure[owner.channel as usize % NUM_CHANNELS]);
    }

    fn handle_event(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn {
                voice_id,
                channel,
                note,
                velocity,
                ..
            } => {
                if velocity <= 0.0 {
                    self.voice.note_off(voice_id, channel, note);
                    self.sync_owner_channel_pressure();
                } else {
                    let id = NoteId {
                        voice_id,
                        channel,
                        key: note,
                    };
                    // Velocity was dropped here before the conversion. It is now a routing source
                    // and nothing else — it writes no parameter and the machine still has no
                    // velocity sensitivity of its own — and it holds through the release.
                    self.velocity = velocity.clamp(0.0, 1.0);
                    self.voice.note_on(id);
                    self.sync_owner_channel_pressure();
                }
            }
            NoteEvent::NoteOff {
                voice_id,
                channel,
                note,
                ..
            } => {
                self.voice.note_off(voice_id, channel, note);
                self.sync_owner_channel_pressure();
            }
            NoteEvent::Choke {
                voice_id,
                channel,
                note,
                ..
            } => {
                self.voice.choke(voice_id, channel, note);
                self.sync_owner_channel_pressure();
            }
            NoteEvent::PolyTuning {
                voice_id,
                channel,
                note,
                tuning,
                ..
            } if tuning.is_finite() => {
                self.voice.set_tuning(voice_id, channel, note, tuning);
            }
            NoteEvent::PolyPressure {
                voice_id,
                channel,
                note,
                pressure,
                ..
            } if pressure.is_finite() => {
                self.voice.set_pressure(voice_id, channel, note, pressure);
            }
            NoteEvent::MidiChannelPressure {
                channel, pressure, ..
            } if pressure.is_finite() => {
                self.channel_pressure[channel as usize % NUM_CHANNELS] = pressure.clamp(0.0, 1.0);
                let owner = self.voice.owner();
                if owner.channel == channel {
                    self.sync_owner_channel_pressure();
                }
            }
            NoteEvent::MidiPitchBend { channel, value, .. } if value.is_finite() => {
                self.bend[channel as usize % NUM_CHANNELS] = (2.0 * (value - 0.5)).clamp(-1.0, 1.0)
            }
            NoteEvent::MidiCC {
                channel, cc, value, ..
            } => match cc {
                // CC 1 is the Wheel routing source. It writes no parameter, which is exactly what
                // the control map's CC 1 reservation allows.
                1 => self.wheel[channel as usize % NUM_CHANNELS] = value.clamp(0.0, 1.0),
                DEV_VIEW_CC if self.dev_cc => self
                    .telemetry
                    .request_view((value.clamp(0.0, 1.0) * 127.0).round() as u8),
                DEV_DISCLOSURE_CC if self.dev_cc => self.telemetry.request_disclosure(value >= 0.5),
                DEV_BROWSER_CC if self.dev_cc => self.telemetry.request_browser(value >= 0.5),
                // A theme by index, 0 light / 1 dark / 2 system: applied to the editor and never
                // saved, so a capture run cannot rewrite the choice made in the app bar.
                DEV_THEME_CC if self.dev_cc => self
                    .telemetry
                    .request_theme((value.clamp(0.0, 1.0) * 127.0).round() as u8),
                control_change::ALL_SOUND_OFF => {
                    self.cancel_transient_actions();
                    self.voice.all_sound_off();
                }
                control_change::ALL_NOTES_OFF => self.voice.all_notes_off(),
                _ => {}
            },
            _ => {}
        }
    }

    fn publish_voice_telemetry(&mut self) {
        let (stage_pulse, pulse_sources) = self.voice.take_pulse_telemetry();
        self.telemetry.publish_voice(telemetry::VoiceSnapshot {
            activity: self.voice.activity(),
            modulation_cv: self.voice.modulation_cv(),
            envelope: self.voice.envelope_level(),
            gates: self.voice.gate_levels(),
            pulser_period: self.voice.pulser_period(),
            stage: self.voice.stage(),
            sequence_value: self.voice.sequence_value(),
            pulse: stage_pulse,
            cv_sources: self.voice.cv_sources(),
            pulse_sources,
        });
    }

    fn status(&self) -> ProcessStatus {
        match self.voice.activity() {
            Activity::Live => ProcessStatus::KeepAlive,
            Activity::Inert => ProcessStatus::Normal,
            Activity::Tailing => ProcessStatus::Tail(self.tail_upper_bound_samples()),
        }
    }

    /// A conservative bound from the moment the graph first reports `Tailing`.
    ///
    /// A transient envelope may still owe attack, duration and decay, the optical gates and
    /// activity detector need another half second to settle, and the spring can receive its last
    /// excitation at the end of that dry interval. The spring time is therefore **added**, not
    /// compared with the dry time. Over-reporting lets a host run silence; under-reporting cuts a
    /// legitimate delayed arrival.
    fn tail_upper_bound_samples(&self) -> u32 {
        let rate = self.sample_rate.clamp(1_000.0, 768_000.0);
        let dry_seconds = self.params.attack.value().clamp(0.002, 10.0)
            + self.params.duration.value().clamp(0.002, 10.0)
            + self.params.decay.value().clamp(0.002, 10.0)
            + 0.5;
        let dry = (f64::from(dry_seconds) * f64::from(rate)).ceil() as u64;
        let spring = if self.params.reverb.value() > 0.0 {
            u64::from(Spring::tail_samples(rate))
        } else {
            0
        };
        dry.saturating_add(spring).clamp(1, u64::from(u32::MAX)) as u32
    }
}

impl Plugin for MxmMono08 {
    const NAME: &'static str = NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    /// Stereo and mono out, **and no input**. The two layouts that offered an external input were
    /// dropped with everything that processed it (the owner's ruling, 2026-09-23): Bitwig gives a
    /// CLAP instrument its first layout, which had none, so the path was unreachable there.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type Editor = editor::MxmMono08Editor;
    type SysExMessage = ();
    type BackgroundTask = EditorTask;
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(
            self.params.clone(),
            self.telemetry.clone(),
            self.actions.clone(),
            async_executor,
        )
    }
    fn activate(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // A new activation starts with no tempo and nothing resolved: the first callback reports
        // the tempo, so neither the audio nor an editor frame before it shows the last session's
        // divisions (`plans/plan-tempo-sync-controls.md`).
        self.telemetry.tempo.publish(None);
        self.host_tempo = None;
        // A rate the DSP's clamps cannot hold is refused before anything changes: a NaN, or one
        // below `MIN_SAMPLE_RATE`, crosses a `clamp` bound and panics on the audio thread.
        if !config.sample_rate.is_finite() || config.sample_rate < mxm_mono_08_dsp::MIN_SAMPLE_RATE
        {
            return false;
        }
        self.sample_rate = config.sample_rate;
        self.voice.set_sample_rate(self.sample_rate);
        self.voice.reset();
        self.telemetry.publish_sample_rate(self.sample_rate);
        true
    }
    fn reset(&mut self) {
        self.cancel_transient_actions();
        self.voice.reset();
        // The topology is rebuilt from the parameters at the next interval; clearing it here means
        // no route counts as *already present*, so every live route is re-armed rather than
        // resuming a smoother from before the reset.
        self.routing = Routing::new();
        self.bend = [0.0; NUM_CHANNELS];
        self.wheel = [0.0; NUM_CHANNELS];
        self.velocity = 1.0;
        self.channel_pressure = [0.0; NUM_CHANNELS];
    }
    /// **A project saved before the tempo syncs** restores each Off rather than keeping this
    /// instance's, and a loaded preset's baseline gains it, so the preset stays clean
    /// (`mxm_preset::add_switches_off`).
    fn filter_state(state: &mut PluginState) {
        mxm_preset::add_switches_off(state, crate::preset::TEMPO_SYNC_IDS);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples();
        let mut next_event = context.next_event();
        let mut start = 0usize;
        // The tempo the clock period's sync resolves against, and the one the editor reads.
        self.host_tempo = context.transport().tempo;
        self.telemetry.tempo.publish(self.host_tempo);
        // **Once per non-empty call, not once per interval.** Nothing inside a call can move the
        // topology: `SAMPLE_ACCURATE_AUTOMATION` is off, so the wrapper applies every parameter
        // change before `process()` and splits only on transport, and `handle_event` writes no
        // parameter. Resolving per interval meant resolving per host event offset, which cost
        // 423 ns each and took a densely automated callback to 3.3% of a core
        // (`plans/handover-mxm-mono-08-event-density.md`). It stays *inside* the loop, on the first
        // rendered interval, for two reasons: an empty callback must still resolve nothing, and the
        // first interval's events must still be drained before the resolution that follows them.
        let mut resolved = false;
        while start < samples {
            let mut end = (start + MAX_BLOCK_SIZE).min(samples);
            loop {
                match next_event {
                    Some(event) if event.timing() as usize <= start => {
                        self.handle_event(event);
                        next_event = context.next_event();
                    }
                    Some(event) if (event.timing() as usize) < end => {
                        end = event.timing() as usize;
                        break;
                    }
                    _ => break,
                }
            }
            self.service_transient_actions();
            if !resolved {
                self.resolve_topology();
                resolved = true;
            }
            let output = buffer.as_slice();
            let mut peak = 0.0f32;
            let waveform = self.telemetry.editor_open();
            for i in start..end {
                let sample = self.render_sample();
                if waveform {
                    self.telemetry.push_complex(self.voice.complex_sample());
                }
                peak = peak.max(sample.abs());
                for channel in output.iter_mut() {
                    channel[i] = sample;
                }
            }
            self.telemetry.publish_peak(peak);
            self.publish_voice_telemetry();
            start = end;
        }

        // nice-plug permits events at the callback's end offset (and therefore at offset zero for
        // an empty callback). They belong to the boundary before the next callback, not to a
        // discarded future block. Draining here also gives transient Once acts a boundary while a
        // host is issuing zero-frame process calls.
        while let Some(event) = next_event {
            self.handle_event(event);
            next_event = context.next_event();
        }
        self.service_transient_actions();
        self.publish_voice_telemetry();
        self.status()
    }
}

impl ClapPlugin for MxmMono08 {
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "A patchable monophonic synthesizer with two low-pass gates and a five-step sequencer",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}
nice_export_clap!(MxmMono08);

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_mono_08_dsp::routing::PULSE_SOURCES;
    use std::collections::VecDeque;

    struct TestContext {
        events: VecDeque<NoteEvent<()>>,
        transport: Transport,
    }
    impl TestContext {
        fn new(events: impl IntoIterator<Item = NoteEvent<()>>) -> Self {
            Self {
                events: events.into_iter().collect(),
                transport: Transport::new(48_000.0),
            }
        }
    }
    impl ProcessContext<MxmMono08> for TestContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute_background(&self, _task: EditorTask) {}
        fn execute_gui(&self, _task: EditorTask) {}
        fn transport(&self) -> &Transport {
            &self.transport
        }
        fn next_event(&mut self) -> Option<NoteEvent<()>> {
            self.events.pop_front()
        }
        fn send_event(&mut self, _event: NoteEvent<()>) {}
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }
    fn process_for_test(
        plugin: &mut MxmMono08,
        frames: usize,
        events: impl IntoIterator<Item = NoteEvent<()>>,
    ) -> ProcessStatus {
        let mut samples = vec![0.0; frames];
        let mut buffer = Buffer::default();
        unsafe {
            buffer.set_slices(frames, |channels| {
                channels.clear();
                channels.push(samples.as_mut_slice());
            });
        }
        let mut inputs = [];
        let mut outputs = [];
        let mut aux = AuxiliaryBuffers {
            inputs: &mut inputs,
            outputs: &mut outputs,
        };
        let mut context = TestContext::new(events);
        plugin.process(&mut buffer, &mut aux, &mut context)
    }
    fn activate_smoothers(plugin: &MxmMono08) {
        for (_, ptr, _) in plugin.params.param_map() {
            unsafe { ptr._internal_update_smoother(48_000.0, true) };
        }
    }
    fn note_on(plugin: &mut MxmMono08, channel: u8, note: u8) {
        plugin.handle_event(NoteEvent::NoteOn {
            timing: 0,
            voice_id: None,
            channel,
            note,
            velocity: 0.8,
        });
    }

    #[test]
    fn identity_and_bundle_agree() {
        assert_eq!(CLAP_ID, format!("dk.mxm.{NAME}"));
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
    }
    #[test]
    fn layouts_are_stereo_and_mono_out_with_no_input() {
        assert_eq!(MxmMono08::AUDIO_IO_LAYOUTS.len(), 2);
        for layout in MxmMono08::AUDIO_IO_LAYOUTS {
            assert!(layout.main_input_channels.is_none());
            assert!(
                layout.aux_input_ports.is_empty(),
                "the external input was dropped"
            );
        }
    }
    /// **The ids the external input's removal retired stay retired** (the owner's ruling,
    /// 2026-09-23). Minting one again would give an old user preset or host automation lane a
    /// meaning it never had: `modmode` and `gate2source` had three options where `modtype` and
    /// `gate2input` have two, and a stored position would select the wrong one.
    #[test]
    fn the_ids_the_external_input_retired_stay_retired() {
        let p = MxmMono08Params::default();
        for (id, _, _) in p.param_map() {
            assert!(
                !matches!(
                    id.as_str(),
                    "preampgain" | "modmode" | "gate2source" | "inverterinput"
                ) && !id.contains("follower"),
                "{id} was retired"
            );
        }
    }
    #[test]
    fn ids_are_unique_and_the_complete_surface_matches_the_host() {
        let p = MxmMono08Params::default();
        let mut host: Vec<_> = p.param_map().into_iter().map(|x| x.0).collect();
        let local: Vec<_> = p
            .all_parameters()
            .into_iter()
            .map(|x| x.0.to_owned())
            .collect();
        assert_eq!(host, local);
        host.sort();
        host.dedup();
        assert_eq!(host.len(), local.len());
        // 160 before the routing conversion; 340 after it; 369 after D2; 370 with the modulation
        // oscillator's high range (owner, 2026-09-23). **349 since the external input was
        // dropped** the same day: 149 routes with an amount and a presence each — nine
        // destinations of fifteen sources and the inverter's of fourteen — plus the twelve trigger
        // enables, plus the thirty-nine panel controls (`preampgain` retired; `modtype` and
        // `gate2input` replaced `modmode` and `gate2source` one for one). **350 with the clock
        // period's tempo sync** (2026-09-25); **380 with the standard Amplitude's fifteen routes**
        // (2026-09-27).
        assert_eq!(host.len(), 380);
    }
    /// **Init's two documented route deviations, and nothing else anywhere in the grid.**
    ///
    /// Gate 1 from the envelope at 0.72 is the approved keyed articulation. *Inverter input* from
    /// Random 1 at full depth is D2's translation of the retired `inverterinput` selector's own
    /// shipped default: existing wiring written as a route, not sound added to Init. It reaches
    /// audio only where a patch routes the inverter somewhere, which Init does not — but leaving it
    /// absent would make a fresh instance's inverter a constant `complement(0.0)` and silently
    /// change every patch that does.
    #[test]
    fn init_has_two_documented_route_deviations() {
        const WIRED: [(&str, &str); 2] = [
            ("cv_gate1_envelope", "cv_gate1_envelopeon"),
            ("cv_inverter_random1", "cv_inverter_random1on"),
        ];
        let p = MxmMono08Params::default();
        // **Amounts by their plain value**, not their fader position: a one-sided offer's fader
        // has its zero at an end, where a two-sided one has it at 0.5.
        let zero = |id: &str, value: f32| {
            if !WIRED.iter().any(|(amount, _)| *amount == id) {
                assert_eq!(value, 0.0, "{id} is not a zero route");
            }
        };
        for (index, group) in p.routes.ordinary() {
            for source in 0..mxm_mono_08_dsp::routing::CV_SOURCES {
                zero(
                    routes::ROUTE_IDS[index][source].0,
                    group.amount_param(source).value(),
                );
            }
        }
        for (slot, &source) in routes::INVERTER_SOURCES.iter().enumerate() {
            let param = p.routes.inverter_input.amount_param(source).unwrap();
            zero(routes::ROUTE_IDS[routes::INVERTER][slot].0, param.value());
        }
        for (id, param) in p.all_parameters() {
            if id.starts_with("cv_") && id.ends_with("on") {
                assert_eq!(
                    param.normalised() > 0.5,
                    WIRED.iter().any(|(_, presence)| *presence == id),
                    "{id}: Init wires Gate 1 from the envelope and the inverter from Random 1, and nothing else"
                );
            }
        }
        assert!((p.routes.gate1.envelope.value() - 0.72).abs() < 1e-6);
        assert!((p.routes.inverter_input.random1.value() - 1.0).abs() < 1e-6);
        for (name, value) in [
            ("Mod depth", p.modulation_index.value()),
            ("LPG 1 level", p.gate1_level.value()),
            ("LPG 2 level", p.gate2_level.value()),
            ("LPG 2 mix", p.mix2.value()),
            ("Reverb", p.reverb.value()),
            ("Glide time", p.portamento.value()),
        ] {
            assert_eq!(value, 0.0, "{name} is an amount and starts at zero");
        }
        assert_eq!(p.routes.envelope.values(), [true, false, false]);
        assert_eq!(p.routes.pulser.values(), [false; PULSE_SOURCES]);
        assert_eq!(p.routes.sequencer.values(), [false; PULSE_SOURCES]);
        assert_eq!(p.routes.random.values(), [false; PULSE_SOURCES]);
        assert!(p.mix1.value() > 0.0 && p.master.value() > 0.0);
    }
    #[test]
    fn pressure_tuning_and_bend_follow_the_sounding_touch() {
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        note_on(&mut x, 2, 60);
        x.handle_event(NoteEvent::PolyPressure {
            timing: 0,
            voice_id: None,
            channel: 2,
            note: 60,
            pressure: 0.75,
        });
        assert!(x.voice.set_pressure(None, 2, 60, 0.75));
        x.handle_event(NoteEvent::PolyTuning {
            timing: 0,
            voice_id: None,
            channel: 2,
            note: 60,
            tuning: 1.5,
        });
        x.handle_event(NoteEvent::MidiPitchBend {
            timing: 0,
            channel: 2,
            value: 1.0,
        });
        assert_eq!(x.next_patch().bend_semitones, 2.0);
    }
    #[test]
    fn channel_pressure_follows_same_and_different_channel_fallbacks() {
        let mut x = MxmMono08::default();
        note_on(&mut x, 2, 48);
        x.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 2,
            pressure: 0.7,
        });
        note_on(&mut x, 2, 60);
        x.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 2,
            pressure: 0.9,
        });
        // MIDI's velocity-zero NoteOn spelling is the same ownership transition as NoteOff.
        x.handle_event(NoteEvent::NoteOn {
            timing: 0,
            voice_id: None,
            channel: 2,
            note: 60,
            velocity: 0.0,
        });
        assert_eq!(
            x.voice.pressure(),
            0.9,
            "same-channel fallback used stale pressure"
        );

        note_on(&mut x, 5, 67);
        x.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 5,
            pressure: 0.25,
        });
        x.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 2,
            pressure: 0.6,
        });
        x.handle_event(NoteEvent::NoteOff {
            timing: 0,
            voice_id: None,
            channel: 5,
            note: 67,
            velocity: 0.0,
        });
        assert_eq!(
            x.voice.pressure(),
            0.6,
            "fallback did not adopt its channel's value"
        );

        x.handle_event(NoteEvent::PolyPressure {
            timing: 0,
            voice_id: None,
            channel: 2,
            note: 48,
            pressure: 0.4,
        });
        x.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 2,
            pressure: 1.0,
        });
        assert_eq!(
            x.voice.pressure(),
            0.4,
            "channel pressure replaced per-note pressure"
        );
    }
    #[test]
    fn one_sample_pulses_survive_the_audio_to_editor_publication_boundary() {
        let mut x = MxmMono08::default();
        x.telemetry.set_editor_open(true);
        activate_smoothers(&x);
        unsafe {
            x.params
                .routes
                .sequencer
                .keyboard
                .as_ptr()
                ._internal_set_normalized_value(1.0);
        }
        note_on(&mut x, 0, 60);
        x.resolve_topology();
        for _ in 0..256 {
            let patch = x.next_patch();
            x.voice.process(&patch, &x.routing);
        }
        x.publish_voice_telemetry();
        let pulses = x.telemetry.take_pulse_sources();
        assert!(pulses[mxm_mono_08_dsp::routing::PulseSource::Keyboard.index()]);
        assert!(pulses[mxm_mono_08_dsp::routing::PulseSource::SequenceStage.index()]);
        assert!(x.telemetry.take_sequence_pulse());
        assert_eq!(x.telemetry.take_pulse_sources(), [false; PULSE_SOURCES]);
        assert!(!x.telemetry.take_sequence_pulse());
    }

    #[test]
    fn shell_consumes_each_once_act_once_even_across_idle_boundaries() {
        let mut x = MxmMono08::default();
        assert_eq!(x.status(), ProcessStatus::Normal);
        let wakes = std::sync::atomic::AtomicUsize::new(0);
        let wake = || {
            wakes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        };
        x.actions.submit_once_from_editor(&wake).unwrap();
        assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 1);
        x.service_transient_actions();
        assert_eq!(x.actions.pending(), 0);
        assert_eq!(x.actions.fired(), 1);
        assert_eq!(x.voice.activity(), Activity::Live);
        x.service_transient_actions();
        assert_eq!(
            x.actions.fired(),
            1,
            "an idle boundary cannot replay an act"
        );
    }
    #[test]
    fn all_notes_off_does_not_cancel_once_but_panic_and_reset_do() {
        let mut x = MxmMono08::default();
        x.actions.submit_once().unwrap();
        x.handle_event(NoteEvent::MidiCC {
            timing: 0,
            channel: 0,
            cc: control_change::ALL_NOTES_OFF,
            value: 0.0,
        });
        assert_eq!(x.actions.pending(), 1);
        x.handle_event(NoteEvent::MidiCC {
            timing: 0,
            channel: 0,
            cc: control_change::ALL_SOUND_OFF,
            value: 0.0,
        });
        assert_eq!(x.actions.pending(), 0);
        x.actions.submit_once().unwrap();
        x.reset();
        assert_eq!(x.actions.pending(), 0);
    }
    #[test]
    fn presets_and_state_cannot_fire_once() {
        let x = MxmMono08::default();
        let fields = x.params.serialize_fields();
        assert!(fields.contains_key("preset"));
        assert!(!fields.keys().any(|key| key.contains("once")));
        assert_eq!(x.actions.pending(), 0);
        let _ = crate::preset::factory(x.params.as_ref());
        assert_eq!(x.actions.pending(), 0);
        let executor = || AsyncExecutor::new(Arc::new(|_| {}), Arc::new(|_| {}));
        let _ = editor::create(
            x.params.clone(),
            x.telemetry.clone(),
            x.actions.clone(),
            executor(),
        );
        let _ = editor::create(
            x.params.clone(),
            x.telemetry.clone(),
            x.actions.clone(),
            executor(),
        );
        assert_eq!(x.actions.pending(), 0, "editor open/reopen is not an act");
    }
    #[test]
    fn developer_channel_is_gated() {
        let mut x = MxmMono08 {
            dev_cc: false,
            ..Default::default()
        };
        let cc = |n, value| NoteEvent::MidiCC {
            timing: 0,
            channel: 0,
            cc: n,
            value,
        };
        x.handle_event(cc(DEV_VIEW_CC, 1.0));
        x.handle_event(cc(DEV_DISCLOSURE_CC, 1.0));
        x.handle_event(cc(DEV_BROWSER_CC, 1.0));
        x.handle_event(cc(DEV_THEME_CC, 1.0 / 127.0));
        assert_eq!(x.telemetry.take_view_request(), None);
        assert_eq!(x.telemetry.take_disclosure_request(), None);
        assert_eq!(x.telemetry.take_browser_request(), None);
        assert_eq!(x.telemetry.take_theme_request(), None);
        x.dev_cc = true;
        x.handle_event(cc(DEV_VIEW_CC, 1.0));
        x.handle_event(cc(DEV_DISCLOSURE_CC, 1.0));
        x.handle_event(cc(DEV_BROWSER_CC, 1.0));
        x.handle_event(cc(DEV_THEME_CC, 1.0 / 127.0));
        assert_eq!(x.telemetry.take_view_request(), Some(127));
        assert_eq!(x.telemetry.take_disclosure_request(), Some(true));
        assert_eq!(x.telemetry.take_browser_request(), Some(true));
        assert_eq!(
            x.telemetry.take_theme_request(),
            Some(1),
            "1 is dark, as mxm_ui::theme::from_index reads it"
        );
    }
    #[test]
    fn event_boundaries_and_internal_blocks_are_bounded() {
        fn boundaries(samples: usize, events: &[usize]) -> Vec<(usize, usize)> {
            let mut out = Vec::new();
            let mut start = 0;
            let mut event = 0;
            while start < samples {
                while event < events.len() && events[event] <= start {
                    event += 1;
                }
                let mut end = (start + MAX_BLOCK_SIZE).min(samples);
                if event < events.len() {
                    end = end.min(events[event]);
                }
                out.push((start, end));
                start = end;
            }
            out
        }
        assert_eq!(
            boundaries(200, &[17, 64, 129]),
            vec![
                (0, 17),
                (17, 64),
                (64, 128),
                (128, 129),
                (129, 193),
                (193, 200)
            ]
        );
        assert!(
            boundaries(4097, &[])
                .iter()
                .all(|(a, b)| b - a <= MAX_BLOCK_SIZE)
        );
    }
    #[test]
    fn hostile_rates_and_sizes_stay_finite_in_the_shell_patch() {
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        for rate in [1_000.0, 1_234.57, 48_000.0, 768_000.0] {
            x.sample_rate = rate;
            x.voice.set_sample_rate(rate);
            x.voice.reset();
            note_on(&mut x, 0, 127);
            x.resolve_topology();
            for _ in 0..257 {
                let patch = x.next_patch();
                let y = x.voice.process(&patch, &x.routing);
                assert!(y.is_finite());
            }
        }
    }
    /// The topology is resolved **once per non-empty call**, whatever the event density, and not
    /// at all for an empty one.
    ///
    /// **A render comparison cannot carry this.** Resolving the same unchanged parameters twice is
    /// identical by construction, so a bit-compare passes whether the call resolves once or once
    /// per interval — it proves behaviour is preserved, which is necessary and not sufficient. The
    /// oracle is therefore structural: it counts.
    #[test]
    fn the_topology_resolves_once_per_non_empty_call_whatever_the_event_density() {
        use super::tests::process_for_test;
        let cc = |timing: u32| NoteEvent::MidiCC {
            timing,
            channel: 0,
            cc: 20,
            value: 0.5,
        };
        // A whole buffer of distinct offsets is the worst case: it used to mean one rebuild each.
        for (frames, events, what) in [
            (512usize, vec![], "no events"),
            (512, vec![cc(0)], "one event"),
            (
                512,
                (0..512u32).map(cc).collect::<Vec<_>>(),
                "an offset per frame",
            ),
            (
                512,
                vec![cc(7), cc(7), cc(7)],
                "three events sharing an offset",
            ),
            (
                64,
                (0..64u32).map(cc).collect::<Vec<_>>(),
                "a short buffer, full of events",
            ),
        ] {
            let mut x = MxmMono08::default();
            process_for_test(&mut x, frames, events);
            assert_eq!(
                x.topology_resolutions, 1,
                "{what}: expected one resolution for a {frames}-frame call, got {}",
                x.topology_resolutions
            );
        }
        // An empty callback resolves nothing -- it drains events and services actions only, and
        // resolving there would arm a newly present route's smoother before an amount change made
        // before the next audible call.
        let mut x = MxmMono08::default();
        process_for_test(&mut x, 0, [cc(0)]);
        assert_eq!(
            x.topology_resolutions, 0,
            "an empty callback must resolve no topology"
        );
    }

    /// An unsmoothed parameter is read **once per non-empty call**, not once per sample, so the
    /// boundary that refreshes it has to be the one a host edit lands on.
    ///
    /// The factory digests cannot see this: every one of them renders a patch that is fixed before
    /// the first sample, so a cache that refreshed *never* would still reproduce all fifty-one. This
    /// moves one after rendering has started and requires the change to reach the sound.
    #[test]
    fn an_unsmoothed_parameter_edit_reaches_the_sound_at_the_next_call() {
        let render = |x: &mut MxmMono08| {
            let mut out = [0.0f32; 256];
            x.render_block_for_test(&mut out);
            out.iter().map(|s| s.abs() as f64).sum::<f64>()
        };
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        note_on(&mut x, 0, 60);
        let before = render(&mut x);

        // `envelope_mode` is unsmoothed, hoisted into `Configured`, and audibly changes the
        // envelope's shape. Anything still reading the stale cache renders `before` again.
        unsafe {
            let ptr = x.params.envelope_mode.as_ptr();
            let _ = ptr._internal_set_normalized_value(1.0);
        }
        note_on(&mut x, 0, 72);
        let after = render(&mut x);
        assert!(
            (after - before).abs() > 1e-9,
            "an unsmoothed edit never reached the render: {before} then {after}"
        );

        // And the cached copy is the parameter's own value, not a snapshot of the default.
        x.resolve_topology();
        assert_eq!(
            x.next_patch().envelope_mode,
            x.params.envelope_mode.value().into(),
            "the interval cache disagrees with the parameter it was read from"
        );
    }

    #[test]
    fn modulation_base_control_is_the_original_low_to_fifty_hertz_range() {
        let x = MxmMono08::default();
        unsafe {
            let ptr = x.params.mod_frequency.as_ptr();
            ptr._internal_set_normalized_value(0.0);
            assert!((x.params.mod_frequency.value() - 0.16).abs() < 1e-6);
            ptr._internal_set_normalized_value(1.0);
            assert!((x.params.mod_frequency.value() - 50.0).abs() < 1e-4);
        }
    }
    /// The high-range switch is a configuration, read at the interval like the other switches, and
    /// it leaves the frequency's stored value alone: the voice multiplies it.
    #[test]
    fn the_high_range_switch_reaches_the_voice_at_the_next_interval() {
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        assert!(
            !x.next_patch().mod_high,
            "a fresh instance is in the low range"
        );
        unsafe {
            let _ = x
                .params
                .mod_high
                .as_ptr()
                ._internal_set_normalized_value(1.0);
        }
        x.resolve_topology();
        let patch = x.next_patch();
        assert!(patch.mod_high);
        assert!((patch.mod_hz - 5.0).abs() < 1e-4, "{}", patch.mod_hz);
    }
    #[test]
    fn modulation_frequency_is_a_smoothed_signal() {
        let x = MxmMono08::default();
        activate_smoothers(&x);
        let initial = x.next_patch().mod_hz;
        unsafe {
            let ptr = x.params.mod_frequency.as_ptr();
            let _ = ptr._internal_set_normalized_value(1.0);
            ptr._internal_update_smoother(48_000.0, false);
        }
        let first = x.next_patch().mod_hz;
        assert!(
            first > initial,
            "the smoother must begin moving immediately"
        );
        assert!(
            first < x.params.mod_frequency.value(),
            "the audible oscillator frequency must not jump to an automated target"
        );
    }
    /// **A range edit under a held bend ramps the pitch; it never steps it.** The channel keeps the
    /// bender's position and the range scales it per sample, so the range is a signal
    /// (`docs/code-review-notes.md` §2). Verified against the defect: with `bend_range.value()` in
    /// `next_patch`, the first sample after the edit is already the whole new range and the largest
    /// per-sample move is ten semitones.
    #[test]
    fn a_range_edit_under_a_held_bend_ramps_rather_than_steps() {
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        note_on(&mut x, 0, 60);
        x.handle_event(NoteEvent::MidiPitchBend {
            timing: 0,
            channel: 0,
            value: 1.0,
        });
        let before = x.next_patch().bend_semitones;
        assert_eq!(before, 2.0, "a full bend at the default range");
        unsafe {
            let ptr = x.params.bend_range.as_ptr();
            let _ = ptr._internal_set_normalized_value(0.5);
            ptr._internal_update_smoother(48_000.0, false);
        }
        let target = x.params.bend_range.value();
        assert_eq!(target, 12.0);

        // The declared ramp is 20 ms, so at 48 kHz the move is spread over about 960 samples.
        let per_sample = (target - before) / (0.020 * 48_000.0);
        let mut previous = before;
        let mut largest_move = 0.0f32;
        let mut settled_at = None;
        for sample in 0..4_800 {
            let bend = x.next_patch().bend_semitones;
            assert!(
                bend >= previous && bend <= target,
                "sample {sample}: {previous} -> {bend} is not a monotonic ramp toward {target}"
            );
            largest_move = largest_move.max(bend - previous);
            if settled_at.is_none() && bend == target {
                settled_at = Some(sample);
            }
            previous = bend;
        }
        assert!(
            largest_move <= per_sample * 1.05,
            "a {largest_move} semitone move in one sample is a step, not a {per_sample} ramp"
        );
        let settled_at = settled_at.expect("the bend never reached the new range");
        assert!(
            settled_at >= 900,
            "the new range arrived after {settled_at} samples, faster than the declared ramp"
        );
    }
    /// **A route steps out and back in, and arrives at its stored depth** — the transition class
    /// `crates/mxm-mono-08-dsp/AGENTS.md` declares. Removed, the pair leaves the live list at the
    /// next interval; edited while absent and re-added, its first sample reads the depth the player
    /// left rather than resuming the ramp its smoother was parked on. Verified against the defect:
    /// without `CvRoutes::arm` in `topology_from`, the first sample reads 0.717 — one ramp step from
    /// Init's 0.72 toward the −0.5 that was set.
    #[test]
    fn a_route_steps_out_and_back_in_at_its_stored_depth() {
        fn set(x: &MxmMono08, id: &str, normalized: f32) {
            let (_, ptr, _) = x
                .params
                .param_map()
                .into_iter()
                .find(|(candidate, _, _)| candidate == id)
                .unwrap_or_else(|| panic!("no parameter {id}"));
            unsafe { ptr._internal_set_normalized_value(normalized) };
        }
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        // Gate 1 from the envelope, which Init wires at 0.72.
        let (destination, source) = (4, 3);
        let (amount_id, presence_id) = routes::ROUTE_IDS[destination][source];
        let pair = (destination as u8, source as u8);
        x.resolve_topology();
        assert!(x.routing.live().contains(&pair));

        set(&x, presence_id, 0.0);
        x.resolve_topology();
        assert!(
            !x.routing.live().contains(&pair),
            "a removed route must leave the live list at the next interval"
        );

        // Edited while absent, as a host or a preset load would: the target moves and nothing
        // advances the smoother toward it.
        let amount = x.params.routes.gate1.amount_param(source);
        set(&x, amount_id, amount.preview_normalized(-0.5));
        amount.smoothed.set_target(48_000.0, amount.value());
        let stored = amount.value();
        assert!((stored - 0.72).abs() > 1.0);

        set(&x, presence_id, 1.0);
        x.resolve_topology();
        x.params.routes.advance(&mut x.routing);
        assert_eq!(
            x.routing.amounts[destination][source], stored,
            "a re-added route must arrive at its stored depth on its first sample"
        );
    }
    #[test]
    fn process_consumes_events_at_the_host_end_boundary_and_in_empty_callbacks() {
        let mut x = MxmMono08::default();
        activate_smoothers(&x);
        let end_panic = NoteEvent::MidiCC {
            timing: 1,
            channel: 0,
            cc: control_change::ALL_SOUND_OFF,
            value: 0.0,
        };
        let status = process_for_test(
            &mut x,
            1,
            [
                NoteEvent::NoteOn {
                    timing: 0,
                    voice_id: None,
                    channel: 0,
                    note: 64,
                    velocity: 0.8,
                },
                end_panic,
            ],
        );
        assert_eq!(status, ProcessStatus::Normal);
        assert_eq!(x.voice.activity(), Activity::Inert);

        x.actions.submit_once().unwrap();
        let status = process_for_test(&mut x, 0, std::iter::empty());
        assert_eq!(status, ProcessStatus::KeepAlive);
        assert_eq!(x.actions.pending(), 0);
        assert_eq!(x.actions.fired(), 1);
    }

    #[test]
    fn the_tail_bound_adds_future_envelope_time_before_the_spring() {
        let x = MxmMono08::default();
        let dry = x.tail_upper_bound_samples();
        unsafe {
            let _ = x.params.reverb.as_ptr()._internal_set_normalized_value(1.0);
        }
        assert_eq!(
            x.tail_upper_bound_samples(),
            dry + Spring::tail_samples(x.sample_rate),
            "the tank may receive its last input only after the dry envelope finishes"
        );
    }
}

/// The pre-conversion reference of `plans/plan-mxm-mono-08-modulation.md` M0.
///
/// ```bash
/// cargo test -p mxm-mono-08 --release --lib baseline -- --ignored --nocapture --test-threads=1
/// ```
///
/// Release only, and only from a quiet machine — a timing taken while something builds is not a
/// measurement (`docs/code-review-notes.md` §3). With `MXM_M0_DUMP=<dir>` the bank's renders are
/// also written there as raw little-endian `f32`, and `the_bank_against_the_m0_dump` compares a
/// later build against them sample by sample.
///
/// **The module uses only what both revisions have** — the seam, the permanent ids of the factory
/// files and the note events — so this same file runs before the conversion and after it. It reads
/// an id the file carries and leaves every other at Init, so the converted tree's larger files and
/// the M0-era ones both load.
#[cfg(test)]
mod baseline {
    use super::*;
    use std::time::Instant;

    const FS: f32 = 48_000.0;
    const BLOCK: usize = 64;

    /// A plugin with every smoother activated (`docs/adding-an-instrument.md` gotcha 13), at the
    /// rate `activate` would give it.
    pub(super) fn plugin() -> MxmMono08 {
        let mut plugin = MxmMono08::default();
        for (_, ptr, _) in plugin.params.param_map() {
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
        plugin.sample_rate = FS;
        plugin.voice.set_sample_rate(FS);
        plugin.voice.reset();
        plugin
    }

    fn key(plugin: &mut MxmMono08, on: bool, note: u8) {
        plugin.handle_event(if on {
            NoteEvent::NoteOn {
                timing: 0,
                voice_id: None,
                channel: 0,
                note,
                velocity: 0.8,
            }
        } else {
            NoteEvent::NoteOff {
                timing: 0,
                voice_id: None,
                channel: 0,
                note,
                velocity: 0.0,
            }
        });
    }

    /// Renders `blocks` blocks of 64 samples onto the end of `out`.
    ///
    /// Per sample this is `render_sample` — what `render_block_for_test` and `process()` both call.
    /// It fed a 173 Hz tone into the external port until that port was dropped (the owner's ruling,
    /// 2026-09-23); the sounds that needed it were replaced, and `BASELINE-M0.md` records which
    /// digests that moved.
    fn run(plugin: &mut MxmMono08, out: &mut Vec<f32>, blocks: usize) {
        for _ in 0..blocks {
            for _ in 0..BLOCK {
                out.push(plugin.render_sample());
            }
        }
    }

    /// **The score every sound plays, fixed forever**: one press; a lower press taking ownership
    /// under last-touch priority; the first press returning when the lower one releases, which
    /// glides at the patch's portamento and retriggers nothing; both releases; and the built-in
    /// spring's tail. It is the shape of the host golden's score
    /// (`plugins/mxm-mono-08/host-tests/tests/golden_audio.rs`) so the two measure the same gestures.
    ///
    /// **No parameter is written**: every sound supplies its own patch, and Init's one wired route
    /// (Gate 1 from the envelope, with the keyboard pulse triggering it) already sounds. A block is
    /// 1.33 ms, so 240 is 320 ms — long enough for a routed pulser to run and a sequence to step.
    fn render_score(plugin: &mut MxmMono08) -> Vec<f32> {
        // The shell compacts the topology at each interval boundary, and this score drives
        // `render_sample` directly. Without this the routing
        // would stay empty, every route absent, and all fifty-one renders would come back silent —
        // a harness fault that reads exactly like a conversion that lost its audio.
        plugin.resolve_topology();
        let mut out = Vec::with_capacity(2_040 * BLOCK);
        key(plugin, true, 60);
        run(plugin, &mut out, 240);
        key(plugin, true, 48);
        run(plugin, &mut out, 240);
        key(plugin, false, 48);
        run(plugin, &mut out, 120);
        key(plugin, false, 60);
        run(plugin, &mut out, 1_440);
        out
    }

    fn throughput(label: &str, plugin: &mut MxmMono08) {
        key(plugin, true, 48);
        let mut out = [0.0f32; BLOCK];
        // Warm the caches: the first blocks pay for page faults, which is not the question.
        for _ in 0..64 {
            plugin.render_block_for_test(&mut out);
        }
        let blocks = 20_000;
        let start = Instant::now();
        for _ in 0..blocks {
            plugin.render_block_for_test(&mut out);
        }
        let taken = start.elapsed().as_secs_f64();
        let samples = (blocks * BLOCK) as f64;
        println!(
            "  mxm-mono-08, {label}, held note: {:.3} ns/sample ({:.0} samples/s)",
            taken * 1e9 / samples,
            samples / taken
        );
    }

    /// Per-sample cost through the plugin's own path, on Init and on one fixed routed patch, three
    /// times each so the spread is visible. **A figure counts only from a quiet machine.**
    ///
    /// **The routed patch is `Random sequence`**, the factory bank's busiest: all four pulse
    /// destinations are driven, so the pulser runs, the sequencer steps and the four held random
    /// voltages redraw, and both of the things this conversion rewrites — the dense weighted sum
    /// and the activity predicate that re-sums it — carry signal.
    #[test]
    #[ignore = "a measurement, not an assertion; release only"]
    fn throughput_of_init_and_a_routed_patch() {
        println!();
        for pass in 1..=3 {
            throughput(&format!("pass {pass}, Init"), &mut plugin());
            let mut routed = plugin();
            apply(&routed, factory("Random sequence"));
            throughput(&format!("pass {pass}, Random sequence"), &mut routed);
        }
        println!();
    }

    /// Per-sample cost with **nothing playing**, after the voice has had time to settle.
    ///
    /// Four seconds of blocks first: a parking patch owes the envelope, the optical gates' own
    /// settle and the spring's tail before `Inert` is reachable at all, and timing the descent
    /// measures the tail rather than the idle it is trying to price.
    fn idle_throughput(label: &str, plugin: &mut MxmMono08) {
        let mut out = [0.0f32; BLOCK];
        for _ in 0..(4 * 48_000 / BLOCK) {
            plugin.render_block_for_test(&mut out);
        }
        let activity = plugin.voice.activity();
        let blocks = 20_000;
        let start = Instant::now();
        for _ in 0..blocks {
            plugin.render_block_for_test(&mut out);
        }
        let taken = start.elapsed().as_secs_f64();
        let samples = (blocks * BLOCK) as f64;
        let per_sample = taken * 1e9 / samples;
        println!(
            "  mxm-mono-08, {label}, no note: {per_sample:.3} ns/sample \
             ({:.2}% of one 48 kHz core, {activity:?})",
            per_sample * 48_000.0 / 1e7
        );
    }

    /// What the instrument costs while **nobody is playing it**, which the held-note gate above
    /// cannot see because it presses a key first.
    ///
    /// *Doing nothing costs nothing* (`plugins/AGENTS.md`) has two halves, and only one of them was
    /// measured anywhere. The reporting half — that a parked voice reaches `Inert` and exact
    /// silence — is held by the DSP crate's `init_is_keyed_and_reaches_exact_idle_silence` and the
    /// parking tests beside it. This is the other half: **what the running half actually costs**,
    /// through the plugin's own per-sample path rather than the voice alone.
    ///
    /// The three cases are chosen to span the range, and the printed `Activity` is as much of the
    /// finding as the nanoseconds:
    ///
    /// - **Init** parks. Its figure is the floor — the shell's own per-sample work, since the voice
    ///   returns at the inert shortcut without advancing a core.
    /// - **`Random sequence`** is the held-note gate's routed patch, kept here so the two read
    ///   against each other. It parks too: its pulse network is driven from the keyboard, so with
    ///   no note nothing re-triggers.
    /// - **`Mod oscillator drone`** cannot park, and is not meant to. Its modulation oscillator
    ///   reaches an open gate with no note held, so the reachability predicate keeps it `Live` and
    ///   the whole voice renders. **A drone costing a drone's price is the correct answer**; the
    ///   case is here so the floor above it is read as a floor and not as what every patch costs.
    #[test]
    #[ignore = "a measurement, not an assertion; release only"]
    fn idle_throughput_of_a_parking_and_a_self_running_patch() {
        println!();
        for pass in 1..=3 {
            idle_throughput(&format!("pass {pass}, Init"), &mut plugin());
            for name in ["Random sequence", "Mod oscillator drone"] {
                let mut patch = plugin();
                apply(&patch, factory(name));
                idle_throughput(&format!("pass {pass}, {name}"), &mut patch);
            }
        }
        println!();
    }

    /// FNV-1a over the raw bits, as `plugins/mxm-mono-08/host-tests/tests/golden_audio.rs` computes it.
    fn digest(samples: &[f32]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for sample in samples {
            for byte in sample.to_bits().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("{hash:016x}")
    }

    fn factory(name: &str) -> &'static str {
        crate::preset::FACTORY_FILES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, text)| *text)
            .unwrap_or_else(|| panic!("no factory sound {name:?}"))
    }

    /// Applies a factory file's stored values by permanent id — **only `v` is read**, as the preset
    /// system reads it — then snaps every smoother, as a fresh load would settle. An id the file
    /// does not carry stays at Init.
    pub(super) fn apply(plugin: &MxmMono08, json: &str) -> usize {
        let designed = crate::preset::Preset::parse(json, CLAP_ID).expect("a factory file parses");
        let map = plugin.params.param_map();
        let mut applied = 0;
        for (id, ptr, _) in &map {
            if let Some(value) = designed.params.get(id) {
                unsafe { ptr._internal_set_normalized_value(value.v) };
                applied += 1;
            }
        }
        for (_, ptr, _) in &map {
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
        applied
    }

    fn slug(name: &str) -> String {
        name.to_lowercase().replace(' ', "-")
    }

    /// Init first, then the fifty in shipped order.
    fn bank() -> Vec<(&'static str, Vec<f32>, usize)> {
        let mut out = Vec::new();
        let mut init = plugin();
        out.push(("Init", render_score(&mut init), 0));
        for (name, json) in crate::preset::FACTORY_FILES {
            let mut p = plugin();
            let applied = apply(&p, json);
            out.push((name, render_score(&mut p), applied));
        }
        out
    }

    /// A shorter score than `render_score`, for the distinctness gate.
    ///
    /// **A separate score on purpose.** `render_score` is pinned forever by `BASELINE-M0.md` and
    /// runs 2 040 blocks; fifty-one of those is a release-only measurement, and the gate has to be
    /// an assertion that runs by default. This plays one note, holds it, releases it and lets the
    /// tail run — 400 blocks, 25 600 samples, about half a second — which is enough to separate an
    /// attack, a decay, a rhythm and a drone.
    pub(super) fn quality_score(plugin: &mut MxmMono08) -> Vec<f32> {
        plugin.resolve_topology();
        let mut out = Vec::with_capacity(1_200 * BLOCK);
        key(plugin, true, 60);
        run(plugin, &mut out, 700);
        key(plugin, false, 60);
        run(plugin, &mut out, 500);
        out
    }

    /// A digest and a peak for Init and every factory sound; with `MXM_M0_DUMP` set, the renders too.
    #[test]
    #[ignore = "a measurement, not an assertion; release only"]
    fn the_bank_digests() {
        let dump = std::env::var_os("MXM_M0_DUMP").map(std::path::PathBuf::from);
        println!();
        for (name, samples, applied) in bank() {
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!(
                "  | `{}` | `{}` | {peak:.4} | {applied} |",
                slug(name),
                digest(&samples)
            );
            if let Some(dir) = &dump {
                let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                std::fs::write(dir.join(format!("{}.f32", slug(name))), bytes).expect("dump");
            }
        }
        println!();
    }

    /// Writes one normalised value by permanent id, as a host automation write would.
    fn set(plugin: &MxmMono08, id: &str, normalised: f32) {
        for (candidate, ptr, _) in plugin.params.param_map() {
            if candidate == id {
                unsafe { ptr._internal_set_normalized_value(normalised) };
                return;
            }
        }
        panic!("no parameter {id}");
    }

    /// Snaps every smoother, as a settled load would.
    fn settle(plugin: &MxmMono08) {
        for (_, ptr, _) in plugin.params.param_map() {
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
    }

    /// The eleven settings the retired `inverterinput` selector could hold, in its declared order,
    /// each beside the route that now expresses it.
    ///
    /// **Ten, where the selector had eleven**: its *Envelope follower* setting (digest
    /// `6ad2bbe82af5569e` in `BASELINE-M0.md`) left with the external input it followed (the
    /// owner's ruling, 2026-09-23), so there is no route left to reproduce it.
    const INVERTER_SETTINGS: [(&str, &str); 10] = [
        ("Key", "cv_inverter_key"),
        ("Pressure", "cv_inverter_pressure"),
        ("Mod oscillator", "cv_inverter_modosc"),
        ("Envelope", "cv_inverter_envelope"),
        ("Pulser", "cv_inverter_pulser"),
        ("Sequence", "cv_inverter_sequencer"),
        ("Random 1", "cv_inverter_random1"),
        ("Random 2", "cv_inverter_random2"),
        ("Random 3", "cv_inverter_random3"),
        ("Random 4", "cv_inverter_random4"),
    ];

    /// The digests those eleven settings produced **before D2**, captured from the post-B4 tree and
    /// recorded in `BASELINE-M0.md`. Held by
    /// [`the_inverter_destination_reaches_every_retired_setting`].
    ///
    /// **Except Key's, re-pinned 2026-09-27 for a ruled change.** The modulation standard made Key
    /// `(note − 60) / 60`, bipolar about middle C, where it was `note / 127`; the retired
    /// selector's Key setting — the inverter reading `1 − note / 127` — is therefore no longer a
    /// route, and the inverter reading the standard Key is what this now holds. It was
    /// `0176ac0f304311bd`. The other nine settings hold to the bit.
    const INVERTER_DIGESTS: [&str; 10] = [
        "f302cab0186b53ac",
        "536e52748f6b84e9",
        "a22761f89e51711e",
        "1ca031c4d7a84ccb",
        "7cf74acf8f6b950d",
        "371e7c00d03b65c2",
        "eaebe5f4148806d9",
        "6063a3fdf132b96a",
        "97f3bc4b8dac7dbc",
        "d9ee6dfe5a92632e",
    ];

    /// One retired selector setting, as the route that replaces it: **exactly one pair present at
    /// full depth**, which is the wiring D2's bit-identity argument is about.
    fn inverter_setting(plugin: &MxmMono08, amount_id: &str) {
        // Init wires the inverter from Random 1, which is the selector's own shipped default.
        // Clear it first, or ten of the eleven settings would sum two sources where the selector
        // could only ever name one.
        set(plugin, "cv_inverter_random1on", 0.0);
        set(plugin, &format!("{amount_id}on"), 1.0);
        set(plugin, amount_id, 1.0);
    }

    /// The bank's score plus channel pressure, which the bank never sends.
    ///
    /// **Why not reuse `render_score`**: that score is fixed forever because fifty-one digests are
    /// pinned to it, and it sends no aftertouch — so under it the `Pressure` setting reads a
    /// constant zero and cannot be told apart from any other source parked at zero.
    fn render_inverter_score(plugin: &mut MxmMono08) -> Vec<f32> {
        plugin.resolve_topology();
        let mut out = Vec::with_capacity(2_040 * BLOCK);
        key(plugin, true, 60);
        plugin.handle_event(NoteEvent::MidiChannelPressure {
            timing: 0,
            channel: 0,
            pressure: 0.6,
        });
        run(plugin, &mut out, 240);
        key(plugin, true, 48);
        run(plugin, &mut out, 240);
        key(plugin, false, 48);
        run(plugin, &mut out, 120);
        key(plugin, false, 60);
        run(plugin, &mut out, 1_440);
        out
    }

    /// The patch the eleven settings are captured against: **every source the selector can name is
    /// moving.**
    ///
    /// The first attempt captured against Init and was worthless — seven of the eleven settings
    /// hashed identically, because Init starts no pulser, steps no sequencer, redraws no random
    /// voltage and sends no pressure, so all seven sources sat at zero, the inverter read a
    /// constant one, and Gate 1 clipped every difference away before it reached audio. A reference
    /// that cannot separate seven of eleven settings would let D2 break them silently.
    ///
    /// So: pulses into the pulser, the sequencer and the random box; `pulserself` so the pulser
    /// keeps cycling rather than firing once; and the inverter into **complex pitch**, which is
    /// exponential and unclamped — timbre, the modulation index and both gates clamp to `0…1`,
    /// which is exactly what flattened the first attempt.
    fn inverter_reference_patch(plugin: &MxmMono08) {
        set(plugin, "pulse_pulser_keyboard", 1.0);
        set(plugin, "pulserself", 1.0);
        set(plugin, "pulse_sequencer_keyboard", 1.0);
        set(plugin, "pulse_sequencer_stage", 1.0);
        set(plugin, "pulse_random_keyboard", 1.0);
        set(plugin, "pulse_random_pulserend", 1.0);
        set(plugin, "cv_complexpitch_inverteron", 1.0);
        // Normalised 0.7 is about 0.16 of full scale — some 0.8 octaves at `CV_OCTAVES` — which is
        // plainly audible without driving the oscillator to an extreme.
        set(plugin, "cv_complexpitch_inverter", 0.7);
    }

    /// **D2's reference.** Each of the eleven settings the retired selector could hold, rendered
    /// through [`inverter_reference_patch`] so the choice actually reaches audio.
    ///
    /// **Captured before D2 converts the selector into an *Inverter input* destination**, because
    /// the plan's §12 claim — *"for each of the eleven `inverterinput` values, the converted pair
    /// renders bit-identical to M0"* — has nothing to compare against otherwise: M0's bank is
    /// fifty-one sounds and not one of them moves this selector, so the whole inverter path is
    /// invisible to it. The chain is M0 → B1/B2's bit-identical bank → this capture → D2.
    ///
    /// The digests this prints are recorded in `BASELINE-M0.md`, and become the constants
    /// `the_inverter_destination_reaches_every_retired_setting` asserts once D2 has landed.
    #[test]
    #[ignore = "a measurement, not an assertion; release only"]
    fn the_inverter_selector_digests() {
        println!();
        let mut seen: Vec<String> = Vec::new();
        for (name, amount_id) in INVERTER_SETTINGS {
            let mut p = plugin();
            inverter_reference_patch(&p);
            inverter_setting(&p, amount_id);
            settle(&p);
            let hash = digest(&render_inverter_score(&mut p));
            println!("  | `{name}` | `{hash}` |");
            seen.push(hash);
        }
        println!();
        // **The reference's own falsification.** If two settings hash alike, the patch is not
        // separating them and the capture is not a reference at all.
        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            INVERTER_SETTINGS.len(),
            "settings that render alike: {seen:?}"
        );
    }

    /// **D2's §12 obligation, as a standing assertion rather than a measurement.** Each of the
    /// eleven settings the retired `inverterinput` selector could hold renders exactly what it
    /// rendered before the conversion, bit for bit.
    ///
    /// This is the whole argument for retiring a permanent id: every sound the selector could make
    /// is still reachable, by one route present at full depth. It is not `#[ignore]`d, because
    /// unlike the fifty-one-sound bank it costs eleven renders rather than fifty-one and it is an
    /// assertion rather than a printed figure.
    #[test]
    fn the_inverter_destination_reaches_every_retired_setting() {
        for ((name, amount_id), expected) in INVERTER_SETTINGS.into_iter().zip(INVERTER_DIGESTS) {
            let mut p = plugin();
            inverter_reference_patch(&p);
            inverter_setting(&p, amount_id);
            settle(&p);
            let actual = digest(&render_inverter_score(&mut p));
            // The pinned digests are Windows': each platform's maths library rounds in its own way,
            // so Linux and macOS render other bits (the owner, 2026-10-06: pin on Windows only).
            if cfg!(target_os = "windows") {
                assert_eq!(
                    actual, expected,
                    "the {name} setting no longer renders what the selector rendered"
                );
            }
        }
    }

    /// Sample-by-sample difference from the M0 renders in `MXM_M0_DUMP`: the worst absolute
    /// difference, and the worst relative to that sound's own peak.
    #[test]
    #[ignore = "a comparison against a local dump, not an assertion"]
    fn the_bank_against_the_m0_dump() {
        let Some(dir) = std::env::var_os("MXM_M0_DUMP").map(std::path::PathBuf::from) else {
            println!("set MXM_M0_DUMP to the directory the_bank_digests wrote");
            return;
        };
        println!();
        let (mut worst_abs, mut worst_rel, mut moved) = (0.0f32, 0.0f32, 0);
        for (name, samples, _) in bank() {
            let bytes = std::fs::read(dir.join(format!("{}.f32", slug(name)))).expect("dump");
            let before: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            assert_eq!(before.len(), samples.len(), "{name}: length changed");
            let diff = before
                .iter()
                .zip(&samples)
                .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
            let peak = before.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rel = if peak > 0.0 { diff / peak } else { diff };
            if diff > 0.0 {
                moved += 1;
            }
            worst_abs = worst_abs.max(diff);
            worst_rel = worst_rel.max(rel);
            println!(
                "  {:<24} max |diff| {diff:.3e}  relative {rel:.3e}",
                slug(name)
            );
        }
        println!(
            "  moved {moved} of 51; worst |diff| {worst_abs:.3e}, worst relative {worst_rel:.3e}\n"
        );
    }
}

/// **Fifty presets have to be fifty sounds.** The gate the shipped bank failed.
///
/// The bank this replaced was fifty names on one patch: `master` and `mix1` identical in all fifty,
/// `complexfreq` identical in forty-six, six presets with byte-identical non-routing parameters, and
/// two to four routes each out of a hundred and fifty-nine. Every test in `preset.rs` passed on it,
/// because each of them asks whether something is *present* — a category, an audible path, a pulse
/// enable behind a random route — and none of them asks whether any two sounds *differ*.
///
/// So this renders every file and compares them with each other. It is deliberately not a digest:
/// a digest proves *unchanged*, which `BASELINE-M0.md` already does and which a bank of fifty
/// identical sounds satisfies perfectly.
#[cfg(test)]
mod bank_quality {
    use super::baseline::{apply, plugin, quality_score};
    use mxm_measure::{level, spectrum};

    /// What one sound is reduced to. Each axis is something a listener would name first, and each
    /// earns its place by separating presets the others could not.
    #[derive(Clone, Copy)]
    struct Fingerprint {
        rms: f64,
        peak: f64,
        centroid: f64,
        flatness: f64,
        attack: f64,
        movement: f64,
    }

    impl Fingerprint {
        const AXES: usize = 6;

        fn axes(&self) -> [f64; Self::AXES] {
            [
                self.rms,
                self.peak,
                self.centroid,
                self.flatness,
                self.attack,
                self.movement,
            ]
        }
    }

    /// The window both spectral axes analyse: `WINDOW` samples around the loudest moment.
    ///
    /// **Not the first `WINDOW` samples.** A patch with a 646 ms attack is still silent a third of a
    /// second in, so analysing the opening gave it a centroid of 0 Hz and a flatness of −142 dB —
    /// two sounds that share nothing would then agree on both axes for the same reason, which is
    /// the opposite of what the fingerprint is for.
    fn loudest(samples: &[f32]) -> &[f32] {
        const WINDOW: usize = 16_384;
        if samples.len() <= WINDOW {
            return samples;
        }
        let peak = samples
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map_or(0, |(i, _)| i);
        let start = peak.saturating_sub(WINDOW / 4).min(samples.len() - WINDOW);
        &samples[start..start + WINDOW]
    }

    /// The centre of mass of the magnitude spectrum, in Hz: "how bright".
    fn centroid(samples: &[f32], rate: f64) -> f64 {
        const N: usize = 16_384;
        let take = samples.len().min(N);
        let mut re: Vec<f64> = samples[..take].iter().map(|s| f64::from(*s)).collect();
        re.resize(N, 0.0);
        let mut im = vec![0.0f64; N];
        if spectrum::fft(&mut re, &mut im).is_none() {
            return 0.0;
        }
        let (mut weighted, mut total) = (0.0, 0.0);
        for bin in 1..N / 2 {
            let magnitude = re[bin].hypot(im[bin]);
            weighted += magnitude * (bin as f64 * rate / N as f64);
            total += magnitude;
        }
        if total > 0.0 { weighted / total } else { 0.0 }
    }

    /// Seconds to half the render's peak: a strike and a swell are not the same sound, and the bank
    /// this replaced moved `attack` on seven of fifty.
    fn attack_seconds(samples: &[f32], rate: f64) -> f64 {
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak <= 0.0 {
            return 0.0;
        }
        samples
            .iter()
            .position(|s| s.abs() >= peak * 0.5)
            .map(|i| i as f64 / rate)
            .unwrap_or(0.0)
    }

    /// How much the loudness contour moves: the difference between a drone and a rhythm.
    ///
    /// It is the spread of the block-by-block RMS **relative to its own mean**, so a quiet moving
    /// patch and a loud moving patch score alike and this axis stays independent of `rms`.
    fn movement(samples: &[f32]) -> f64 {
        const FRAME: usize = 2_048;
        let frames: Vec<f64> = samples.chunks(FRAME).filter_map(level::rms).collect();
        if frames.len() < 2 {
            return 0.0;
        }
        let mean = frames.iter().sum::<f64>() / frames.len() as f64;
        if mean <= 0.0 {
            return 0.0;
        }
        let variance = frames.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / frames.len() as f64;
        variance.sqrt() / mean
    }

    fn fingerprint(samples: &[f32], rate: f64) -> Fingerprint {
        let loud = loudest(samples);
        let widened: Vec<f64> = loud.iter().map(|s| f64::from(*s)).collect();
        Fingerprint {
            rms: level::rms(samples).unwrap_or(0.0),
            peak: f64::from(level::peak(samples).unwrap_or(0.0)),
            centroid: centroid(loud, rate),
            flatness: spectrum::spectral_flatness_db(&widened, rate, 50.0, 16_000.0).unwrap_or(0.0),
            attack: attack_seconds(samples, rate),
            movement: movement(samples),
        }
    }

    /// Every axis divided by its own spread across the bank, so a distance is in comparable units
    /// and no axis dominates merely for being measured in Hz.
    fn normalised(prints: &[Fingerprint]) -> Vec<[f64; Fingerprint::AXES]> {
        let mut scales = [1.0f64; Fingerprint::AXES];
        for (axis, scale) in scales.iter_mut().enumerate() {
            let values: Vec<f64> = prints.iter().map(|p| p.axes()[axis]).collect();
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            *scale = if hi > lo { hi - lo } else { 1.0 };
        }
        prints
            .iter()
            .map(|p| {
                let mut axes = p.axes();
                for (value, scale) in axes.iter_mut().zip(scales) {
                    *value /= scale;
                }
                axes
            })
            .collect()
    }

    fn distance(a: &[f64; Fingerprint::AXES], b: &[f64; Fingerprint::AXES]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// **The distinctness floor, measured rather than chosen.**
    ///
    /// It started at 0.35, which was a guess made before anything had been rendered, and the data
    /// says that number is not reachable: fifty sounds in six correlated dimensions have a median
    /// nearest-neighbour distance around 0.23, so a floor above that would fail a bank whose
    /// sounds are plainly different from one another.
    ///
    /// The three numbers that matter. **The bank this replaced: 0.000** — `Pressure bend` and
    /// `Stage length` rendered identically, and twelve pairs sat at or below 0.01. **This bank:
    /// 0.125** at its closest pair, with a median nearest-neighbour distance of 0.23. **This floor:
    /// 0.10**, a fifth below the closest pair so the bank can be retuned without a spurious
    /// failure, and still an order of magnitude above what its predecessor managed.
    ///
    /// Raising it is welcome. Lowering it to make a new preset fit is the failure this test exists
    /// to catch — change what the preset does instead.
    const FLOOR: f64 = 0.10;

    /// Rendered once for the whole module. Fifty renders is the expensive part of this file in a
    /// debug build, and both assertions and the printed table want the same fifty.
    fn bank_fingerprints() -> &'static [(&'static str, Fingerprint)] {
        static BANK: std::sync::OnceLock<Vec<(&'static str, Fingerprint)>> =
            std::sync::OnceLock::new();
        BANK.get_or_init(|| {
            crate::preset::FACTORY_FILES
                .iter()
                .map(|(name, json)| {
                    let mut p = plugin();
                    apply(&p, json);
                    (*name, fingerprint(&quality_score(&mut p), 48_000.0))
                })
                .collect()
        })
    }

    /// The closest pair in the bank, and the two names it belongs to.
    fn closest_pair(bank: &[(&'static str, Fingerprint)]) -> (f64, &'static str, &'static str) {
        let prints: Vec<_> = bank.iter().map(|(_, print)| *print).collect();
        let axes = normalised(&prints);
        let mut closest = (f64::INFINITY, "", "");
        for (i, a) in axes.iter().enumerate() {
            for (j, b) in axes.iter().enumerate().skip(i + 1) {
                let d = distance(a, b);
                if d < closest.0 {
                    closest = (d, bank[i].0, bank[j].0);
                }
            }
        }
        closest
    }

    #[test]
    fn the_factory_bank_is_fifty_different_sounds() {
        let (apart, one, other) = closest_pair(bank_fingerprints());
        assert!(
            apart >= FLOOR,
            "`{one}` and `{other}` are the same sound ({apart:.3} apart, floor {FLOOR}). \
             Change what they do, not this number."
        );
    }

    /// A preset that cannot be heard, that clips, or that sits perfectly still is not a sound.
    ///
    /// The movement floor is what the old `Stage length` failed on its own documentation
    /// (`BASELINE-M0.md`): it routed Random 1 to sequence length while nothing carried the
    /// sequencer's voltage into audio, so the patch its name described never reached the output.
    #[test]
    fn every_factory_sound_is_audible_finite_and_moves() {
        for (name, print) in bank_fingerprints() {
            assert!(
                print.rms.is_finite() && print.peak.is_finite(),
                "{name} renders a non-finite sample"
            );
            assert!(
                print.rms > 0.005,
                "{name} is inaudible ({:.5} RMS)",
                print.rms
            );
            assert!(print.peak < 1.6, "{name} clips ({:.3} peak)", print.peak);
            assert!(
                print.movement > 0.05,
                "{name} never changes ({:.3} movement)",
                print.movement
            );
        }
    }

    /// Prints the whole bank, nearest neighbour included. Not a gate — the tool for choosing the
    /// next design, and for reading what the two assertions above are working from.
    #[test]
    #[ignore = "a printed table, not an assertion"]
    fn the_bank_fingerprints() {
        let bank = bank_fingerprints();
        let prints: Vec<_> = bank.iter().map(|(_, print)| *print).collect();
        let axes = normalised(&prints);
        println!(
            "\n{:<22}{:>8}{:>8}{:>10}{:>10}{:>9}{:>10}   nearest",
            "sound", "rms", "peak", "centroid", "flatness", "attack", "movement"
        );
        for (i, (name, print)) in bank.iter().enumerate() {
            let mut nearest = (f64::INFINITY, "");
            for (j, other) in axes.iter().enumerate() {
                if i != j && distance(&axes[i], other) < nearest.0 {
                    nearest = (distance(&axes[i], other), bank[j].0);
                }
            }
            println!(
                "{name:<22}{:>8.4}{:>8.3}{:>10.0}{:>10.1}{:>9.3}{:>10.3}   {} ({:.2})",
                print.rms,
                print.peak,
                print.centroid,
                print.flatness,
                print.attack,
                print.movement,
                nearest.1,
                nearest.0
            );
        }
        let (apart, one, other) = closest_pair(bank);
        println!("\n  closest pair: `{one}` and `{other}`, {apart:.3} apart (floor {FLOOR})\n");
    }
}

/// The host's sample rate at activation: the floor the DSP's clamps are safe above.
#[cfg(test)]
mod sample_rate_floor {
    use super::*;
    use mxm_mono_08_dsp::MIN_SAMPLE_RATE;

    struct Activation;

    impl ActivateContext<MxmMono08> for Activation {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _task: EditorTask) {}
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    fn activate_at(plugin: &mut MxmMono08, sample_rate: f32) -> bool {
        plugin.activate(
            &MxmMono08::AUDIO_IO_LAYOUTS[0],
            &BufferConfig {
                sample_rate,
                min_buffer_size: Some(1),
                max_buffer_size: 4096,
                process_mode: ProcessMode::Realtime,
            },
            &mut Activation,
        )
    }

    fn render(plugin: &mut MxmMono08, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames];
        plugin.render_block_for_test(&mut out);
        out
    }

    /// **The floor activates and plays, whatever the parameters say.** Every parameter at its
    /// default, then all at the bottom of their ranges, then all at the top — every route present
    /// at full — with a note held for four seconds at 1 kHz.
    #[test]
    fn the_rate_floor_activates_and_plays_at_every_parameter_extreme() {
        for extreme in [None, Some(0.0), Some(1.0)] {
            let mut plugin = MxmMono08::default();
            for (_, ptr, _) in plugin.params.param_map() {
                if let Some(value) = extreme {
                    let _ = unsafe { ptr._internal_set_normalized_value(value) };
                }
                unsafe { ptr._internal_update_smoother(MIN_SAMPLE_RATE, true) };
            }
            assert!(activate_at(&mut plugin, MIN_SAMPLE_RATE), "{extreme:?}");
            assert_eq!(plugin.sample_rate, MIN_SAMPLE_RATE);
            plugin.handle_event(NoteEvent::NoteOn {
                timing: 0,
                voice_id: None,
                channel: 0,
                note: 48,
                velocity: 0.8,
            });
            let out = render(&mut plugin, 4_000);
            assert!(out.iter().all(|s| s.is_finite()), "{extreme:?}");
        }
    }

    /// **A rate the DSP's clamps cannot hold is refused at activation.** `f32::clamp` panics on
    /// a NaN or inverted bound, so a NaN rate or one low enough to cross a corner's floor over its
    /// Nyquist fraction panicked on the audio thread. A refusal leaves the plugin as it was.
    #[test]
    fn activation_refuses_a_non_finite_rate_and_any_below_the_floor() {
        for unsupported in [
            MIN_SAMPLE_RATE.next_down(),
            100.0,
            20.0,
            1.0,
            0.0,
            -48_000.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let mut refused = MxmMono08::default();
            assert!(
                !activate_at(&mut refused, unsupported),
                "accepted {unsupported} Hz"
            );
            assert_eq!(refused.sample_rate, 48_000.0, "{unsupported} Hz");
        }
    }
}

/// **A synced value reaches the patch** (`plans/plan-tempo-sync-controls.md`): what a sync resolved
/// for this callback is what the DSP is given, and with none the free value is.
#[cfg(test)]
mod tempo_sync_path {
    use super::*;

    #[test]
    fn the_synced_clock_period_is_the_patchs() {
        use nice_plug::params::InternalParamMut;
        let mut plugin = MxmMono08 {
            host_tempo: Some(120.0),
            ..Default::default()
        };
        plugin.resolve_topology();
        let free = plugin.configured.pulser_period_s;
        assert_eq!(free, plugin.params.pulser_period.value(), "off is the knob");
        // SAFETY: the parameters are this test's own and nothing else reads them.
        unsafe {
            let _ = plugin
                .params
                .pulser_sync
                ._internal_set_normalized_value(1.0);
        }
        plugin.resolve_topology();
        let synced = plugin
            .params
            .synced_pulser_period(Some(120.0))
            .expect("synced at a tempo");
        assert_eq!(plugin.configured.pulser_period_s, synced);
        plugin.host_tempo = None;
        plugin.resolve_topology();
        assert_eq!(
            plugin.configured.pulser_period_s, free,
            "no tempo is the knob"
        );
    }
}

/// **Activation forgets the last session's tempo and resolved syncs**: the first callback reports the
/// tempo, so nothing — the audio, or an editor frame before it — starts from the previous session's
/// divisions.
#[cfg(test)]
mod activation_forgets_the_tempo {
    use super::*;

    #[test]
    fn activation_forgets_the_last_tempo_and_resolved_syncs() {
        use nice_plug::prelude::Plugin as _;
        let mut plugin = MxmMono08::default();
        plugin.telemetry.tempo.publish(Some(120.0));
        plugin.host_tempo = Some(120.0);
        let layout = MxmMono08::AUDIO_IO_LAYOUTS[0];
        let config = BufferConfig {
            sample_rate: 48_000.0,
            min_buffer_size: None,
            max_buffer_size: 512,
            process_mode: ProcessMode::Realtime,
        };
        let _ = plugin.activate(&layout, &config, &mut NoInit);
        assert_eq!(plugin.telemetry.tempo.get(), None);
        assert_eq!(plugin.host_tempo, None);
    }

    /// An activation context that asks nothing of a host.
    struct NoInit;

    impl ActivateContext<MxmMono08> for NoInit {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _task: <MxmMono08 as Plugin>::BackgroundTask) {}
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }
}

/// What a player reads — on hover in the editor, and in a host's plugin browser — speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}
