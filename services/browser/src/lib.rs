//! JARVIS Browser Management & Automation Subsystem (M09.01 & M09.02)
//!
//! Provides browser session detection, launch, active window tracking,
//! controlled navigation, history navigation (back/forward/reload), and
//! tab management (list/new/switch/close) across supported browsers.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use jarvis_platform::{PlatformAdapter, Rect};

// ============================================================
// Domain Models
// ============================================================

/// Supported browser types in JARVIS.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserType {
    Chrome,
    Edge,
    Firefox,
    Brave,
    Other(String),
}

impl BrowserType {
    /// Returns human-readable display name for the browser.
    pub fn name(&self) -> &str {
        match self {
            BrowserType::Chrome => "Google Chrome",
            BrowserType::Edge => "Microsoft Edge",
            BrowserType::Firefox => "Mozilla Firefox",
            BrowserType::Brave => "Brave Browser",
            BrowserType::Other(s) => s.as_str(),
        }
    }

    /// Returns default OS executable name for launching/matching.
    pub fn executable_name(&self) -> &str {
        match self {
            #[cfg(target_os = "windows")]
            BrowserType::Chrome => "chrome.exe",
            #[cfg(not(target_os = "windows"))]
            BrowserType::Chrome => "google-chrome",

            #[cfg(target_os = "windows")]
            BrowserType::Edge => "msedge.exe",
            #[cfg(not(target_os = "windows"))]
            BrowserType::Edge => "msedge",

            #[cfg(target_os = "windows")]
            BrowserType::Firefox => "firefox.exe",
            #[cfg(not(target_os = "windows"))]
            BrowserType::Firefox => "firefox",

            #[cfg(target_os = "windows")]
            BrowserType::Brave => "brave.exe",
            #[cfg(not(target_os = "windows"))]
            BrowserType::Brave => "brave",

            BrowserType::Other(s) => s.as_str(),
        }
    }

    /// Returns process match patterns (case-insensitive substring matching).
    pub fn process_match_names(&self) -> Vec<&'static str> {
        match self {
            BrowserType::Chrome => vec!["chrome", "chrome.exe", "google-chrome"],
            BrowserType::Edge => vec!["msedge", "msedge.exe"],
            BrowserType::Firefox => vec!["firefox", "firefox.exe"],
            BrowserType::Brave => vec!["brave", "brave.exe"],
            BrowserType::Other(_) => vec![],
        }
    }

    /// Resolves full path to browser executable if available, or returns default executable name.
    pub fn resolved_executable_path(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            match self {
                BrowserType::Chrome => {
                    let candidates = [
                        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                    ];
                    for candidate in &candidates {
                        if std::path::Path::new(candidate).exists() {
                            return candidate.to_string();
                        }
                    }
                    "chrome.exe".to_string()
                }
                BrowserType::Edge => {
                    let candidate = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
                    if std::path::Path::new(candidate).exists() {
                        candidate.to_string()
                    } else {
                        "msedge.exe".to_string()
                    }
                }
                BrowserType::Firefox => {
                    let candidate = r"C:\Program Files\Mozilla Firefox\firefox.exe";
                    if std::path::Path::new(candidate).exists() {
                        candidate.to_string()
                    } else {
                        "firefox.exe".to_string()
                    }
                }
                BrowserType::Brave => "brave.exe".to_string(),
                BrowserType::Other(s) => s.clone(),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            self.executable_name().to_string()
        }
    }

    /// Parses string representation into `BrowserType`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("chrome") {
            BrowserType::Chrome
        } else if lower.contains("edge") {
            BrowserType::Edge
        } else if lower.contains("firefox") {
            BrowserType::Firefox
        } else if lower.contains("brave") {
            BrowserType::Brave
        } else {
            BrowserType::Other(s.to_string())
        }
    }
}

impl fmt::Display for BrowserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Structured detection status for a browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserStatus {
    pub browser_name: String,
    pub process_name: String,
    pub process_id: Option<u32>,
    pub running: bool,
    pub window_count: usize,
    pub foreground: bool,
    pub active_window_title: Option<String>,
}

/// Structured information about an active browser window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWindowInfo {
    pub window_title: String,
    pub process_name: String,
    pub process_id: u32,
    pub bounds: Option<Rect>,
    pub foreground: bool,
}

/// Navigation request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigationRequest {
    pub url: String,
    pub browser: BrowserType,
    pub new_tab: bool,
}

/// Result of a browser navigation operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigationResult {
    pub success: bool,
    pub url: String,
    pub browser: String,
    pub message: String,
    pub window_title: Option<String>,
    pub latency_ms: u64,
}

/// Comprehensive browser session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSessionState {
    pub browser: String,
    pub running: bool,
    pub process_id: Option<u32>,
    pub window_count: usize,
    pub active_window: Option<BrowserWindowInfo>,
    pub current_url: Option<String>,
    pub current_page_title: Option<String>,
    pub limitations: Vec<String>,
}

