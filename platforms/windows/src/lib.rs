//! JARVIS Windows Platform Adapter
//!
//! Implements the `PlatformAdapter` trait for Windows 10/11.
//!
//! Uses native Windows API (user32.dll / kernel32.dll) and PowerShell fallbacks for:
//! - Application launching (CreateProcess / ShellExecute)
//! - Window management (EnumWindows, GetForegroundWindow, SetForegroundWindow, ShowWindow, SetWindowPos)
//! - Process management (CreateToolhelp32Snapshot)
//! - Screen capture (GDI+ / PowerShell)
//! - Clipboard access (OpenClipboard / GetClipboardData)
//! - Notifications (Windows.UI.Notifications)
//!
//! IMPLEMENTATION STATUS: Phase 6, Milestone M06.03 — Desktop Window Management & Active Window Focus

pub mod uia;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{info, instrument, warn};

use jarvis_platform::{
    ClipboardContent, DiskInfo, ImageFormat, LaunchOptions, MemoryInfo, NotificationRequest,
    OperatingSystem, PlatformAdapter, PlatformInfo, ProcessInfo, Rect, Screenshot, WindowInfo,
};

// ============================================================
// Native Win32 FFI Bindings (user32.dll / kernel32.dll)
// ============================================================

#[cfg(target_os = "windows")]
mod sys {
    use std::ffi::c_void;

    pub type HWND = *mut c_void;
    pub type BOOL = i32;
    pub type DWORD = u32;
    pub type LPARAM = isize;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn EnumWindows(
            lpEnumFunc: unsafe extern "system" fn(HWND, LPARAM) -> BOOL,
            lParam: LPARAM,
        ) -> BOOL;
        pub fn GetForegroundWindow() -> HWND;
        pub fn IsWindowVisible(hWnd: HWND) -> BOOL;
        pub fn IsIconic(hWnd: HWND) -> BOOL;
        pub fn IsZoomed(hWnd: HWND) -> BOOL;
        pub fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
        pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
        pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        pub fn SetWindowPos(
            hWnd: HWND,
            hWndInsertAfter: HWND,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> BOOL;
        pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
        pub fn LockWorkStation() -> BOOL;
        pub fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
        pub fn CloseClipboard() -> BOOL;
        pub fn EmptyClipboard() -> BOOL;
        pub fn GetClipboardData(uFormat: u32) -> *mut c_void;
        pub fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
        pub fn IsClipboardFormatAvailable(format: u32) -> BOOL;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
        pub fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
        pub fn GlobalUnlock(hMem: *mut c_void) -> BOOL;
        pub fn GlobalFree(hMem: *mut c_void) -> *mut c_void;
    }

    pub const CF_UNICODETEXT: u32 = 13;
    pub const GMEM_MOVEABLE: u32 = 0x0002;
    pub const SW_RESTORE: i32 = 9;
    pub const SW_MINIMIZE: i32 = 6;
    pub const SW_MAXIMIZE: i32 = 3;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
}

// Global window collection buffer for EnumWindows callback
#[cfg(target_os = "windows")]
#[allow(clippy::type_complexity)]
static ENUM_WINDOWS_BUFFER: Mutex<Vec<(usize, u32, String, bool, bool, Option<Rect>)>> =
    Mutex::new(Vec::new());

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_windows_callback(hwnd: sys::HWND, _: sys::LPARAM) -> sys::BOOL {
    if sys::IsWindowVisible(hwnd) == 0 {
        return 1;
    }

    let mut buf = [0u16; 512];
    let len = sys::GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
    if len <= 0 {
        return 1;
    }

    let title = String::from_utf16_lossy(&buf[..len as usize]);
    let title_trimmed = title.trim();
    if title_trimmed.is_empty()
        || title_trimmed == "Program Manager"
        || title_trimmed == "Default IME"
        || title_trimmed == "MSCTFIME UI"
    {
        return 1;
    }

    let mut pid: sys::DWORD = 0;
    sys::GetWindowThreadProcessId(hwnd, &mut pid);

    let mut rect = sys::RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let bounds = if sys::GetWindowRect(hwnd, &mut rect) != 0 {
        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        if width < 10 || height < 10 {
            return 1; // Filter out 0-size invisible window tooltips/shadows
        }
        Some(Rect {
            x: rect.left,
            y: rect.top,
            width,
            height,
        })
    } else {
        None
    };

    let is_minimized = sys::IsIconic(hwnd) != 0;
    let is_maximized = sys::IsZoomed(hwnd) != 0;

    if let Ok(mut list) = ENUM_WINDOWS_BUFFER.lock() {
        list.push((
            hwnd as usize,
            pid,
            title_trimmed.to_string(),
            is_minimized,
            is_maximized,
            bounds,
        ));
    }

    1 // Continue enumeration
}

