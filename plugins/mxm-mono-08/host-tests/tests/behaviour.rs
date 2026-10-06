//! mxm-mono-08 rendered through MXM Player's ordinary bundle-discovery and hosting path.
//!
//! Bundle-dependent tests skip with the build command instead of linking product-specific code
//! into the player. Build first with `cargo xtask bundle mxm-mono-08 --release`.

use mxm_player::events::input::Payload;
use mxm_player::session::{FRAMES_PER_BLOCK, Session};
use mxm_player_harness::app_harness;

/// The session's render rate, named here because the shared probe takes it as an argument.
const SAMPLE_RATE: f64 = 48_000.0;
use std::path::PathBuf;

const PLUGIN: &str = "dk.mxm.mxm-mono-08";
const SKIP: &str = "skipping: run `cargo xtask bundle mxm-mono-08 --release`";
fn bundle() -> Option<(PathBuf, PathBuf)> {
    let dir = app_harness::any_bundled_dir()?;
    let file = dir.join("mxm-mono-08.clap");
    file.exists().then_some((dir, file))
}
fn session(name: &str) -> Option<Session> {
    let (dir, file) = bundle()?;
    let mut s = Session::scratch(name, vec![dir]);
    s.load(&file, PLUGIN);
    Some(s)
}
use mxm_measure::channels::left;
/// Peak magnitude of a capture.
///
/// **A shim over `mxm-measure`, and the `expect` is the point.** The shared ruler reports absence for
/// a **non-finite** buffer rather than the largest number in it, because `f32::max` would otherwise
/// let a render that is half NaN measure as perfectly healthy — and then pass every "is it quiet?"
/// assertion below. Panicking here is the loud failure that behaviour deserves.
fn peak(samples: &[f32]) -> f32 {
    mxm_measure::level::peak(samples).expect("the capture is finite")
}
/// How much of one frequency is in a captured window — **a relative figure, not an amplitude.**
///
/// Two reasons it is relative, and both matter to anyone quoting a number from these tests:
///
/// - **The capture length is the session's, not ours.** `component_amplitude` reads a component's
///   true amplitude only over a whole number of cycles; these windows are whole blocks, so the
///   reading carries spectral leakage. Comparing one pitch against another in the same window is
///   sound — the leakage is common to both — and calling the result an absolute amplitude is not.
/// - **The absolute value moved by 6 dB with the migration to the shared probe**, which is a
///   correction rather than a regression: every local copy of this helper computed `|X|/N`, half a
///   component's amplitude, and the shared probe reports the amplitude. A figure quoted from an
///   older run of these tests is 6 dB low.
///
/// **Absence panics rather than reading as zero.** The probe declines for two reasons — an empty
/// window, which cannot happen here, and a **non-finite render**, which can. Folding that into `0.0`
/// would let a NaN-producing plugin sail through every "quieter than" and "silent" assertion below,
/// which is the precise failure the shared crate's result-form contract exists to prevent.
fn magnitude_at(samples: &[f32], hz: f64) -> f64 {
    mxm_measure::spectrum::component_amplitude(samples, hz, SAMPLE_RATE)
        .expect("the capture is non-empty and finite")
}
fn brightness(x: &[f32]) -> f32 {
    if x.len() < 2 {
        return 0.0;
    }
    x.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / (x.len() - 1) as f32
}
fn set(s: &mut Session, name: &str, fraction: f64) {
    let p = s
        .state()
        .param(name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .clone();
    s.app().engine_mut().push_gui_event(Payload::ParamValue {
        param_id: p.id,
        value: p.min + fraction * (p.max - p.min),
    });
    s.advance_blocks(4).unwrap();
}
fn capture(s: &mut Session, blocks: u64) -> Vec<f32> {
    s.clear_capture();
    s.advance_blocks(blocks).unwrap();
    let x = s.captured();
    let skip = (FRAMES_PER_BLOCK * 2 * 3).min(x.len());
    left(&x[skip..])
}

#[test]
fn player_discovers_the_bundle_and_its_control_map_without_product_code() {
    let Some((dir, _)) = bundle() else {
        eprintln!("{SKIP}");
        return;
    };
    let mut app = app_harness::AppHarness::new("mono08-discovery", vec![dir]);
    app.harness.state_mut().rescan();
    app.run();

    let state = app.state();
    assert!(
        state.lists_plugin(PLUGIN),
        "the ordinary scan did not discover {PLUGIN}: {:?}",
        state
            .found
            .iter()
            .map(|found| &found.id)
            .collect::<Vec<_>>()
    );
    assert!(
        app.app().control_map().knows_instrument(PLUGIN),
        "the map beside the discovered bundle was not loaded"
    );
}

#[test]
fn bundle_reports_the_full_generic_parameter_surface() {
    let Some(mut s) = session("mono08-params") else {
        eprintln!("{SKIP}");
        return;
    };
    // 160 before the routing conversion, 340 after it, 369 after D2, 370 with the modulation
    // oscillator's high range. **349 since the external input was dropped** (owner, 2026-09-23):
    // 149 routes with an amount and a presence each — nine destinations of fifteen sources, and
    // the inverter's input of fourteen, because the inverter is not offered to itself — the
    // twelve trigger enables, and the thirty-nine panel controls. 350 with the clock period's tempo
    // sync (2026-09-25); 380 with the standard Amplitude's fifteen routes (2026-09-27).
    assert_eq!(s.state().plugin.as_ref().unwrap().params.len(), 380);
    for name in [
        "Complex frequency",
        "Complex timbre",
        "Sequence length",
        "Step 5 trigger",
        "LPG 2 level from Random 4",
        // The tenth destination, so this shows the host actually seeing D2's routes rather than
        // only a changed count.
        "Inverter input from Random 1",
        "Clock trigger from Clock",
    ] {
        assert!(s.state().param(name).is_some(), "missing {name}");
    }
    let reverb = s.state().param("Reverb").unwrap().id;
    assert!(s.app().control_map().knows_instrument(PLUGIN));
    assert_eq!(
        s.app().control_map().param_for(PLUGIN, "fx.reverb"),
        Some(reverb)
    );
}
#[test]
fn rest_is_exactly_silent() {
    let Some(mut s) = session("mono08-rest") else {
        eprintln!("{SKIP}");
        return;
    };
    s.advance_blocks(20).unwrap();
    assert_eq!(peak(&s.captured()), 0.0);
}
#[test]
fn a_note_is_audible_and_its_tail_reaches_exact_silence() {
    let Some(mut s) = session("mono08-note") else {
        eprintln!("{SKIP}");
        return;
    };
    s.app().note_on(57, 0.8);
    let held = capture(&mut s, 35);
    assert!(peak(&held) > 0.01, "peak {}", peak(&held));
    s.app().note_off(57);
    s.advance_blocks(600).unwrap();
    s.clear_capture();
    s.advance_blocks(30).unwrap();
    assert_eq!(
        peak(&s.captured()),
        0.0,
        "the finite optical/envelope tail must go inert"
    );
}
#[test]
fn the_latest_touch_takes_pitch_and_the_held_touch_returns() {
    let Some(mut s) = session("mono08-touch-owner") else {
        eprintln!("{SKIP}");
        return;
    };
    s.app().note_on(57, 0.8);
    let low = capture(&mut s, 28);
    assert!(magnitude_at(&low, 110.0) > magnitude_at(&low, 220.0));
    s.app().note_on(69, 0.8);
    let high = capture(&mut s, 28);
    assert!(magnitude_at(&high, 220.0) > magnitude_at(&high, 110.0));
    s.app().note_off(69);
    let returned = capture(&mut s, 28);
    assert!(magnitude_at(&returned, 110.0) > magnitude_at(&returned, 220.0));
    s.app().note_off(57);
}

#[test]
fn a_host_parameter_edit_reaches_the_rendered_sound() {
    let Some(mut s) = session("mono08-timbre") else {
        eprintln!("{SKIP}");
        return;
    };
    s.app().note_on(52, 0.8);
    let dark = capture(&mut s, 30);
    s.app().note_off(52);
    s.advance_blocks(100).unwrap();
    set(&mut s, "Complex timbre", 0.95);
    s.app().note_on(52, 0.8);
    let bright = capture(&mut s, 30);
    assert!(
        brightness(&bright) > 1.15 * brightness(&dark),
        "{} vs {}",
        brightness(&bright),
        brightness(&dark)
    );
}

#[test]
fn clap_state_restores_the_same_instances_complete_parameter_patch() {
    let Some(mut s) = session("mono08-state") else {
        eprintln!("{SKIP}");
        return;
    };
    assert!(
        s.app()
            .run_cli_command("set Complex_timbre 0.83")
            .contains("\"ok\"")
    );
    assert!(
        s.app()
            .run_cli_command("set Step_5_level 0.17")
            .contains("\"ok\"")
    );
    s.advance_blocks(4).unwrap();
    let before_timbre = s.state().param("Complex timbre").unwrap().value;
    let before_stage = s.state().param("Step 5 level").unwrap().value;
    assert!(s.app().run_cli_command("dumpstate").contains("\"ok\""));

    assert!(
        s.app()
            .run_cli_command("set Complex_timbre 0.09")
            .contains("\"ok\"")
    );
    assert!(
        s.app()
            .run_cli_command("set Step_5_level 0.91")
            .contains("\"ok\"")
    );
    s.advance_blocks(4).unwrap();
    assert!((s.state().param("Complex timbre").unwrap().value - before_timbre).abs() > 0.5);
    assert!((s.state().param("Step 5 level").unwrap().value - before_stage).abs() > 0.5);

    assert!(s.app().run_cli_command("loadstate").contains("\"ok\""));
    s.advance_blocks(4).unwrap();
    // Read the plugin itself. `PlayerState` intentionally corrects parameter readback to the
    // player's remembered patch while sequencing, which is a different host-side contract.
    let actual = s.app().engine_mut().read_params();
    let after_timbre = actual
        .params
        .iter()
        .find(|p| p.name == "Complex timbre")
        .unwrap()
        .value;
    let after_stage = actual
        .params
        .iter()
        .find(|p| p.name == "Step 5 level")
        .unwrap()
        .value;
    assert!(
        (after_timbre - before_timbre).abs() < 1e-6,
        "Complex timbre restored {after_timbre}, expected {before_timbre}"
    );
    assert!(
        (after_stage - before_stage).abs() < 1e-6,
        "Step 5 restored {after_stage}, expected {before_stage}"
    );
}

#[test]
fn the_shipped_five_stage_sequencer_changes_a_keyed_patch() {
    let Some(mut s) = session("mono08-sequence") else {
        eprintln!("{SKIP}");
        return;
    };
    set(&mut s, "Sequencer trigger from Key", 1.0);
    // The pulse enable above *is* its presence, but a CV route has two parameters: an amount only
    // reaches audio through a route that exists, and Init wires Gate 1 from the envelope alone.
    set(&mut s, "Complex timbre from Sequencer on", 1.0);
    set(&mut s, "Complex timbre from Sequencer", 0.9);
    let mut measures = Vec::new();
    for note in [48u8, 50, 52, 53, 55] {
        s.app().note_on(note, 0.8);
        measures.push(brightness(&capture(&mut s, 10)));
        s.app().note_off(note);
        s.advance_blocks(25).unwrap();
    }
    let (lo, hi) = measures
        .iter()
        .fold((f32::MAX, 0.0f32), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    assert!(hi > 1.1 * lo, "five stage readings were {measures:?}");
}
#[test]
fn reverb_keeps_a_real_tail_then_returns_to_exact_silence() {
    let Some(mut s) = session("mono08-tail") else {
        eprintln!("{SKIP}");
        return;
    };
    set(&mut s, "Reverb", 0.8);
    s.app().note_on(60, 0.8);
    s.advance_blocks(20).unwrap();
    s.app().note_off(60);
    let early = capture(&mut s, 80);
    assert!(peak(&early) > 1e-5, "spring tail was cut off");
    s.advance_blocks(900).unwrap();
    s.clear_capture();
    s.advance_blocks(20).unwrap();
    assert_eq!(peak(&s.captured()), 0.0);
}

/// Real CLAP boundary for the non-parameter Once wake. The parent starts a dedicated child with
/// immutable probe configuration, and the child uses atomic file creation to signal the debug
/// bundle's lifecycle-bounded producer. No process mutates its environment after startup.
#[test]
#[ignore = "requires `cargo xtask bundle mxm-mono-08` (debug bundle)"]
fn editor_once_wakes_a_sleeping_real_host_and_produces_one_burst() {
    const CHILD: &str = "MXM_MONO_08_TEST_ONCE_CHILD";
    const DIRECTORY: &str = "MXM_MONO_08_TEST_ONCE_DIR";
    const TEST: &str = "editor_once_wakes_a_sleeping_real_host_and_produces_one_burst";

    if std::env::var_os(CHILD).is_none() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the system clock predates the Unix epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mxm-mono-08-once-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).expect("could not create the private Once probe directory");
        let outcome = std::process::Command::new(
            std::env::current_exe().expect("the test executable has no path"),
        )
        .args(["--exact", TEST, "--ignored", "--nocapture"])
        .env(CHILD, "1")
        .env(DIRECTORY, &directory)
        .status();
        std::fs::remove_dir_all(&directory).expect("could not remove the Once probe directory");
        assert!(
            outcome
                .expect("could not start the isolated Once test process")
                .success(),
            "the isolated Once test process failed"
        );
        return;
    }

    let directory = PathBuf::from(
        std::env::var_os(DIRECTORY).expect("the child has no immutable probe directory"),
    );
    let Some(mut s) = session("mono08-real-host-once") else {
        eprintln!("{SKIP}");
        return;
    };
    assert!(
        (0..100).any(|_| {
            if directory.join("ready").is_file() {
                true
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                false
            }
        }),
        "the debug editor producer did not claim the private probe"
    );
    // **Both halves, deliberately.** This test asserts the instance falls inert; with the route
    // absent it would fall silent because nothing is wired, passing for a reason that has nothing
    // to do with the activity predicate it exists to check.
    set(&mut s, "LPG 1 level from Clock ramp on", 1.0);
    set(&mut s, "LPG 1 level from Clock ramp", 1.0);
    s.advance_blocks(240).unwrap();
    s.clear_capture();
    s.advance_blocks(20).unwrap();
    assert_eq!(peak(&s.captured()), 0.0, "instance did not become inert");
    assert!(
        !s.app().engine_mut().process_request_pending(),
        "an unrelated process request remained before Once"
    );

    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("fire"))
        .expect("could not atomically signal the editor producer");
    let requested = (0..100).any(|_| {
        if s.app().engine_mut().process_request_pending() {
            true
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10));
            false
        }
    });
    assert!(
        requested,
        "editor producer did not cross nice-plug into CLAP request_process"
    );

    s.clear_capture();
    s.advance_blocks(180).unwrap();
    let fired = s.captured().to_vec();
    assert!(peak(&fired) > 1e-4, "Once did not wake and fire the pulser");
    let active: Vec<_> = fired
        .chunks(FRAMES_PER_BLOCK * 2)
        .map(|block| peak(block) > 1e-5)
        .collect();
    let bursts = active
        .iter()
        .enumerate()
        .filter(|(index, on)| **on && (*index == 0 || !active[*index - 1]))
        .count();
    assert_eq!(bursts, 1, "one editor act produced {bursts} audible bursts");
    s.advance_blocks(500).unwrap();
    s.clear_capture();
    s.advance_blocks(20).unwrap();
    assert_eq!(peak(&s.captured()), 0.0, "Once replayed after its tail");
}
