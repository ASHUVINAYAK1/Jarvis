//! JARVIS Platform Abstraction Layer
//!
//! Defines the `PlatformAdapter` trait that all platform-specific implementations
//! (Windows, Linux, Android) must implement.
//!
//! # Architecture
//!
//! The agent never calls Windows APIs directly. Instead:
//!
//! ```text
//! Agent Core
//!     ↓
//! platform.open_application("chrome")
//!     ↓
//! WindowsPlatformAdapter::open_application("chrome")
//!     ↓
//! CreateProcess(chrome.exe)
//! ```
//!
//! This separation ensures:
//! 1. The agent code is platform-independent
//! 2. Platform adapters can be tested independently
//! 3. Future platforms can be added without changing agent logic
//!
//! IMPLEMENTATION STATUS: Phase 6, Milestone M06.01 (trait definition)
//! The Windows implementation is in platforms/windows/
//! The Linux implementation is in platforms/linux/

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ============================================================
// Platform Information
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: OperatingSystem,
    pub os_version: String,
    pub arch: Architecture,
    pub hostname: String,
    pub username: String,
    pub home_dir: PathBuf,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingSystem {
    Windows,
    Linux,
    MacOS,
    Android,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    X86_64,
    Aarch64,
    X86,
}

// ============================================================
// Window Information
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Platform-specific window handle (as a string for cross-platform passing)
    pub handle: String,
    /// Window title
    pub title: String,
    /// Application/process name
    pub process_name: String,
    /// Process ID
    pub pid: u32,
    /// Whether the window is visible
    pub visible: bool,
    /// Whether the window has focus
    pub focused: bool,
    /// Window geometry
    pub bounds: Option<Rect>,
    /// Whether the window is minimized
    #[serde(default)]
    pub is_minimized: bool,
    /// Whether the window is maximized
    #[serde(default)]
    pub is_maximized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// ============================================================
// UI Automation / Accessibility Tree (Phase 8, M08.04)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    pub name: String,
    pub automation_id: String,
    pub control_type: String,
    pub class_name: String,
    pub bounds: Rect,
    pub center_x: i32,
    pub center_y: i32,
    pub enabled: bool,
    pub offscreen: bool,
    pub focused: bool,
    pub supported_patterns: Vec<String>,
}

impl UiElement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        automation_id: impl Into<String>,
        control_type: impl Into<String>,
        class_name: impl Into<String>,
        bounds: Rect,
        enabled: bool,
        offscreen: bool,
        focused: bool,
    ) -> Self {
        let center_x = bounds.x + (bounds.width as i32 / 2);
        let center_y = bounds.y + (bounds.height as i32 / 2);
        Self {
            name: name.into(),
            automation_id: automation_id.into(),
            control_type: control_type.into(),
            class_name: class_name.into(),
            bounds,
            center_x,
            center_y,
            enabled,
            offscreen,
            focused,
            supported_patterns: vec![],
        }
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }
        let q = trimmed.to_lowercase();
        let name_lower = self.name.to_lowercase();
        let type_lower = self.control_type.to_lowercase();
        let id_lower = self.automation_id.to_lowercase();

        name_lower.contains(&q) || type_lower.contains(&q) || id_lower.contains(&q)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTreeResult {
    pub window_title: String,
    pub process_name: String,
    pub elements: Vec<UiElement>,
    pub total_elements_scanned: usize,
    pub is_truncated: bool,
    pub source: String,
}

impl UiTreeResult {
    pub fn empty() -> Self {
        Self {
            window_title: String::new(),
            process_name: String::new(),
            elements: vec![],
            total_elements_scanned: 0,
            is_truncated: false,
            source: "WindowsUIAutomation".to_string(),
        }
    }
}

// ============================================================
// Process Information
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub executable_path: Option<PathBuf>,
    pub command_line: Option<String>,
    pub running: bool,
}

// ============================================================
// Screenshot
// ============================================================

#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Raw image bytes (PNG format)
    pub data: Vec<u8>,
    /// Image format
    pub format: ImageFormat,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Which display this was captured from (0 = primary)
    pub display_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

// ============================================================
// Notification
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub title: String,
    pub body: String,
    pub icon: Option<PathBuf>,
    /// If Some, the notification will expire after this many seconds
    pub timeout_secs: Option<u32>,
    /// Notification priority (affects how it's displayed)
    pub priority: NotificationPriority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[allow(clippy::derivable_impls)]
impl Default for NotificationPriority {
    fn default() -> Self {
        NotificationPriority::Normal
    }
}

// ============================================================
// Application Launch Options
// ============================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchOptions {
    /// Working directory for the process
    pub working_dir: Option<PathBuf>,
    /// Environment variables to set
    pub env_vars: Vec<(String, String)>,
    /// Whether to wait for the process to complete
    pub wait: bool,
    /// Command-line arguments
    pub args: Vec<String>,
}

