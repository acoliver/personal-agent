//! Launch-at-login control (Issue #177).
//!
//! Provides a thin, platform-agnostic trait over the macOS
//! `SMAppService.mainApp` API so the presenter layer does not have to know
//! about `AppKit`. Non-macOS targets get a stub that reports the feature as
//! unsupported; this keeps the Linux and Windows build paths untouched.
//!
//! The trait is deliberately narrow (query + register/unregister) so it is
//! trivial to fake in unit tests.

/// High-level state of the login-item registration.
///
/// This mirrors the Apple `SMAppServiceStatus` enum but stays independent of
/// the `objc2-service-management` crate so non-macOS builds can reference it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItemStatus {
    /// The app is registered and will launch at login.
    Enabled,
    /// The app is not registered as a login item.
    NotRegistered,
    /// Registration succeeded but the user must approve it in
    /// System Settings → General → Login Items.
    RequiresApproval,
    /// The OS reports the registration target could not be found (typically
    /// because the running binary is not inside a valid `.app` bundle, e.g.
    /// raw `cargo run` or an unpackaged Homebrew binary).
    NotFound,
    /// The platform does not support launch-at-login (non-macOS builds).
    Unsupported,
}

/// Errors surfaced from `LoginItemService`. Kept as a simple wrapper around
/// a user-facing message so the presenter can forward it straight into the
/// view command without additional formatting.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct LoginItemError(pub String);

/// Control the "launch at login" state.
///
/// Implementations are expected to be cheap to call; the macOS implementation
/// simply forwards to `SMAppService.mainApp`.
pub trait LoginItemService: Send + Sync {
    /// Current OS-level registration status.
    ///
    /// # Errors
    /// Returns a `LoginItemError` if the underlying OS call fails.
    fn status(&self) -> Result<LoginItemStatus, LoginItemError>;

    /// Register the running app as a login item.
    ///
    /// On success, the OS reports either `Enabled` (immediate) or
    /// `RequiresApproval` (user must flip the toggle in System Settings).
    ///
    /// # Errors
    /// Returns a `LoginItemError` if the OS rejects the registration (most
    /// commonly because the running binary is not a Developer ID–signed
    /// `.app` bundle).
    fn register(&self) -> Result<LoginItemStatus, LoginItemError>;

    /// Unregister the running app.
    ///
    /// # Errors
    /// Returns a `LoginItemError` if the OS call fails.
    fn unregister(&self) -> Result<LoginItemStatus, LoginItemError>;

    /// Whether this implementation actually controls the OS (false for the
    /// non-macOS stub). The presenter uses this to decide whether to show
    /// the toggle at all.
    fn is_supported(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// macOS implementation — SMAppService.mainApp
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "SMAppService bindings (objc2-service-management) require \
              unsafe Objective-C method calls; no public unsafe API is \
              exposed by this module."
)]
mod macos_impl {
    use super::{LoginItemError, LoginItemService, LoginItemStatus};
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    /// Concrete macOS implementation backed by `SMAppService.mainApp`.
    pub struct SmAppServiceLoginItem;

    impl SmAppServiceLoginItem {
        #[must_use]
        pub const fn new() -> Self {
            Self
        }

        fn main_app() -> objc2::rc::Retained<SMAppService> {
            // SAFETY: `mainAppService` is a class method that returns an
            // autoreleased singleton handle; objc2 retains it for us. No
            // thread-affinity requirements documented by Apple.
            unsafe { SMAppService::mainAppService() }
        }

        const fn map_status(raw: SMAppServiceStatus) -> LoginItemStatus {
            match raw {
                SMAppServiceStatus::Enabled => LoginItemStatus::Enabled,
                SMAppServiceStatus::RequiresApproval => LoginItemStatus::RequiresApproval,
                SMAppServiceStatus::NotFound => LoginItemStatus::NotFound,
                // `NotRegistered` and any future variant fall through to the
                // safe default.
                _ => LoginItemStatus::NotRegistered,
            }
        }

        fn ns_error_message(err: &objc2_foundation::NSError) -> String {
            let localized = err.localizedDescription();
            let domain = err.domain();
            let code = err.code();
            format!("{localized} (domain={domain}, code={code})")
        }
    }

    impl Default for SmAppServiceLoginItem {
        fn default() -> Self {
            Self::new()
        }
    }

    impl LoginItemService for SmAppServiceLoginItem {
        fn status(&self) -> Result<LoginItemStatus, LoginItemError> {
            let service = Self::main_app();
            // SAFETY: `status` is a read-only property on a retained handle.
            let raw = unsafe { service.status() };
            Ok(Self::map_status(raw))
        }

        fn register(&self) -> Result<LoginItemStatus, LoginItemError> {
            let service = Self::main_app();
            // SAFETY: `registerAndReturnError:` is documented thread-safe and
            // must be called on a retained `SMAppService` handle.
            let result = unsafe { service.registerAndReturnError() };
            match result {
                Ok(()) => {
                    let raw = unsafe { service.status() };
                    Ok(Self::map_status(raw))
                }
                Err(err) => Err(LoginItemError(format!(
                    "Could not register for launch at login. \
                     This usually means the app is not a Developer-ID signed \
                     .app bundle, or macOS blocked the request. Details: {}",
                    Self::ns_error_message(&err)
                ))),
            }
        }