// ============================================================
// Windows Platform Adapter
// ============================================================

/// Implementation of `PlatformAdapter` for Windows 10/11.
pub struct WindowsPlatformAdapter {
    /// Known application aliases to executable names
    app_aliases: HashMap<String, String>,
}

impl WindowsPlatformAdapter {
    /// Create a new Windows platform adapter with default app aliases.
    pub fn new() -> Self {
        let mut aliases = HashMap::new();

        // Browser aliases
        aliases.insert("chrome".to_string(), "chrome.exe".to_string());
        aliases.insert("google chrome".to_string(), "chrome.exe".to_string());
        aliases.insert("firefox".to_string(), "firefox.exe".to_string());
        aliases.insert("edge".to_string(), "msedge.exe".to_string());
        aliases.insert("microsoft edge".to_string(), "msedge.exe".to_string());
        aliases.insert("brave".to_string(), "brave.exe".to_string());

        // Editor aliases
        aliases.insert("vscode".to_string(), "Code.exe".to_string());
        aliases.insert("vs code".to_string(), "Code.exe".to_string());
        aliases.insert("notepad".to_string(), "notepad.exe".to_string());
        aliases.insert("notepad++".to_string(), "notepad++.exe".to_string());

        // Terminal aliases
        aliases.insert("terminal".to_string(), "wt.exe".to_string());
        aliases.insert("windows terminal".to_string(), "wt.exe".to_string());
        aliases.insert("cmd".to_string(), "cmd.exe".to_string());
        aliases.insert("powershell".to_string(), "powershell.exe".to_string());
        aliases.insert("pwsh".to_string(), "pwsh.exe".to_string());

        // System apps
        aliases.insert("explorer".to_string(), "explorer.exe".to_string());
        aliases.insert("file explorer".to_string(), "explorer.exe".to_string());
        aliases.insert("task manager".to_string(), "taskmgr.exe".to_string());
        aliases.insert("calculator".to_string(), "calc.exe".to_string());

        // Communication & Media
        aliases.insert("discord".to_string(), "Discord.exe".to_string());
        aliases.insert("slack".to_string(), "slack.exe".to_string());
        aliases.insert("teams".to_string(), "Teams.exe".to_string());
        aliases.insert("zoom".to_string(), "Zoom.exe".to_string());
        aliases.insert("spotify".to_string(), "Spotify.exe".to_string());

        Self {
            app_aliases: aliases,
        }
    }

    /// Resolve an app name/alias to an executable path.
    fn resolve_app(&self, app: &str) -> String {
        let clean = app.trim().trim_matches(|c| {
            c == '.' || c == ',' || c == '!' || c == '?' || c == '"' || c == '\''
        });
        let normalized = clean.to_lowercase();
        let target_name = self
            .app_aliases
            .get(&normalized)
            .cloned()
            .unwrap_or_else(|| {
                if clean.ends_with(".exe") || clean.contains('/') || clean.contains('\\') {
                    clean.to_string()
                } else {
                    format!("{}.exe", clean)
                }
            });

        target_name
    }

