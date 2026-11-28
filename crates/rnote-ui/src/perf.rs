//! Debug-only performance instrumentation for the Android spike (ticket #12).
//!
//! Off by default. Active only in a debug build on Android, and only after an
//! explicit opt-in: the `RANOTE_PERF` environment variable, or an app-private
//! `perf.on` marker file (see `just android-app-perf-on`).
//!
//! It records frame intervals, non-stylus touch input-to-snapshot latency
//! (a lower bound for input-to-pixel; present time is not observable here),
//! stylus pressure samples and process RSS. Only numbers and fixed labels are
//! logged: never note contents, file names or paths.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{info, warn};

const LOG_TARGET: &str = "rnote::perf";
const MARKER_FILE: &str = "perf.on";

/// Spike gate: >= 55 fps median on a 20-page document.
pub(crate) const GATE_MEDIAN_FPS: f64 = 55.0;
/// Spike gate: p95 frame time <= 33 ms.
pub(crate) const GATE_P95_FRAME_MS: f64 = 33.0;

/// Keep roughly ten seconds of frames at 60 Hz.
const FRAME_WINDOW: usize = 600;
const LATENCY_WINDOW: usize = 300;
const SUMMARY_INTERVAL: Duration = Duration::from_secs(5);
const MEMORY_INTERVAL_SECS: u32 = 10;
/// A longer gap is an idle period, not a rendered frame.
const MAX_FRAME_GAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrameStats {
    pub(crate) count: usize,
    pub(crate) median_fps: f64,
    pub(crate) p95_ms: f64,
}

impl FrameStats {
    pub(crate) fn passes_gate(&self) -> bool {
        self.median_fps >= GATE_MEDIAN_FPS && self.p95_ms <= GATE_P95_FRAME_MS
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LatencyStats {
    pub(crate) count: usize,
    pub(crate) median_ms: f64,
    pub(crate) p95_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PressureStats {
    pub(crate) count: u64,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) mean: f64,
}

impl PressureStats {
    fn from_accumulator(count: u64, min: f64, max: f64, sum: f64) -> Option<Self> {
        (count > 0).then(|| Self {
            count,
            min,
            max,
            mean: sum / count as f64,
        })
    }
}

#[derive(Debug, Default)]
struct PerfState {
    last_frame: Option<Instant>,
    frames_ms: VecDeque<f64>,
    pending_input: Option<Instant>,
    latencies_ms: VecDeque<f64>,
    pressure_count: u64,
    pressure_min: Option<f64>,
    pressure_max: Option<f64>,
    pressure_sum: f64,
    last_summary: Option<Instant>,
}

fn state() -> &'static Mutex<PerfState> {
    static STATE: OnceLock<Mutex<PerfState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(PerfState::default()))
}

fn enabled_slot() -> &'static OnceLock<bool> {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    &ENABLED
}

/// Whether the recorder may run. Resolved once, so the marker-file probe is not
/// repeated for every frame.
pub(crate) fn enabled() -> bool {
    *enabled_slot().get_or_init(|| {
        if !cfg!(target_os = "android") || !cfg!(debug_assertions) {
            return false;
        }
        let env_value = std::env::var("RANOTE_PERF").ok();
        opted_in(env_value.as_deref(), marker_exists())
    })
}

/// Look for `perf.on` where the app can read it: the XDG data dirs (GTK points
/// these at the app's internal and external share dirs on Android) and the app
/// cache dir. `android-app-perf-on` writes into the internal data dir.
fn marker_exists() -> bool {
    let mut data_dirs = gtk4::glib::system_data_dirs();
    data_dirs.push(gtk4::glib::user_data_dir());
    if data_dirs.iter().any(|dir| {
        dir.join(crate::config::APP_NAME)
            .join(MARKER_FILE)
            .is_file()
    }) {
        return true;
    }
    crate::env::app_cache_dir().is_ok_and(|dir| dir.join(MARKER_FILE).is_file())
}

/// Pure opt-in decision, kept separate so it is testable on desktop.
fn opted_in(env_value: Option<&str>, marker_exists: bool) -> bool {
    marker_exists
        || matches!(
            env_value.map(|value| value.trim().to_ascii_lowercase()),
            Some(ref value) if value == "1" || value == "true" || value == "yes" || value == "on"
        )
}

