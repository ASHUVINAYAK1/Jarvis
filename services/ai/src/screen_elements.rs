//! Screen Element Detection Models and Parsing (M08.04)
//!
//! Provides strongly typed structures for detected UI elements on the desktop.
//! Detection uses the existing VisionModelProvider (OllamaVisionProvider / moondream)
//! and the existing OCR pipeline.
//!
//! # Architecture
//! ```text
//! PlatformAdapter::take_screenshot()
//!     down
//! VisionImage (PNG bytes)
//!     down
//! VisionRequest (structured JSON prompt)
//!     down
//! VisionModelProvider (OllamaVisionProvider -> moondream)
//!     down
//! parse_elements_from_vision_response()
//!     down
//! ElementDetectionResult { elements: Vec<ScreenElement> }
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 8, Milestone M08.04

use serde::{Deserialize, Serialize};

// ============================================================
// Element Type
// ============================================================

/// The UI category of a detected screen element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Button,
    TextInput,
    Icon,
    Link,
    Image,
    Text,
    Checkbox,
    Dropdown,
    Menu,
    Tab,
    Window,
    Dialog,
    Toolbar,
    Label,
    Unknown,
}

impl std::str::FromStr for ElementType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().trim() {
            "button" | "btn" => Self::Button,
            "textinput" | "text_input" | "input" | "textbox" | "text box" | "field" | "editbox" => {
                Self::TextInput
            }
            "icon" => Self::Icon,
            "link" | "hyperlink" | "url" => Self::Link,
            "image" | "img" | "photo" | "picture" => Self::Image,
            "text" | "label" | "heading" | "paragraph" | "p" => Self::Text,
            "checkbox" | "check" | "toggle" => Self::Checkbox,
            "dropdown" | "select" | "combobox" | "combo" => Self::Dropdown,
            "menu" | "menuitem" | "menu_item" => Self::Menu,
            "tab" => Self::Tab,
            "window" => Self::Window,
            "dialog" | "modal" => Self::Dialog,
            "toolbar" => Self::Toolbar,
            _ => Self::Unknown,
        })
    }
}

impl ElementType {
    /// Parse a string label produced by the vision model into an ElementType.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        <Self as std::str::FromStr>::from_str(s).unwrap()
    }
}

impl std::fmt::Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Button => "button",
            Self::TextInput => "text_input",
            Self::Icon => "icon",
            Self::Link => "link",
            Self::Image => "image",
            Self::Text => "text",
            Self::Checkbox => "checkbox",
            Self::Dropdown => "dropdown",
            Self::Menu => "menu",
            Self::Tab => "tab",
            Self::Window => "window",
            Self::Dialog => "dialog",
            Self::Toolbar => "toolbar",
            Self::Label => "label",
            Self::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

// ============================================================
// Detection Source
// ============================================================

/// Which subsystem produced the element detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionSource {
    /// Vision model (e.g. moondream via OllamaVisionProvider) provided the element data.
    Vision,
    /// OCR (Tesseract) provided the element data via text region bounding boxes.
    Ocr,
    /// Both OCR and Vision contributed to this element.
    Combined,
}

// ============================================================
// ScreenElement
// ============================================================

/// A single detected UI element on the desktop screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenElement {
    /// UI category (button, icon, text_input, etc.)
    #[serde(rename = "type")]
    pub element_type: ElementType,

    /// Human-readable label/name if available (e.g. "Chrome", "Search", "Submit")
    pub label: Option<String>,

    /// Top-left X coordinate in screen pixels
    pub x: i32,

    /// Top-left Y coordinate in screen pixels
    pub y: i32,

    /// Element width in pixels
    pub width: u32,

    /// Element height in pixels
    pub height: u32,

    /// X coordinate of the center of the bounding box
    pub center_x: i32,

    /// Y coordinate of the center of the bounding box
    pub center_y: i32,

    /// Detection confidence in range [0.0, 1.0]
    pub confidence: f32,

    /// Which subsystem produced this detection
    pub source: DetectionSource,

    /// Optional additional description from the model
    pub description: Option<String>,
}