/// Tab information structure for M09.02.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserTabInfo {
    pub tab_id: usize,
    pub title: String,
    pub url: Option<String>,
    pub active: bool,
}

/// Target specification for tab switching/closing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TabTarget {
    Index(usize),
    Title(String),
    Active,
}

/// Result of generic browser action operations (back, forward, reload, close_tab).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    pub success: bool,
    pub action: String,
    pub browser: String,
    pub message: String,
    pub current_url: Option<String>,
    pub current_title: Option<String>,
    pub latency_ms: u64,
}

// ============================================================
// URL Utilities
// ============================================================

/// Normalizes and validates an input URL string.
///
/// Ensures HTTP/HTTPS scheme, trims whitespace, prepends `https://` if missing,
/// and rejects dangerous or unsupported protocols (e.g. `javascript:`, `file://`).
pub fn normalize_url(raw_url: &str) -> Result<String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("URL string cannot be empty"));
    }

    let lower = trimmed.to_lowercase();

    // Reject dangerous schemes
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("file:")
    {
        return Err(anyhow!("Unsupported or restricted URL protocol scheme"));
    }

    let final_url = if !lower.starts_with("http://") && !lower.starts_with("https://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    };

    // Basic structural validation
    if !final_url.contains('.') && !final_url.contains("localhost") {
        return Err(anyhow!("Invalid domain or hostname format in URL: {}", raw_url));
    }

    Ok(final_url)
}

// ============================================================
// BrowserProvider Trait & Manager
// ============================================================

/// Core abstraction for browser detection, launch, navigation, history, tabs, and inspection.
#[async_trait]
pub trait BrowserProvider: Send + Sync {
    // --- M09.01 Methods ---
    async fn detect_browser(&self, browser: BrowserType) -> Result<BrowserStatus>;
    async fn launch_browser(&self, browser: BrowserType) -> Result<BrowserStatus>;
    async fn get_active_window(&self, browser: BrowserType) -> Result<Option<BrowserWindowInfo>>;
    async fn navigate(&self, request: BrowserNavigationRequest) -> Result<BrowserNavigationResult>;
    async fn get_session_state(&self, browser: BrowserType) -> Result<BrowserSessionState>;

    // --- M09.02 History & Navigation Methods ---
    async fn back(&self, browser: BrowserType) -> Result<BrowserActionResult>;
    async fn forward(&self, browser: BrowserType) -> Result<BrowserActionResult>;
    async fn reload(&self, browser: BrowserType) -> Result<BrowserActionResult>;

    // --- M09.02 Tab Management Methods ---
    async fn list_tabs(&self, browser: BrowserType) -> Result<Vec<BrowserTabInfo>>;
    async fn new_tab(&self, browser: BrowserType, url: Option<String>) -> Result<BrowserTabInfo>;
    async fn switch_tab(&self, browser: BrowserType, target: TabTarget) -> Result<BrowserTabInfo>;
    async fn close_tab(&self, browser: BrowserType, target: Option<TabTarget>) -> Result<BrowserActionResult>;
}

// ============================================================
// PlatformBrowserProvider Implementation
// ============================================================

/// Native platform implementation of `BrowserProvider` using `PlatformAdapter`.
pub struct PlatformBrowserProvider {
    platform_adapter: Arc<dyn PlatformAdapter>,
}

impl PlatformBrowserProvider {
    pub fn new(platform_adapter: Arc<dyn PlatformAdapter>) -> Self {
        Self { platform_adapter }
    }

    /// Helper to find matching browser processes.
    async fn find_browser_processes(&self, browser: &BrowserType) -> Result<Vec<jarvis_platform::ProcessInfo>> {
        let processes = self.platform_adapter.list_processes().await?;
        let match_names = browser.process_match_names();

        let matched: Vec<_> = processes
            .into_iter()
            .filter(|p| {
                let name = p.name.to_lowercase();
                match_names.iter().any(|m| name.contains(m))
            })
            .collect();

        Ok(matched)
    }

    /// Helper to find matching browser windows.
    async fn find_browser_windows(&self, browser: &BrowserType) -> Result<Vec<jarvis_platform::WindowInfo>> {
        let windows = self.platform_adapter.list_windows().await?;
        let match_names = browser.process_match_names();
        let browser_name_lower = browser.name().to_lowercase();

        let matched: Vec<_> = windows
            .into_iter()
            .filter(|w| {
                let proc_name = w.process_name.to_lowercase();
                let title = w.title.to_lowercase();
                match_names.iter().any(|m| proc_name.contains(m)) || title.contains(&browser_name_lower)
            })
            .collect();

        Ok(matched)
    }
}

