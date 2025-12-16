//! Async runtime and task management for the TUI
//!
//! This module implements the dual-channel event-driven architecture:
//! - Input channel (priority): User input events that are never dropped
//! - Data channel: Data updates that may be dropped under backpressure
//!
//! The main loop uses `tokio::select!` with bias toward the input channel
//! to prevent input starvation under heavy data update loads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::slurm::SlurmInterface;
use crate::tui::app::{App, TuiJobInfo};
use crate::tui::event::{DataEvent, DataSource, EventResult, InputEvent};

/// Channel capacities
const INPUT_CHANNEL_CAPACITY: usize = 16;
const DATA_CHANNEL_CAPACITY: usize = 32;

/// Default refresh intervals
const DEFAULT_JOBS_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_NODES_INTERVAL: Duration = Duration::from_secs(10);
const DEFAULT_FAIRSHARE_INTERVAL: Duration = Duration::from_secs(60);
const DEFAULT_SCHEDULER_STATS_INTERVAL: Duration = Duration::from_secs(30);
const ANIMATION_TICK_INTERVAL: Duration = Duration::from_millis(200);

/// Idle detection threshold (30 seconds)
const IDLE_THRESHOLD: Duration = Duration::from_secs(30);
/// Multiplier applied when idle (2x slowdown)
const IDLE_MULTIPLIER: f32 = 2.0;

/// Helper function to handle common fetch result patterns.
///
/// Consolidates the repeated error handling in fetch_and_send_* functions.
/// On success, sends the data event. On error, records the error in throttle
/// and sends a FetchError event with appropriate logging.
fn handle_fetch_result<T, F>(
    result: Result<Result<T, anyhow::Error>, tokio::task::JoinError>,
    tx: &mpsc::Sender<DataEvent>,
    throttle: &FetcherThrottle,
    source: DataSource,
    success_event: F,
) where
    F: FnOnce(T) -> DataEvent,
{
    match result {
        Ok(Ok(data)) => {
            if tx.try_send(success_event(data)).is_err() {
                throttle.record_backpressure();
            }
        }
        Ok(Err(e)) => {
            throttle.record_error();
            if tx
                .try_send(DataEvent::FetchError {
                    source,
                    error: e.to_string(),
                })
                .is_err()
            {
                tracing::warn!(
                    "Could not send {} fetch error notification (channel full)",
                    source
                );
            }
        }
        Err(e) => {
            throttle.record_error();
            if tx
                .try_send(DataEvent::FetchError {
                    source,
                    error: format!("Task join error: {}", e),
                })
                .is_err()
            {
                tracing::warn!(
                    "Could not send {} task error notification (channel full)",
                    source
                );
            }
        }
    }
}

/// Shared state for adaptive throttling
pub struct FetcherThrottle {
    /// Multiplier applied to base interval (stored as multiplier * 100 for atomicity)
    multiplier: AtomicU32,
    /// Recent error count (rolling window)
    error_count: AtomicU32,
    /// Recent channel-full count (rolling window)
    backpressure_count: AtomicU32,
    /// Last user activity timestamp (seconds since start)
    last_activity: std::sync::atomic::AtomicU64,
    /// When the throttle was created (for calculating elapsed time)
    start_time: Instant,
}

impl Default for FetcherThrottle {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            multiplier: AtomicU32::new(100), // 1.0x
            error_count: AtomicU32::new(0),
            backpressure_count: AtomicU32::new(0),
            last_activity: std::sync::atomic::AtomicU64::new(0),
            start_time: now,
        }
    }
}

impl FetcherThrottle {
    /// Get the effective multiplier (includes idle detection)
    #[must_use]
    pub fn get_multiplier(&self) -> f32 {
        let base = self.multiplier.load(Ordering::Relaxed) as f32 / 100.0;
        if self.is_idle() {
            base * IDLE_MULTIPLIER
        } else {
            base
        }
    }

    /// Check if the user has been idle for longer than the threshold
    #[must_use]
    pub fn is_idle(&self) -> bool {
        let last = self.last_activity.load(Ordering::Relaxed);
        let now = self.start_time.elapsed().as_secs();
        now.saturating_sub(last) > IDLE_THRESHOLD.as_secs()
    }

    /// Record user activity (call this on any user input)
    pub fn record_activity(&self) {
        let now = self.start_time.elapsed().as_secs();
        self.last_activity.store(now, Ordering::Relaxed);
    }