impl ScreenElement {
    /// Construct a ScreenElement from raw bounding-box coordinates.
    /// Automatically computes center_x and center_y.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        element_type: ElementType,
        label: Option<String>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        confidence: f32,
        source: DetectionSource,
    ) -> Self {
        let center_x = x + (width as i32 / 2);
        let center_y = y + (height as i32 / 2);
        Self {
            element_type,
            label,
            x,
            y,
            width,
            height,
            center_x,
            center_y,
            confidence: confidence.clamp(0.0, 1.0),
            source,
            description: None,
        }
    }

    /// Attach an optional description string.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Returns true if the confidence is above the given threshold.
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }

    /// Returns true if this element's label matches the given query (case-insensitive, substring).
    pub fn label_matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        if let Some(label) = &self.label {
            if label.to_lowercase().contains(&q) {
                return true;
            }
        }
        if let Some(desc) = &self.description {
            if desc.to_lowercase().contains(&q) {
                return true;
            }
        }
        false
    }

    /// Returns true if this element's type matches the given query string.
    pub fn type_matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.element_type.to_string().contains(&q)
            || format!("{:?}", self.element_type)
                .to_lowercase()
                .contains(&q)
    }
}

// ============================================================
// Detection Request
// ============================================================

/// A request to detect screen elements in an image.
#[derive(Debug, Clone)]
pub struct ElementDetectionRequest {
    /// Raw PNG bytes of the screenshot
    pub image_bytes: Vec<u8>,

    /// Optional natural-language query to focus detection
    pub query: Option<String>,

    /// Minimum confidence threshold for included elements (default: 0.5)
    pub min_confidence: f32,

    /// Maximum number of elements to return (0 = unlimited)
    pub max_elements: usize,
}

impl ElementDetectionRequest {
    pub fn new(image_bytes: Vec<u8>) -> Self {
        Self {
            image_bytes,
            query: None,
            min_confidence: 0.5,
            max_elements: 50,
        }
    }

    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    pub fn with_min_confidence(mut self, threshold: f32) -> Self {
        self.min_confidence = threshold.clamp(0.0, 1.0);
        self
    }
}

// ============================================================
// Detection Result
// ============================================================

/// The output of a screen-element detection operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDetectionResult {
    /// Detected elements (may be empty)
    pub elements: Vec<ScreenElement>,

    /// The query that was used for filtering (if any)
    pub query: Option<String>,

    /// If the detection model could not provide reliable coordinates,
    /// this field contains the reason rather than fabricated data.
    pub detection_limitation: Option<String>,

    /// The raw prose description from the vision model (for debugging)
    pub raw_description: Option<String>,

    /// Processing latency in milliseconds
    pub latency_ms: u64,
}

impl ElementDetectionResult {
    /// Construct a successful result with detected elements.
    pub fn success(elements: Vec<ScreenElement>, query: Option<String>, latency_ms: u64) -> Self {
        Self {
            elements,
            query,
            detection_limitation: None,
            raw_description: None,
            latency_ms,
        }
    }

    /// Construct a result indicating the model could not provide coordinates.
    pub fn limited(
        reason: impl Into<String>,
        query: Option<String>,
        raw: Option<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            elements: vec![],
            query,
            detection_limitation: Some(reason.into()),
            raw_description: raw,
            latency_ms,
        }
    }

    /// Attach raw description text for debugging.
    pub fn with_raw_description(mut self, raw: impl Into<String>) -> Self {
        self.raw_description = Some(raw.into());
        self
    }

    /// Returns true when at least one element was detected.
    pub fn has_elements(&self) -> bool {
        !self.elements.is_empty()
    }

    /// Returns true when the detection was limited (no reliable coordinates).
    pub fn is_limited(&self) -> bool {
        self.detection_limitation.is_some()
    }

    /// Filter elements matching the given query string (label or type).
    pub fn filtered_by_query(&self, query: &str) -> Vec<&ScreenElement> {
        if query.is_empty() {
            return self.elements.iter().collect();
        }
        self.elements
            .iter()
            .filter(|e| e.label_matches(query) || e.type_matches(query))
            .collect()
    }
}