#[async_trait]
impl BrowserProvider for PlatformBrowserProvider {
    async fn detect_browser(&self, browser: BrowserType) -> Result<BrowserStatus> {
        let processes = self.find_browser_processes(&browser).await?;
        let running = !processes.is_empty();
        let main_pid = processes.first().map(|p| p.pid);

        let windows = self.find_browser_windows(&browser).await?;
        let window_count = windows.len();

        let active_win = self.platform_adapter.get_active_window().await.ok();
        let match_names = browser.process_match_names();

        let (foreground, active_title) = if let Some(ref win) = active_win {
            let proc_name = win.process_name.to_lowercase();
            let is_match = match_names.iter().any(|m| proc_name.contains(m));
            if is_match {
                (true, Some(win.title.clone()))
            } else {
                let first_title = windows.first().map(|w| w.title.clone());
                (false, first_title)
            }
        } else {
            let first_title = windows.first().map(|w| w.title.clone());
            (false, first_title)
        };

        Ok(BrowserStatus {
            browser_name: browser.name().to_string(),
            process_name: browser.executable_name().to_string(),
            process_id: main_pid,
            running,
            window_count,
            foreground,
            active_window_title: active_title,
        })
    }

    async fn launch_browser(&self, browser: BrowserType) -> Result<BrowserStatus> {
        let current_status = self.detect_browser(browser.clone()).await?;

        if current_status.running {
            tracing::info!(
                browser = %browser.name(),
                pid = ?current_status.process_id,
                "Browser is already running; avoiding duplicate launch and bringing window to focus"
            );

            if let Some(ref title) = current_status.active_window_title {
                let _ = self.platform_adapter.focus_window(title).await;
            } else {
                let _ = self.platform_adapter.focus_window(browser.executable_name()).await;
            }

            return self.detect_browser(browser).await;
        }

        tracing::info!(browser = %browser.name(), "Launching browser process...");
        let exec_path = browser.resolved_executable_path();
        #[cfg(target_os = "windows")]
        {
            let _ = tokio::process::Command::new("cmd")
                .args(["/c", "start", "", &exec_path])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }

        #[cfg(not(target_os = "windows"))]
        {
            self.platform_adapter
                .open_application(&exec_path, None)
                .await
                .map_err(|e| anyhow!("Failed to launch browser '{}': {}", exec_path, e))?;
        }

        let start = Instant::now();
        while start.elapsed().as_secs() < 5 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            let status = self.detect_browser(browser.clone()).await?;
            if status.running {
                return Ok(status);
            }
        }