/// Start the periodic sampler. No-op unless enabled.
pub(crate) fn start() {
    if !enabled() {
        return;
    }
    info!(
        target: LOG_TARGET,
        gate_median_fps = GATE_MEDIAN_FPS,
        gate_p95_frame_ms = GATE_P95_FRAME_MS,
        "enabled"
    );
    gtk4::glib::timeout_add_seconds_local(MEMORY_INTERVAL_SECS, || {
        sample_memory();
        gtk4::glib::ControlFlow::Continue
    });
}

/// Record a timestamp for a non-stylus pointer event. Stylus latency is deferred
/// to the stylus verification ticket; stylus pressure is sampled separately.
pub(crate) fn record_input() {
    if !enabled() {
        return;
    }
    if let Ok(mut state) = state().lock() {
        state.pending_input = Some(Instant::now());
    }
}

/// Record a stylus pressure sample (0.0..=1.0). Stylus input-to-pixel latency
/// is still deferred to the stylus verification ticket.
pub(crate) fn record_pressure(pressure: f64) {
    if !enabled() || !pressure.is_finite() {
        return;
    }
    if let Ok(mut state) = state().lock() {
        state.pressure_count += 1;
        state.pressure_sum += pressure;
        state.pressure_min = Some(state.pressure_min.map_or(pressure, |min| min.min(pressure)));
        state.pressure_max = Some(state.pressure_max.map_or(pressure, |max| max.max(pressure)));
    }
}

/// Record a rendered frame. Called after the canvas snapshot has been drawn, so
/// a pending touch sample becomes an input-to-snapshot latency.
pub(crate) fn record_frame() {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    let summary = {
        let Ok(mut state) = state().lock() else {
            return;
        };

        if let Some(last) = state.last_frame.replace(now) {
            let delta = now.duration_since(last);
            if delta <= MAX_FRAME_GAP {
                state.frames_ms.push_back(delta.as_secs_f64() * 1000.0);
                while state.frames_ms.len() > FRAME_WINDOW {
                    state.frames_ms.pop_front();
                }
            }
        }

        if let Some(input) = state.pending_input.take() {
            state
                .latencies_ms
                .push_back((now - input).as_secs_f64() * 1000.0);
            while state.latencies_ms.len() > LATENCY_WINDOW {
                state.latencies_ms.pop_front();
            }
        }

        if state
            .last_summary
            .is_none_or(|last| now.duration_since(last) >= SUMMARY_INTERVAL)
        {
            state.last_summary = Some(now);
            let pressure = PressureStats::from_accumulator(
                state.pressure_count,
                state.pressure_min.unwrap_or_default(),
                state.pressure_max.unwrap_or_default(),
                state.pressure_sum,
            );
            state.pressure_count = 0;
            state.pressure_min = None;
            state.pressure_max = None;
            state.pressure_sum = 0.0;
            Some((
                state.frames_ms.iter().copied().collect::<Vec<f64>>(),
                state.latencies_ms.iter().copied().collect::<Vec<f64>>(),
                pressure,
            ))
        } else {
            None
        }
    };

    if let Some((frames, latencies, pressure)) = summary {
        log_summary(&frames, &latencies, pressure);
    }
}

fn log_summary(frames_ms: &[f64], latencies_ms: &[f64], pressure: Option<PressureStats>) {
    let Some(frames) = frame_stats(frames_ms) else {
        return;
    };
    info!(
        target: LOG_TARGET,
        frames = frames.count,
        median_fps = frames.median_fps,
        p95_frame_ms = frames.p95_ms,
        gate = if frames.passes_gate() { "pass" } else { "fail" },
        "frame_summary"
    );
    if let Some(latency) = latency_stats(latencies_ms) {
        info!(
            target: LOG_TARGET,
            samples = latency.count,
            median_ms = latency.median_ms,
            p95_ms = latency.p95_ms,
            "touch_latency_summary"
        );
    }
    if let Some(pressure) = pressure {
        info!(
            target: LOG_TARGET,
            samples = pressure.count,
            min = pressure.min,
            max = pressure.max,
            mean = pressure.mean,
            "pressure_summary"
        );
    }
}

/// Sample and log process RSS. Also invoked periodically while the app is idle.
pub(crate) fn sample_memory() {
    if !enabled() {
        return;
    }
    match process_rss_kb() {
        Some(rss_kb) => info!(
            target: LOG_TARGET,
            rss_mb = rss_kb as f64 / 1024.0,
            "memory"
        ),
        None => warn!(target: LOG_TARGET, "memory_unavailable"),
    }
}