// ============================================================
// Prompt Construction
// ============================================================

/// Build the vision prompt for structured element detection.
pub fn build_detection_prompt(query: Option<&str>) -> String {
    let focus = match query {
        Some(q) if !q.is_empty() => format!("Focus specifically on finding: {}. ", q),
        _ => String::new(),
    };

    format!(
        "{}Analyze this desktop screenshot and identify all visible UI elements. \
        For each element you can identify, provide: its type (button/icon/text_input/link/text/checkbox/dropdown/menu/tab/window/toolbar/unknown), \
        its label or name if visible, and its approximate pixel coordinates as x, y (top-left corner), width, and height. \
        Also give a confidence score between 0 and 1. \
        Return your answer as a JSON array named 'elements'. Example format: \
        {{\"elements\": [{{\"type\": \"button\", \"label\": \"Submit\", \"x\": 100, \"y\": 200, \"width\": 80, \"height\": 30, \"confidence\": 0.9}}]}}. \
        If you cannot determine reliable pixel coordinates, set x, y, width, height to -1 and explain in a 'limitation' field at the top level. \
        Only include elements you can identify with reasonable confidence.",
        focus
    )
}

// ============================================================
// Response Parsing
// ============================================================

/// Parse the vision model text response into an ElementDetectionResult.
pub fn parse_elements_from_vision_response(
    response_text: &str,
    query: Option<String>,
    latency_ms: u64,
) -> ElementDetectionResult {
    let json_str = extract_json_from_text(response_text);

    if let Some(raw_json) = json_str {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw_json) {
            if let Some(limitation) = parsed.get("limitation").and_then(|v| v.as_str()) {
                return ElementDetectionResult::limited(
                    limitation,
                    query,
                    Some(response_text.to_string()),
                    latency_ms,
                );
            }

            if let Some(arr) = parsed.get("elements").and_then(|v| v.as_array()) {
                let mut elements = Vec::new();
                for item in arr {
                    if let Some(element) = parse_element_from_json(item) {
                        elements.push(element);
                    }
                }

                let all_invalid =
                    !elements.is_empty() && elements.iter().all(|e| e.x == -1 && e.y == -1);

                if all_invalid {
                    return ElementDetectionResult::limited(
                        "Vision model identified elements but could not determine reliable pixel coordinates for this screenshot.",
                        query,
                        Some(response_text.to_string()),
                        latency_ms,
                    );
                }

                let valid_elements: Vec<ScreenElement> = elements
                    .into_iter()
                    .filter(|e| e.x >= 0 && e.y >= 0)
                    .collect();

                if valid_elements.is_empty() && !arr.is_empty() {
                    return ElementDetectionResult::limited(
                        "Vision model identified elements but could not determine reliable pixel coordinates. \
                        For accurate bounding boxes, a dedicated object detection model (e.g. OWL-ViT or YOLO) would be needed.",
                        query,
                        Some(response_text.to_string()),
                        latency_ms,
                    );
                }

                return ElementDetectionResult::success(valid_elements, query, latency_ms)
                    .with_raw_description(response_text.to_string());
            }
        }
    }

    ElementDetectionResult::limited(
        "The vision model returned a text description rather than structured element data with coordinates. \
        For pixel-accurate bounding boxes, a dedicated object detection model would be required.",
        query,
        Some(response_text.to_string()),
        latency_ms,
    )
}

