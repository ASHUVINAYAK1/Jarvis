//! JARVIS Security Policy Engine
//!
//! Evaluates tool requests against safety rules and autonomy policies
//! before any action touches the operating system.
//!
//! # Architecture
//!
//! ```text
//! ToolRequest
//!     ↓
//! PolicyEngine::evaluate(request, autonomy_level)
//!     ↓
//! PolicyDecision:
//!     ├── Allowed
//!     ├── Denied { reason }
//!     └── ApprovalRequired { reason, timeout_secs }
//! ```
//!
//! The LLM proposes. The Policy authorizes.
//!
//! IMPLEMENTATION STATUS: Phase 11 / Vertical Slice 1 Foundation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{instrument, warn};

/// Autonomy level configuration for JARVIS (0 to 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AutonomyLevel {
    /// Level 0: Chat only — no tool executions allowed.
    Level0ChatOnly = 0,
    /// Level 1: Read-only inspection (system info, file read) — no mutations.
    Level1ReadOnly = 1,
    /// Level 2: Supervised — ask before any mutation or app launch.
    Level2Supervised = 2,
    /// Level 3: Conservative — allow low-risk actions (launch app, read files), ask for consequential ones.
    Level3Conservative = 3,
    /// Level 4: Automatic — allow medium-risk actions, ask only for high-risk (file deletion, network send).
    Level4Automatic = 4,
    /// Level 5: Full autonomy within safety sandbox.
    Level5Full = 5,
}

#[allow(clippy::derivable_impls)]
impl Default for AutonomyLevel {
    fn default() -> Self {
        AutonomyLevel::Level3Conservative
    }
}

/// Inherent risk level of a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Safe, read-only or informational (e.g. get_time, get_system_info).
    Low = 0,
    /// Standard desktop operations (e.g. open_application, take_screenshot).
    Medium = 1,
    /// State-altering actions (e.g. close_application, write_file).
    High = 2,
    /// Irreversible or critical actions (e.g. delete_file, kill_process, credential_use).
    Critical = 3,
}

/// The decision produced by the policy engine for a tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    /// Execution is authorized without human confirmation.
    Allowed,
    /// Execution requires explicit human confirmation.
    ApprovalRequired {
        reason: String,
        suggested_action: String,
    },
    /// Execution is blocked entirely by security policy.
    Denied { reason: String },
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyDecision::Allowed)
    }
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Policy violation: {0}")]
    Violation(String),
    #[error("Invalid policy rule: {0}")]
    InvalidRule(String),
}

/// The policy engine evaluating tool actions.
pub struct PolicyEngine {
    /// Tool risk overrides (tool_name -> RiskLevel)
    risk_overrides: HashMap<String, RiskLevel>,
    /// Explicitly blocked tools
    blocked_tools: Vec<String>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        let mut overrides = HashMap::new();
        // Default classification
        overrides.insert("open_application".to_string(), RiskLevel::Low);
        overrides.insert("take_screenshot".to_string(), RiskLevel::Low);
        overrides.insert("get_clipboard".to_string(), RiskLevel::Low);
        overrides.insert("set_clipboard".to_string(), RiskLevel::Low);
        overrides.insert("show_notification".to_string(), RiskLevel::Low);
        overrides.insert("get_time".to_string(), RiskLevel::Low);
        overrides.insert("get_system_info".to_string(), RiskLevel::Low);
        overrides.insert("browser_status".to_string(), RiskLevel::Low);
        overrides.insert("open_browser".to_string(), RiskLevel::Low);
        overrides.insert("browser_navigate".to_string(), RiskLevel::Low);
        overrides.insert("browser_back".to_string(), RiskLevel::Low);
        overrides.insert("browser_forward".to_string(), RiskLevel::Low);
        overrides.insert("browser_reload".to_string(), RiskLevel::Low);
        overrides.insert("browser_current_page".to_string(), RiskLevel::Low);
        overrides.insert("browser_list_tabs".to_string(), RiskLevel::Low);
        overrides.insert("browser_new_tab".to_string(), RiskLevel::Low);
        overrides.insert("browser_switch_tab".to_string(), RiskLevel::Low);
        overrides.insert("browser_close_tab".to_string(), RiskLevel::Low);
        overrides.insert("close_application".to_string(), RiskLevel::Medium);
        overrides.insert("delete_file".to_string(), RiskLevel::Critical);

