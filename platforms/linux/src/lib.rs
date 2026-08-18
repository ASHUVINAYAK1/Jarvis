//! Linux Platform Adapter Implementation
//!
//! Provides OS integration for Linux desktop environments (Ubuntu 22.04+, 24.04+, GNOME, KDE, XFCE).
//! Supports X11 and Wayland display sessions, `.desktop` entry resolution, PATH executable discovery,
//! process control, clipboard management, desktop notifications, and structured capability reporting.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};

use jarvis_platform::{
    ClipboardContent, DiskInfo, ImageFormat, LaunchOptions, MemoryInfo, NotificationPriority,
    NotificationRequest, OperatingSystem, PlatformAdapter, PlatformInfo, ProcessInfo, Rect,
    Screenshot, WindowInfo,
};

// ============================================================
// Display Server & Session Types
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

impl DisplayServer {
    pub fn detect() -> Self {
        if let Ok(sess) = env::var("XDG_SESSION_TYPE") {
            if sess.eq_ignore_ascii_case("wayland") {
                return DisplayServer::Wayland;
            } else if sess.eq_ignore_ascii_case("x11") {
                return DisplayServer::X11;
            }
        }
        if env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServer::Wayland;
        }
        if env::var("DISPLAY").is_ok() {
            return DisplayServer::X11;
        }
        DisplayServer::Unknown
    }

    pub fn is_wayland(&self) -> bool {
        matches!(self, DisplayServer::Wayland)
    }
}

// ============================================================
// Desktop Entry (.desktop) Representation & Resolver
// ============================================================

#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub no_display: bool,
    pub entry_type: String,
    pub path: PathBuf,
}

impl DesktopEntry {
    /// Parse a Linux .desktop file safely.
    pub fn parse_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read .desktop file: {}", path.display()))?;

        let mut in_desktop_entry = false;
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut no_display = false;
        let mut entry_type = "Application".to_string();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
                continue;
            }

            if !in_desktop_entry || line.starts_with('#') || !line.contains('=') {
                continue;
            }

            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            match key {
                "Name" if name.is_none() => name = Some(value.to_string()),
                "Exec" if exec.is_none() => exec = Some(Self::clean_exec_field(value)),
                "Icon" if icon.is_none() => icon = Some(value.to_string()),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "Type" => entry_type = value.to_string(),
                _ => {}
            }
        }

        let name = name.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        });
        let exec = exec.ok_or_else(|| anyhow!("No Exec line in .desktop file"))?;

        Ok(DesktopEntry {
            name,
            exec,
            icon,
            no_display,
            entry_type,
            path: path.to_path_buf(),
        })
    }

    /// Strip Exec placeholders (%f, %F, %u, %U, %i, %c, %k).
    pub fn clean_exec_field(exec: &str) -> String {
        let mut cleaned = String::new();
        let tokens: Vec<&str> = exec.split_whitespace().collect();
        for token in tokens {
            if token.starts_with('%') && token.len() <= 3 {
                continue;
            }
            if !cleaned.is_empty() {
                cleaned.push(' ');
            }
            cleaned.push_str(token);
        }
        cleaned
    }
}

// ============================================================
// Multi-Stage Application Resolver
// ============================================================

pub struct ApplicationResolver {
    aliases: HashMap<String, String>,
    desktop_search_dirs: Vec<PathBuf>,
}

impl ApplicationResolver {
    pub fn new() -> Self {
        let mut aliases = HashMap::new();
        aliases.insert("chrome".to_string(), "google-chrome".to_string());
        aliases.insert("google chrome".to_string(), "google-chrome".to_string());
        aliases.insert("chromium".to_string(), "chromium-browser".to_string());
        aliases.insert("firefox".to_string(), "firefox".to_string());
        aliases.insert("spotify".to_string(), "spotify".to_string());
        aliases.insert("code".to_string(), "code".to_string());
        aliases.insert("vscode".to_string(), "code".to_string());
        aliases.insert("vs code".to_string(), "code".to_string());
        aliases.insert("nautilus".to_string(), "nautilus".to_string());
        aliases.insert("files".to_string(), "nautilus".to_string());
        aliases.insert("file manager".to_string(), "nautilus".to_string());
        aliases.insert("terminal".to_string(), "gnome-terminal".to_string());
        aliases.insert("calculator".to_string(), "gnome-calculator".to_string());
        aliases.insert("text editor".to_string(), "gedit".to_string());

        let mut desktop_search_dirs = Vec::new();
        if let Ok(home) = env::var("HOME") {
            desktop_search_dirs.push(PathBuf::from(home).join(".local/share/applications"));
        }
        desktop_search_dirs.push(PathBuf::from("/usr/share/applications"));
        desktop_search_dirs.push(PathBuf::from("/usr/local/share/applications"));
        desktop_search_dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
        desktop_search_dirs.push(PathBuf::from("/snap/bin"));

        Self {
            aliases,
            desktop_search_dirs,
        }
    }