    /// Called when try_send fails (channel full)
    pub fn record_backpressure(&self) {
        let count = self.backpressure_count.fetch_add(1, Ordering::Relaxed);
        if count >= 5 {
            self.increase_multiplier();
        }
    }

    /// Called on Slurm command error
    pub fn record_error(&self) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed);
        if count >= 3 {
            self.increase_multiplier();
        }
    }

    fn increase_multiplier(&self) {
        // Cap at 4x slowdown
        let current = self.multiplier.load(Ordering::Relaxed);
        if current < 400 {
            self.multiplier
                .store((current + 50).min(400), Ordering::Relaxed);
        }
    }

    /// Called periodically to gradually restore normal speed
    pub fn decay(&self) {
        let current = self.multiplier.load(Ordering::Relaxed);
        if current > 100 {
            self.multiplier
                .store((current - 10).max(100), Ordering::Relaxed);
        }
        // Reset counters
        self.error_count.store(0, Ordering::Relaxed);
        self.backpressure_count.store(0, Ordering::Relaxed);
    }
}

/// TUI runtime managing all background tasks
pub struct TuiRuntime {
    cancel_token: CancellationToken,
    task_handles: Vec<JoinHandle<()>>,
}

impl TuiRuntime {
    /// Create a new TUI runtime
    pub fn new() -> Self {
        Self {
            cancel_token: CancellationToken::new(),
            task_handles: Vec::new(),
        }
    }

    /// Get a clone of the cancellation token for spawning tasks
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Add a task handle to track
    pub fn track(&mut self, handle: JoinHandle<()>) {
        self.task_handles.push(handle);
    }

    /// Signal shutdown and wait for tasks to complete
    pub async fn shutdown(self) {
        // Signal all tasks to stop
        self.cancel_token.cancel();

        // Wait for graceful shutdown with timeout
        let shutdown = async {
            for handle in self.task_handles {
                let _ = handle.await;
            }
        };

        tokio::select! {
            _ = shutdown => {}
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                // Tasks did not stop in time; they will be dropped
            }
        }
    }
}

/// Spawn the input event reader task
pub fn spawn_input_task(tx: mpsc::Sender<InputEvent>, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = EventStream::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                maybe_event = reader.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            let input_event = match event {
                                Event::Key(key) => Some(InputEvent::Key(key)),
                                Event::Mouse(mouse) => Some(InputEvent::Mouse(mouse)),
                                Event::Resize(w, h) => Some(InputEvent::Resize(w, h)),
                                _ => None,
                            };

                            if let Some(evt) = input_event {
                                // Input channel should never be full, but handle it gracefully
                                if tx.send(evt).await.is_err() {
                                    break; // Receiver dropped
                                }
                            }
                        }
                        Some(Err(e)) => {
                            // Check for fatal terminal errors that should trigger shutdown
                            let is_fatal = matches!(
                                e.kind(),
                                std::io::ErrorKind::BrokenPipe
                                    | std::io::ErrorKind::ConnectionReset
                                    | std::io::ErrorKind::UnexpectedEof
                            );

                            if is_fatal {
                                tracing::info!("Terminal disconnected: {:?}", e);
                                break; // Graceful shutdown on terminal disconnect
                            } else {
                                // Log non-fatal errors (signal interruptions, temporary issues)
                                tracing::warn!("Terminal event read error: {:?}", e);
                            }
                        }
                        None => break, // Stream ended
                    }
                }
            }
        }
    })
}

/// Spawn the job data fetcher task
pub fn spawn_job_fetcher(
    tx: mpsc::Sender<DataEvent>,
    cancel: CancellationToken,
    throttle: Arc<FetcherThrottle>,
    username: String,
    show_all: Arc<AtomicBool>,
    slurm_bin_path: Option<std::path::PathBuf>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let slurm = SlurmInterface::with_config(slurm_bin_path.as_deref());

        // Initial fetch immediately
        fetch_and_send_jobs(
            &slurm,
            &tx,
            &throttle,
            &username,
            show_all.load(Ordering::Relaxed),
        )
        .await;

        loop {
            // Calculate current interval with throttle multiplier
            let multiplier = throttle.get_multiplier();
            let current_interval =
                Duration::from_secs_f32(DEFAULT_JOBS_INTERVAL.as_secs_f32() * multiplier);

            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(current_interval) => {
                    fetch_and_send_jobs(
                        &slurm,
                        &tx,
                        &throttle,
                        &username,
                        show_all.load(Ordering::Relaxed)
                    ).await;
                }
            }
        }
    })
}

