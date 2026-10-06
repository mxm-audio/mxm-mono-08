//! Linear envelope and pulser, five stored sequence stages and four held random voltages.
//! Behaviour comes from `research:modulation/buchla-208-control-sources.md`; every unknown policy is
//! named in the crate AGENTS.md.

use crate::Rng;

pub const MIN_TIME_S: f32 = 0.002;
pub const MAX_TIME_S: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvelopeMode {
    #[default]
    Transient,
    Sustained,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Hold,
    Decay,
}

#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    level: f32,
    stage: EnvelopeStage,
    elapsed: f32,
    gate: bool,
}
impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}
impl Envelope {
    pub const fn new() -> Self {
        Self {
            level: 0.0,
            stage: EnvelopeStage::Idle,
            elapsed: 0.0,
            gate: false,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn trigger(&mut self) {
        self.stage = EnvelopeStage::Attack;
        self.elapsed = 0.0;
    }
    pub fn set_gate(&mut self, high: bool) {
        self.gate = high;
    }
    pub fn level(&self) -> f32 {
        self.level
    }
    pub fn stage(&self) -> EnvelopeStage {
        self.stage
    }
    pub fn active(&self) -> bool {
        self.stage != EnvelopeStage::Idle
    }
    #[inline]
    pub fn process(
        &mut self,
        attack: f32,
        duration: f32,
        decay: f32,
        mode: EnvelopeMode,
        fs: f32,
    ) -> f32 {
        let fs = fs.max(1.0);
        let (a, d, r) = (
            attack.clamp(MIN_TIME_S, MAX_TIME_S),
            duration.clamp(MIN_TIME_S, MAX_TIME_S),
            decay.clamp(MIN_TIME_S, MAX_TIME_S),
        );
        match self.stage {
            EnvelopeStage::Idle => self.level = 0.0,
            EnvelopeStage::Attack => {
                self.level = (self.level + 1.0 / (a * fs)).min(1.0);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.elapsed = 0.0;
                    self.stage = EnvelopeStage::Hold;
                }
            }
            EnvelopeStage::Hold => {
                self.elapsed += 1.0 / fs;
                if (mode == EnvelopeMode::Transient && self.elapsed >= d)
                    || (mode == EnvelopeMode::Sustained && !self.gate)
                {
                    self.stage = EnvelopeStage::Decay;
                }
            }
            EnvelopeStage::Decay => {
                self.level = (self.level - 1.0 / (r * fs)).max(0.0);
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }
        self.level
    }
}

/// The pulser's descending ramp and end pulse are one phase. Self only routes that end pulse back;
/// selecting it cannot invent the first event (`research:modulation/buchla-208-control-sources.md` §3.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pulser {
    phase: f64,
    running: bool,
}
impl Pulser {
    pub const fn new() -> Self {
        Self {
            phase: 0.0,
            running: false,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.running = true;
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn ramp(&self) -> f32 {
        if self.running {
            (1.0 - self.phase) as f32
        } else {
            0.0
        }
    }
    /// Returns the ramp for this sample and whether its end pulse must be queued for the next.
    pub fn process(&mut self, period_s: f32, fs: f32) -> (f32, bool) {
        if !self.running {
            return (0.0, false);
        }
        let out = self.ramp();
        self.phase += 1.0 / (period_s.clamp(MIN_TIME_S, MAX_TIME_S) as f64 * fs.max(1.0) as f64);
        if self.phase >= 1.0 {
            self.phase = 1.0;
            self.running = false;
            (out, true)
        } else {
            (out, false)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Sequencer {
    stage: usize,
    levels: [f32; 5],
    pulses: [bool; 5],
}
impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
impl Sequencer {
    pub const fn new() -> Self {
        Self {
            stage: 0,
            levels: [0.0; 5],
            pulses: [true; 5],
        }
    }
    pub fn reset(&mut self) {
        self.stage = 0;
    }
    pub fn set(&mut self, levels: [f32; 5], pulses: [bool; 5]) {
        self.levels = levels.map(|x| x.clamp(0.0, 1.0));
        self.pulses = pulses;
    }
    pub fn stage(&self) -> usize {
        self.stage
    }
    pub fn value(&self) -> f32 {
        self.levels[self.stage]
    }
    pub fn pulse_enabled(&self) -> bool {
        self.pulses[self.stage]
    }
    pub fn advance(&mut self, length: usize) -> bool {
        let n = length.clamp(2, 5);
        self.stage = if self.stage + 1 >= n {
            0
        } else {
            self.stage + 1
        };
        self.pulses[self.stage]
    }
}

#[derive(Debug, Clone)]
pub struct RandomVoltages {
    values: [f32; 4],
    rng: Rng,
}
impl Default for RandomVoltages {
    fn default() -> Self {
        Self::new()
    }
}
impl RandomVoltages {
    pub const fn new() -> Self {
        Self {
            values: [0.0; 4],
            rng: Rng::new(0x0208_1973),
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn trigger(&mut self) {
        for v in &mut self.values {
            *v = self.rng.unipolar();
        }
    }
    pub fn values(&self) -> [f32; 4] {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FS: f32 = 48_000.0;
    #[test]
    fn envelope_segments_are_linear_and_reach_the_documented_minimum() {
        let mut e = Envelope::new();
        e.set_gate(true);
        e.trigger();
        let mut attack = Vec::new();
        for _ in 0..96 {
            attack.push(e.process(0.002, 0.002, 0.002, EnvelopeMode::Transient, FS));
        }
        assert!((attack[47] - 0.5).abs() < 0.02);
        assert_eq!(attack[95], 1.0);
        for _ in 0..96 {
            e.process(0.002, 0.002, 0.002, EnvelopeMode::Transient, FS);
        }
        let mut decay = Vec::new();
        for _ in 0..100 {
            decay.push(e.process(0.002, 0.002, 0.002, EnvelopeMode::Transient, FS));
        }
        assert!(decay.windows(2).all(|x| x[1] <= x[0]));
        assert_eq!(*decay.last().unwrap(), 0.0);
    }
    #[test]
    fn sustained_envelope_holds_until_release() {
        let mut e = Envelope::new();
        e.set_gate(true);
        e.trigger();
        for _ in 0..FS as usize {
            e.process(0.002, 0.002, 0.01, EnvelopeMode::Sustained, FS);
        }
        assert_eq!(e.level(), 1.0);
        e.set_gate(false);
        for _ in 0..600 {
            e.process(0.002, 0.002, 0.01, EnvelopeMode::Sustained, FS);
        }
        assert_eq!(e.level(), 0.0);
    }
    #[test]
    fn self_mode_does_not_start_itself() {
        let mut p = Pulser::new();
        for _ in 0..1000 {
            assert_eq!(p.process(0.002, FS), (0.0, false));
        }
        p.trigger();
        let mut ends = 0;
        for _ in 0..200 {
            let (_, end) = p.process(0.002, FS);
            ends += end as usize;
        }
        assert_eq!(ends, 1);
    }
    #[test]
    fn disabled_pulse_does_not_skip_the_voltage_stage() {
        let mut s = Sequencer::new();
        s.set([0., 0.2, 0.4, 0.6, 0.8], [true, false, true, true, true]);
        assert!(!s.advance(5));
        assert_eq!(s.stage(), 1);
        assert_eq!(s.value(), 0.2);
        assert!(s.advance(5));
        assert_eq!(s.stage(), 2);
        assert_eq!(s.value(), 0.4);
    }
    #[test]
    fn every_active_length_cycles_forward_without_erasing_five_values() {
        for n in 2..=5 {
            let mut s = Sequencer::new();
            s.set([0.1, 0.2, 0.3, 0.4, 0.5], [true; 5]);
            for expected in (0..n).cycle().skip(1).take(n * 3) {
                s.advance(n);
                assert_eq!(s.stage(), expected);
            }
            assert_eq!(s.levels, [0.1, 0.2, 0.3, 0.4, 0.5]);
        }
    }
    #[test]
    fn random_outputs_are_four_fresh_independent_held_values() {
        let mut r = RandomVoltages::new();
        assert_eq!(r.values(), [0.; 4]);
        r.trigger();
        let a = r.values();
        assert_eq!(r.values(), a);
        assert!(a.windows(2).any(|w| w[0] != w[1]));
        r.trigger();
        assert_ne!(r.values(), a);
        assert!(r.values().iter().all(|x| (0.0..1.0).contains(x)));
        let mut b = RandomVoltages::new();
        b.trigger();
        assert_eq!(a, b.values());
    }
}
