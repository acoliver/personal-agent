//! Backup scheduler for automatic database backups
//!
//! Runs a background task that periodically triggers database backups
//! based on configured intervals. Supports graceful shutdown and
//! startup stale backup detection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::backup::DatabaseBackupSettings;
use crate::services::BackupService;

/// Minimum backoff time (in seconds) to prevent tight scheduling loops.
/// When a backup is scheduled for "now" (no previous backup or stale),
/// this ensures we wait at least this long before attempting again.
const MINIMUM_BACKOFF_SECONDS: i64 = 60;

/// Global flag to prevent multiple scheduler instances
static SCHEDULER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Reset the scheduler flag (for tests only)
#[doc(hidden)]
pub fn reset_scheduler_flag_for_tests() {
    SCHEDULER_RUNNING.store(false, Ordering::SeqCst);
}

/// Scheduler for automatic database backups
///
/// Runs in the background and triggers backups at configured intervals.
/// Supports graceful shutdown via cancellation token.
pub struct BackupScheduler {
    backup_service: Arc<dyn BackupService>,
    shutdown_rx: watch::Receiver<bool>,
}

impl BackupScheduler {
    /// Create a new backup scheduler
    ///
    /// # Arguments
    /// * `backup_service` - The backup service to use for creating backups
    /// * `shutdown_rx` - Watch receiver for graceful shutdown signal
    #[must_use]
    pub fn new(backup_service: Arc<dyn BackupService>, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            backup_service,
            shutdown_rx,
        }
    }

    /// Compute the next backup time based on last backup and interval
    ///
    /// Returns the next scheduled backup time. If no previous backup exists,
    /// returns the current time plus a minimum backoff to prevent tight loops.
    fn compute_next_backup_time(
        last_backup_time: Option<DateTime<Utc>>,
        interval_hours: u32,
    ) -> DateTime<Utc> {
        let now = Utc::now();
        let min_backoff = TimeDelta::seconds(MINIMUM_BACKOFF_SECONDS);

        last_backup_time.map_or_else(
            // No previous backup: schedule with minimum backoff
            || now + min_backoff,
            |last_time| {
                let interval = TimeDelta::hours(i64::from(interval_hours));
                let next_time = last_time + interval;

                // If the next time is in the past (we missed a backup), schedule for now
                // but apply minimum backoff to prevent tight loops
                if next_time <= now {
                    now + min_backoff
                } else {
                    next_time
                }
            },
        )
    }

    /// Check and run backup on startup if stale
    ///
    /// If `run_on_startup_if_stale` is enabled and a backup is needed,
    /// this will trigger an immediate backup before entering the main loop.
    async fn check_startup_backup(&self, settings: &DatabaseBackupSettings) {
        if !settings.run_on_startup_if_stale {
            debug!("Startup stale backup check disabled");
            return;
        }

        info!("Checking if startup backup is needed");

        match self.backup_service.should_backup().await {
            Ok(true) => {
                info!("Backup is stale, running startup backup");
                self.run_backup().await;
            }
            Ok(false) => {
                debug!("Backup is not stale, skipping startup backup");
            }
            Err(e) => {
                error!("Failed to check if backup is needed on startup: {}", e);
            }
        }
    }

    fn log_backup_result(result: crate::backup::BackupResult) {
        match result {
            crate::backup::BackupResult::Success { path, duration_ms } => {
                info!(
                    "Backup completed successfully: {} (took {}ms)",
                    path.display(),
                    duration_ms
                );
            }
            crate::backup::BackupResult::Skipped { reason } => {
                info!("Backup skipped: {}", reason);
            }
            crate::backup::BackupResult::Failed { error } => {
                error!("Backup failed: {}", error);
            }
        }
    }

    async fn create_and_log_backup(&self) {
        match self.backup_service.create_backup().await {
            Ok(result) => Self::log_backup_result(result),
            Err(e) => error!("Backup operation failed: {}", e),
        }
    }

    /// Run a single backup operation
    ///
    /// Calls `should_backup()` first, then `create_backup()` if needed.
    /// Logs the result appropriately.
    async fn run_backup(&self) {
        info!("Checking if backup is needed");

        match self.backup_service.should_backup().await {
            Ok(true) => {
                info!("Backup is needed, starting backup operation");
                self.create_and_log_backup().await;
            }
            Ok(false) => debug!("Backup not needed at this time"),
            Err(e) => error!("Failed to check if backup is needed: {}", e),
        }
    }

    async fn load_initial_settings(&self) -> Option<DatabaseBackupSettings> {
        match self.backup_service.get_settings().await {
            Ok(settings) => Some(settings),
            Err(e) => {
                error!("Failed to get backup settings: {}. Scheduler aborting.", e);
                None
            }
        }
    }

    async fn load_last_backup_time(&self) -> Option<DateTime<Utc>> {
        match self.backup_service.get_last_backup_time().await {
            Ok(last_backup_time) => last_backup_time,
            Err(e) => {
                warn!(
                    "Failed to get last backup time: {}. Assuming no previous backup.",
                    e
                );
                None
            }
        }
    }

    async fn load_current_settings(
        &self,
        fallback_settings: &DatabaseBackupSettings,
    ) -> DatabaseBackupSettings {
        match self.backup_service.get_settings().await {
            Ok(settings) => settings,
            Err(e) => {
                error!("Failed to get current settings: {}", e);
                fallback_settings.clone()
            }
        }
    }

    async fn resolve_current_last_backup(
        &self,
        fallback_last_backup: Option<DateTime<Utc>>,
    ) -> Option<DateTime<Utc>> {
        self.backup_service
            .get_last_backup_time()
            .await
            .map_or(fallback_last_backup, |time| time.or(fallback_last_backup))
    }

    fn log_next_backup_schedule(next_backup_time: DateTime<Utc>, wait_duration: TimeDelta) {
        info!(
            "Next backup scheduled at {} (waiting {} seconds)",
            next_backup_time,
            wait_duration.num_seconds()
        );
    }

    async fn wait_for_next_backup(&mut self, wait_duration: TimeDelta) -> bool {
        let tokio_duration =
            Duration::from_secs(wait_duration.num_seconds().try_into().unwrap_or(u64::MAX));
        let sleep_deadline = Instant::now() + tokio_duration;
        let sleep_fut = sleep_until(sleep_deadline);

        tokio::select! {
            () = sleep_fut => true,
            _ = self.shutdown_rx.changed() => {
                if *self.shutdown_rx.borrow() {
                    info!("Shutdown signal received during wait, exiting");
                    false
                } else {
                    true
                }
            }
        }
    }

    async fn scheduler_iteration(
        &mut self,
        fallback_settings: &DatabaseBackupSettings,
        fallback_last_backup: Option<DateTime<Utc>>,
    ) -> bool {
        if *self.shutdown_rx.borrow() {
            info!("Shutdown signal received, backup scheduler exiting");
            return false;
        }

        let current_settings = self.load_current_settings(fallback_settings).await;
        if !current_settings.enabled {
            info!("Automatic backups disabled, scheduler exiting");
            return false;
        }

        let current_last_backup = self.resolve_current_last_backup(fallback_last_backup).await;
        let next_backup_time =
            Self::compute_next_backup_time(current_last_backup, current_settings.interval_hours);
        let now = Utc::now();
        let wait_duration = if next_backup_time > now {
            next_backup_time - now
        } else {
            TimeDelta::zero()
        };

        Self::log_next_backup_schedule(next_backup_time, wait_duration);

        if !self.wait_for_next_backup(wait_duration).await || *self.shutdown_rx.borrow() {
            return false;
        }

        self.run_backup().await;
        true
    }

    /// Run the scheduler loop
    ///
    /// Loops indefinitely, waiting for the next scheduled backup time
    /// and triggering backups. Responds to shutdown signals for graceful exit.
    ///
    /// # Shutdown
    /// The scheduler will exit when the shutdown watch channel sends `true`.
    pub async fn run(&mut self) {
        info!("Backup scheduler starting");

        let Some(settings) = self.load_initial_settings().await else {
            return;
        };

        if !settings.enabled {
            info!("Automatic backups are disabled, scheduler exiting");
            return;
        }

        self.check_startup_backup(&settings).await;
        let last_backup_time = self.load_last_backup_time().await;

        info!(
            "Backup scheduler running with {} hour interval",
            settings.interval_hours
        );

        while self.scheduler_iteration(&settings, last_backup_time).await {}

        info!("Backup scheduler stopped");
    }
}