    /// Helper to resolve a handle string or application/title name to a target WindowInfo.
    async fn resolve_window_info(&self, handle_or_name: &str) -> Result<WindowInfo> {
        let windows = self.list_windows().await?;

        // 1. Direct handle match (e.g. "0x10204" or numeric string)
        if let Some(w) = windows
            .iter()
            .find(|w| w.handle.eq_ignore_ascii_case(handle_or_name))
        {
            return Ok(w.clone());
        }

        let target = handle_or_name.trim().to_lowercase();
        let clean_target = target.trim_end_matches(".exe");

        // 2. Match process_name (e.g. "chrome", "spotify", "code")
        if let Some(w) = windows.iter().find(|w| {
            let p_name = w.process_name.to_lowercase();
            let p_clean = p_name.trim_end_matches(".exe");
            p_name == target || p_clean == clean_target || p_name.contains(clean_target)
        }) {
            return Ok(w.clone());
        }

        // 3. Match window title (e.g. "Google Chrome", "Spotify Free", "VS Code")
        if let Some(w) = windows
            .iter()
            .find(|w| w.title.to_lowercase().contains(&target))
        {
            return Ok(w.clone());
        }

        Err(anyhow!(
            "No open window found matching handle or application name '{}'",
            handle_or_name
        ))
    }

    /// Parse HWND pointer from string handle ("0x10204" or decimal).
    fn parse_hwnd(handle_str: &str) -> Result<usize> {
        if handle_str.starts_with("0x") || handle_str.starts_with("0X") {
            usize::from_str_radix(&handle_str[2..], 16)
                .map_err(|_| anyhow!("Invalid window handle hex string: '{}'", handle_str))
        } else {
            handle_str
                .parse::<usize>()
                .map_err(|_| anyhow!("Invalid window handle decimal string: '{}'", handle_str))
        }
    }
}