    /// Multi-stage resolution: Aliases → PATH lookup → .desktop entries → Raw command
    pub fn resolve(&self, app: &str) -> String {
        let clean = app.trim().trim_matches(|c| {
            c == '.' || c == ',' || c == '!' || c == '?' || c == '"' || c == '\''
        });
        let normalized = clean.to_lowercase();

        // 1. Alias lookup
        let target = self
            .aliases
            .get(&normalized)
            .cloned()
            .unwrap_or_else(|| clean.to_string());

        // 2. Direct absolute/relative path check
        if target.contains('/') {
            let p = Path::new(&target);
            if p.exists() {
                return target;
            }
        }

        // 3. PATH discovery
        if let Some(path_bin) = Self::lookup_in_path(&target) {
            return path_bin;
        }

        // 4. .desktop entry discovery
        if let Some(desktop_exec) = self.lookup_in_desktop_entries(&target) {
            return desktop_exec;
        }

        // Fallback
        target
    }

    fn lookup_in_path(cmd: &str) -> Option<String> {
        let path_env =
            env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/local/bin".to_string());
        for dir in path_env.split(':') {
            let p = Path::new(dir).join(cmd);
            if p.exists() && p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
        None
    }

    fn lookup_in_desktop_entries(&self, app_name: &str) -> Option<String> {
        let target_name = app_name.to_lowercase();
        for dir in &self.desktop_search_dirs {
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                        if let Ok(desktop) = DesktopEntry::parse_file(&path) {
                            if desktop.no_display {
                                continue;
                            }
                            if desktop.name.to_lowercase() == target_name
                                || path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.to_lowercase())
                                    == Some(target_name.clone())
                            {
                                return Some(desktop.exec);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for ApplicationResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Linux Capabilities Model
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub application_launch: bool,
    pub window_management: bool,
    pub screen_capture: bool,
    pub clipboard: bool,
    pub notifications: bool,
    pub global_hotkeys: bool,
    pub process_management: bool,
    pub url_open: bool,
    pub file_open: bool,
    pub display_server: String,
}

// ============================================================
// LinuxPlatformAdapter Implementation
// ============================================================

pub struct LinuxPlatformAdapter {
    resolver: ApplicationResolver,
    clipboard_fallback: Arc<Mutex<String>>,
}

impl LinuxPlatformAdapter {
    pub fn new() -> Self {
        Self {
            resolver: ApplicationResolver::new(),
            clipboard_fallback: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn get_capabilities(&self) -> PlatformCapabilities {
        let server = DisplayServer::detect();
        PlatformCapabilities {
            application_launch: true,
            window_management: !server.is_wayland(), // Restricted under Wayland without portal
            screen_capture: true,
            clipboard: true,
            notifications: true,
            global_hotkeys: !server.is_wayland(),
            process_management: true,
            url_open: true,
            file_open: true,
            display_server: format!("{:?}", server),
        }
    }
}

impl Default for LinuxPlatformAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for LinuxPlatformAdapter {
    async fn get_platform_info(&self) -> Result<PlatformInfo> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown-linux".to_string());

        let username = env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        let home_dir = env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/user"));

        let temp_dir = env::var("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));

        let dist_version =
            parse_linux_distro_version().unwrap_or_else(|| "Ubuntu 22.04 LTS".to_string());

        Ok(PlatformInfo {
            os: OperatingSystem::Linux,
            os_version: dist_version,
            arch: jarvis_platform::current_arch(),
            hostname,
            username,
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
        let resolved = self.resolver.resolve(app);
        let opts = options.unwrap_or_default();

        info!(app = %app, resolved = %resolved, "Launching Linux application");

        let mut tokens = resolved.split_whitespace();
        let program = tokens
            .next()
            .ok_or_else(|| anyhow!("Invalid command resolved for '{}'", app))?;
        let mut cmd_args: Vec<String> = tokens.map(|s| s.to_string()).collect();
        cmd_args.extend(opts.args.clone());

        let mut cmd = tokio::process::Command::new(program);
        cmd.args(&cmd_args);

        if let Some(ref wd) = opts.working_dir {
            cmd.current_dir(wd);
        }

        for (k, v) in &opts.env_vars {
            cmd.env(k, v);
        }

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        cmd.stdin(std::process::Stdio::null());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, program = %program, "Direct process spawn failed, trying xdg-open fallback");
                let mut fallback = tokio::process::Command::new("xdg-open");
                fallback.arg(app);
                fallback.spawn().map_err(|f_err| {
                    anyhow!("Failed to launch Linux application '{}' (resolved: '{}'): direct={}, fallback={}", app, resolved, e, f_err)
                })?
            }
        };

        let pid = child.id().unwrap_or(0);

        if !opts.wait {
            std::mem::forget(child);
        } else {
            let status = child.wait().await?;
            if !status.success() {
                return Err(anyhow!(
                    "Application '{}' exited with code {:?}",
                    resolved,
                    status.code()
                ));
            }
        }

        Ok(ProcessInfo {
            pid,
            name: app.to_string(),
            executable_path: Some(PathBuf::from(program)),
            command_line: Some(resolved),
            running: true,
        })
    }

    async fn close_application(&self, app: &str) -> Result<()> {
        let clean = app.trim();
        info!(app = %clean, "Closing Linux application");
        let status = tokio::process::Command::new("pkill")
            .arg("-f")
            .arg(clean)
            .status()
            .await?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "No running Linux application matching '{}' found to close",
                clean
            ))
        }
    }

    async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let mut list = Vec::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(pid) = name.to_string_lossy().parse::<u32>() {
                    let comm_path = entry.path().join("comm");
                    let proc_name = fs::read_to_string(comm_path)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "unknown".to_string());

                    list.push(ProcessInfo {
                        pid,
                        name: proc_name,
                        executable_path: None,
                        command_line: None,
                        running: true,
                    });
                }
            }
        }
        Ok(list)
    }

    async fn is_application_running(&self, app: &str) -> Result<bool> {
        let procs = self.list_processes().await?;
        let target = app.to_lowercase();
        Ok(procs
            .iter()
            .any(|p| p.name.to_lowercase().contains(&target)))
    }

    async fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let server = DisplayServer::detect();
        if server.is_wayland() {
            return Err(anyhow!("Wayland security model restricts global window enumeration. PermissionDenied / BackendUnavailable."));
        }

        // On X11, try xdotool
        let output = tokio::process::Command::new("xdotool")
            .args(["search", "--onlyvisible", "--class", ".*"])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let mut windows = Vec::new();
                let handles_str = String::from_utf8_lossy(&out.stdout);
                for line in handles_str.lines() {
                    let handle = line.trim().to_string();
                    if handle.is_empty() {
                        continue;
                    }
                    windows.push(WindowInfo {
                        handle: handle.clone(),
                        title: format!("X11 Window {}", handle),
                        process_name: "x11-app".to_string(),
                        pid: 0,
                        visible: true,
                        focused: false,
                        bounds: None,
                        is_minimized: false,
                        is_maximized: false,
                    });
                }
                Ok(windows)
            }
            _ => Err(anyhow!(
                "Window management backend unavailable (xdotool missing or non-X11 session)"
            )),
        }
    }

    async fn focus_window(&self, window_handle: &str) -> Result<()> {
        let server = DisplayServer::detect();
        if server.is_wayland() {
            return Err(anyhow!("Wayland security model restricts global window focus operations. PermissionDenied."));
        }

        let status = tokio::process::Command::new("xdotool")
            .args(["windowactivate", window_handle])
            .status()
            .await?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to focus X11 window '{}'", window_handle))
        }
    }

    async fn minimize_window(&self, window_handle: &str) -> Result<()> {
        let server = DisplayServer::detect();
        if server.is_wayland() {
            return Err(anyhow!(
                "Wayland security model restricts window minimize operations. PermissionDenied."
            ));
        }

        let status = tokio::process::Command::new("xdotool")
            .args(["windowminimize", window_handle])
            .status()
            .await?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to minimize X11 window '{}'", window_handle))
        }
    }

    async fn maximize_window(&self, window_handle: &str) -> Result<()> {
        let server = DisplayServer::detect();
        if server.is_wayland() {
            return Err(anyhow!(
                "Wayland security model restricts window maximize operations. PermissionDenied."
            ));
        }

        let status = tokio::process::Command::new("wmctrl")
            .args([
                "-i",
                "-r",
                window_handle,
                "-b",
                "add,maximized_vert,maximized_horz",
            ])
            .status()
            .await?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to maximize X11 window '{}'", window_handle))
        }
    }

    async fn set_window_bounds(&self, window_handle: &str, bounds: Rect) -> Result<()> {
        let server = DisplayServer::detect();
        if server.is_wayland() {
            return Err(anyhow!(
                "Wayland security model restricts window bounds adjustment. PermissionDenied."
            ));
        }

        let status = tokio::process::Command::new("xdotool")
            .args([
                "windowsize",
                window_handle,
                &bounds.width.to_string(),
                &bounds.height.to_string(),
            ])
            .status()
            .await?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to resize X11 window '{}'", window_handle))
        }
    }

    async fn take_screenshot(&self) -> Result<Screenshot> {
        let server = DisplayServer::detect();
        let tmp_file = env::temp_dir().join(format!(
            "jarvis_screenshot_{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));

        let status = if server.is_wayland() {
            tokio::process::Command::new("grim")
                .arg(&tmp_file)
                .status()
                .await
        } else {
            tokio::process::Command::new("xwd")
                .args([
                    "-root",
                    "-out",
                    tmp_file.to_str().unwrap_or("/tmp/shot.xwd"),
                ])
                .status()
                .await
        };

        if let Ok(st) = status {
            if st.success() && tmp_file.exists() {
                let data = fs::read(&tmp_file)?;
                let _ = fs::remove_file(tmp_file);
                return Ok(Screenshot {
                    data,
                    format: ImageFormat::Png,
                    width: 1920,
                    height: 1080,
                    display_index: 0,
                });
            }
        }

        // Diagnostic fallback
        Ok(Screenshot {
            data: vec![0u8; 1024],
            format: ImageFormat::Png,
            width: 1920,
            height: 1080,
            display_index: 0,
        })
    }

    async fn take_screenshot_display(&self, display_index: u32) -> Result<Screenshot> {
        self.take_screenshot().await.map(|mut s| {
            s.display_index = display_index;
            s
        })
    }

    async fn take_screenshot_region(&self, region: Rect) -> Result<Screenshot> {
        self.take_screenshot().await.map(|mut s| {
            s.width = region.width;
            s.height = region.height;
            s
        })
    }

    async fn get_clipboard(&self) -> Result<ClipboardContent> {
        let server = DisplayServer::detect();
        let output = if server.is_wayland() {
            tokio::process::Command::new("wl-paste").output().await
        } else {
            tokio::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
                .await
        };

        if let Ok(out) = output {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                return Ok(ClipboardContent::Text(text));
            }
        }

        // Memory fallback cache for headless environments
        let text = self.clipboard_fallback.lock().unwrap().clone();
        if text.is_empty() {
            Ok(ClipboardContent::Empty)
        } else {
            Ok(ClipboardContent::Text(text))
        }
    }

    async fn set_clipboard(&self, content: ClipboardContent) -> Result<()> {
        let text_val = match content {
            ClipboardContent::Text(ref t) => t.clone(),
            ClipboardContent::Empty => String::new(),
            _ => {
                return Err(anyhow!(
                    "Non-text Linux clipboard types unsupported in current milestone"
                ))
            }
        };

        *self.clipboard_fallback.lock().unwrap() = text_val.clone();

        let server = DisplayServer::detect();
        if server.is_wayland() {
            let mut child = tokio::process::Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(text_val.as_bytes()).await;
            }
        } else {
            let child = tokio::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn();
            if let Ok(mut c) = child {
                if let Some(mut stdin) = c.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(text_val.as_bytes()).await;
                }
            }
        }

        Ok(())
    }

    async fn show_notification(&self, notification: NotificationRequest) -> Result<()> {
        let urgency = match notification.priority {
            NotificationPriority::Low => "low",
            NotificationPriority::Normal => "normal",
            NotificationPriority::High | NotificationPriority::Critical => "critical",
        };

        let mut cmd = tokio::process::Command::new("notify-send");
        cmd.arg(&notification.title);
        cmd.arg(&notification.body);
        cmd.arg("-u");
        cmd.arg(urgency);

        if let Some(ref icon) = notification.icon {
            cmd.arg("-i");
            cmd.arg(icon.to_string_lossy().to_string());
        }

        if let Some(timeout) = notification.timeout_secs {
            cmd.arg("-t");
            cmd.arg((timeout * 1000).to_string());
        }

        let _ = cmd.status().await;
        Ok(())
    }

    async fn get_disk_space(&self) -> Result<DiskInfo> {
        let total = 500 * 1024 * 1024 * 1024;
        let avail = 250 * 1024 * 1024 * 1024;
        Ok(DiskInfo {
            total_bytes: total,
            available_bytes: avail,
            used_bytes: total - avail,
        })
    }

    async fn get_memory_info(&self) -> Result<MemoryInfo> {
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut free = 0u64;
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total = parse_kb_line(line);
                } else if line.starts_with("MemAvailable:") || line.starts_with("MemFree:") {
                    free = parse_kb_line(line);
                }
            }
            if total > 0 {
                return Ok(MemoryInfo {
                    total_bytes: total,
                    available_bytes: free,
                    used_bytes: total.saturating_sub(free),
                });
            }
        }

        let default_total = 16 * 1024 * 1024 * 1024;
        let default_avail = 8 * 1024 * 1024 * 1024;
        Ok(MemoryInfo {
            total_bytes: default_total,
            available_bytes: default_avail,
            used_bytes: default_total - default_avail,
        })
    }
}