        Self {
            risk_overrides: overrides,
            blocked_tools: Vec::new(),
        }
    }

    /// Set risk level for a tool.
    pub fn set_tool_risk(&mut self, tool_name: &str, risk: RiskLevel) {
        self.risk_overrides.insert(tool_name.to_lowercase(), risk);
    }

    /// Block a tool completely.
    pub fn block_tool(&mut self, tool_name: &str) {
        self.blocked_tools.push(tool_name.to_lowercase());
    }

    /// Evaluate if a tool request is permitted under the given autonomy level.
    #[instrument(skip(self), fields(tool = %tool_name, autonomy = ?autonomy))]
    pub fn evaluate(
        &self,
        tool_name: &str,
        default_risk: RiskLevel,
        autonomy: AutonomyLevel,
    ) -> PolicyDecision {
        let normalized = tool_name.to_lowercase();

        // 1. Check if tool is explicitly blocked
        if self.blocked_tools.contains(&normalized) {
            warn!(tool = %tool_name, "Tool is explicitly blocked by policy");
            return PolicyDecision::Denied {
                reason: format!("Tool '{}' is blocked by security policy", tool_name),
            };
        }

        // 2. Determine effective risk level
        let effective_risk = self
            .risk_overrides
            .get(&normalized)
            .copied()
            .unwrap_or(default_risk);

        // 3. Evaluate against autonomy level
        match autonomy {
            AutonomyLevel::Level0ChatOnly => PolicyDecision::Denied {
                reason: "Autonomy is set to Level 0 (Chat Only). All tool executions are disabled."
                    .to_string(),
            },
            AutonomyLevel::Level1ReadOnly => {
                if effective_risk == RiskLevel::Low {
                    PolicyDecision::Allowed
                } else {
                    PolicyDecision::Denied {
                        reason: format!(
                            "Autonomy Level 1 allows read-only operations only. Tool '{}' requires Level 2+.",
                            tool_name
                        ),
                    }
                }
            }
            AutonomyLevel::Level2Supervised => {
                if effective_risk == RiskLevel::Low {
                    PolicyDecision::Allowed
                } else {
                    PolicyDecision::ApprovalRequired {
                        reason: format!(
                            "Autonomy Level 2 requires user confirmation for '{}'.",
                            tool_name
                        ),
                        suggested_action: format!("Execute tool '{}'", tool_name),
                    }
                }
            }
            AutonomyLevel::Level3Conservative => match effective_risk {
                RiskLevel::Low | RiskLevel::Medium => PolicyDecision::Allowed,
                RiskLevel::High | RiskLevel::Critical => PolicyDecision::ApprovalRequired {
                    reason: format!(
                        "Tool '{}' is classified as {:?} risk and requires user authorization.",
                        tool_name, effective_risk
                    ),
                    suggested_action: format!("Execute high-risk tool '{}'", tool_name),
                },
            },
            AutonomyLevel::Level4Automatic => match effective_risk {
                RiskLevel::Low | RiskLevel::Medium | RiskLevel::High => PolicyDecision::Allowed,
                RiskLevel::Critical => PolicyDecision::ApprovalRequired {
                    reason: format!(
                        "Critical action '{}' requires explicit confirmation.",
                        tool_name
                    ),
                    suggested_action: format!("Authorize critical action '{}'", tool_name),
                },
            },
            AutonomyLevel::Level5Full => {
                if effective_risk == RiskLevel::Critical {
                    PolicyDecision::ApprovalRequired {
                        reason: format!(
                            "Critical action '{}' safety barrier requires confirmation.",
                            tool_name
                        ),
                        suggested_action: format!("Authorize critical action '{}'", tool_name),
                    }
                } else {
                    PolicyDecision::Allowed
                }
            }
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_application_allowed_at_level_3() {
        let policy = PolicyEngine::new();
        let decision = policy.evaluate(
            "open_application",
            RiskLevel::Low,
            AutonomyLevel::Level3Conservative,
        );
        assert_eq!(decision, PolicyDecision::Allowed);
    }

    #[test]
    fn test_tool_blocked_at_level_0() {
        let policy = PolicyEngine::new();
        let decision = policy.evaluate(
            "open_application",
            RiskLevel::Low,
            AutonomyLevel::Level0ChatOnly,
        );
        assert!(matches!(decision, PolicyDecision::Denied { .. }));
    }

    #[test]
    fn test_critical_tool_requires_approval() {
        let policy = PolicyEngine::new();
        let decision = policy.evaluate(
            "delete_file",
            RiskLevel::Critical,
            AutonomyLevel::Level4Automatic,
        );
        assert!(matches!(decision, PolicyDecision::ApprovalRequired { .. }));
    }

    #[test]
    fn test_explicitly_blocked_tool() {
        let mut policy = PolicyEngine::new();
        policy.block_tool("bad_tool");
        let decision = policy.evaluate("bad_tool", RiskLevel::Low, AutonomyLevel::Level5Full);
        assert!(matches!(decision, PolicyDecision::Denied { .. }));
    }
}
