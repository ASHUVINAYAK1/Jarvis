//! Windows UI Automation (UIA) Integration
//!
//! Provides native accessibility tree inspection for the active foreground window
//! using Windows UI Automation COM APIs (`IUIAutomation`, `IUIAutomationElement`).

use jarvis_platform::{Rect, UiElement, UiTreeResult};
use tracing::info;

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::HWND,
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
    Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker},
    Win32::UI::WindowsAndMessaging::{GetDesktopWindow, GetForegroundWindow},
};

/// Maps native Windows UIA ControlType ID integer to human-readable string name.
pub fn control_type_to_string(control_type_id: i32) -> &'static str {
    match control_type_id {
        50000 => "Button",
        50001 => "Calendar",
        50002 => "CheckBox",
        50003 => "ComboBox",
        50004 => "Edit",
        50005 => "Hyperlink",
        50006 => "Image",
        50007 => "ListItem",
        50008 => "List",
        50009 => "Menu",
        50010 => "MenuBar",
        50011 => "MenuItem",
        50012 => "ProgressBar",
        50013 => "RadioButton",
        50014 => "ScrollBar",
        50015 => "Slider",
        50016 => "Spinner",
        50017 => "StatusBar",
        50018 => "Tab",
        50019 => "TabItem",
        50020 => "Text",
        50021 => "ToolBar",
        50022 => "ToolTip",
        50023 => "Tree",
        50024 => "TreeItem",
        50025 => "Custom",
        50026 => "Group",
        50027 => "Thumb",
        50028 => "DataGrid",
        50029 => "DataItem",
        50030 => "Document",
        50031 => "SplitButton",
        50032 => "Window",
        50033 => "Pane",
        50034 => "Header",
        50035 => "HeaderItem",
        50036 => "Table",
        50037 => "TitleBar",
        50038 => "Separator",
        _ => "Unknown",
    }
}

/// Native Windows implementation inspecting active window accessibility tree via UIA.
#[cfg(target_os = "windows")]
pub fn inspect_active_window_uia(
    query: Option<&str>,
    max_depth: usize,
    max_elements: usize,
) -> anyhow::Result<UiTreeResult> {
    // 1. Initialize COM library
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    // 2. Instantiate CUIAutomation COM class
    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?
    };

    // 3. Get active foreground window HWND, falling back to desktop window
    let mut hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        hwnd = unsafe { GetDesktopWindow() };
    }

    // 4. Get root UIA element for target window
    let root_element: IUIAutomationElement = unsafe {
        if !hwnd.0.is_null() {
            match automation.ElementFromHandle(hwnd) {
                Ok(elem) => elem,
                Err(_) => automation.GetRootElement()?,
            }
        } else {
            automation.GetRootElement()?
        }
    };

    // Extract window title & class name
    let window_title = unsafe {
        root_element
            .CurrentName()
            .map(|b| b.to_string())
            .unwrap_or_else(|_| "Active Window".to_string())
    };

    let process_name = unsafe {
        root_element
            .CurrentClassName()
            .map(|b| b.to_string())
            .unwrap_or_else(|_| "unknown.exe".to_string())
    };

    // 5. Get ControlViewWalker for safe UI element traversal
    let walker: IUIAutomationTreeWalker = unsafe { automation.ControlViewWalker()? };

    let mut collected_elements = Vec::new();
    let mut total_scanned = 0;
    let mut is_truncated = false;

    // 6. Perform bounded breadth-first traversal starting from root element
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((root_element, 0usize));

    let max_depth_limit = if max_depth == 0 { 8 } else { max_depth };
    let max_elements_limit = if max_elements == 0 { 100 } else { max_elements };

    while let Some((element, depth)) = queue.pop_front() {
        total_scanned += 1;

        if collected_elements.len() >= max_elements_limit {
            is_truncated = true;
            break;
        }

        // Extract UI element properties safely
        if let Some(ui_elem) = extract_ui_element_info(&element) {
            // Check query filter if provided
            let matches = match query {
                Some(q) => ui_elem.matches_query(q),
                None => true,
            };

            // Filter out empty unnamed structural Panes unless explicitly matched
            let is_meaningful = !ui_elem.name.trim().is_empty()
                || ui_elem.control_type != "Pane"
                || query.is_some();

            if matches && is_meaningful {
                collected_elements.push(ui_elem);
            }
        }

        // Traverse children if under depth limit
        if depth < max_depth_limit {
            unsafe {
                if let Ok(child) = walker.GetFirstChildElement(&element) {
                    let mut current_child = Some(child);
                    while let Some(c) = current_child {
                        queue.push_back((c.clone(), depth + 1));
                        current_child = walker.GetNextSiblingElement(&c).ok();
                        if queue.len() > 500 {
                            // Guard queue size
                            break;
                        }
                    }
                }
            }
        }
    }

    info!(
        window = %window_title,
        elements_found = collected_elements.len(),
        total_scanned,
        is_truncated,
        "Windows UI Automation tree inspection completed"
    );

    Ok(UiTreeResult {
        window_title,
        process_name,
        elements: collected_elements,
        total_elements_scanned: total_scanned,
        is_truncated,
        source: "WindowsUIAutomation".to_string(),
    })
}

/// Fallback for non-Windows platforms (or testing).
#[cfg(not(target_os = "windows"))]
pub fn inspect_active_window_uia(
    _query: Option<&str>,
    _max_depth: usize,
    _max_elements: usize,
) -> anyhow::Result<UiTreeResult> {
    Ok(UiTreeResult::empty())
}

#[cfg(target_os = "windows")]
fn extract_ui_element_info(element: &IUIAutomationElement) -> Option<UiElement> {
    unsafe {
        let name = element.CurrentName().map(|b| b.to_string()).unwrap_or_default();
        let automation_id = element.CurrentAutomationId().map(|b| b.to_string()).unwrap_or_default();
        let class_name = element.CurrentClassName().map(|b| b.to_string()).unwrap_or_default();

        let control_type_id = element.CurrentControlType().map(|id| id.0).unwrap_or(0);
        let control_type = control_type_to_string(control_type_id).to_string();

        let rect = element.CurrentBoundingRectangle().ok()?;

        let width = if rect.right >= rect.left { (rect.right - rect.left) as u32 } else { 0 };
        let height = if rect.bottom >= rect.top { (rect.bottom - rect.top) as u32 } else { 0 };

        // Skip invalid 0x0 or collapsed offscreen elements unless named
        if width == 0 && height == 0 && name.is_empty() {
            return None;
        }

        let bounds = Rect {
            x: rect.left,
            y: rect.top,
            width,
            height,
        };

        let enabled = element.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(true);
        let offscreen = element.CurrentIsOffscreen().map(|b| b.as_bool()).unwrap_or(false);
        let focused = element.CurrentHasKeyboardFocus().map(|b| b.as_bool()).unwrap_or(false);

        Some(UiElement::new(
            name,
            automation_id,
            control_type,
            class_name,
            bounds,
            enabled,
            offscreen,
            focused,
        ))
    }
}