        self.detect_browser(browser).await
    }

    async fn get_active_window(&self, browser: BrowserType) -> Result<Option<BrowserWindowInfo>> {
        let windows = self.find_browser_windows(&browser).await?;
        if windows.is_empty() {
            return Ok(None);
        }

        let active_win = self.platform_adapter.get_active_window().await.ok();
        let match_names = browser.process_match_names();

        if let Some(win) = active_win {
            let proc_name = win.process_name.to_lowercase();
            if match_names.iter().any(|m| proc_name.contains(m)) {
                return Ok(Some(BrowserWindowInfo {
                    window_title: win.title,
                    process_name: win.process_name,
                    process_id: win.pid,
                    bounds: win.bounds,
                    foreground: true,
                }));
            }
        }

        let first = &windows[0];
        Ok(Some(BrowserWindowInfo {
            window_title: first.title.clone(),
            process_name: first.process_name.clone(),
            process_id: first.pid,
            bounds: first.bounds.clone(),
            foreground: first.focused,
        }))
    }

    async fn navigate(&self, request: BrowserNavigationRequest) -> Result<BrowserNavigationResult> {
        let start = Instant::now();
        let normalized_url = normalize_url(&request.url)?;

        let status = self.launch_browser(request.browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot navigate: Browser '{}' is not running", request.browser.name()));
        }

        #[cfg(target_os = "windows")]
        {
            let exec_path = request.browser.resolved_executable_path();
            let script = format!(
                "Start-Process '{}' -ArgumentList '{}'",
                exec_path,
                normalized_url
            );
            let output = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .output()
                .await?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Failed to navigate browser: {}", err.trim()));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            tokio::process::Command::new(request.browser.executable_name())
                .arg(&normalized_url)
                .spawn()?;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let active_win = self.get_active_window(request.browser.clone()).await?;
        let win_title = active_win.map(|w| w.window_title);
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(BrowserNavigationResult {
            success: true,
            url: normalized_url.clone(),
            browser: request.browser.name().to_string(),
            message: format!("Successfully navigated to {}", normalized_url),
            window_title: win_title,
            latency_ms: elapsed,
        })
    }

    async fn get_session_state(&self, browser: BrowserType) -> Result<BrowserSessionState> {
        let status = self.detect_browser(browser.clone()).await?;
        let active_win = self.get_active_window(browser.clone()).await?;

        let mut current_page_title = None;
        let mut current_url = None;
        let mut limitations = Vec::new();

        if let Some(ref win) = active_win {
            let title = &win.window_title;
            let suffix = format!(" - {}", browser.name());
            if let Some(prefix) = title.strip_suffix(&suffix) {
                current_page_title = Some(prefix.to_string());
            } else {
                current_page_title = Some(title.clone());
            }

            if let Ok(uia_res) = self.platform_adapter.inspect_ui_tree(Some("Address"), 4, 30).await {
                for elem in uia_res.elements {
                    if (elem.control_type == "Edit" || elem.control_type == "Text")
                        && !elem.name.is_empty()
                        && (elem.name.starts_with("http") || elem.name.contains('.'))
                    {
                        current_url = Some(elem.name.clone());
                        break;
                    }
                }
            }

            if current_url.is_none() {
                limitations.push("Direct URL extraction from browser address bar requires dedicated extension or automation pipe.".to_string());
            }
        } else {
            limitations.push("No active window detected for browser.".to_string());
        }

        Ok(BrowserSessionState {
            browser: browser.name().to_string(),
            running: status.running,
            process_id: status.process_id,
            window_count: status.window_count,
            active_window: active_win,
            current_url,
            current_page_title,
            limitations,
        })
    }

    async fn back(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let start = Instant::now();
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot navigate back: Browser '{}' is not running", browser.name()));
        }

        if let Some(ref title) = status.active_window_title {
            let _ = self.platform_adapter.focus_window(title).await;
        }

        #[cfg(target_os = "windows")]
        {
            let script = "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('%{LEFT}')";
            let _ = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .output()
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let session = self.get_session_state(browser.clone()).await?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(BrowserActionResult {
            success: true,
            action: "back".to_string(),
            browser: browser.name().to_string(),
            message: "Navigated back in browser history".to_string(),
            current_url: session.current_url,
            current_title: session.current_page_title,
            latency_ms: elapsed,
        })
    }

    async fn forward(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let start = Instant::now();
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot navigate forward: Browser '{}' is not running", browser.name()));
        }

        if let Some(ref title) = status.active_window_title {
            let _ = self.platform_adapter.focus_window(title).await;
        }

        #[cfg(target_os = "windows")]
        {
            let script = "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('%{RIGHT}')";
            let _ = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .output()
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let session = self.get_session_state(browser.clone()).await?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(BrowserActionResult {
            success: true,
            action: "forward".to_string(),
            browser: browser.name().to_string(),
            message: "Navigated forward in browser history".to_string(),
            current_url: session.current_url,
            current_title: session.current_page_title,
            latency_ms: elapsed,
        })
    }

    async fn reload(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let start = Instant::now();
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot reload page: Browser '{}' is not running", browser.name()));
        }

        if let Some(ref title) = status.active_window_title {
            let _ = self.platform_adapter.focus_window(title).await;
        }

        #[cfg(target_os = "windows")]
        {
            let script = "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('{F5}')";
            let _ = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .output()
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let session = self.get_session_state(browser.clone()).await?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(BrowserActionResult {
            success: true,
            action: "reload".to_string(),
            browser: browser.name().to_string(),
            message: "Reloaded current page".to_string(),
            current_url: session.current_url,
            current_title: session.current_page_title,
            latency_ms: elapsed,
        })
    }

    async fn list_tabs(&self, browser: BrowserType) -> Result<Vec<BrowserTabInfo>> {
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot list tabs: Browser '{}' is not running", browser.name()));
        }

        let mut tabs = Vec::new();

        if let Ok(uia_res) = self.platform_adapter.inspect_ui_tree(Some("tab"), 6, 50).await {
            let tab_elems: Vec<_> = uia_res
                .elements
                .into_iter()
                .filter(|e| e.control_type == "TabItem" || e.control_type == "HeaderItem" || e.name.contains("tab") || e.name.contains("Tab"))
                .collect();

            if !tab_elems.is_empty() {
                for (idx, elem) in tab_elems.into_iter().enumerate() {
                    tabs.push(BrowserTabInfo {
                        tab_id: idx + 1,
                        title: if elem.name.is_empty() { format!("Tab {}", idx + 1) } else { elem.name },
                        url: None,
                        active: elem.focused,
                    });
                }
            }
        }

        if tabs.is_empty() {
            let windows = self.find_browser_windows(&browser).await?;
            if windows.is_empty() {
                let active_title = status.active_window_title.unwrap_or_else(|| browser.name().to_string());
                tabs.push(BrowserTabInfo {
                    tab_id: 1,
                    title: active_title,
                    url: None,
                    active: true,
                });
            } else {
                for (idx, win) in windows.into_iter().enumerate() {
                    tabs.push(BrowserTabInfo {
                        tab_id: idx + 1,
                        title: win.title,
                        url: None,
                        active: win.focused || idx == 0,
                    });
                }
            }
        }

        Ok(tabs)
    }

    async fn new_tab(&self, browser: BrowserType, url: Option<String>) -> Result<BrowserTabInfo> {
        let status = self.launch_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot open new tab: Browser '{}' is not running", browser.name()));
        }

        if let Some(target_url) = url {
            let normalized = normalize_url(&target_url)?;
            let nav_req = BrowserNavigationRequest {
                url: normalized,
                browser: browser.clone(),
                new_tab: true,
            };
            let _ = self.navigate(nav_req).await?;
        } else {
            #[cfg(target_os = "windows")]
            {
                let script = "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^{t}')";
                let _ = tokio::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", script])
                    .output()
                    .await;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let tabs = self.list_tabs(browser.clone()).await?;
        let active_tab = tabs.into_iter().find(|t| t.active).unwrap_or(BrowserTabInfo {
            tab_id: 1,
            title: format!("New Tab - {}", browser.name()),
            url: None,
            active: true,
        });

        Ok(active_tab)
    }

    async fn switch_tab(&self, browser: BrowserType, target: TabTarget) -> Result<BrowserTabInfo> {
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot switch tab: Browser '{}' is not running", browser.name()));
        }

        if let Some(ref title) = status.active_window_title {
            let _ = self.platform_adapter.focus_window(title).await;
        }

        let tabs = self.list_tabs(browser.clone()).await?;
        if tabs.is_empty() {
            return Err(anyhow!("No open tabs detected for browser '{}'", browser.name()));
        }

        let target_tab = match target {
            TabTarget::Index(idx) => {
                if idx == 0 || idx > tabs.len() {
                    return Err(anyhow!("Invalid tab index {}: Available tabs count is {}", idx, tabs.len()));
                }
                tabs[idx - 1].clone()
            }
            TabTarget::Title(ref query) => {
                let lower_q = query.to_lowercase();
                tabs.iter()
                    .find(|t| t.title.to_lowercase().contains(&lower_q))
                    .cloned()
                    .ok_or_else(|| anyhow!("No tab found matching title query '{}'", query))?
            }
            TabTarget::Active => {
                tabs.iter().find(|t| t.active).cloned().unwrap_or(tabs[0].clone())
            }
        };

        #[cfg(target_os = "windows")]
        {
            let key = match target_tab.tab_id {
                1..=8 => format!("^{}", target_tab.tab_id),
                _ => "^{9}".to_string(),
            };
            let script = format!("$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('{}')", key);
            let _ = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .output()
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let mut switched = target_tab;
        switched.active = true;
        Ok(switched)
    }

    async fn close_tab(&self, browser: BrowserType, target: Option<TabTarget>) -> Result<BrowserActionResult> {
        let start = Instant::now();
        let status = self.detect_browser(browser.clone()).await?;
        if !status.running {
            return Err(anyhow!("Cannot close tab: Browser '{}' is not running", browser.name()));
        }

        if let Some(tgt) = target {
            let _ = self.switch_tab(browser.clone(), tgt).await?;
        } else if let Some(ref title) = status.active_window_title {
            let _ = self.platform_adapter.focus_window(title).await;
        }

        #[cfg(target_os = "windows")]
        {
            let script = "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^{w}')";
            let _ = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .output()
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let session = self.get_session_state(browser.clone()).await.unwrap_or(BrowserSessionState {
            browser: browser.name().to_string(),
            running: false,
            process_id: None,
            window_count: 0,
            active_window: None,
            current_url: None,
            current_page_title: None,
            limitations: vec![],
        });
        let elapsed = start.elapsed().as_millis() as u64;

        Ok(BrowserActionResult {
            success: true,
            action: "close_tab".to_string(),
            browser: browser.name().to_string(),
            message: "Closed browser tab".to_string(),
            current_url: session.current_url,
            current_title: session.current_page_title,
            latency_ms: elapsed,
        })
    }
}

// ============================================================
// MockBrowserProvider Implementation for Unit Tests
// ============================================================

/// In-memory mock implementation of `BrowserProvider` for testing.
pub struct MockBrowserProvider {
    pub running: Mutex<bool>,
    pub process_id: Mutex<Option<u32>>,
    pub active_window: Mutex<Option<BrowserWindowInfo>>,
    pub current_url: Mutex<Option<String>>,
    pub current_title: Mutex<Option<String>>,
    pub navigation_history: Mutex<Vec<String>>,
    pub history_index: Mutex<usize>,
    pub tabs: Mutex<Vec<BrowserTabInfo>>,
    pub fail_on_navigate: Mutex<bool>,
}

impl MockBrowserProvider {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(false),
            process_id: Mutex::new(None),
            active_window: Mutex::new(None),
            current_url: Mutex::new(None),
            current_title: Mutex::new(Some("Mock Page".to_string())),
            navigation_history: Mutex::new(Vec::new()),
            history_index: Mutex::new(0),
            tabs: Mutex::new(vec![BrowserTabInfo {
                tab_id: 1,
                title: "Mock Tab 1".to_string(),
                url: Some("https://google.com".to_string()),
                active: true,
            }]),
            fail_on_navigate: Mutex::new(false),
        }
    }

    pub fn with_running(self, running: bool, pid: u32, window_title: &str) -> Self {
        *self.running.lock().unwrap() = running;
        *self.process_id.lock().unwrap() = if running { Some(pid) } else { None };
        if running {
            *self.active_window.lock().unwrap() = Some(BrowserWindowInfo {
                window_title: window_title.to_string(),
                process_name: "chrome.exe".to_string(),
                process_id: pid,
                bounds: Some(Rect { x: 0, y: 0, width: 1280, height: 720 }),
                foreground: true,
            });
        }
        self
    }
}