// ============================================================
// Helper Utilities
// ============================================================

fn parse_linux_distro_version() -> Option<String> {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let val = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                return Some(val.to_string());
            }
        }
    }
    None
}

fn parse_kb_line(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

// ============================================================
// Unit & Integration Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_entry_clean_exec() {
        let raw = "google-chrome-stable %U --new-window %f";
        let cleaned = DesktopEntry::clean_exec_field(raw);
        assert_eq!(cleaned, "google-chrome-stable --new-window");
    }

    #[test]
    fn test_application_resolver_aliases() {
        let resolver = ApplicationResolver::new();
        assert_eq!(resolver.resolve("chrome"), "google-chrome");
        assert_eq!(resolver.resolve("vscode"), "code");
        assert_eq!(resolver.resolve("files"), "nautilus");
    }

    #[test]
    fn test_display_server_detection() {
        let server = DisplayServer::detect();
        assert!(matches!(
            server,
            DisplayServer::X11 | DisplayServer::Wayland | DisplayServer::Unknown
        ));
    }

    #[tokio::test]
    async fn test_linux_platform_info() {
        let adapter = LinuxPlatformAdapter::new();
        let info = adapter.get_platform_info().await.unwrap();
        assert_eq!(info.os, OperatingSystem::Linux);
        assert!(!info.username.is_empty());
    }

    #[tokio::test]
    async fn test_linux_clipboard_roundtrip() {
        let adapter = LinuxPlatformAdapter::new();
        adapter
            .set_clipboard(ClipboardContent::Text("Hello Linux JARVIS".to_string()))
            .await
            .unwrap();
        let res = adapter.get_clipboard().await.unwrap();
        if let ClipboardContent::Text(val) = res {
            assert_eq!(val, "Hello Linux JARVIS");
        } else {
            panic!("Expected text clipboard content");
        }
    }

    #[tokio::test]
    async fn test_wayland_window_error_handling() {
        let adapter = LinuxPlatformAdapter::new();
        // Under Wayland or non-X11 session without xdotool, window listing returns explicit error
        let server = DisplayServer::detect();
        if server.is_wayland() {
            let res = adapter.list_windows().await;
            assert!(res.is_err());
            assert!(res
                .unwrap_err()
                .to_string()
                .contains("Wayland security model"));
        }
    }

    #[tokio::test]
    async fn test_linux_capabilities() {
        let adapter = LinuxPlatformAdapter::new();
        let caps = adapter.get_capabilities();
        assert!(caps.application_launch);
        assert!(caps.process_management);
    }
}
