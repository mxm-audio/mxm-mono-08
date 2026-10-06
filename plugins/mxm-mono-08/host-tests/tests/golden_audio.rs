//! Golden audio for mxm-mono-08 through the real MXM Player host path.
//!
//! A deliberate DSP or shell change should fail this test. Listen to the emitted WAV, establish
//! that the move was intended, and update the digest in the same change; never update it silently.

use mxm_player::events::input::Payload;
use mxm_player::session::{FRAMES_PER_BLOCK, Session};
use mxm_player_harness::app_harness;
use std::path::PathBuf;
const PLUGIN: &str = "dk.mxm.mxm-mono-08";
/// First pinned with the non-editor shell. Measured by `behaviour`; not yet listened to.
const GOLDEN_DIGEST: &str = "19a90ffe21920f05";
const GOLDEN_SAMPLES: usize = 86 * FRAMES_PER_BLOCK * 2;
fn bundle() -> Option<(PathBuf, PathBuf)> {
    let dir = app_harness::any_bundled_dir()?;
    let file = dir.join("mxm-mono-08.clap");
    file.exists().then_some((dir, file))
}
fn set(s: &mut Session, name: &str, f: f64) {
    let p = s.state().param(name).unwrap().clone();
    s.app().engine_mut().push_gui_event(Payload::ParamValue {
        param_id: p.id,
        value: p.min + f * (p.max - p.min),
    });
}
/// Fixed forever: one press, a lower press taking ownership, the first returning, sequence routing,
/// both releases, and the built-in spring tail.
fn score(s: &mut Session) -> Result<(), String> {
    set(s, "Reverb", 0.35);
    set(s, "Sequencer trigger from Key", 1.0);
    // The route the amount below travels on. Added with the routing conversion: a pulse enable is
    // its own presence, but a CV pair needs one, and Init wires only Gate 1 from the envelope. The
    // amount's 10 ms ramp still completes inside the four silent blocks before the first note, so
    // the pinned digest is expected to hold — a move here is a defect to find, not a digest to
    // re-pin (`plans/plan-mxm-mono-08-modulation.md` §6).
    set(s, "Complex timbre from Sequencer on", 1.0);
    // +36 %: 0.8 on the square-law fader every route had, 0.68 on the linear one the modulation
    // standard gave it — the same plain amount to the bit, so the digest holds.
    set(s, "Complex timbre from Sequencer", 0.68);
    s.advance_blocks(4)?;
    s.app().note_on(60, 0.8);
    s.advance_blocks(16)?;
    s.app().note_on(48, 0.7);
    s.advance_blocks(14)?;
    s.app().note_off(48);
    s.advance_blocks(14)?;
    s.app().note_off(60);
    s.advance_blocks(38)
}
fn render(name: &str) -> Option<(Vec<f32>, PathBuf)> {
    let (dir, file) = bundle()?;
    let mut s = Session::scratch(name, vec![dir]);
    s.load(&file, PLUGIN);
    score(&mut s).unwrap();
    let samples = s.captured();
    let (wav, _) = s.write_artifacts(name).unwrap();
    Some((samples, wav))
}
#[test]
fn the_fixed_real_host_score_has_not_moved() {
    let Some((samples, wav)) = render("golden-mono-08") else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-08 --release`");
        return;
    };
    assert_eq!(samples.len(), GOLDEN_SAMPLES);
    assert!(samples.iter().any(|x| x.abs() > 1e-4));
    let actual = digest(&samples);
    assert_eq!(
        actual,
        GOLDEN_DIGEST,
        "render moved; listen to {} and, if intended, pin {actual}",
        wav.display()
    );
}
#[test]
fn the_reference_is_sensitive_to_the_complex_oscillator() {
    let Some((dir, file)) = bundle() else {
        eprintln!("skipping: run `cargo xtask bundle mxm-mono-08 --release`");
        return;
    };
    let mut s = Session::scratch("golden-mono-08-sensitive", vec![dir]);
    s.load(&file, PLUGIN);
    set(&mut s, "Complex timbre", 0.98);
    score(&mut s).unwrap();
    assert_ne!(digest(&s.captured()), GOLDEN_DIGEST);
}
fn digest(samples: &[f32]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for s in samples {
        for b in s.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}
