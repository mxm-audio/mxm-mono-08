//! Render one short passage per defining mechanism. This proves the framework-free path runs; it
//! is not a listening comparison with hardware, and fidelity remains unverified.
/// Writes a listening demo, applying this collection's demo headroom law **at the call site**.
///
/// `mxm_audio_file` encodes what it is given and applies no gain — normalisation is a judgement
/// about the material and the file crate carries no policy. The law here is the one the
/// six hand-written writers all applied internally: leave 2 % of headroom, and scale down further if
/// the material is over full scale.
fn write_demo(path: &str, interleaved: &[f32], channels: u16, rate: u32) {
    let peak = mxm_measure::level::peak(interleaved)
        .expect("a rendered demo is finite; a NaN here is a DSP defect, not a level");
    let gain = if peak > 1.0 { 0.98 / peak } else { 0.98 };
    let scaled: Vec<f32> = interleaved.iter().map(|s| s * gain).collect();
    mxm_audio_file::write(
        path,
        &scaled,
        channels,
        rate,
        mxm_audio_file::Target::Wav(mxm_audio_file::Bits::Sixteen),
    )
    .expect("the demo is written");
}

/// **Where this demo's channel count and sample rate are decided — once, for `main` and for the
/// test below.** Both call this, so a change to either constant changes both paths and the test's
/// literal expectations catch it. With the two supplied separately at each site, a `main` passing
/// the wrong channel count left the test perfectly green.
const DEMO_CHANNELS: u16 = 1;