impl Default for WindowsPlatformAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for WindowsPlatformAdapter {
    async fn get_platform_info(&self) -> Result<PlatformInfo> {
        let hostname_str = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let username_str = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string());
        let home_dir = std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\default"));
        let temp_dir = std::env::var("TEMP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Windows\\Temp"));

        Ok(PlatformInfo {
            os: OperatingSystem::Windows,
            os_version: "Windows 11".to_string(),
            arch: jarvis_platform::Architecture::X86_64,
            hostname: hostname_str,
            username: username_str,
            home_dir,
            temp_dir,
        })
    }

    #[instrument(skip(self), fields(app = %app))]
    async fn open_application(
        &self,
        app: &str,
        options: Option<LaunchOptions>,
    ) -> Result<ProcessInfo> {
        let target_name = self.resolve_app(app);
        info!(
            raw_app = %app,
            resolved_app = %target_name,
            "Launching application on Windows"
        );

        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(["/C", "start", "", &target_name]);

        if let Some(opts) = options {
            if let Some(dir) = opts.working_dir {
                cmd.current_dir(dir);
            }
            for arg in opts.args {
                cmd.arg(arg);
            }
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| anyhow!("Failed to launch application '{}': {}", target_name, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                app = %target_name,
                stderr = %stderr,
                "cmd start returned non-zero, trying direct executable launch"
            );

            tokio::process::Command::new(&target_name)
                .spawn()
                .map_err(|e| anyhow!("Failed to launch '{}' directly: {}", target_name, e))?;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let running_processes = self.list_processes().await.unwrap_or_default();
        let target_lower = target_name.to_lowercase();

        let found = running_processes.into_iter().find(|p| {
            let p_lower = p.name.to_lowercase();
            p_lower == target_lower || p_lower == format!("{}.exe", target_lower)
        });

        if let Some(proc_info) = found {
            info!(app = %target_name, pid = proc_info.pid, "Application confirmed running");
            Ok(proc_info)
        } else {
            Ok(ProcessInfo {
                pid: 0,
                name: target_name,
                executable_path: None,
                command_line: None,
                running: true,
            })
        }
    }

    async fn close_application(&self, app: &str) -> Result<()> {
        let target_name = self.resolve_app(app);
        info!(app = %target_name, "Closing application");

        let output = tokio::process::Command::new("taskkill")
            .args(["/IM", &target_name, "/F"])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run taskkill for '{}': {}", target_name, e))?;

        if output.status.success() {
            info!(app = %target_name, "Application terminated successfully");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!(
                "Failed to close application '{}': {}",
                target_name,
                stderr
            ))
        }
    }

    async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let output = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-Process | Select-Object Id, ProcessName | ConvertTo-Json",
            ])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to list processes via PowerShell: {}", e))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Array(Vec::new()));

        let items = match &json {
            serde_json::Value::Array(arr) => arr.clone(),
            obj @ serde_json::Value::Object(_) => vec![obj.clone()],
            _ => Vec::new(),
        };

        let processes = items
            .iter()
            .filter_map(|item| {
                let pid = item["Id"].as_u64()? as u32;
                let name = item["ProcessName"].as_str()?.to_string();
                Some(ProcessInfo {
                    pid,
                    name,
                    executable_path: None,
                    command_line: None,
                    running: true,
                })
            })
            .collect();

        Ok(processes)
    }

    async fn is_application_running(&self, app: &str) -> Result<bool> {
        let target_name = self.resolve_app(app);
        let executable = target_name
            .strip_suffix(".exe")
            .unwrap_or(&target_name)
            .to_string();

        let processes = self.list_processes().await?;
        let running = processes
            .iter()
            .any(|p| p.name.to_lowercase() == executable.to_lowercase());
        Ok(running)
    }

    // ============================================================
    // Window Management Implementation (Step 3 to Step 10)
    // ============================================================

    async fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        #[cfg(target_os = "windows")]
        {
            let mut process_map = HashMap::new();
            if let Ok(procs) = self.list_processes().await {
                for p in procs {
                    let exe_name = if p.name.ends_with(".exe") {
                        p.name.clone()
                    } else {
                        format!("{}.exe", p.name)
                    };
                    process_map.insert(p.pid, exe_name);
                }
            }

            if let Ok(mut list) = ENUM_WINDOWS_BUFFER.lock() {
                list.clear();
            }

            unsafe {
                sys::EnumWindows(enum_windows_callback, 0);
            }

            let foreground_hwnd = unsafe { sys::GetForegroundWindow() } as usize;

            let buffer = ENUM_WINDOWS_BUFFER
                .lock()
                .map(|l| l.clone())
                .unwrap_or_default();

            let windows = buffer
                .into_iter()
                .map(
                    |(hwnd_val, pid, title, is_minimized, is_maximized, bounds)| {
                        let process_name = process_map
                            .get(&pid)
                            .cloned()
                            .unwrap_or_else(|| "unknown.exe".to_string());

                        let handle_str = format!("0x{:x}", hwnd_val);
                        let focused = hwnd_val == foreground_hwnd;

                        WindowInfo {
                            handle: handle_str,
                            title,
                            process_name,
                            pid,
                            visible: true,
                            focused,
                            bounds,
                            is_minimized,
                            is_maximized,
                        }
                    },
                )
                .collect();

            Ok(windows)
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(Vec::new())
        }
    }

    async fn get_active_window(&self) -> Result<WindowInfo> {
        let windows = self.list_windows().await?;
        if let Some(focused) = windows.into_iter().find(|w| w.focused) {
            return Ok(focused);
        }

        #[cfg(target_os = "windows")]
        {
            let foreground_hwnd = unsafe { sys::GetForegroundWindow() };
            if !foreground_hwnd.is_null() {
                let mut pid: sys::DWORD = 0;
                unsafe { sys::GetWindowThreadProcessId(foreground_hwnd, &mut pid) };

                let handle_str = format!("0x{:x}", foreground_hwnd as usize);
                return Ok(WindowInfo {
                    handle: handle_str,
                    title: "Active Window".to_string(),
                    process_name: "unknown.exe".to_string(),
                    pid,
                    visible: true,
                    focused: true,
                    bounds: None,
                    is_minimized: false,
                    is_maximized: false,
                });
            }
        }

        Err(anyhow!("No active foreground window found"))
    }

    async fn focus_window(&self, window_handle: &str) -> Result<()> {
        let target = self.resolve_window_info(window_handle).await?;

        #[cfg(target_os = "windows")]
        {
            let hwnd_val = Self::parse_hwnd(&target.handle)?;
            let hwnd = hwnd_val as sys::HWND;

            unsafe {
                if sys::IsIconic(hwnd) != 0 {
                    sys::ShowWindow(hwnd, sys::SW_RESTORE);
                }
                sys::SetForegroundWindow(hwnd);
            }

            info!(
                handle = %target.handle,
                title = %target.title,
                process = %target.process_name,
                "Window focused successfully"
            );
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            Ok(())
        }
    }

    async fn minimize_window(&self, window_handle: &str) -> Result<()> {
        let target = self.resolve_window_info(window_handle).await?;

        #[cfg(target_os = "windows")]
        {
            let hwnd_val = Self::parse_hwnd(&target.handle)?;
            let hwnd = hwnd_val as sys::HWND;

            unsafe {
                sys::ShowWindow(hwnd, sys::SW_MINIMIZE);
            }

            info!(
                handle = %target.handle,
                title = %target.title,
                process = %target.process_name,
                "Window minimized successfully"
            );
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            Ok(())
        }
    }

    async fn maximize_window(&self, window_handle: &str) -> Result<()> {
        let target = self.resolve_window_info(window_handle).await?;

        #[cfg(target_os = "windows")]
        {
            let hwnd_val = Self::parse_hwnd(&target.handle)?;
            let hwnd = hwnd_val as sys::HWND;

            unsafe {
                sys::ShowWindow(hwnd, sys::SW_MAXIMIZE);
            }

            info!(
                handle = %target.handle,
                title = %target.title,
                process = %target.process_name,
                "Window maximized successfully"
            );
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            Ok(())
        }
    }

    async fn restore_window(&self, window_handle: &str) -> Result<()> {
        let target = self.resolve_window_info(window_handle).await?;

        #[cfg(target_os = "windows")]
        {
            let hwnd_val = Self::parse_hwnd(&target.handle)?;
            let hwnd = hwnd_val as sys::HWND;

            unsafe {
                sys::ShowWindow(hwnd, sys::SW_RESTORE);
            }

            info!(
                handle = %target.handle,
                title = %target.title,
                process = %target.process_name,
                "Window restored successfully"
            );
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            Ok(())
        }
    }

    async fn set_window_bounds(&self, window_handle: &str, bounds: Rect) -> Result<()> {
        if bounds.width == 0 || bounds.height == 0 {
            return Err(anyhow!(
                "Invalid window dimensions: width ({}) and height ({}) must be greater than zero",
                bounds.width,
                bounds.height
            ));
        }

        let target = self.resolve_window_info(window_handle).await?;

        #[cfg(target_os = "windows")]
        {
            let hwnd_val = Self::parse_hwnd(&target.handle)?;
            let hwnd = hwnd_val as sys::HWND;

            unsafe {
                if sys::IsIconic(hwnd) != 0 || sys::IsZoomed(hwnd) != 0 {
                    sys::ShowWindow(hwnd, sys::SW_RESTORE);
                }

                let ret = sys::SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    bounds.x,
                    bounds.y,
                    bounds.width as i32,
                    bounds.height as i32,
                    sys::SWP_NOZORDER | sys::SWP_NOACTIVATE,
                );

                if ret == 0 {
                    return Err(anyhow!(
                        "Failed to set bounds for window '{}'",
                        target.title
                    ));
                }
            }

            info!(
                handle = %target.handle,
                title = %target.title,
                x = bounds.x,
                y = bounds.y,
                width = bounds.width,
                height = bounds.height,
                "Window bounds updated successfully"
            );
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            let _ = bounds;
            Ok(())
        }
    }

    #[instrument(skip(self))]
    async fn take_screenshot(&self) -> Result<Screenshot> {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

$ms = New-Object System.IO.MemoryStream
$bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$bytes = $ms.ToArray()
$graphics.Dispose()
$bitmap.Dispose()
$ms.Dispose()
$b64 = [Convert]::ToBase64String($bytes)
"$($bounds.Width),$($bounds.Height)|$b64"
"#;

        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await
            .map_err(|e| anyhow!("Screenshot failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Screenshot PowerShell error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let (dim_part, b64) = stdout.split_once('|').unwrap_or(("0,0", &stdout));
        let (w_str, h_str) = dim_part.split_once(',').unwrap_or(("0", "0"));
        let width: u32 = w_str.parse().unwrap_or(0);
        let height: u32 = h_str.parse().unwrap_or(0);

        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
            .map_err(|e| anyhow!("Failed to decode screenshot: {}", e))?;

        info!(bytes = %data.len(), width = width, height = height, "Screenshot captured");

        Ok(Screenshot {
            data,
            format: ImageFormat::Png,
            width,
            height,
            display_index: 0,
        })
    }

    async fn take_screenshot_display(&self, display_index: u32) -> Result<Screenshot> {
        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screens = [System.Windows.Forms.Screen]::AllScreens
$idx = [Math]::Min({idx}, $screens.Count - 1)
$screen = $screens[$idx]
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

$ms = New-Object System.IO.MemoryStream
$bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$bytes = $ms.ToArray()
$graphics.Dispose()
$bitmap.Dispose()
$ms.Dispose()
$b64 = [Convert]::ToBase64String($bytes)
"$($bounds.Width),$($bounds.Height)|$b64"
"#,
            idx = display_index
        );

        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .await
            .map_err(|e| anyhow!("Display screenshot failed: {}", e))?;

        if !output.status.success() {
            return self.take_screenshot().await;
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let (dim_part, b64) = stdout.split_once('|').unwrap_or(("0,0", &stdout));
        let (w_str, h_str) = dim_part.split_once(',').unwrap_or(("0", "0"));
        let width: u32 = w_str.parse().unwrap_or(0);
        let height: u32 = h_str.parse().unwrap_or(0);

        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
            .map_err(|e| anyhow!("Failed to decode display screenshot: {}", e))?;

        Ok(Screenshot {
            data,
            format: ImageFormat::Png,
            width,
            height,
            display_index,
        })
    }

    async fn take_screenshot_region(&self, region: Rect) -> Result<Screenshot> {
        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$bitmap = New-Object System.Drawing.Bitmap({width}, {height})
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen({x}, {y}, 0, 0, [System.Drawing.Size]::new({width}, {height}))
$ms = New-Object System.IO.MemoryStream
$bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$bytes = $ms.ToArray()
$graphics.Dispose()
$bitmap.Dispose()
$ms.Dispose()
[Convert]::ToBase64String($bytes)
"#,
            x = region.x,
            y = region.y,
            width = region.width,
            height = region.height
        );

        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .await?;

        let b64 = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
            .map_err(|e| anyhow!("Failed to decode screenshot region: {}", e))?;

        Ok(Screenshot {
            data,
            format: ImageFormat::Png,
            width: region.width,
            height: region.height,
            display_index: 0,
        })
    }

    async fn get_clipboard(&self) -> Result<ClipboardContent> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                if sys::OpenClipboard(std::ptr::null_mut()) != 0 {
                    struct ClipboardGuard;
                    impl Drop for ClipboardGuard {
                        fn drop(&mut self) {
                            unsafe {
                                sys::CloseClipboard();
                            }
                        }
                    }
                    let _guard = ClipboardGuard;

                    if sys::IsClipboardFormatAvailable(sys::CF_UNICODETEXT) != 0 {
                        let handle = sys::GetClipboardData(sys::CF_UNICODETEXT);
                        if !handle.is_null() {
                            let ptr = sys::GlobalLock(handle) as *const u16;
                            if !ptr.is_null() {
                                let mut len = 0;
                                while *ptr.add(len) != 0 {
                                    len += 1;
                                }
                                let slice = std::slice::from_raw_parts(ptr, len);
                                let text = String::from_utf16_lossy(slice);
                                sys::GlobalUnlock(handle);

                                if text.is_empty() {
                                    return Ok(ClipboardContent::Empty);
                                } else {
                                    return Ok(ClipboardContent::Text(text));
                                }
                            }
                        }
                    } else {
                        return Ok(ClipboardContent::Empty);
                    }
                }
            }
        }

        let script = "Get-Clipboard";
        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.is_empty() {
                Ok(ClipboardContent::Empty)
            } else {
                Ok(ClipboardContent::Text(text))
            }
        } else {
            Ok(ClipboardContent::Empty)
        }
    }

    async fn set_clipboard(&self, content: ClipboardContent) -> Result<()> {
        let text = match content {
            ClipboardContent::Text(t) => t,
            ClipboardContent::Empty => String::new(),
            _ => return Err(anyhow!("Clipboard format not supported on Windows yet")),
        };

        #[cfg(target_os = "windows")]
        {
            unsafe {
                let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes_len = utf16.len() * std::mem::size_of::<u16>();

                let h_mem = sys::GlobalAlloc(sys::GMEM_MOVEABLE, bytes_len);
                if !h_mem.is_null() {
                    let ptr = sys::GlobalLock(h_mem) as *mut u16;
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
                        sys::GlobalUnlock(h_mem);

                        if sys::OpenClipboard(std::ptr::null_mut()) != 0 {
                            sys::EmptyClipboard();
                            let res = sys::SetClipboardData(sys::CF_UNICODETEXT, h_mem);
                            sys::CloseClipboard();
                            if !res.is_null() {
                                return Ok(());
                            }
                        }
                    }
                    sys::GlobalFree(h_mem);
                }
            }
        }

        let script = format!("Set-Clipboard -Value \"{}\"", text.replace('"', "`\""));
        tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .await?;
        Ok(())
    }

    async fn show_notification(&self, notification: NotificationRequest) -> Result<()> {
        let escape_ps =
            |s: &str| -> String { s.replace('`', "``").replace('"', "`\"").replace('$', "`$") };

        let title = escape_ps(&notification.title);
        let body = escape_ps(&notification.body);
        let timeout_ms = notification.timeout_secs.unwrap_or(5) * 1000;

        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$notification = New-Object System.Windows.Forms.NotifyIcon
$notification.Icon = [System.Drawing.SystemIcons]::Information
$notification.BalloonTipTitle = "{title}"
$notification.BalloonTipText = "{body}"
$notification.Visible = $true
$notification.ShowBalloonTip({timeout_ms})
Start-Sleep -Milliseconds 1500
$notification.Dispose()
"#,
            title = title,
            body = body,
            timeout_ms = timeout_ms
        );

        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to execute notification script: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to show notification: {}", err.trim()));
        }

        Ok(())
    }

    async fn get_disk_space(&self) -> Result<DiskInfo> {
        let script = "Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='C:'\" | Select-Object Size, FreeSpace | ConvertTo-Json";
        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: serde_json::Value = serde_json::from_str(&stdout)?;
            let total = json["Size"].as_u64().unwrap_or(0);
            let free = json["FreeSpace"].as_u64().unwrap_or(0);
            Ok(DiskInfo {
                total_bytes: total,
                available_bytes: free,
                used_bytes: total.saturating_sub(free),
            })
        } else {
            Err(anyhow!("Failed to query disk space"))
        }
    }

    async fn get_memory_info(&self) -> Result<MemoryInfo> {
        let script = "Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize, FreePhysicalMemory | ConvertTo-Json";
        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: serde_json::Value = serde_json::from_str(&stdout)?;
            let total_kb = json["TotalVisibleMemorySize"].as_u64().unwrap_or(0);
            let free_kb = json["FreePhysicalMemory"].as_u64().unwrap_or(0);
            let total = total_kb * 1024;
            let free = free_kb * 1024;
            Ok(MemoryInfo {
                total_bytes: total,
                available_bytes: free,
                used_bytes: total.saturating_sub(free),
            })
        } else {
            Err(anyhow!("Failed to query memory info"))
        }
    }

    async fn lock_workstation(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            unsafe { sys::LockWorkStation() };
            info!("Workstation locked via LockWorkStation Win32 API");
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(())
        }
    }

    async fn set_system_volume(&self, level: u32) -> Result<()> {
        let target_vol = level.min(100);
        let script = format!(
            r#"
            $wsh = New-Object -ComObject WScript.Shell
            1..50 | ForEach-Object {{ $wsh.SendKeys([char]174) }}
            $steps = [math]::Round({target_vol} / 2)
            if ($steps -gt 0) {{
                1..$steps | ForEach-Object {{ $wsh.SendKeys([char]175) }}
            }}
            "#
        );

        tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to set system volume: {}", e))?;

        info!(level = target_vol, "System volume updated");
        Ok(())
    }

    async fn set_system_mute(&self, _mute: bool) -> Result<()> {
        let script = r#"(New-Object -ComObject WScript.Shell).SendKeys([char]173)"#;
        tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to toggle system mute: {}", e))?;

        info!("System mute toggled");
        Ok(())
    }

    async fn shutdown_system(&self, force: bool) -> Result<()> {
        let args = if force {
            vec!["/s", "/f", "/t", "0"]
        } else {
            vec!["/s", "/t", "0"]
        };
        tokio::process::Command::new("shutdown")
            .args(&args)
            .output()
            .await
            .map_err(|e| anyhow!("Failed to initiate system shutdown: {}", e))?;
        Ok(())
    }

    async fn restart_system(&self, force: bool) -> Result<()> {
        let args = if force {
            vec!["/r", "/f", "/t", "0"]
        } else {
            vec!["/r", "/t", "0"]
        };
        tokio::process::Command::new("shutdown")
            .args(&args)
            .output()
            .await
            .map_err(|e| anyhow!("Failed to initiate system restart: {}", e))?;
        Ok(())
    }

    async fn sleep_system(&self) -> Result<()> {
        let script = "Add-Type -Assembly System.Windows.Forms; [System.Windows.Forms.Application]::SetSuspendState([System.Windows.Forms.PowerState]::Suspend, $false, $false)";
        tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to initiate system sleep: {}", e))?;
        Ok(())
    }

    async fn inspect_ui_tree(
        &self,
        query: Option<&str>,
        max_depth: usize,
        max_elements: usize,
    ) -> Result<jarvis_platform::UiTreeResult> {
        let query_owned = query.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            uia::inspect_active_window_uia(query_owned.as_deref(), max_depth, max_elements)
        })
        .await
        .map_err(|e| anyhow!("UIA inspection task join error: {}", e))?
    }
}