/// Spawn a backup scheduler task
///
/// Creates a new `BackupScheduler` and spawns it as a Tokio task.
/// Returns a `JoinHandle` for the spawned task and a shutdown sender
/// that can be used to signal graceful shutdown.
///
/// # Arguments
/// * `backup_service` - The backup service to use
///
/// # Returns
/// A tuple containing:
/// - `JoinHandle<()>` for the spawned scheduler task
/// - `watch::Sender<bool>` for sending shutdown signals
///
/// # Example
/// ```
/// use std::sync::Arc;
/// use personal_agent::backup::spawn_backup_scheduler;
///
/// # async fn example() {
/// # let backup_service: Arc<dyn personal_agent::services::BackupService> = unimplemented!();
/// let (handle, shutdown_tx) = spawn_backup_scheduler(backup_service);
///
/// // To shut down gracefully:
/// let _ = shutdown_tx.send(true);
/// handle.await.unwrap();
/// # }
/// ```
pub fn spawn_backup_scheduler(
    backup_service: Arc<dyn BackupService>,
) -> (JoinHandle<()>, watch::Sender<bool>) {
    // Drop guard to ensure the flag is cleared even if scheduler.run() panics
    struct SchedulerGuard;
    impl Drop for SchedulerGuard {
        fn drop(&mut self) {
            SCHEDULER_RUNNING.store(false, Ordering::SeqCst);
        }
    }

    // Check if a scheduler is already running
    if SCHEDULER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        warn!("Backup scheduler already running, skipping duplicate spawn");
        let (shutdown_tx, _shutdown_rx) = watch::channel(true); // Already shutdown
        let handle = tokio::spawn(async {});
        return (handle, shutdown_tx);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let mut scheduler = BackupScheduler::new(backup_service, shutdown_rx);

    let handle = tokio::spawn(async move {
        let _guard = SchedulerGuard;
        scheduler.run().await;
    });

    (handle, shutdown_tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackupService;

    #[async_trait::async_trait]
    impl BackupService for MockBackupService {
        async fn create_backup(
            &self,
        ) -> crate::services::ServiceResult<crate::backup::BackupResult> {
            unimplemented!()
        }

        async fn list_backups(
            &self,
        ) -> crate::services::ServiceResult<Vec<crate::backup::BackupInfo>> {
            unimplemented!()
        }

        async fn restore_backup(
            &self,
            _path: &std::path::Path,
        ) -> crate::services::ServiceResult<crate::backup::RestoreResult> {
            unimplemented!()
        }

        async fn get_settings(&self) -> crate::services::ServiceResult<DatabaseBackupSettings> {
            Ok(DatabaseBackupSettings::default())
        }

        async fn update_settings(
            &self,
            _settings: DatabaseBackupSettings,
        ) -> crate::services::ServiceResult<()> {
            unimplemented!()
        }

        async fn get_last_backup_time(
            &self,
        ) -> crate::services::ServiceResult<Option<DateTime<Utc>>> {
            Ok(None)
        }

        async fn should_backup(&self) -> crate::services::ServiceResult<bool> {
            Ok(true)
        }
    }

    #[test]
    fn test_compute_next_backup_time_no_previous() {
        let next = BackupScheduler::compute_next_backup_time(None, 12);
        let now = Utc::now();

        // Should be at least MINIMUM_BACKOFF_SECONDS in the future
        let diff = (next - now).num_seconds();
        assert!(
            diff >= MINIMUM_BACKOFF_SECONDS - 1, // Allow 1 second tolerance
            "Expected next backup time to be at least {MINIMUM_BACKOFF_SECONDS} seconds in future, got {diff} seconds"
        );
        assert!(
            diff < MINIMUM_BACKOFF_SECONDS + 5,
            "Expected next backup time to be approximately {MINIMUM_BACKOFF_SECONDS} seconds in future, got {diff} seconds"
        );
    }

    #[test]
    fn test_compute_next_backup_time_with_previous() {
        // Use a last_backup time that's slightly in the future so that
        // last_backup + 12 hours is also in the future (relative to now)
        let last_backup = Utc::now() + TimeDelta::hours(1);
        let next = BackupScheduler::compute_next_backup_time(Some(last_backup), 12);

        // Expected: last_backup + 12 hours (which should be in the future)
        let expected = last_backup + TimeDelta::hours(12);
        assert_eq!(next, expected);
    }

    #[test]
    fn test_compute_next_backup_time_stale() {
        // Last backup was 24 hours ago with 12 hour interval (stale)
        let last_backup = Utc::now() - TimeDelta::hours(24);
        let next = BackupScheduler::compute_next_backup_time(Some(last_backup), 12);
        let now = Utc::now();

        // Should be now + minimum backoff (not just "now")
        let diff = (next - now).num_seconds();
        assert!(
            diff >= MINIMUM_BACKOFF_SECONDS - 1,
            "Expected stale backup to schedule with minimum backoff ({MINIMUM_BACKOFF_SECONDS} seconds), got {diff} seconds"
        );
        assert!(
            diff < MINIMUM_BACKOFF_SECONDS + 5,
            "Expected stale backup to schedule with approximately {MINIMUM_BACKOFF_SECONDS} seconds backoff, got {diff} seconds"
        );
    }

    #[test]
    fn test_scheduler_new() {
        let (tx, rx) = watch::channel(false);
        let service: Arc<dyn BackupService> = Arc::new(MockBackupService);
        let scheduler = BackupScheduler::new(Arc::clone(&service), rx);

        // Just verify it compiles and fields are set
        assert!(!*scheduler.shutdown_rx.borrow());
        let _ = tx;
    }

    #[tokio::test]
    async fn test_single_scheduler_instance_enforced() {
        // Reset the global flag
        SCHEDULER_RUNNING.store(false, Ordering::SeqCst);

        let service: Arc<dyn BackupService> = Arc::new(MockBackupService);

        // First spawn should succeed
        let (handle1, shutdown1) = spawn_backup_scheduler(Arc::clone(&service));
        assert!(SCHEDULER_RUNNING.load(Ordering::SeqCst));

        // Second spawn should be rejected
        let (handle2, shutdown2) = spawn_backup_scheduler(Arc::clone(&service));

        // The second handle should be a no-op (spawn already running)
        // We can't easily test this without waiting, so just verify the flag is still set
        assert!(SCHEDULER_RUNNING.load(Ordering::SeqCst));

        // Clean up
        let _ = shutdown1.send(true);
        let _ = shutdown2.send(true);
        // Wait for scheduler to exit and clear the flag
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(handle1);
        drop(handle2);
    }
}
