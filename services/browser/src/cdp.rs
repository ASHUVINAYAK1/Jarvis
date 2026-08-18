//! Chrome DevTools Protocol (CDP) DOM Automation Client
//!
//! Provides direct HTML DOM element finding, clicking, focusing, text extraction,
//! and attribute inspection via Chrome Remote Debugging (CDP WebSocket / HTTP APIs).

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdpTarget {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: Option<String>,
}

pub struct CdpClient;

impl CdpClient {
    /// Attempts to query Chrome's CDP HTTP target list on the given port (default: 9222).
    pub async fn get_targets(port: u16) -> Result<Vec<CdpTarget>> {
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()?;

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "CDP HTTP list failed with status: {}",
                resp.status()
            ));
        }

        let targets: Vec<CdpTarget> = resp.json().await?;
        Ok(targets)
    }

    /// Finds the active page target (non-newtab page, or matching active URL/title).
    pub async fn get_active_page_target(port: u16) -> Result<CdpTarget> {
        let targets = Self::get_targets(port).await?;
        let pages: Vec<_> = targets
            .into_iter()
            .filter(|t| t.target_type == "page")
            .collect();

        if pages.is_empty() {
            return Err(anyhow!("No active CDP page targets found on port {}", port));
        }

        if let Some(target) = pages
            .iter()
            .find(|p| !p.url.starts_with("chrome://newtab") && !p.url.is_empty())
        {
            return Ok(target.clone());
        }

        Ok(pages[0].clone())
    }

    /// Executes JavaScript DOM evaluation script inside active CDP target via WebSocket.
    pub async fn evaluate_dom_script(
        ws_url: &str,
        query: &str,
        action: &str,
    ) -> Result<serde_json::Value> {
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| anyhow!("CDP WebSocket connection failed: {}", e))?;
        let (mut write, mut read) = ws_stream.split();

        let query_json = serde_json::to_string(query)?;
        let action_json = serde_json::to_string(action)?;

        let js_expression = format!(
            r##"(function() {{
                const query = {};
                const action = {};

                function getVisibleText(el) {{
                    if (!el) return "";
                    return (el.innerText || el.textContent || el.value || el.placeholder || el.getAttribute("aria-label") || el.getAttribute("alt") || "").trim();
                }}

                function getRect(el) {{
                    const r = el.getBoundingClientRect();
                    return {{
                        x: Math.round(r.left + window.scrollX),
                        y: Math.round(r.top + window.scrollY),
                        width: Math.round(r.width),
                        height: Math.round(r.height),
                        center_x: Math.round(r.left + r.width / 2 + window.scrollX),
                        center_y: Math.round(r.top + r.height / 2 + window.scrollY)
                    }};
                }}

                function getAttrs(el) {{
                    const attrs = {{}};
                    if (!el || !el.attributes) return attrs;
                    for (let i = 0; i < el.attributes.length; i++) {{
                        const a = el.attributes[i];
                        attrs[a.name] = a.value;
                    }}
                    attrs["tag"] = el.tagName.toLowerCase();
                    attrs["id"] = el.id || "";
                    attrs["name"] = el.getAttribute("name") || "";
                    attrs["class"] = el.className || "";
                    attrs["type"] = el.getAttribute("type") || "";
                    attrs["role"] = el.getAttribute("role") || "";
                    attrs["aria-label"] = el.getAttribute("aria-label") || "";
                    if (el.href) attrs["href"] = el.href;
                    return attrs;
                }}

                const allElems = Array.from(document.querySelectorAll("*"));
                const queryLower = (query || "").trim().toLowerCase();
                let matches = [];

                if (queryLower.startsWith("#") || queryLower.startsWith(".") || queryLower.startsWith("[")) {{
                    try {{
                        matches = Array.from(document.querySelectorAll(query.trim()));
                    }} catch(e) {{
                        matches = [];
                    }}
                }}

                if (matches.length === 0 && queryLower) {{
                    for (const el of allElems) {{
                        const tag = el.tagName.toLowerCase();
                        if (tag === "script" || tag === "style" || tag === "head" || tag === "html") continue;

                        const text = getVisibleText(el);
                        const id = (el.id || "").toLowerCase();
                        const name = (el.getAttribute("name") || "").toLowerCase();
                        const placeholder = (el.getAttribute("placeholder") || "").toLowerCase();
                        const ariaLabel = (el.getAttribute("aria-label") || "").toLowerCase();

                        const textMatch = text && (text.toLowerCase() === queryLower || text.toLowerCase().includes(queryLower));
                        const idMatch = id && (id === queryLower || id.includes(queryLower));
                        const nameMatch = name && (name === queryLower || name.includes(queryLower));
                        const placeholderMatch = placeholder && placeholder.includes(queryLower);
                        const ariaMatch = ariaLabel && ariaLabel.includes(queryLower);
                        const tagMatch = tag === queryLower;

                        if (textMatch || idMatch || nameMatch || placeholderMatch || ariaMatch || tagMatch) {{
                            matches.push(el);
                        }}
                    }}

                    if (matches.length > 1) {{
                        const interactive = matches.filter(el => {{
                            const t = el.tagName.toLowerCase();
                            return t === "button" || t === "input" || t === "a" || t === "select" || t === "textarea" || t === "h1" || t === "h2" || t === "h3" || t === "p" || el.id || el.getAttribute("name");
                        }});
                        if (interactive.length > 0 && interactive.length < matches.length) {{
                            matches = interactive;
                        }}
                    }}
                }}

                const candidates = matches.map((el, idx) => {{
                    const rect = getRect(el);
                    const text = getVisibleText(el);
                    const tag = el.tagName.toLowerCase();
                    let controlType = "Pane";
                    if (tag === "button" || el.getAttribute("type") === "submit" || el.getAttribute("type") === "button") controlType = "Button";
                    else if (tag === "input" || tag === "textarea") controlType = "Edit";
                    else if (tag === "a") controlType = "Hyperlink";
                    else if (tag.startsWith("h") || tag === "p" || tag === "span" || tag === "label") controlType = "Text";

                    return {{
                        element_id: el.id || ("elem_" + (idx + 1)),
                        tag_name: tag,
                        name: text || el.id || tag,
                        text: text,
                        control_type: controlType,
                        attributes: getAttrs(el),
                        bounds: rect,
                        center_x: rect.center_x,
                        center_y: rect.center_y,
                        enabled: !el.disabled,
                        focused: document.activeElement === el
                    }};
                }});

                if (action === "find") {{
                    return {{
                        success: candidates.length > 0,
                        query: query,
                        match_count: candidates.length,
                        ambiguous: candidates.length > 1,
                        candidates: candidates,
                        element: candidates.length === 1 ? candidates[0] : null,
                        message: candidates.length > 0 ? "Found " + candidates.length + " matching element(s)" : "No DOM element found matching '" + query + "'",
                        latency_ms: 0
                    }};
                }} else if (action === "click") {{
                    if (candidates.length === 0) return {{ success: false, error: "No element found matching '" + query + "'" }};
                    if (candidates.length > 1) return {{ success: false, ambiguous: true, match_count: candidates.length, candidates: candidates, error: "Ambiguous query '" + query + "' matched " + candidates.length + " candidates" }};

                    const targetEl = matches[0];
                    targetEl.focus();
                    targetEl.click();
                    if (targetEl.tagName.toLowerCase() === "button" && targetEl.type === "submit" && targetEl.form) {{
                        targetEl.form.dispatchEvent(new Event("submit", {{ cancelable: true, bubbles: true }}));
                    }}

                    return {{
                        success: true,
                        action: "click",
                        element_id: candidates[0].element_id,
                        tag_name: candidates[0].tag_name,
                        text: getVisibleText(targetEl),
                        attributes: getAttrs(targetEl),
                        bounds: getRect(targetEl),
                        message: "Successfully clicked DOM element matching '" + query + "'"
                    }};
                }} else if (action === "focus") {{
                    if (candidates.length === 0) return {{ success: false, error: "No element found matching '" + query + "'" }};
                    if (candidates.length > 1) return {{ success: false, ambiguous: true, match_count: candidates.length, candidates: candidates, error: "Ambiguous query '" + query + "' matched " + candidates.length + " candidates" }};

                    const targetEl = matches[0];
                    targetEl.focus();
                    return {{
                        success: true,
                        action: "focus",
                        element_id: candidates[0].element_id,
                        tag_name: candidates[0].tag_name,
                        text: getVisibleText(targetEl),
                        attributes: getAttrs(targetEl),
                        bounds: getRect(targetEl),
                        message: "Successfully focused DOM element matching '" + query + "'"
                    }};
                }} else if (action === "get_text") {{
                    if (candidates.length === 0) return {{ success: false, error: "No element found matching '" + query + "'" }};
                    if (candidates.length > 1) return {{ success: false, ambiguous: true, match_count: candidates.length, candidates: candidates, error: "Ambiguous query '" + query + "' matched " + candidates.length + " candidates" }};

                    const targetEl = matches[0];
                    return {{
                        success: true,
                        action: "get_text",
                        element_id: candidates[0].element_id,
                        tag_name: candidates[0].tag_name,
                        text: getVisibleText(targetEl),
                        attributes: getAttrs(targetEl),
                        message: "Element text: '" + getVisibleText(targetEl) + "'"
                    }};
                }} else if (action === "get_attributes") {{
                    if (candidates.length === 0) return {{ success: false, error: "No element found matching '" + query + "'" }};
                    if (candidates.length > 1) return {{ success: false, ambiguous: true, match_count: candidates.length, candidates: candidates, error: "Ambiguous query '" + query + "' matched " + candidates.length + " candidates" }};

                    const targetEl = matches[0];
                    return {{
                        success: true,
                        action: "get_attributes",
                        element_id: candidates[0].element_id,
                        tag_name: candidates[0].tag_name,
                        text: getVisibleText(targetEl),
                        attributes: getAttrs(targetEl),
                        message: "Retrieved DOM element attributes successfully"
                    }};
                }}

                return {{ success: false, error: "Unknown action" }};
            }})()"##,
            query_json, action_json
        );

        let req_id = 1u64;
        let cdp_msg = json!({
            "id": req_id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": js_expression,
                "returnByValue": true,
                "awaitPromise": true
            }
        });

        write.send(Message::Text(cdp_msg.to_string())).await?;

        while let Some(msg_res) = read.next().await {
            let msg = msg_res?;
            if let Message::Text(text) = msg {
                let v: serde_json::Value = serde_json::from_str(&text)?;
                if v.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                    if let Some(val) = v.pointer("/result/result/value") {
                        return Ok(val.clone());
                    }
                    if let Some(err) = v.get("error") {
                        return Err(anyhow!("CDP evaluation error: {}", err));
                    }
                }
            }
        }

        Err(anyhow!("CDP evaluation timed out or websocket closed"))
    }
}