// ============================================================
// Unit & Integration Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_window_info_serialization() {
        let win = WindowInfo {
            handle: "0x10204".to_string(),
            title: "Google Chrome".to_string(),
            process_name: "chrome.exe".to_string(),
            pid: 1234,
            visible: true,
            focused: true,
            bounds: Some(Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            }),
            is_minimized: false,
            is_maximized: false,
        };

        let json = serde_json::to_string(&win).unwrap();
        let deserialized: WindowInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.handle, "0x10204");
        assert_eq!(deserialized.title, "Google Chrome");
        assert!(deserialized.focused);
    }

    #[tokio::test]
    async fn test_invalid_resize_dimensions_rejected() {
        let adapter = WindowsPlatformAdapter::new();
        let err = adapter
            .set_window_bounds(
                "0x10204",
                Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 100,
                },
            )
            .await;
        assert!(err.is_err());
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("Invalid window dimensions"));
    }

    #[test]
    fn test_parse_hwnd_valid_and_invalid() {
        assert_eq!(
            WindowsPlatformAdapter::parse_hwnd("0x10204").unwrap(),
            0x10204
        );
        assert_eq!(WindowsPlatformAdapter::parse_hwnd("66052").unwrap(), 66052);
        assert!(WindowsPlatformAdapter::parse_hwnd("invalid_hwnd").is_err());
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_window_fails() {
        let adapter = WindowsPlatformAdapter::new();
        let err = adapter
            .resolve_window_info("non_existent_app_xyz_999")
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_windows_window_enumeration_and_active_window() {
        let adapter = WindowsPlatformAdapter::new();
        let _windows = adapter.list_windows().await.unwrap();
        if let Ok(active) = adapter.get_active_window().await {
            assert!(active.visible);
        }
    }
}