        fn unregister(&self) -> Result<LoginItemStatus, LoginItemError> {
            let service = Self::main_app();
            // SAFETY: see `register` above.
            let result = unsafe { service.unregisterAndReturnError() };
            match result {
                Ok(()) => {
                    let raw = unsafe { service.status() };
                    Ok(Self::map_status(raw))
                }
                Err(err) => Err(LoginItemError(format!(
                    "Could not unregister launch-at-login: {}",
                    Self::ns_error_message(&err)
                ))),
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::SmAppServiceLoginItem;

// ---------------------------------------------------------------------------
// Non-macOS stub
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::{LoginItemError, LoginItemService, LoginItemStatus};

    /// Stub used on Linux/Windows. All operations report `Unsupported` so the
    /// Settings view can render a disabled toggle with an informative tooltip
    /// instead of silently failing.
    pub struct UnsupportedLoginItem;

    impl UnsupportedLoginItem {
        #[must_use]
        pub const fn new() -> Self {
            Self
        }
    }

    impl Default for UnsupportedLoginItem {
        fn default() -> Self {
            Self::new()
        }
    }

    impl LoginItemService for UnsupportedLoginItem {
        fn status(&self) -> Result<LoginItemStatus, LoginItemError> {
            Ok(LoginItemStatus::Unsupported)
        }

        fn register(&self) -> Result<LoginItemStatus, LoginItemError> {
            Err(LoginItemError(
                "Launch-at-login is only supported on macOS 13+.".to_string(),
            ))
        }

        fn unregister(&self) -> Result<LoginItemStatus, LoginItemError> {
            Err(LoginItemError(
                "Launch-at-login is only supported on macOS 13+.".to_string(),
            ))
        }

        fn is_supported(&self) -> bool {
            false
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::UnsupportedLoginItem;

// ---------------------------------------------------------------------------
// Construct a default service for the current platform.
// ---------------------------------------------------------------------------

/// Create the platform-default implementation used in production wiring.
#[must_use]
pub fn default_login_item_service() -> Box<dyn LoginItemService> {
    #[cfg(target_os = "macos")]
    {
        Box::new(SmAppServiceLoginItem::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(UnsupportedLoginItem::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake implementation used by presenter tests. Records calls and
    /// returns scripted responses; we keep it here (behind `cfg(test)`) so
    /// every call site sees the same test double.
    pub struct FakeLoginItemService {
        pub status: std::sync::Mutex<LoginItemStatus>,
        pub register_result: std::sync::Mutex<Option<Result<LoginItemStatus, LoginItemError>>>,
        pub unregister_result: std::sync::Mutex<Option<Result<LoginItemStatus, LoginItemError>>>,
        pub register_calls: std::sync::atomic::AtomicUsize,
        pub unregister_calls: std::sync::atomic::AtomicUsize,
    }

    impl FakeLoginItemService {
        pub fn new(initial: LoginItemStatus) -> Self {
            Self {
                status: std::sync::Mutex::new(initial),
                register_result: std::sync::Mutex::new(None),
                unregister_result: std::sync::Mutex::new(None),
                register_calls: std::sync::atomic::AtomicUsize::new(0),
                unregister_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl LoginItemService for FakeLoginItemService {
        fn status(&self) -> Result<LoginItemStatus, LoginItemError> {
            Ok(*self.status.lock().unwrap())
        }

        fn register(&self) -> Result<LoginItemStatus, LoginItemError> {
            self.register_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let scripted = self.register_result.lock().unwrap().take();
            if let Some(result) = scripted {
                if let Ok(new_status) = &result {
                    *self.status.lock().unwrap() = *new_status;
                }
                return result;
            }
            *self.status.lock().unwrap() = LoginItemStatus::Enabled;
            Ok(LoginItemStatus::Enabled)
        }

        fn unregister(&self) -> Result<LoginItemStatus, LoginItemError> {
            self.unregister_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let scripted = self.unregister_result.lock().unwrap().take();
            if let Some(result) = scripted {
                if let Ok(new_status) = &result {
                    *self.status.lock().unwrap() = *new_status;
                }
                return result;
            }
            *self.status.lock().unwrap() = LoginItemStatus::NotRegistered;
            Ok(LoginItemStatus::NotRegistered)
        }
    }

    #[test]
    fn default_factory_returns_a_working_service() {
        let svc = default_login_item_service();
        // Status must return something without panicking on every platform.
        let status = svc.status().expect("status() should not fail");

        #[cfg(target_os = "macos")]
        {
            assert!(svc.is_supported());
            // On macOS we expect one of the real statuses (not Unsupported).
            assert_ne!(status, LoginItemStatus::Unsupported);
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(!svc.is_supported());
            assert_eq!(status, LoginItemStatus::Unsupported);
        }
    }

    #[test]
    fn fake_service_records_register_and_unregister_calls() {
        let fake = FakeLoginItemService::new(LoginItemStatus::NotRegistered);

        assert_eq!(fake.status().unwrap(), LoginItemStatus::NotRegistered);

        let after_register = fake.register().unwrap();
        assert_eq!(after_register, LoginItemStatus::Enabled);
        assert_eq!(fake.status().unwrap(), LoginItemStatus::Enabled);
        assert_eq!(
            fake.register_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let after_unregister = fake.unregister().unwrap();
        assert_eq!(after_unregister, LoginItemStatus::NotRegistered);
        assert_eq!(fake.status().unwrap(), LoginItemStatus::NotRegistered);
        assert_eq!(
            fake.unregister_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn fake_service_returns_scripted_errors() {
        let fake = FakeLoginItemService::new(LoginItemStatus::NotRegistered);
        *fake.register_result.lock().unwrap() = Some(Err(LoginItemError("nope".to_string())));

        let err = fake.register().unwrap_err();
        assert_eq!(err.0, "nope");
        // Status must remain unchanged on error.
        assert_eq!(fake.status().unwrap(), LoginItemStatus::NotRegistered);
    }
}