fn write_demo_file(path: &str, interleaved: &[f32]) {
    write_demo(path, interleaved, DEMO_CHANNELS, FS as u32);
}
use mxm_mono_08_dsp::oscillator::{ComplexEndpoint, ModWave};
use mxm_mono_08_dsp::routing::{CvDestination, CvSource, PulseDestination, PulseSource, Routing};
use mxm_mono_08_dsp::voice::{Gate2Source, ModulationMode, NoteId, Params, Voice};
const FS: f32 = 48_000.0;
fn note(key: u8) -> NoteId {
    NoteId {
        voice_id: None,
        channel: 0,
        key,
    }
}
fn passage(out: &mut Vec<f32>, v: &mut Voice, p: &Params, routing: &Routing, seconds: f32) {
    for _ in 0..(seconds * FS) as usize {
        out.push(v.process(p, routing));
    }
}
fn main() {
    let (mut v, mut p) = (Voice::new(), Params::default());
    let mut routing = Routing::init();
    v.set_topology(&routing);
    let mut out = Vec::new();
    eprintln!("0s keyed optical strike");
    v.note_on(note(60));
    passage(&mut out, &mut v, &p, &routing, 2.0);
    v.note_off(None, 0, 60);
    passage(&mut out, &mut v, &p, &routing, 1.0);
    eprintln!("3s separate-core timbre and square endpoint");
    p.timbre = 0.9;
    p.wave_mix = 0.15;
    v.note_on(note(64));
    passage(&mut out, &mut v, &p, &routing, 1.5);
    p.complex_endpoint = ComplexEndpoint::Square;
    p.wave_mix = 0.8;
    passage(&mut out, &mut v, &p, &routing, 1.5);
    v.note_off(None, 0, 64);
    eprintln!("6s five-stage CV/pulse feedback rhythm");
    p = Params::default();
    p.pulser_self = true;
    p.pulser_period_s = 0.32;
    p.sequence_levels = [0.0, 0.3, 0.65, 0.2, 0.9];
    p.sequence_pulses = [true, false, true, true, false];
    routing = Routing::new();
    routing.pulses[PulseDestination::Sequencer.index()][PulseSource::PulserEnd.index()] = true;
    routing.pulses[PulseDestination::Envelope.index()][PulseSource::SequenceStage.index()] = true;
    routing.present[CvDestination::PulserPeriod.index()][CvSource::Sequencer.index()] = true;
    routing.amounts[CvDestination::PulserPeriod.index()][CvSource::Sequencer.index()] = 0.7;
    v.set_topology(&routing);
    v.once();
    passage(&mut out, &mut v, &p, &routing, 5.0);
    v.all_sound_off();
    eprintln!("11s low-rate monitored oscillator through Gate 2");
    p = Params::default();
    routing = Routing::init();
    v.set_topology(&routing);
    p.mix1 = 0.0;
    p.mix2 = 0.8;
    p.gate2_source = Gate2Source::ModOsc;
    p.gate2_level = 1.0;
    p.mod_hz = 18.0;
    p.mod_wave = ModWave::Square;
    passage(&mut out, &mut v, &p, &routing, 2.0);
    eprintln!("13s modulation oscillator frequency-modulating the complex oscillator");
    p = Params::default();
    p.modulation_mode = ModulationMode::Fm;
    p.modulation_index = 0.6;
    p.mod_hz = 40.0;
    v.note_on(note(57));
    passage(&mut out, &mut v, &p, &routing, 2.0);
    v.note_off(None, 0, 57);
    eprintln!("15s opposite-polarity two-gate mix into spring");
    p = Params::default();
    routing = Routing::init();
    v.set_topology(&routing);
    p.gate1_level = 0.8;
    p.gate2_level = 0.8;
    p.mix1 = 1.0;
    p.mix2 = 0.65;
    p.reverb = 0.45;
    passage(&mut out, &mut v, &p, &routing, 2.0);
    p.gate1_level = 0.0;
    p.gate2_level = 0.0;
    passage(&mut out, &mut v, &p, &routing, 5.0);
    write_demo_file("mxm-mono-08-easel-demo.wav", &out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The demo's own write path, exercised through the same wrapper `main` uses.
    ///
    /// The shared encoder is proved in `mxm-measure` against fixed header and payload bytes. What
    /// that cannot see is *this* file later writing the wrong channel count or rate, so the
    /// expectations here are **literals** — the facts about this instrument — rather than the
    /// constants under test.
    #[test]
    fn the_demo_write_path_produces_a_playable_file() {
        let frames = 256;
        let samples: Vec<f32> = (0..frames * DEMO_CHANNELS as usize)
            .map(|i| {
                let t = i as f32 / 48_000 as f32;
                // Past full scale, so the headroom branch is taken rather than skipped.
                1.6 * (std::f32::consts::TAU * 220.0 * t).sin()
            })
            .collect();

        let mut path = std::env::temp_dir();
        path.push(format!("easel-demo-demo-{}.wav", std::process::id()));
        write_demo_file(path.to_str().expect("a utf-8 path"), &samples);

        let read = mxm_audio_file_decode::decode_file(
            &path,
            &mxm_audio_file_decode::Limits::new(
                usize::MAX,
                mxm_audio_file_decode::AtLimit::Refuse,
                mxm_audio_file_decode::Keep::AllUpTo(2),
            ),
        )
        .expect("the demo file parses");
        assert_eq!(read.channels, 1, "the demo wrote the wrong channel count");
        assert_eq!(
            read.sample_rate, 48_000,
            "the demo wrote the wrong sample rate"
        );
        assert_eq!(read.frames(), frames, "the demo dropped or invented frames");

        // The headroom law, asserted rather than assumed: a source at 1.6 comes back just under
        // full scale, not clipped to it and not left loud.
        let peak = mxm_measure::level::peak(&read.interleaved).expect("a finite file");
        assert!(
            (0.97..=0.985).contains(&peak),
            "the 0.98 headroom law did not run: peak {peak}"
        );
        std::fs::remove_file(&path).ok();
    }

    /// **The whole production path, `main` included.** This is what a writer test cannot otherwise
    /// reach: the render itself, the buffer `main` chooses, and the channel count and rate it hands
    /// over. An empty or truncated render fails here and nowhere else.
    ///
    /// `#[ignore]`d because it renders the demo in full, which is tens of seconds of audio; run it
    /// with `cargo test --all-targets -- --ignored` when the demo or its write path changes.
    #[test]
    #[ignore = "renders the whole demo; run with --ignored"]
    fn the_whole_demo_renders_and_writes_a_playable_file() {
        main();
        let read = mxm_audio_file_decode::decode_file(
            "mxm-mono-08-easel-demo.wav",
            &mxm_audio_file_decode::Limits::new(
                usize::MAX,
                mxm_audio_file_decode::AtLimit::Refuse,
                mxm_audio_file_decode::Keep::AllUpTo(2),
            ),
        )
        .expect("the demo file parses");
        assert_eq!(read.channels, 1, "the demo wrote the wrong channel count");
        assert_eq!(
            read.sample_rate, 48000,
            "the demo wrote the wrong sample rate"
        );
        assert!(
            read.frames() > 48000,
            "the demo rendered under a second of audio"
        );
        let peak = mxm_measure::level::peak(&read.interleaved).expect("a finite render");
        assert!(peak > 0.1, "the demo rendered near-silence: peak {peak}");

        // `main` writes into the working directory, which under `cargo test` is the crate root.
        // Leaving it there drops an untracked WAV into the tree every time this runs.
        std::fs::remove_file("mxm-mono-08-easel-demo.wav").ok();
    }
}
