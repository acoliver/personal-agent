//! DB worker thread.
//!
//! A single-threaded `rusqlite::Connection` is owned by a dedicated OS thread.
//! All callers dispatch closures via `DbHandle::execute`, which sends the closure
//! to the thread and awaits the result on a `tokio::sync::oneshot` channel.
//!
//! See spec §1.5 for the full threading model rationale and lifecycle contract.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::services::ServiceError;

use super::schema::initialize_schema;

// ---------------------------------------------------------------------------
// Job type
// ---------------------------------------------------------------------------

/// A boxed closure dispatched to the DB thread.
///
/// Each closure captures its own `tokio::sync::oneshot::Sender` internally so
/// the job queue carries no generic type parameter.
pub type DbJob = Box<dyn FnOnce(&mut Option<rusqlite::Connection>) + Send + 'static>;

// ---------------------------------------------------------------------------
// DbHandle
// ---------------------------------------------------------------------------

/// A cheaply-cloneable handle to the DB worker thread.
///
/// Dropping all clones causes `rx.recv()` on the worker to return `RecvError`,
/// which exits the recv loop and drops the `Connection` (flushing the WAL).
#[derive(Clone)]
pub struct DbHandle {
    inner: Arc<DbHandleInner>,
}

struct DbHandleInner {
    sender: std::sync::mpsc::Sender<DbJob>,
    db_path: PathBuf,
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
            if let Some(c) = conn {
                let result = f(c);
                let _ = tx.send(result);
            } else {
                let _ = tx.send(Err(rusqlite::Error::InvalidPath(
                    "Database connection is closed".into(),
                )));
            }
        });
        self.inner
            .sender
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

    /// Close the database connection, allowing the file to be replaced.
    ///
    /// After calling this, the connection is closed. Use `reopen()` to
    /// open a new connection to the same path.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::Storage` if the DB thread has shut down.
    pub async fn close(&self) -> Result<(), ServiceError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
        let job: DbJob = Box::new(move |conn| {
            *conn = None;
            let _ = tx.send(Ok(()));
        });
        self.inner
            .sender
            .send(job)
            .map_err(|_| ServiceError::Storage("DB thread has shut down".into()))?;
        rx.await
            .map_err(|_| ServiceError::Storage("DB thread dropped response".into()))?
            .map_err(ServiceError::Storage)
    }

    /// Reopen the database connection after it was closed.
    ///
    /// Opens a new connection to the same path and initializes the schema.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::Storage` if the DB thread has shut down or
    /// if opening the database fails.
    pub async fn reopen(&self) -> Result<(), ServiceError> {
        let path = self.inner.db_path.clone();
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
        let job: DbJob = Box::new(move |conn| match rusqlite::Connection::open(&path) {
            Ok(c) => match initialize_schema(&c) {
                Ok(()) => {
                    *conn = Some(c);
                    let _ = tx.send(Ok(()));
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("failed to initialize schema: {e}")));
                }
            },
            Err(e) => {
                let _ = tx.send(Err(format!("failed to open database: {e}")));
            }
        });
        self.inner
            .sender
            .send(job)
            .map_err(|_| ServiceError::Storage("DB thread has shut down".into()))?;
        rx.await
            .map_err(|_| ServiceError::Storage("DB thread dropped response".into()))?
            .map_err(ServiceError::Storage)
    }

    /// Get the database file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.db_path
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
            let mut conn: Option<rusqlite::Connection> = match rusqlite::Connection::open(&path) {
                Ok(c) => Some(c),
                Err(e) => {
                    let _ = init_tx.send(Err(format!("failed to open database: {e}")));
                    return;
                }
            };
            if let Some(ref c) = conn {
                match initialize_schema(c) {
                    Ok(()) => {
                        let _ = init_tx.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = init_tx.send(Err(format!("failed to initialize schema: {e}")));
                        return;
                    }
                }
            }
            while let Ok(job) = rx.recv() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut conn)));
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

    Ok(DbHandle {
        inner: Arc::new(DbHandleInner {
            sender: tx,
            db_path: db_path.to_owned(),
        }),
    })
}
