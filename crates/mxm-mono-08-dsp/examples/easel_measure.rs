//! Reproducible model measurements for chosen provisional constants. These characterize this DSP;
//! they are not measurements of original hardware.
use mxm_mono_08_dsp::control::{Envelope, EnvelopeMode};
use mxm_mono_08_dsp::lpg::{GateMode, LowPassGate};
use mxm_mono_08_dsp::oscillator::{
    ComplexEndpoint, ComplexInput, ComplexOscillator, ModWave, ModulationOscillator,
};
use std::hint::black_box;
use std::time::Instant;
const FS: f32 = 48_000.0;
fn main() {
    let mut env = Envelope::new();
    env.set_gate(true);
    env.trigger();
    let mut half = 0;
    for i in 0..4800 {
        if env.process(0.1, 0.1, 0.1, EnvelopeMode::Transient, FS) >= 0.5 {
            half = i;
            break;
        }
    }
    println!(
        "linear envelope: 50% attack at {:.3} ms",
        half as f32 / FS * 1000.0
    );
    let mut gate = LowPassGate::new();
    for _ in 0..(FS * 0.03) as usize {
        gate.process(0.0, 1.0, GateMode::Combination, FS);
    }
    let open = gate.optical_level();
    for _ in 0..(FS * 0.03) as usize {
        gate.process(0.0, 0.0, GateMode::Combination, FS);
    }
    println!(
        "chosen 208 optical model: level {:.4} after 30 ms open, {:.4} after 30 ms release",
        open,
        gate.optical_level()
    );
    let mut mo = ModulationOscillator::new();
    let (mut early, mut late) = (0.0f32, 0.0f32);
    for i in 0..(FS * 0.3) as usize {
        let cv = mo.process_cv(2.0, ModWave::Square, FS);
        let audio = mo.monitored_audio(cv, FS);
        if i == (FS * 0.01) as usize {
            early = audio;
        } else if i == (FS * 0.2) as usize {
            late = audio;
        }
    }
    println!(
        "2 Hz monitored square tilt: |10 ms| {:.5}, |200 ms| {:.5}, ratio {:.2}",
        early.abs(),
        late.abs(),
        early.abs() / late.abs().max(1e-20)
    );
    // The complex oscillator's audio becomes a routing source, and a source frame bounds every value
    // it holds to unit magnitude, so this peak is what decides the scale it is published at
    // (`plans/plan-mxm-mono-08-modulation.md` §2 and §15: a chosen constant recorded with its
    // measurement, because changing it later changes every patch that uses the route).
    //
    // Measured **at the keyboard**: the pitch each key gives at the panel's default 220 Hz, at every
    // endpoint, wave mix, timbre and modulation setting a player can reach. `process` clamps its
    // inputs but does not bound its own output, which is why this is measured rather than assumed.
    // The AM branch needs no sweep: `am_crossfade` is `soft_clip(x) * (2·cv − 1)`, so it cannot
    // exceed 1.0 whatever this reaches.
    let mut peak = 0.0f32;
    let mut over_unit = 0usize;
    let mut worst = (0.0f32, "", 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for (endpoint, name) in [
        (ComplexEndpoint::Spike, "spike"),
        (ComplexEndpoint::Square, "square"),
        (ComplexEndpoint::Triangle, "triangle"),
    ] {
        for key in (0..=127u16).step_by(3) {
            let hz = 220.0 * ((f32::from(key) - 69.0) / 12.0).exp2();
            for wave_mix in [0.0, 0.5, 1.0] {
                for timbre in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    for fm in [0.0, -1.0, 1.0] {
                        for mod_index in [0.0, 1.0] {
                            let input = ComplexInput {
                                hz,
                                endpoint,
                                wave_mix,
                                timbre,
                                fm,
                                mod_index,
                            };
                            // **The window follows the pitch.** A fixed 20 ms sees 8% of one cycle
                            // at the bottom key, and a peak never observed is a peak not measured:
                            // this runs at least one and a half cycles, capped so the sweep stays
                            // affordable.
                            let seconds = (1.5 / hz.max(0.01)).clamp(0.02, 0.5);
                            let mut osc = ComplexOscillator::new();
                            let mut local = 0.0f32;
                            for _ in 0..(FS * seconds) as usize {
                                local = local.max(osc.process(input, FS).abs());
                            }
                            if local > 1.0 {
                                over_unit += 1;
                            }
                            if local > peak {
                                peak = local;
                                worst = (hz, name, wave_mix, timbre, fm, mod_index);
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "complex audio peak across the keyboard: {peak:.6} at {:.1} Hz, {} endpoint, mix {:.2}, timbre {:.2}, fm {:+.0}, index {:.0}",
        worst.0, worst.1, worst.2, worst.3, worst.4, worst.5
    );
    println!(
        "  settings above unit magnitude: {over_unit}; peak - 1 = {:e}",
        peak - 1.0
    );
    println!(
        "  a routing source must fit unit magnitude, so its published scale is a power of two <= {:.6}",
        1.0 / peak.max(1.0)
    );

    let controls = ComplexInput {
        hz: 1900.0,
        endpoint: ComplexEndpoint::Triangle,
        wave_mix: 0.2,
        timbre: 1.0,
        fm: 0.7,
        mod_index: 1.0,
    };
    let mut complex = ComplexOscillator::new();
    let samples = 1_000_000;
    let start = Instant::now();
    for _ in 0..samples {
        black_box(complex.process(black_box(controls), FS));
    }
    println!(
        "4x coupled oscillator: {:.1} ns/host sample (one release run)",
        start.elapsed().as_nanos() as f64 / samples as f64
    );
    println!("All figures above are in-repo DSP measurements; fidelity remains UNVERIFIED.");
}
