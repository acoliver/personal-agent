//! Hardware capability detection for local model inference.
//!
//! Determines if the system can reliably run a local LLM by checking
//! RAM, CPU cores, and GPU availability.

use sysinfo::System;

/// Hardware capabilities relevant for local LLM inference.
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// Total system RAM in gigabytes.
    pub total_ram_gb: f64,
    /// Currently available (free) RAM in gigabytes.
    pub available_ram_gb: f64,
    /// Number of CPU cores.
    pub cpu_cores: usize,
    /// Whether Metal is available (macOS only).
    pub has_metal: bool,
    /// Operating system name.
    pub os_name: String,
}

impl HardwareCapabilities {
    /// Detect current hardware capabilities.
    ///
    /// This queries the system for RAM, CPU, and GPU information.
    /// The detection is fast (~10-50ms) and can be run at startup.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn detect() -> Self {
        let sys = System::new_all();

        let total_ram_gb = sys.total_memory() as f64 / 1_073_741_824.0;
        let available_ram_gb = sys.available_memory() as f64 / 1_073_741_824.0;
        let cpu_cores = sys.cpus().len();

        // macOS always has Metal on supported hardware
        #[cfg(target_os = "macos")]
        let has_metal = true;

        #[cfg(not(target_os = "macos"))]
        let has_metal = false;

        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());

        Self {
            total_ram_gb,
            available_ram_gb,
            cpu_cores,
            has_metal,
            os_name,
        }
    }

    /// Check if this machine meets the minimum requirements for local inference.
    ///
    /// Requirements:
    /// - 16GB total RAM (hard minimum)
    /// - 4+ CPU cores (for acceptable token generation speed)
    #[must_use]
    pub const fn can_run_local_model(&self) -> bool {
        self.total_ram_gb >= 16.0 && self.cpu_cores >= 4
    }

    /// Check if available memory is sufficient for loading the model.
    ///
    /// Qwen3.5-4B `Q4_K_M` needs approximately:
    ///   - 2.7GB for model weights
    ///   - 1-2GB for KV cache (depends on context length)
    ///   - 2GB headroom for OS and app
    ///   - Total: ~6GB available needed
    #[must_use]
    pub fn has_sufficient_available_memory(&self) -> bool {
        self.available_ram_gb >= 6.0
    }

    /// Check if we should warn about memory pressure.
    ///
    /// Returns true if the machine can run local models but available
    /// memory is low (between 2GB and 6GB).
    #[must_use]
    pub fn should_warn_memory_pressure(&self) -> bool {
        self.can_run_local_model() && self.available_ram_gb >= 2.0 && self.available_ram_gb < 6.0
    }

    /// Check if available memory is critically low.
    ///
    /// Returns true if available memory is below 2GB - the model
    /// will likely fail to load or cause system instability.
    #[must_use]
    pub fn is_memory_critical(&self) -> bool {
        self.available_ram_gb < 2.0
    }

    /// Get a human-readable status message.
    #[must_use]
    pub fn status_message(&self) -> HardwareStatus {
        if !self.can_run_local_model() {
            HardwareStatus::InsufficientHardware {
                reason: if self.total_ram_gb < 16.0 {
                    format!(
                        "This machine has {:.0}GB RAM. Local models require 16GB.",
                        self.total_ram_gb
                    )
                } else {
                    "Insufficient CPU cores (need 4+)".to_string()
                },
            }
        } else if self.is_memory_critical() {
            HardwareStatus::MemoryCritical {
                available_gb: self.available_ram_gb,
            }
        } else if self.should_warn_memory_pressure() {
            HardwareStatus::MemoryWarning {
                available_gb: self.available_ram_gb,
            }
        } else {
            HardwareStatus::Ready {
                total_gb: self.total_ram_gb,
                available_gb: self.available_ram_gb,
            }
        }
    }
}

impl Default for HardwareCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

/// Hardware status for display to users.
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareStatus {
    /// System is ready for local inference.
    Ready { total_gb: f64, available_gb: f64 },
    /// System can run but memory is constrained.
    MemoryWarning { available_gb: f64 },
    /// System can run but memory is critically low.
    MemoryCritical { available_gb: f64 },
    /// System does not meet minimum requirements.
    InsufficientHardware { reason: String },
}

impl std::fmt::Display for HardwareStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready {
                total_gb,
                available_gb,
            } => {
                write!(
                    f,
                    "Your system has {total_gb:.0}GB RAM with {available_gb:.1}GB available - local models supported",
                )
            }
            Self::MemoryWarning { available_gb } => {
                write!(
                    f,
                    "Warning: Only {available_gb:.1}GB RAM available. Consider closing other apps before running local models.",
                )
            }
            Self::MemoryCritical { available_gb } => {
                write!(
                    f,
                    "Critical: Only {available_gb:.1}GB RAM available. Model may fail to load.",
                )
            }
            Self::InsufficientHardware { reason } => {
                write!(f, "{reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware() {
        let hw = HardwareCapabilities::detect();
        // Should always succeed on any platform
        assert!(hw.total_ram_gb > 0.0);
        assert!(hw.cpu_cores > 0);
    }

    #[test]
    fn test_can_run_local_model_16gb() {
        let hw = HardwareCapabilities {
            total_ram_gb: 16.0,
            available_ram_gb: 8.0,
            cpu_cores: 4,
            has_metal: true,
            os_name: "macOS".to_string(),
        };
        assert!(hw.can_run_local_model());
        assert!(hw.has_sufficient_available_memory());
    }

    #[test]
    fn test_cannot_run_8gb() {
        let hw = HardwareCapabilities {
            total_ram_gb: 8.0,
            available_ram_gb: 4.0,
            cpu_cores: 4,
            has_metal: true,
            os_name: "macOS".to_string(),
        };
        assert!(!hw.can_run_local_model());
    }

    #[test]
    fn test_memory_warning() {
        let hw = HardwareCapabilities {
            total_ram_gb: 16.0,
            available_ram_gb: 4.0, // Between 2 and 6
            cpu_cores: 4,
            has_metal: true,
            os_name: "macOS".to_string(),
        };
        assert!(hw.can_run_local_model());
        assert!(!hw.has_sufficient_available_memory());
        assert!(hw.should_warn_memory_pressure());
        assert!(!hw.is_memory_critical());
    }

    #[test]
    fn test_memory_critical() {
        let hw = HardwareCapabilities {
            total_ram_gb: 16.0,
            available_ram_gb: 1.5, // Below 2GB
            cpu_cores: 4,
            has_metal: true,
            os_name: "macOS".to_string(),
        };
        assert!(hw.can_run_local_model());
        assert!(hw.is_memory_critical());
    }
}