fn extract_json_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            let json_block = text[start + 7..start + 7 + end].trim();
            if serde_json::from_str::<serde_json::Value>(json_block).is_ok() {
                return Some(json_block.to_string());
            }
        }
    }

    if let Some(start) = text.find('{') {
        let mut depth = 0i32;
        let bytes = text.as_bytes();
        let mut end = None;
        for (i, &b) in bytes[start..].iter().enumerate() {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = end {
            let candidate = &text[start..start + e + 1];
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

fn parse_element_from_json(item: &serde_json::Value) -> Option<ScreenElement> {
    let element_type_str = item
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let element_type = ElementType::from_str(element_type_str);

    let label = item
        .get("label")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let x = item.get("x").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
    let y = item.get("y").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
    let width = item.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let height = item.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let confidence = item
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7) as f32;

    let description = item
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut element = ScreenElement::new(
        element_type,
        label,
        x,
        y,
        width,
        height,
        confidence,
        DetectionSource::Vision,
    );

    if let Some(desc) = description {
        element = element.with_description(desc);
    }

    Some(element)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_element_center_calculation() {
        let elem = ScreenElement::new(
            ElementType::Button,
            Some("OK".to_string()),
            100,
            200,
            80,
            30,
            0.9,
            DetectionSource::Vision,
        );
        assert_eq!(elem.center_x, 140);
        assert_eq!(elem.center_y, 215);
    }

    #[test]
    fn test_screen_element_center_at_origin() {
        let elem = ScreenElement::new(
            ElementType::Icon,
            None,
            0,
            0,
            64,
            64,
            0.8,
            DetectionSource::Ocr,
        );
        assert_eq!(elem.center_x, 32);
        assert_eq!(elem.center_y, 32);
    }

    #[test]
    fn test_screen_element_confidence_clamped() {
        let e1 = ScreenElement::new(
            ElementType::Button,
            None,
            0,
            0,
            10,
            10,
            1.5,
            DetectionSource::Vision,
        );
        assert_eq!(e1.confidence, 1.0);
        let e2 = ScreenElement::new(
            ElementType::Button,
            None,
            0,
            0,
            10,
            10,
            -0.5,
            DetectionSource::Vision,
        );
        assert_eq!(e2.confidence, 0.0);
    }

    #[test]
    fn test_screen_element_is_confident() {
        let elem = ScreenElement::new(
            ElementType::Button,
            None,
            0,
            0,
            10,
            10,
            0.85,
            DetectionSource::Vision,
        );
        assert!(elem.is_confident(0.8));
        assert!(!elem.is_confident(0.9));
    }

    #[test]
    fn test_element_type_from_str() {
        assert_eq!(ElementType::from_str("button"), ElementType::Button);
        assert_eq!(ElementType::from_str("btn"), ElementType::Button);
        assert_eq!(ElementType::from_str("icon"), ElementType::Icon);
        assert_eq!(ElementType::from_str("input"), ElementType::TextInput);
        assert_eq!(ElementType::from_str("textbox"), ElementType::TextInput);
        assert_eq!(ElementType::from_str("link"), ElementType::Link);
        assert_eq!(ElementType::from_str("checkbox"), ElementType::Checkbox);
        assert_eq!(ElementType::from_str("dropdown"), ElementType::Dropdown);
        assert_eq!(ElementType::from_str("xyz_unknown"), ElementType::Unknown);
    }

    #[test]
    fn test_label_matches_substring() {
        let elem = ScreenElement::new(
            ElementType::Icon,
            Some("Google Chrome".to_string()),
            0,
            0,
            64,
            64,
            0.9,
            DetectionSource::Vision,
        );
        assert!(elem.label_matches("chrome"));
        assert!(elem.label_matches("Chrome"));
        assert!(elem.label_matches("google"));
        assert!(!elem.label_matches("firefox"));
        assert!(elem.label_matches(""));
    }

    #[test]
    fn test_type_matches() {
        let elem = ScreenElement::new(
            ElementType::Button,
            None,
            0,
            0,
            80,
            30,
            0.9,
            DetectionSource::Vision,
        );
        assert!(elem.type_matches("button"));
        assert!(!elem.type_matches("icon"));
    }

    #[test]
    fn test_element_detection_result_has_elements() {
        let elem = ScreenElement::new(
            ElementType::Button,
            Some("OK".to_string()),
            10,
            10,
            80,
            30,
            0.9,
            DetectionSource::Vision,
        );
        let result = ElementDetectionResult::success(vec![elem], None, 100);
        assert!(result.has_elements());
        assert!(!result.is_limited());
    }

    #[test]
    fn test_element_detection_result_limited() {
        let result = ElementDetectionResult::limited("No coordinates available", None, None, 50);
        assert!(!result.has_elements());
        assert!(result.is_limited());
        assert_eq!(
            result.detection_limitation.as_deref(),
            Some("No coordinates available")
        );
    }

    #[test]
    fn test_element_detection_result_filtered_by_query() {
        let chrome = ScreenElement::new(
            ElementType::Icon,
            Some("Chrome".to_string()),
            0,
            0,
            64,
            64,
            0.9,
            DetectionSource::Vision,
        );
        let notepad = ScreenElement::new(
            ElementType::Icon,
            Some("Notepad".to_string()),
            100,
            0,
            64,
            64,
            0.8,
            DetectionSource::Vision,
        );
        let result = ElementDetectionResult::success(vec![chrome, notepad], None, 100);
        let filtered = result.filtered_by_query("chrome");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label.as_deref(), Some("Chrome"));
    }

    #[test]
    fn test_parse_elements_from_valid_json() {
        let response = r#"{"elements": [{"type": "button", "label": "OK", "x": 100, "y": 200, "width": 80, "height": 30, "confidence": 0.9}]}"#;
        let result = parse_elements_from_vision_response(response, None, 100);
        assert!(result.has_elements());
        assert_eq!(result.elements.len(), 1);
        assert_eq!(result.elements[0].element_type, ElementType::Button);
        assert_eq!(result.elements[0].label.as_deref(), Some("OK"));
        assert_eq!(result.elements[0].x, 100);
        assert_eq!(result.elements[0].y, 200);
        assert_eq!(result.elements[0].center_x, 140);
        assert_eq!(result.elements[0].center_y, 215);
    }

    #[test]
    fn test_parse_elements_from_json_in_prose() {
        let response = r#"Here are the elements I found: {"elements": [{"type": "icon", "label": "Chrome", "x": 50, "y": 1000, "width": 64, "height": 64, "confidence": 0.85}]}"#;
        let result = parse_elements_from_vision_response(response, None, 100);
        assert!(result.has_elements());
        assert_eq!(result.elements[0].label.as_deref(), Some("Chrome"));
    }

    #[test]
    fn test_parse_elements_with_limitation() {
        let response = r#"{"limitation": "I cannot determine pixel coordinates from this image.", "elements": []}"#;
        let result = parse_elements_from_vision_response(response, None, 100);
        assert!(result.is_limited());
        assert!(!result.has_elements());
    }

    #[test]
    fn test_parse_elements_from_prose_only_returns_limited() {
        let response =
            "I can see a Chrome browser window and a taskbar at the bottom with several icons.";
        let result = parse_elements_from_vision_response(response, None, 100);
        assert!(result.is_limited());
        assert!(!result.has_elements());
        assert!(result.raw_description.is_some());
    }

    #[test]
    fn test_parse_elements_all_invalid_coords_returns_limited() {
        let response = r#"{"elements": [{"type": "button", "label": "OK", "x": -1, "y": -1, "width": 0, "height": 0, "confidence": 0.9}]}"#;
        let result = parse_elements_from_vision_response(response, None, 100);
        assert!(result.is_limited());
    }

    #[test]
    fn test_parse_elements_with_query_stored() {
        let response = r#"{"elements": []}"#;
        let result = parse_elements_from_vision_response(
            response,
            Some("find the Chrome icon".to_string()),
            100,
        );
        assert_eq!(result.query.as_deref(), Some("find the Chrome icon"));
    }

    #[test]
    fn test_build_detection_prompt_with_query() {
        let prompt = build_detection_prompt(Some("find the Chrome icon"));
        assert!(prompt.contains("find the Chrome icon"));
        assert!(prompt.contains("elements"));
    }

    #[test]
    fn test_build_detection_prompt_without_query() {
        let prompt = build_detection_prompt(None);
        assert!(prompt.contains("JSON array"));
        assert!(prompt.contains("confidence"));
    }

    #[test]
    fn test_element_detection_request_builder() {
        let req = ElementDetectionRequest::new(vec![1, 2, 3])
            .with_query("find the search box")
            .with_min_confidence(0.7);
        assert_eq!(req.query.as_deref(), Some("find the search box"));
        assert_eq!(req.min_confidence, 0.7);
    }
}