impl Default for MockBrowserProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BrowserProvider for MockBrowserProvider {
    async fn detect_browser(&self, browser: BrowserType) -> Result<BrowserStatus> {
        let running = *self.running.lock().unwrap();
        let pid = *self.process_id.lock().unwrap();
        let win = self.active_window.lock().unwrap().clone();

        Ok(BrowserStatus {
            browser_name: browser.name().to_string(),
            process_name: browser.executable_name().to_string(),
            process_id: pid,
            running,
            window_count: if running { self.tabs.lock().unwrap().len() } else { 0 },
            foreground: win.as_ref().map(|w| w.foreground).unwrap_or(false),
            active_window_title: win.map(|w| w.window_title),
        })
    }

    async fn launch_browser(&self, browser: BrowserType) -> Result<BrowserStatus> {
        *self.running.lock().unwrap() = true;
        if self.process_id.lock().unwrap().is_none() {
            *self.process_id.lock().unwrap() = Some(1234);
        }
        if self.active_window.lock().unwrap().is_none() {
            *self.active_window.lock().unwrap() = Some(BrowserWindowInfo {
                window_title: format!("New Tab - {}", browser.name()),
                process_name: browser.executable_name().to_string(),
                process_id: 1234,
                bounds: Some(Rect { x: 0, y: 0, width: 1280, height: 720 }),
                foreground: true,
            });
        }
        self.detect_browser(browser).await
    }

