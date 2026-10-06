//! The only audio-thread to editor channel: atomics and one preallocated waveform ring.

use mxm_mono_08_dsp::routing::{CV_SOURCES, PULSE_SOURCES};
use mxm_mono_08_dsp::voice::Activity;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub const WAVEFORM_LEN: usize = 2048;
const NO_REQUEST: u8 = u8::MAX;
const EDITOR_OPEN_BIT: u64 = 1;

#[derive(Debug, Clone, Copy)]
pub struct VoiceSnapshot {
    pub activity: Activity,
    pub modulation_cv: f32,
    pub envelope: f32,
    pub gates: [f32; 2],
    pub pulser_period: f32,
    pub stage: usize,
    pub sequence_value: f32,
    pub pulse: bool,
    pub cv_sources: [f32; CV_SOURCES],
    pub pulse_sources: [bool; PULSE_SOURCES],
}

#[derive(Debug)]
pub struct Telemetry {
    peak: AtomicU32,
    clipped: AtomicBool,
    activity: AtomicU8,
    sample_rate: AtomicU32,
    complex: Box<[AtomicU32]>,
    complex_head: AtomicUsize,
    /// Monotonic editor generation; odd generations are open. Pulse slots store the generation in
    /// which they fired, so an old publication cannot cross a close/reopen boundary.
    editor_epoch: AtomicU64,
    modulation_cv: AtomicU32,
    envelope: AtomicU32,
    gate1: AtomicU32,
    gate2: AtomicU32,
    pulser_period: AtomicU32,
    sequence_value: AtomicU32,
    sequence_stage: AtomicU8,
    sequence_pulse: AtomicU64,
    cv_sources: [AtomicU32; CV_SOURCES],
    pulse_sources: [AtomicU64; PULSE_SOURCES],
    once_fired: AtomicU64,
    once_cancelled: AtomicU64,
    once_rejected: AtomicU64,
    dev_view: AtomicU8,
    dev_disclosure: AtomicU8,
    dev_browser: AtomicU8,
    dev_theme: AtomicU8,
    /// The host tempo in force, so a synced clock period reads its division.
    pub tempo: mxm_tempo::TempoCell,
}
impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}
impl Telemetry {
    pub fn new() -> Self {
        Self {
            peak: AtomicU32::new(0),
            clipped: AtomicBool::new(false),
            activity: AtomicU8::new(0),
            sample_rate: AtomicU32::new(48_000f32.to_bits()),
            complex: (0..WAVEFORM_LEN).map(|_| AtomicU32::new(0)).collect(),
            complex_head: AtomicUsize::new(0),
            editor_epoch: AtomicU64::new(0),
            modulation_cv: AtomicU32::new(0),
            envelope: AtomicU32::new(0),
            gate1: AtomicU32::new(0),
            gate2: AtomicU32::new(0),
            pulser_period: AtomicU32::new(0),
            sequence_value: AtomicU32::new(0),
            sequence_stage: AtomicU8::new(0),
            sequence_pulse: AtomicU64::new(0),
            cv_sources: std::array::from_fn(|_| AtomicU32::new(0)),
            pulse_sources: std::array::from_fn(|_| AtomicU64::new(0)),
            once_fired: AtomicU64::new(0),
            once_cancelled: AtomicU64::new(0),
            once_rejected: AtomicU64::new(0),
            dev_view: AtomicU8::new(NO_REQUEST),
            dev_disclosure: AtomicU8::new(NO_REQUEST),
            dev_browser: AtomicU8::new(NO_REQUEST),
            dev_theme: AtomicU8::new(NO_REQUEST),
            tempo: mxm_tempo::TempoCell::new(),
        }
    }
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
    pub fn publish_sample_rate(&self, value: f32) {
        self.sample_rate.store(value.to_bits(), Ordering::Relaxed);
    }
    pub fn publish_peak(&self, peak: f32) {
        let mut current = self.peak.load(Ordering::Relaxed);
        loop {
            let combined = f32::from_bits(current).max(peak);
            match self.peak.compare_exchange_weak(
                current,
                combined.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(seen) => current = seen,
            }
        }
        if peak >= 1.0 {
            self.clipped.store(true, Ordering::Relaxed);
        }
    }
    pub fn publish_voice(&self, snapshot: VoiceSnapshot) {
        // Capture before the scalar stores deliberately: tests can hold this publication across an
        // editor lifecycle transition, and its pulse events must remain tagged with the old epoch.
        let pulse_epoch = self.pulse_epoch();
        self.activity.store(
            match snapshot.activity {
                Activity::Inert => 0,
                Activity::Tailing => 1,
                Activity::Live => 2,
            },
            Ordering::Relaxed,
        );
        self.modulation_cv
            .store(snapshot.modulation_cv.to_bits(), Ordering::Relaxed);
        self.envelope
            .store(snapshot.envelope.to_bits(), Ordering::Relaxed);
        self.gate1
            .store(snapshot.gates[0].to_bits(), Ordering::Relaxed);
        self.gate2
            .store(snapshot.gates[1].to_bits(), Ordering::Relaxed);
        self.pulser_period
            .store(snapshot.pulser_period.to_bits(), Ordering::Relaxed);
        self.sequence_stage
            .store(snapshot.stage.min(4) as u8, Ordering::Relaxed);
        self.sequence_value
            .store(snapshot.sequence_value.to_bits(), Ordering::Relaxed);
        for (slot, value) in self.cv_sources.iter().zip(snapshot.cv_sources) {
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
        self.publish_pulses(pulse_epoch, snapshot.pulse, snapshot.pulse_sources);
    }
    fn pulse_epoch(&self) -> u64 {
        self.editor_epoch.load(Ordering::Acquire)
    }
    fn publish_pulses(&self, epoch: u64, sequence: bool, sources: [bool; PULSE_SOURCES]) {
        if epoch & EDITOR_OPEN_BIT == 0 {
            return;
        }
        // fetch_max makes a delayed old-generation publication unable to overwrite a newer event.
        if sequence {
            self.sequence_pulse.fetch_max(epoch, Ordering::Release);
        }
        for (slot, fired) in self.pulse_sources.iter().zip(sources) {
            if fired {
                slot.fetch_max(epoch, Ordering::Release);
            }
        }
    }
    #[inline]
    pub fn push_complex(&self, value: f32) {
        let i = self.complex_head.fetch_add(1, Ordering::Relaxed) % WAVEFORM_LEN;
        self.complex[i].store(value.to_bits(), Ordering::Relaxed);
    }
    pub fn editor_open(&self) -> bool {
        self.pulse_epoch() & EDITOR_OPEN_BIT != 0
    }
    pub fn set_editor_open(&self, open: bool) {
        let mut current = self.pulse_epoch();
        loop {
            if (current & EDITOR_OPEN_BIT != 0) == open {
                return;
            }
            let next = current.checked_add(1).expect("editor generation exhausted");
            match self.editor_epoch.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(seen) => current = seen,
            }
        }
    }
    pub fn publish_once(&self, fired: u64, cancelled: u64, rejected: u64) {
        self.once_fired.store(fired, Ordering::Relaxed);
        self.once_cancelled.store(cancelled, Ordering::Relaxed);
        self.once_rejected.store(rejected, Ordering::Relaxed);
    }
    pub fn request_view(&self, value: u8) {
        self.dev_view
            .store(value.min(NO_REQUEST - 1), Ordering::Relaxed);
    }
    pub fn request_disclosure(&self, open: bool) {
        self.dev_disclosure.store(u8::from(open), Ordering::Relaxed);
    }
    pub fn request_browser(&self, open: bool) {
        self.dev_browser.store(u8::from(open), Ordering::Relaxed);
    }
    /// A theme by index — 0 light, 1 dark, 2 system, as `mxm_ui::theme::from_index` reads it.
    pub fn request_theme(&self, theme: u8) {
        self.dev_theme
            .store(theme.min(NO_REQUEST - 1), Ordering::Relaxed);
    }

    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
    pub fn clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }
    pub fn clear_clip(&self) {
        self.clipped.store(false, Ordering::Relaxed);
    }
    pub fn activity(&self) -> Activity {
        match self.activity.load(Ordering::Relaxed) {
            2 => Activity::Live,
            1 => Activity::Tailing,
            _ => Activity::Inert,
        }
    }
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Relaxed))
    }
    pub fn sequence(&self) -> (usize, f32, bool) {
        let epoch = self.pulse_epoch();
        (
            usize::from(self.sequence_stage.load(Ordering::Relaxed)),
            f32::from_bits(self.sequence_value.load(Ordering::Relaxed)),
            epoch & EDITOR_OPEN_BIT != 0 && self.sequence_pulse.load(Ordering::Acquire) == epoch,
        )
    }
    pub fn gate_levels(&self) -> [f32; 2] {
        [
            f32::from_bits(self.gate1.load(Ordering::Relaxed)),
            f32::from_bits(self.gate2.load(Ordering::Relaxed)),
        ]
    }
    pub fn control_levels(&self) -> (f32, f32, f32) {
        (
            f32::from_bits(self.modulation_cv.load(Ordering::Relaxed)),
            f32::from_bits(self.envelope.load(Ordering::Relaxed)),
            f32::from_bits(self.pulser_period.load(Ordering::Relaxed)),
        )
    }
    pub fn route_sources(&self) -> [f32; CV_SOURCES] {
        std::array::from_fn(|index| f32::from_bits(self.cv_sources[index].load(Ordering::Relaxed)))
    }
    fn take_pulse_for_epoch(&self, slot: &AtomicU64, epoch: u64) -> bool {
        epoch & EDITOR_OPEN_BIT != 0
            && slot
                .compare_exchange(epoch, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
    /// Drain source-pulse latches once per rendered Mod view. The paint remains visible until the
    /// next editor frame, while generation tags quarantine close/reopen races.
    pub fn take_pulse_sources(&self) -> [bool; PULSE_SOURCES] {
        let epoch = self.pulse_epoch();
        std::array::from_fn(|index| self.take_pulse_for_epoch(&self.pulse_sources[index], epoch))
    }
    pub fn take_sequence_pulse(&self) -> bool {
        let epoch = self.pulse_epoch();
        self.take_pulse_for_epoch(&self.sequence_pulse, epoch)
    }
    pub fn once_counts(&self) -> (u64, u64, u64) {
        (
            self.once_fired.load(Ordering::Relaxed),
            self.once_cancelled.load(Ordering::Relaxed),
            self.once_rejected.load(Ordering::Relaxed),
        )
    }
    pub fn take_view_request(&self) -> Option<usize> {
        match self.dev_view.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            x => Some(usize::from(x)),
        }
    }
    pub fn take_disclosure_request(&self) -> Option<bool> {
        match self.dev_disclosure.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            x => Some(x != 0),
        }
    }
    pub fn take_browser_request(&self) -> Option<bool> {
        match self.dev_browser.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            x => Some(x != 0),
        }
    }
    pub fn take_theme_request(&self) -> Option<u8> {
        match self.dev_theme.swap(NO_REQUEST, Ordering::Relaxed) {
            NO_REQUEST => None,
            theme => Some(theme),
        }
    }
    pub fn waveform_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        let head = self.complex_head.load(Ordering::Relaxed);
        out.extend((0..WAVEFORM_LEN).map(|k| {
            f32::from_bits(
                self.complex[head.wrapping_add(k) % WAVEFORM_LEN].load(Ordering::Relaxed),
            )
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn peak_combines_resets_and_clip_latches() {
        let t = Telemetry::new();
        t.publish_peak(0.8);
        t.publish_peak(0.2);
        assert_eq!(t.take_peak(), 0.8);
        assert_eq!(t.take_peak(), 0.0);
        t.publish_peak(1.0);
        t.publish_peak(0.0);
        assert!(t.clipped());
        t.clear_clip();
        assert!(!t.clipped());
    }
    #[test]
    fn requests_are_taken_once() {
        let t = Telemetry::new();
        t.request_view(2);
        t.request_disclosure(true);
        t.request_browser(false);
        t.request_theme(2);
        assert_eq!(t.take_view_request(), Some(2));
        assert_eq!(t.take_disclosure_request(), Some(true));
        assert_eq!(t.take_browser_request(), Some(false));
        assert_eq!(t.take_theme_request(), Some(2));
        assert_eq!(t.take_view_request(), None);
        assert_eq!(t.take_theme_request(), None);
    }
    #[test]
    fn audio_publications_cannot_erase_unseen_one_sample_pulses() {
        let t = Telemetry::new();
        t.set_editor_open(true);
        let snapshot = |pulse, sources| VoiceSnapshot {
            activity: Activity::Live,
            modulation_cv: 0.0,
            envelope: 0.0,
            gates: [0.0; 2],
            pulser_period: 0.5,
            stage: 2,
            sequence_value: 0.5,
            pulse,
            cv_sources: [0.0; CV_SOURCES],
            pulse_sources: sources,
        };
        t.publish_voice(snapshot(true, [true, false, true]));
        for _ in 0..32 {
            t.publish_voice(snapshot(false, [false; PULSE_SOURCES]));
        }
        assert_eq!(t.take_pulse_sources(), [true, false, true]);
        assert!(t.take_sequence_pulse());
        assert_eq!(t.take_pulse_sources(), [false; PULSE_SOURCES]);
        assert!(!t.take_sequence_pulse());

        t.publish_voice(snapshot(true, [true; PULSE_SOURCES]));
        t.set_editor_open(false);
        assert_eq!(t.take_pulse_sources(), [false; PULSE_SOURCES]);
        assert!(!t.take_sequence_pulse(), "closing retained a stale flash");
    }

    #[test]
    fn editor_epochs_quarantine_publications_across_close_and_reopen() {
        let t = Telemetry::new();
        t.set_editor_open(true);
        let old_open = t.pulse_epoch();
        t.set_editor_open(false);
        t.set_editor_open(true);
        t.publish_pulses(old_open, true, [true; PULSE_SOURCES]);
        assert_eq!(t.take_pulse_sources(), [false; PULSE_SOURCES]);
        assert!(!t.take_sequence_pulse(), "an old-open pulse crossed reopen");

        t.set_editor_open(false);
        let observed_closed = t.pulse_epoch();
        t.set_editor_open(true);
        let reopened = t.pulse_epoch();
        t.publish_pulses(reopened, true, [true; PULSE_SOURCES]);
        t.publish_pulses(observed_closed, false, [false; PULSE_SOURCES]);
        assert_eq!(t.take_pulse_sources(), [true; PULSE_SOURCES]);
        assert!(
            t.take_sequence_pulse(),
            "a delayed closed publication erased the reopened pulse"
        );
    }

    #[test]
    fn visualization_work_is_parked_without_an_editor() {
        let t = Telemetry::new();
        assert!(!t.editor_open());
        t.set_editor_open(true);
        assert!(t.editor_open());
    }
}