// ============================================================
// Clipboard
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    Image { data: Vec<u8>, format: String },
    Files(Vec<PathBuf>),
    Empty,
}

// ============================================================
// Platform Adapter Trait
// ============================================================

/// The primary interface between the JARVIS agent and the operating system.
///
/// Every operation that touches the OS must go through this trait.
/// No crate other than platform adapters should call OS APIs directly.
///
/// # Security Note
///
/// All methods on this trait should check the active permission policy
/// before executing. The policy check is performed by the caller
/// (Tool Runtime), but adapter implementations should also validate
/// that they're not being called with dangerous arguments.
///
/// # Error Handling
///
/// Methods return `Result<T>`. Callers must handle errors.
/// Adapters should provide actionable error messages.
/// For example: "Chrome not found: no process named 'chrome.exe'"
/// rather than: "Error: operation failed".
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    // --------------------------------------------------------
    // Platform Info
    // --------------------------------------------------------

    /// Get information about the current platform.
    async fn get_platform_info(&self) -> Result<PlatformInfo>;

    // --------------------------------------------------------
    // Application Control
    // --------------------------------------------------------

    /// Launch an application by name or path.
    ///
    /// `app` can be:
    /// - An application name: "chrome", "notepad", "vscode"
    /// - A full path: "/usr/bin/firefox"
    /// - A Windows-style path: "C:\\Program Files\\..."
    ///
    /// The adapter is responsible for resolving the name to an executable.
    async fn open_application(
        &self,
        app: &str,
        options: Option<LaunchOptions>,
    ) -> Result<ProcessInfo>;

    /// Close an application by process ID or name.
    async fn close_application(&self, app: &str) -> Result<()>;

    /// Get a list of running processes.
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>>;

    /// Check if an application is running.
    async fn is_application_running(&self, app: &str) -> Result<bool>;

    // --------------------------------------------------------
    // Window Management
    // --------------------------------------------------------

    /// Get a list of all visible windows.
    async fn list_windows(&self) -> Result<Vec<WindowInfo>>;

    /// Get the currently active/focused foreground window.
    async fn get_active_window(&self) -> Result<WindowInfo> {
        let windows = self.list_windows().await?;
        windows
            .into_iter()
            .find(|w| w.focused)
            .ok_or_else(|| anyhow::anyhow!("No active foreground window found"))
    }

    /// Focus a specific window (bring to front).
    async fn focus_window(&self, window_handle: &str) -> Result<()>;

    /// Minimize a window.
    async fn minimize_window(&self, window_handle: &str) -> Result<()>;

    /// Maximize a window.
    async fn maximize_window(&self, window_handle: &str) -> Result<()>;

    /// Restore a window to normal windowed state.
    async fn restore_window(&self, window_handle: &str) -> Result<()> {
        let _ = window_handle;
        Err(anyhow::anyhow!(
            "Restore window not implemented on this platform"
        ))
    }

    /// Resize and/or move a window.
    async fn set_window_bounds(&self, window_handle: &str, bounds: Rect) -> Result<()>;

    // --------------------------------------------------------
    // Screen Capture
    // --------------------------------------------------------

    /// Capture the entire screen (primary display).
    async fn take_screenshot(&self) -> Result<Screenshot>;

    /// Capture a specific display.
    async fn take_screenshot_display(&self, display_index: u32) -> Result<Screenshot>;

    /// Capture a specific region of the screen.
    async fn take_screenshot_region(&self, region: Rect) -> Result<Screenshot>;

    // --------------------------------------------------------
    // Clipboard
    // --------------------------------------------------------

    /// Get the current clipboard content.
    async fn get_clipboard(&self) -> Result<ClipboardContent>;

    /// Set the clipboard content.
    async fn set_clipboard(&self, content: ClipboardContent) -> Result<()>;

    // --------------------------------------------------------
    // Notifications
    // --------------------------------------------------------

    /// Show a system notification.
    async fn show_notification(&self, notification: NotificationRequest) -> Result<()>;

    // --------------------------------------------------------
    // UI Automation & Accessibility (Phase 8, M08.04)
    // --------------------------------------------------------

    /// Inspect the accessibility tree of the currently active foreground window using native UI Automation.
    async fn inspect_ui_tree(
        &self,
        query: Option<&str>,
        max_depth: usize,
        max_elements: usize,
    ) -> Result<UiTreeResult> {
        let _ = (query, max_depth, max_elements);
        Ok(UiTreeResult::empty())
    }

    // --------------------------------------------------------
    // System State
    // --------------------------------------------------------

    /// Get available disk space.
    async fn get_disk_space(&self) -> Result<DiskInfo>;

    /// Get memory usage.
    async fn get_memory_info(&self) -> Result<MemoryInfo>;

    // --------------------------------------------------------
    // System Control & Power Management
    // --------------------------------------------------------

    /// Get current master volume level (0..100).
    async fn get_system_volume(&self) -> Result<u32> {
        Err(anyhow::anyhow!(
            "get_system_volume not implemented on this platform"
        ))
    }

    /// Set master volume level (0..100).
    async fn set_system_volume(&self, level: u32) -> Result<()> {
        let _ = level;
        Err(anyhow::anyhow!(
            "set_system_volume not implemented on this platform"
        ))
    }

    /// Mute or unmute system audio.
    async fn set_system_mute(&self, mute: bool) -> Result<()> {
        let _ = mute;
        Err(anyhow::anyhow!(
            "set_system_mute not implemented on this platform"
        ))
    }

    /// Lock workstation/session.
    async fn lock_workstation(&self) -> Result<()> {
        Err(anyhow::anyhow!(
            "lock_workstation not implemented on this platform"
        ))
    }

    /// Shutdown operating system.
    async fn shutdown_system(&self, force: bool) -> Result<()> {
        let _ = force;
        Err(anyhow::anyhow!(
            "shutdown_system not implemented on this platform"
        ))
    }

    /// Restart operating system.
    async fn restart_system(&self, force: bool) -> Result<()> {
        let _ = force;
        Err(anyhow::anyhow!(
            "restart_system not implemented on this platform"
        ))
    }

    /// Put system into sleep/suspend state.
    async fn sleep_system(&self) -> Result<()> {
        Err(anyhow::anyhow!(
            "sleep_system not implemented on this platform"
        ))
    }
}