    async fn get_active_window(&self, _browser: BrowserType) -> Result<Option<BrowserWindowInfo>> {
        Ok(self.active_window.lock().unwrap().clone())
    }

    async fn navigate(&self, request: BrowserNavigationRequest) -> Result<BrowserNavigationResult> {
        let start = Instant::now();
        if *self.fail_on_navigate.lock().unwrap() {
            return Err(anyhow!("Mock browser navigation failure triggered"));
        }

        let normalized_url = normalize_url(&request.url)?;
        let mut hist = self.navigation_history.lock().unwrap();
        hist.push(normalized_url.clone());
        *self.history_index.lock().unwrap() = hist.len().saturating_sub(1);
        *self.current_url.lock().unwrap() = Some(normalized_url.clone());
        *self.running.lock().unwrap() = true;

        Ok(BrowserNavigationResult {
            success: true,
            url: normalized_url.clone(),
            browser: request.browser.name().to_string(),
            message: format!("Successfully navigated to {}", normalized_url),
            window_title: Some(format!("Google - {}", request.browser.name())),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_session_state(&self, browser: BrowserType) -> Result<BrowserSessionState> {
        let running = *self.running.lock().unwrap();
        let pid = *self.process_id.lock().unwrap();
        let win = self.active_window.lock().unwrap().clone();
        let url = self.current_url.lock().unwrap().clone();
        let title = self.current_title.lock().unwrap().clone();
        let tab_count = self.tabs.lock().unwrap().len();

        Ok(BrowserSessionState {
            browser: browser.name().to_string(),
            running,
            process_id: pid,
            window_count: if running { tab_count } else { 0 },
            active_window: win,
            current_url: url,
            current_page_title: title,
            limitations: vec![],
        })
    }

    async fn back(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot navigate back: Browser '{}' is not running", browser.name()));
        }

        let mut idx = self.history_index.lock().unwrap();
        let hist = self.navigation_history.lock().unwrap();
        if *idx > 0 && !hist.is_empty() {
            *idx -= 1;
            *self.current_url.lock().unwrap() = Some(hist[*idx].clone());
        }

        Ok(BrowserActionResult {
            success: true,
            action: "back".to_string(),
            browser: browser.name().to_string(),
            message: "Navigated back in browser history".to_string(),
            current_url: self.current_url.lock().unwrap().clone(),
            current_title: self.current_title.lock().unwrap().clone(),
            latency_ms: 10,
        })
    }

    async fn forward(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot navigate forward: Browser '{}' is not running", browser.name()));
        }

        let mut idx = self.history_index.lock().unwrap();
        let hist = self.navigation_history.lock().unwrap();
        if *idx + 1 < hist.len() {
            *idx += 1;
            *self.current_url.lock().unwrap() = Some(hist[*idx].clone());
        }

        Ok(BrowserActionResult {
            success: true,
            action: "forward".to_string(),
            browser: browser.name().to_string(),
            message: "Navigated forward in browser history".to_string(),
            current_url: self.current_url.lock().unwrap().clone(),
            current_title: self.current_title.lock().unwrap().clone(),
            latency_ms: 10,
        })
    }

    async fn reload(&self, browser: BrowserType) -> Result<BrowserActionResult> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot reload page: Browser '{}' is not running", browser.name()));
        }

