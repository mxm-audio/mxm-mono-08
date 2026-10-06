//! Hostile activation and process shapes for mxm-mono-08 through the player's direct CLAP host.
//!
//! Run this against a debug bundle as well as release. With `assert_process_allocs` enabled by the
//! workspace, a plugin-side audio-thread allocation aborts the callback instead of passing quietly.

use mxm_player::events::input::Payload;
use mxm_player_harness::harness;
const ID: &str = "dk.mxm.mxm-mono-08";
fn on(key: u8) -> Payload {
    Payload::NoteOn {
        channel: 0,
        key,
        velocity: 0.8,
    }
}
fn off(key: u8) -> Payload {
    Payload::NoteOff {
        channel: 0,
        key,
        velocity: 0.0,
    }
}

#[test]
fn hostile_sample_rates_and_every_block_boundary_stay_finite() {
    let Some(bundle) = harness::mxm_mono_08() else {
        return;
    };
    for rate in [1_000.0, 1_234.57, 44_100.0, 192_000.0, 768_000.0] {
        let mut h =
            harness::Harness::with_configuration(&bundle, ID, 1, rate, 8192).expect("hosts");
        assert!(h.push(0, on(127)));
        for frames in [1usize, 2, 63, 64, 65, 127, 255, 1024, 4097, 8192] {
            let audio = h.render(frames);
            assert!(
                audio.iter().all(|x| x.is_finite()),
                "non-finite at {rate} Hz / {frames} frames"
            );
        }
        assert!(h.push(0, off(127)));
        for _ in 0..20 {
            h.render(8192);
        }
        assert!(h.render(64).iter().all(|x| x.is_finite()));
        h.shutdown();
    }
}

#[test]
fn two_events_inside_one_large_callback_are_split_at_their_offsets() {
    let Some(bundle) = harness::mxm_mono_08() else {
        return;
    };
    let mut h =
        harness::Harness::with_configuration(&bundle, ID, 1, 48_000.0, 4096).expect("hosts");
    h.render(256);
    h.render(256);
    let base = h.clock.now_nanos();
    assert!(h.push_at(0, base, on(60)));
    assert!(h.push_at(0, base + 2_000_000, off(60)));
    let audio = h.render(4096);
    assert!(audio.iter().all(|x| x.is_finite()));
    assert!(
        audio.iter().any(|x| x.abs() > 1e-5),
        "the interval between note events vanished"
    );
    for _ in 0..500 {
        h.render(512);
    }
    assert_eq!(h.peak(), 0.0, "the split NoteOff must reach the voice");
    h.shutdown();
}