pub(crate) fn frame_stats(samples: &[f64]) -> Option<FrameStats> {
    if samples.is_empty() {
        return None;
    }
    let sorted = sorted(samples);
    let median_ms = percentile(&sorted, 0.5);
    Some(FrameStats {
        count: samples.len(),
        median_fps: if median_ms > 0.0 {
            1000.0 / median_ms
        } else {
            f64::INFINITY
        },
        p95_ms: percentile(&sorted, 0.95),
    })
}

pub(crate) fn latency_stats(samples: &[f64]) -> Option<LatencyStats> {
    if samples.is_empty() {
        return None;
    }
    let sorted = sorted(samples);
    Some(LatencyStats {
        count: samples.len(),
        median_ms: percentile(&sorted, 0.5),
        p95_ms: percentile(&sorted, 0.95),
    })
}

fn sorted(samples: &[f64]) -> Vec<f64> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
}

/// Nearest-rank percentile: the smallest value whose rank covers `p`.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub(crate) fn parse_rss_kb(status: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

fn process_rss_kb() -> Option<u64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        parse_rss_kb(&std::fs::read_to_string("/proc/self/status").ok()?)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_in_needs_env_or_marker() {
        assert!(!opted_in(None, false));
        assert!(!opted_in(Some(""), false));
        assert!(!opted_in(Some("0"), false));
        assert!(!opted_in(Some("no"), false));
        for value in ["1", "true", "yes", "on", " ON "] {
            assert!(opted_in(Some(value), false), "{value:?} should opt in");
        }
        assert!(opted_in(None, true));
    }

    #[test]
    fn nearest_rank_percentile_picks_the_covering_sample() {
        // Independent of the code: nearest-rank p50 of 5 values is the 3rd,
        // p95 is the 5th.
        let sorted = [10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(percentile(&sorted, 0.5), 30.0);
        assert_eq!(percentile(&sorted, 0.95), 50.0);

        let sorted = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&sorted, 0.5), 2.0);
        assert_eq!(percentile(&sorted, 0.95), 4.0);
    }

    #[test]
    fn frame_stats_report_median_fps_and_p95() {
        let stats = frame_stats(&[16.0, 18.0, 16.5, 17.0, 16.0]).unwrap();
        assert_eq!(stats.count, 5);
        assert!((stats.median_fps - 1000.0 / 16.5).abs() < 0.001);
        assert_eq!(stats.p95_ms, 18.0);
        assert!(stats.passes_gate());
    }

    #[test]
    fn frame_stats_fail_the_gate_below_55_fps() {
        // 25 ms per frame is 40 fps, below the median threshold.
        let stats = frame_stats(&[25.0; 60]).unwrap();
        assert!((stats.median_fps - 40.0).abs() < 0.001);
        assert!(!stats.passes_gate());
    }

    #[test]
    fn frame_stats_fail_the_gate_on_a_p95_spike() {
        // 18 good frames and one 40 ms frame: nearest-rank p95 is 40 ms > 33 ms.
        let mut samples = vec![16.0; 18];
        samples.push(40.0);
        let stats = frame_stats(&samples).unwrap();
        assert!(stats.median_fps >= GATE_MEDIAN_FPS);
        assert_eq!(stats.p95_ms, 40.0);
        assert!(!stats.passes_gate());
    }

    #[test]
    fn latency_stats_report_median_and_p95() {
        let stats = latency_stats(&[10.0, 12.0, 11.0, 30.0, 9.0]).unwrap();
        assert_eq!(stats.count, 5);
        assert_eq!(stats.median_ms, 11.0);
        assert_eq!(stats.p95_ms, 30.0);
        assert!(latency_stats(&[]).is_none());
        assert!(frame_stats(&[]).is_none());
    }

    #[test]
    fn pressure_stats_summarize_the_window() {
        let stats = PressureStats::from_accumulator(4, 0.1, 0.9, 2.0).unwrap();
        assert_eq!(stats.count, 4);
        assert_eq!(stats.min, 0.1);
        assert_eq!(stats.max, 0.9);
        assert_eq!(stats.mean, 0.5);
        assert!(PressureStats::from_accumulator(0, 0.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn rss_is_read_from_proc_status() {
        let status = "Name:\trnote\nVmPeak:\t  900000 kB\nVmRSS:\t  123456 kB\n";
        assert_eq!(parse_rss_kb(status), Some(123456));
        assert_eq!(parse_rss_kb("VmRSS:\t not-a-number kB"), None);
        assert_eq!(parse_rss_kb("Name:\trnote"), None);
    }
}
