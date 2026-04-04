//! DB worker thread.
//!
//! A single-threaded `rusqlite::Connection` is owned by a dedicated OS thread.
//! All callers dispatch closures via `DbHandle::execute`, which sends the closure
//! to the thread and awaits the result on a `tokio::sync::oneshot` channel.
//!
//! See spec §1.5 for the full threading model rationale and lifecycle contract.

use std::path::Path;

use crate::services::ServiceError;

use super::schema::initialize_schema;

// ---------------------------------------------------------------------------
// Job type
// ---------------------------------------------------------------------------

/// A boxed closure dispatched to the DB thread.
///
/// Each closure captures its own `tokio::sync::oneshot::Sender` internally so
/// the job queue carries no generic type parameter.
pub type DbJob = Box<dyn FnOnce(&rusqlite::Connection) + Send + 'static>;

// ---------------------------------------------------------------------------
// DbHandle
// ---------------------------------------------------------------------------

/// A cheaply-cloneable handle to the DB worker thread.
///
/// Dropping all clones causes `rx.recv()` on the worker to return `RecvError`,
/// which exits the recv loop and drops the `Connection` (flushing the WAL).
pub struct DbHandle {
    sender: std::sync::mpsc::Sender<DbJob>,
}

impl DbHandle {
    /// Execute a closure on the DB thread and return its result.
    ///
    /// The closure receives exclusive access to the `Connection` for its
    /// duration. The future resolves once the DB thread completes the job.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::Storage` if the DB thread has shut down, drops
    /// the response channel, or if the closure returns a `rusqlite::Error`.
    /// Returns `ServiceError::NotFound` if the closure returns
    /// `rusqlite::Error::QueryReturnedNoRows`.
    pub async fn execute<F, R>(&self, f: F) -> Result<R, ServiceError>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job: DbJob = Box::new(move |conn| {
            let result = f(conn);
            let _ = tx.send(result);
        });
        self.sender
            .send(job)
            .map_err(|_| ServiceError::Storage("DB thread has shut down".into()))?;
        rx.await
            .map_err(|_| ServiceError::Storage("DB thread dropped response".into()))?
            .map_err(|e| match &e {
                rusqlite::Error::QueryReturnedNoRows => {
                    ServiceError::NotFound("record not found".into())
                }
                _ => ServiceError::Storage(format!("SQLite error: {e}")),
            })
    }
}

impl Clone for DbHandle {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_db_thread
// ---------------------------------------------------------------------------

/// Spawn the DB worker thread, open the `SQLite` connection, and initialize the
/// schema.
///
/// Returns a `DbHandle` on success, or a `ServiceError` if the thread fails to
/// start, the database fails to open, or schema initialization fails.
///
/// # Lifecycle
///
/// The worker thread runs until all `DbHandle` clones are dropped, at which
/// point `rx.recv()` returns `Err(RecvError)`, the loop exits, and the
/// `Connection` is dropped (flushing the WAL).
///
/// # Panic safety
///
/// Each dispatched job is wrapped in `std::panic::catch_unwind`. If a job
/// panics, the panic is caught and the worker continues processing subsequent
/// jobs. The panicked job's oneshot sender is dropped without sending, so the
/// caller receives `RecvError` mapped to `ServiceError::Storage(...)`.
///
/// # Errors
///
/// Returns `ServiceError::Storage` if the OS thread cannot be spawned, if
/// opening the database file fails, or if schema initialization fails.
pub fn spawn_db_thread(db_path: &Path) -> Result<DbHandle, ServiceError> {
    let (tx, rx) = std::sync::mpsc::channel::<DbJob>();
    let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let path = db_path.to_owned();

    std::thread::Builder::new()
        .name("db-worker".into())
        .spawn(move || {
            let conn = match rusqlite::Connection::open(&path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("failed to open database: {e}")));
                    return;
                }
            };
            match initialize_schema(&conn) {
                Ok(()) => {
                    let _ = init_tx.send(Ok(()));
                }
                Err(e) => {
                    let _ = init_tx.send(Err(format!("failed to initialize schema: {e}")));
                    return;
                }
            }
            while let Ok(job) = rx.recv() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&conn)));
                // If job panicked, its captured oneshot tx was dropped → caller gets RecvError.
            }
            // Connection dropped here — SQLite flushes the WAL.
        })
        .map_err(|e| ServiceError::Storage(format!("failed to spawn DB thread: {e}")))?;

    // Block (on the calling async thread) until the DB thread reports init
    // success or failure. `blocking_recv` is safe here because `spawn_db_thread`
    // is itself called from a blocking context (app startup), not from inside an
    // async task.
    init_rx
        .blocking_recv()
        .map_err(|_| ServiceError::Storage("DB thread died during init".into()))?
        .map_err(ServiceError::Storage)?;

    Ok(DbHandle { sender: tx })
}