// --------------------------------------------------------
// System Info Types
// --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
}

// ============================================================
// Platform Detection Helper
// ============================================================

/// Returns the current operating system.
pub fn current_os() -> OperatingSystem {
    #[cfg(target_os = "windows")]
    return OperatingSystem::Windows;

    #[cfg(target_os = "linux")]
    return OperatingSystem::Linux;

    #[cfg(target_os = "macos")]
    return OperatingSystem::MacOS;

    #[cfg(target_os = "android")]
    return OperatingSystem::Android;

    // Fallback
    #[allow(unreachable_code)]
    OperatingSystem::Linux
}

/// Returns the current CPU architecture.
pub fn current_arch() -> Architecture {
    #[cfg(target_arch = "x86_64")]
    return Architecture::X86_64;

    #[cfg(target_arch = "aarch64")]
    return Architecture::Aarch64;

    #[cfg(target_arch = "x86")]
    return Architecture::X86;

    #[allow(unreachable_code)]
    Architecture::X86_64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let os = current_os();
        // Should be Windows on Windows, Linux on Linux, etc.
        #[cfg(target_os = "windows")]
        assert_eq!(os, OperatingSystem::Windows);
        #[cfg(target_os = "linux")]
        assert_eq!(os, OperatingSystem::Linux);
    }

    #[test]
    fn test_arch_detection() {
        let arch = current_arch();
        // On most CI systems and dev machines: X86_64
        assert!(matches!(
            arch,
            Architecture::X86_64 | Architecture::Aarch64 | Architecture::X86
        ));
    }

    #[test]
    fn test_notification_priority_default() {
        let priority = NotificationPriority::default();
        assert!(matches!(priority, NotificationPriority::Normal));
    }

    #[test]
    fn test_ui_element_construction_and_center_calculation() {
        let elem = UiElement::new(
            "Soft Reset",
            "btn_soft_reset",
            "Button",
            "ButtonClass",
            Rect {
                x: 100,
                y: 200,
                width: 80,
                height: 40,
            },
            true,
            false,
            false,
        );

        assert_eq!(elem.name, "Soft Reset");
        assert_eq!(elem.automation_id, "btn_soft_reset");
        assert_eq!(elem.control_type, "Button");
        assert_eq!(elem.center_x, 140);
        assert_eq!(elem.center_y, 220);
        assert!(elem.enabled);
        assert!(!elem.offscreen);
        assert!(!elem.focused);
    }

    #[test]
    fn test_ui_element_query_matching() {
        let elem = UiElement::new(
            "Soft Reset",
            "btn_soft_reset",
            "Button",
            "ButtonClass",
            Rect {
                x: 100,
                y: 200,
                width: 80,
                height: 40,
            },
            true,
            false,
            false,
        );

        // Exact name match
        assert!(elem.matches_query("Soft Reset"));
        // Case-insensitive match
        assert!(elem.matches_query("soft reset"));
        // Substring match
        assert!(elem.matches_query("Reset"));
        // Control type match
        assert!(elem.matches_query("button"));
        // Automation ID match
        assert!(elem.matches_query("btn_soft"));
        // Empty query matches all
        assert!(elem.matches_query(""));
        // Non-matching query
        assert!(!elem.matches_query("Hard Reboot"));
    }
}