async fn fetch_and_send_jobs(
    slurm: &SlurmInterface,
    tx: &mpsc::Sender<DataEvent>,
    throttle: &FetcherThrottle,
    username: &str,
    show_all: bool,
) {
    let users = if show_all {
        None
    } else {
        Some(vec![username.to_string()])
    };

    // Run blocking Slurm command in a separate thread
    let slurm_clone = slurm.clone();
    let result = tokio::task::spawn_blocking(move || {
        slurm_clone.get_jobs(users.as_deref(), None, None, None, None)
    })
    .await;

    handle_fetch_result(result, tx, throttle, DataSource::Jobs, |jobs| {
        let tui_jobs: Vec<TuiJobInfo> = jobs.iter().map(TuiJobInfo::from_job_info).collect();
        DataEvent::JobsUpdated(tui_jobs)
    });
}

/// Spawn the node data fetcher task
pub fn spawn_node_fetcher(
    tx: mpsc::Sender<DataEvent>,
    cancel: CancellationToken,
    throttle: Arc<FetcherThrottle>,
    slurm_bin_path: Option<std::path::PathBuf>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let slurm = SlurmInterface::with_config(slurm_bin_path.as_deref());

        // Initial fetch immediately
        fetch_and_send_nodes(&slurm, &tx, &throttle).await;

        loop {
            let multiplier = throttle.get_multiplier();
            let current_interval =
                Duration::from_secs_f32(DEFAULT_NODES_INTERVAL.as_secs_f32() * multiplier);

            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(current_interval) => {
                    fetch_and_send_nodes(&slurm, &tx, &throttle).await;
                }
            }
        }
    })
}

async fn fetch_and_send_nodes(
    slurm: &SlurmInterface,
    tx: &mpsc::Sender<DataEvent>,
    throttle: &FetcherThrottle,
) {
    let slurm_clone = slurm.clone();
    let result =
        tokio::task::spawn_blocking(move || slurm_clone.get_nodes(None, None, None, false)).await;

    handle_fetch_result(
        result,
        tx,
        throttle,
        DataSource::Nodes,
        DataEvent::NodesUpdated,
    );
}

/// Spawn the fairshare data fetcher task
pub fn spawn_fairshare_fetcher(
    tx: mpsc::Sender<DataEvent>,
    cancel: CancellationToken,
    throttle: Arc<FetcherThrottle>,
    username: String,
    slurm_bin_path: Option<std::path::PathBuf>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let slurm = SlurmInterface::with_config(slurm_bin_path.as_deref());

        // Initial fetch immediately
        fetch_and_send_fairshare(&slurm, &tx, &throttle, &username).await;

        loop {
            let multiplier = throttle.get_multiplier();
            let current_interval =
                Duration::from_secs_f32(DEFAULT_FAIRSHARE_INTERVAL.as_secs_f32() * multiplier);

            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(current_interval) => {
                    fetch_and_send_fairshare(&slurm, &tx, &throttle, &username).await;
                }
            }
        }
    })
}

async fn fetch_and_send_fairshare(
    slurm: &SlurmInterface,
    tx: &mpsc::Sender<DataEvent>,
    throttle: &FetcherThrottle,
    username: &str,
) {
    let slurm_clone = slurm.clone();
    let username_owned = username.to_string();
    let result =
        tokio::task::spawn_blocking(move || slurm_clone.get_fairshare(Some(&username_owned), None))
            .await;

    handle_fetch_result(
        result,
        tx,
        throttle,
        DataSource::Fairshare,
        DataEvent::FairshareUpdated,
    );
}

/// Spawn the scheduler stats fetcher task
///
/// Note: sdiag may require admin privileges on some clusters.
/// This fetcher gracefully handles permission denied errors.
pub fn spawn_scheduler_stats_fetcher(
    tx: mpsc::Sender<DataEvent>,
    cancel: CancellationToken,
    throttle: Arc<FetcherThrottle>,
    slurm_bin_path: Option<std::path::PathBuf>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let slurm = SlurmInterface::with_config(slurm_bin_path.as_deref());

        // Initial fetch immediately
        fetch_and_send_scheduler_stats(&slurm, &tx, &throttle).await;

        loop {
            let multiplier = throttle.get_multiplier();
            let current_interval = Duration::from_secs_f32(
                DEFAULT_SCHEDULER_STATS_INTERVAL.as_secs_f32() * multiplier,
            );

            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(current_interval) => {
                    fetch_and_send_scheduler_stats(&slurm, &tx, &throttle).await;
                }
            }
        }
    })
}