        Ok(BrowserActionResult {
            success: true,
            action: "reload".to_string(),
            browser: browser.name().to_string(),
            message: "Reloaded current page".to_string(),
            current_url: self.current_url.lock().unwrap().clone(),
            current_title: self.current_title.lock().unwrap().clone(),
            latency_ms: 10,
        })
    }

    async fn list_tabs(&self, browser: BrowserType) -> Result<Vec<BrowserTabInfo>> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot list tabs: Browser '{}' is not running", browser.name()));
        }
        Ok(self.tabs.lock().unwrap().clone())
    }

    async fn new_tab(&self, _browser: BrowserType, url: Option<String>) -> Result<BrowserTabInfo> {
        let running = *self.running.lock().unwrap();
        if !running {
            *self.running.lock().unwrap() = true;
        }

        let mut tabs = self.tabs.lock().unwrap();
        for t in tabs.iter_mut() {
            t.active = false;
        }

        let new_id = tabs.len() + 1;
        let tab = BrowserTabInfo {
            tab_id: new_id,
            title: format!("Tab {}", new_id),
            url: url.clone(),
            active: true,
        };
        tabs.push(tab.clone());

        if let Some(u) = url {
            *self.current_url.lock().unwrap() = Some(u);
        }

        Ok(tab)
    }

    async fn switch_tab(&self, browser: BrowserType, target: TabTarget) -> Result<BrowserTabInfo> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot switch tab: Browser '{}' is not running", browser.name()));
        }

        let mut tabs = self.tabs.lock().unwrap();
        if tabs.is_empty() {
            return Err(anyhow!("No open tabs detected"));
        }

        let target_idx = match target {
            TabTarget::Index(idx) => {
                if idx == 0 || idx > tabs.len() {
                    return Err(anyhow!("Invalid tab index {}: Available tabs count is {}", idx, tabs.len()));
                }
                idx - 1
            }
            TabTarget::Title(ref query) => {
                let q_lower = query.to_lowercase();
                tabs.iter().position(|t| t.title.to_lowercase().contains(&q_lower))
                    .ok_or_else(|| anyhow!("No tab found matching title query '{}'", query))?
            }
            TabTarget::Active => 0,
        };

        for (i, t) in tabs.iter_mut().enumerate() {
            t.active = i == target_idx;
        }

        Ok(tabs[target_idx].clone())
    }

    async fn close_tab(&self, browser: BrowserType, target: Option<TabTarget>) -> Result<BrowserActionResult> {
        let running = *self.running.lock().unwrap();
        if !running {
            return Err(anyhow!("Cannot close tab: Browser '{}' is not running", browser.name()));
        }

        let mut tabs = self.tabs.lock().unwrap();
        if tabs.is_empty() {
            return Err(anyhow!("No open tabs to close"));
        }

        let remove_idx = if let Some(tgt) = target {
            match tgt {
                TabTarget::Index(idx) => {
                    if idx == 0 || idx > tabs.len() {
                        return Err(anyhow!("Invalid tab index {}: Available tabs count is {}", idx, tabs.len()));
                    }
                    idx - 1
                }
                TabTarget::Title(ref query) => {
                    let q_lower = query.to_lowercase();
                    tabs.iter().position(|t| t.title.to_lowercase().contains(&q_lower))
                        .ok_or_else(|| anyhow!("No tab found matching title query '{}'", query))?
                }
                TabTarget::Active => tabs.iter().position(|t| t.active).unwrap_or(0),
            }
        } else {
            tabs.iter().position(|t| t.active).unwrap_or(0)
        };

        tabs.remove(remove_idx);

        if !tabs.is_empty() {
            let active_idx = remove_idx.min(tabs.len() - 1);
            tabs[active_idx].active = true;
        } else {
            *self.running.lock().unwrap() = false;
        }

        Ok(BrowserActionResult {
            success: true,
            action: "close_tab".to_string(),
            browser: browser.name().to_string(),
            message: "Closed browser tab".to_string(),
            current_url: self.current_url.lock().unwrap().clone(),
            current_title: self.current_title.lock().unwrap().clone(),
            latency_ms: 10,
        })
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_type_parsing_and_names() {
        assert_eq!(BrowserType::from_str("chrome"), BrowserType::Chrome);
        assert_eq!(BrowserType::from_str("Google Chrome"), BrowserType::Chrome);
        assert_eq!(BrowserType::from_str("edge"), BrowserType::Edge);
        assert_eq!(BrowserType::from_str("firefox"), BrowserType::Firefox);
        assert_eq!(BrowserType::from_str("brave"), BrowserType::Brave);

        assert_eq!(BrowserType::Chrome.name(), "Google Chrome");
        assert!(BrowserType::Chrome.process_match_names().contains(&"chrome"));
    }

    #[test]
    fn test_url_normalization_valid() {
        assert_eq!(normalize_url("google.com").unwrap(), "https://google.com");
        assert_eq!(normalize_url("www.linkedin.com").unwrap(), "https://www.linkedin.com");
        assert_eq!(normalize_url("https://github.com/rust-lang").unwrap(), "https://github.com/rust-lang");
        assert_eq!(normalize_url("http://localhost:8080").unwrap(), "http://localhost:8080");
    }

    #[test]
    fn test_url_normalization_rejected() {
        assert!(normalize_url("").is_err());
        assert!(normalize_url("   ").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(normalize_url("file:///C:/passwords.txt").is_err());
        assert!(normalize_url("data:text/html,<h1>hack</h1>").is_err());
    }

    #[tokio::test]
    async fn test_mock_browser_provider_detection() {
        let mock = MockBrowserProvider::new();
        let status = mock.detect_browser(BrowserType::Chrome).await.unwrap();
        assert!(!status.running);

        let mock_running = MockBrowserProvider::new().with_running(true, 5678, "Google Chrome");
        let status2 = mock_running.detect_browser(BrowserType::Chrome).await.unwrap();
        assert!(status2.running);
        assert_eq!(status2.process_id, Some(5678));
        assert_eq!(status2.active_window_title, Some("Google Chrome".to_string()));
    }

    #[tokio::test]
    async fn test_mock_browser_provider_launch_and_navigate() {
        let mock = MockBrowserProvider::new();
        let req = BrowserNavigationRequest {
            url: "linkedin.com".to_string(),
            browser: BrowserType::Chrome,
            new_tab: false,
        };

        let result = mock.navigate(req).await.unwrap();
        assert!(result.success);
        assert_eq!(result.url, "https://linkedin.com");
        assert_eq!(result.browser, "Google Chrome");
    }

    #[tokio::test]
    async fn test_mock_browser_provider_history_and_tabs() {
        let mock = MockBrowserProvider::new().with_running(true, 1234, "Google Chrome");

        // Test navigation history & back/forward
        let req1 = BrowserNavigationRequest {
            url: "google.com".to_string(),
            browser: BrowserType::Chrome,
            new_tab: false,
        };
        let req2 = BrowserNavigationRequest {
            url: "wikipedia.org".to_string(),
            browser: BrowserType::Chrome,
            new_tab: false,
        };
        mock.navigate(req1).await.unwrap();
        mock.navigate(req2).await.unwrap();

        assert_eq!(mock.current_url.lock().unwrap().as_deref(), Some("https://wikipedia.org"));

        let back_res = mock.back(BrowserType::Chrome).await.unwrap();
        assert!(back_res.success);
        assert_eq!(mock.current_url.lock().unwrap().as_deref(), Some("https://google.com"));

        let fwd_res = mock.forward(BrowserType::Chrome).await.unwrap();
        assert!(fwd_res.success);
        assert_eq!(mock.current_url.lock().unwrap().as_deref(), Some("https://wikipedia.org"));

        // Test tabs
        let t2 = mock.new_tab(BrowserType::Chrome, Some("https://rust-lang.org".to_string())).await.unwrap();
        assert_eq!(t2.tab_id, 2);
        assert!(t2.active);

        let tabs = mock.list_tabs(BrowserType::Chrome).await.unwrap();
        assert_eq!(tabs.len(), 2);

        let switched = mock.switch_tab(BrowserType::Chrome, TabTarget::Index(1)).await.unwrap();
        assert_eq!(switched.tab_id, 1);
        assert!(switched.active);

        let close_res = mock.close_tab(BrowserType::Chrome, None).await.unwrap();
        assert!(close_res.success);
        let tabs_after = mock.list_tabs(BrowserType::Chrome).await.unwrap();
        assert_eq!(tabs_after.len(), 1);
    }
}