async fn fetch_and_send_scheduler_stats(
    slurm: &SlurmInterface,
    tx: &mpsc::Sender<DataEvent>,
    throttle: &FetcherThrottle,
) {
    let slurm_clone = slurm.clone();
    let result = tokio::task::spawn_blocking(move || slurm_clone.get_scheduler_stats()).await;

    match result {
        Ok(stats) => {
            // Always send stats, even if unavailable (the UI handles this)
            if tx
                .try_send(DataEvent::SchedulerStatsUpdated(stats))
                .is_err()
            {
                throttle.record_backpressure();
                tracing::debug!("Could not send scheduler stats (channel full)");
            }
        }
        Err(e) => {
            // Task join error - scheduler stats are non-critical, but log for debugging
            throttle.record_error();
            if tx
                .try_send(DataEvent::FetchError {
                    source: DataSource::SchedulerStats,
                    error: format!("Task join error: {}", e),
                })
                .is_err()
            {
                tracing::warn!("Could not send scheduler stats error notification (channel full)");
            }
        }
    }
}

/// Spawn the animation tick task
pub fn spawn_animation_tick(
    tx: mpsc::Sender<DataEvent>,
    cancel: CancellationToken,
    animation_visible: Arc<AtomicBool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(ANIMATION_TICK_INTERVAL);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    // Only send tick if animation is visible
                    if animation_visible.load(Ordering::Relaxed) {
                        let _ = tx.try_send(DataEvent::AnimationTick);
                    }
                }
            }
        }
    })
}

/// Spawn the throttle decay task
pub fn spawn_throttle_decay(
    cancel: CancellationToken,
    throttle: Arc<FetcherThrottle>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    throttle.decay();
                }
            }
        }
    })
}

/// Run the main TUI event loop
pub async fn run_event_loop(
    mut app: App,
    mut input_rx: mpsc::Receiver<InputEvent>,
    mut data_rx: mpsc::Receiver<DataEvent>,
    throttle: Arc<FetcherThrottle>,
    mut render_fn: impl FnMut(&App) -> Result<()>,
) -> Result<()> {
    let mut needs_render = true;

    // Mark initial activity
    throttle.record_activity();

    loop {
        if needs_render {
            render_fn(&app)?;
            needs_render = false;
        }

        if !app.running {
            break;
        }

        tokio::select! {
            // Bias toward input channel to prevent input starvation
            biased;

            Some(input) = input_rx.recv() => {
                // Record user activity for adaptive refresh
                throttle.record_activity();

                match app.handle_input(input) {
                    EventResult::Continue => needs_render = true,
                    EventResult::Unchanged => {}
                    EventResult::Quit => break,
                }
            }

            Some(data) = data_rx.recv() => {
                match app.handle_data(data) {
                    EventResult::Continue => needs_render = true,
                    EventResult::Unchanged => {}
                    EventResult::Quit => break,
                }
            }

            else => break,
        }
    }

    Ok(())
}

/// Create the dual channels for the TUI
pub fn create_channels() -> (
    mpsc::Sender<InputEvent>,
    mpsc::Receiver<InputEvent>,
    mpsc::Sender<DataEvent>,
    mpsc::Receiver<DataEvent>,
) {
    let (input_tx, input_rx) = mpsc::channel(INPUT_CHANNEL_CAPACITY);
    let (data_tx, data_rx) = mpsc::channel(DATA_CHANNEL_CAPACITY);
    (input_tx, input_rx, data_tx, data_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throttle_default() {
        let throttle = FetcherThrottle::default();
        assert_eq!(throttle.get_multiplier(), 1.0);
    }

    #[test]
    fn test_throttle_backpressure() {
        let throttle = FetcherThrottle::default();

        // Recording backpressure increases multiplier after threshold
        for _ in 0..6 {
            throttle.record_backpressure();
        }

        assert!(throttle.get_multiplier() > 1.0);
    }

    #[test]
    fn test_throttle_decay() {
        let throttle = FetcherThrottle::default();
        throttle.multiplier.store(200, Ordering::Relaxed); // 2.0x

        throttle.decay();
        assert!(throttle.get_multiplier() < 2.0);
    }

    #[test]
    fn test_throttle_cap() {
        let throttle = FetcherThrottle::default();

        // Should cap at 4x
        for _ in 0..100 {
            throttle.record_error();
        }

        assert_eq!(throttle.get_multiplier(), 4.0);
    }
}
