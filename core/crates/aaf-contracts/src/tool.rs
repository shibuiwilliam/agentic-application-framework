//! Tool contract types for the agentic tool loop (E4).
//!
//! Every tool available to an agent during inference is derived from a
//! registered [`crate::CapabilityContract`] (Rule 25). These types
//! define the wire format between the LLM provider, the runtime, and
//! the policy engine.

use crate::capability::SideEffect;
use crate::ids::CapabilityId;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Tool definition
// ---------------------------------------------------------------------------

/// A tool definition derived from a [`crate::CapabilityContract`].
///
/// Presented to the LLM as a typed tool it may call during inference.
/// The `input_schema` and `output_schema` come directly from the
/// capability contract, ensuring policy enforcement consistency
/// between planned and dynamic tool calls (Rule 25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ToolDefinition {
    /// Human-readable tool name (matches capability name).
    pub name: String,
    /// What the tool does — injected into the LLM prompt.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// JSON Schema for the tool's output.
    #[serde(default)]
    pub output_schema: serde_json::Value,
    /// Side-effect classification — used by the policy engine to gate
    /// tool invocations (Rule 26).
    #[serde(default)]
    pub side_effect: SideEffect,
    /// The backing capability in the registry.
    #[serde(default)]
    pub capability_id: CapabilityId,
}

// ---------------------------------------------------------------------------
// Tool choice
// ---------------------------------------------------------------------------

/// How the LLM should choose tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// LLM decides whether to call a tool.
    Auto,
    /// LLM must not call any tool.
    None,
    /// LLM must call the named tool.
    Specific(String),
}

impl Default for ToolChoice {
    fn default() -> Self {
        Self::Auto
    }
}

// ---------------------------------------------------------------------------
// Stop reason
// ---------------------------------------------------------------------------

/// Why an agent turn ended.
///
/// Recorded in the trace (Rule 12) and used by the agentic loop to
/// decide whether to continue iterating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Agent chose to stop and produce a final answer.
    EndTurn,
    /// Agent wants to call a tool.
    ToolUse,
    /// Output token limit hit.
    MaxTokens,
    /// Budget bound reached (Rule 8).
    BudgetExhausted,
    /// User or system cancelled.
    Cancelled,
}

impl Default for StopReason {
    fn default() -> Self {
        Self::EndTurn
    }
}

// ---------------------------------------------------------------------------
// Tool-use / tool-result blocks
// ---------------------------------------------------------------------------

/// A tool invocation requested by the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// Unique identifier for this tool call (used to correlate with
    /// the result).
    pub id: String,
    /// Name of the tool to call.
    pub name: String,
    /// JSON arguments for the tool.
    #[serde(default)]
    pub input: serde_json::Value,
}

/// The result of executing a tool, sent back to the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// Correlates with [`ToolUseBlock::id`].
    pub tool_use_id: String,
    /// Tool output as a string (may be JSON-encoded).
    pub content: String,
    /// Whether the tool invocation failed.
    #[serde(default)]
    pub is_error: bool,
}

// ---------------------------------------------------------------------------
// Capability → Tool conversion (Rule 25)
// ---------------------------------------------------------------------------

impl From<&crate::capability::CapabilityContract> for ToolDefinition {
    fn from(cap: &crate::capability::CapabilityContract) -> Self {
        Self {
            name: cap.name.clone(),
            description: cap.description.clone(),
            input_schema: cap.input_schema.clone(),
            output_schema: cap.output_schema.clone(),
            side_effect: cap.side_effect,
            capability_id: cap.id.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_round_trip() {
        let td = ToolDefinition {
            name: "check_stock".into(),
            description: "Check product availability".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: serde_json::json!({"type": "object"}),
            side_effect: SideEffect::Read,
            capability_id: CapabilityId::from_raw("cap-stock-check"),
        };
        let json = serde_json::to_string(&td).unwrap();
        let back: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(td, back);
    }

    #[test]
    fn tool_choice_serde() {
        let auto: ToolChoice = serde_json::from_str(r#""auto""#).unwrap();
        assert_eq!(auto, ToolChoice::Auto);

        let none: ToolChoice = serde_json::from_str(r#""none""#).unwrap();
        assert_eq!(none, ToolChoice::None);

        let specific = ToolChoice::Specific("my_tool".into());
        let json = serde_json::to_string(&specific).unwrap();
        let back: ToolChoice = serde_json::from_str(&json).unwrap();
        assert_eq!(specific, back);
    }

    #[test]
    fn stop_reason_default_is_end_turn() {
        assert_eq!(StopReason::default(), StopReason::EndTurn);
    }

    #[test]
    fn stop_reason_serde() {
        let sr: StopReason = serde_json::from_str(r#""tool_use""#).unwrap();
        assert_eq!(sr, StopReason::ToolUse);

        let json = serde_json::to_string(&StopReason::BudgetExhausted).unwrap();
        assert_eq!(json, r#""budget_exhausted""#);
    }

    #[test]
    fn tool_use_block_round_trip() {
        let block = ToolUseBlock {
            id: "tc-1".into(),
            name: "search".into(),
            input: serde_json::json!({"query": "rust"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let back: ToolUseBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(block, back);
    }

    #[test]
    fn tool_result_block_round_trip() {
        let block = ToolResultBlock {
            tool_use_id: "tc-1".into(),
            content: "found 42 results".into(),
            is_error: false,
        };
        let json = serde_json::to_string(&block).unwrap();
        let back: ToolResultBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(block, back);
    }

    #[test]
    fn tool_choice_default_is_auto() {
        assert_eq!(ToolChoice::default(), ToolChoice::Auto);
    }

    #[test]
    fn capability_to_tool_definition() {
        use crate::capability::{
            CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilitySla,
            DataClassification, EndpointKind, SideEffect,
        };
        use crate::ids::CapabilityId;

        let cap = CapabilityContract {
            id: CapabilityId::from_raw("cap-stock-check"),
            name: "check_stock".into(),
            description: "Check availability".into(),
            version: "1.0.0".into(),
            provider_agent: "warehouse-agent".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::InProcess,
                address: String::new(),
                method: None,
            },
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: serde_json::json!({"type": "integer"}),
            side_effect: SideEffect::Read,
            idempotent: true,
            reversible: false,
            deterministic: true,
            compensation: None,
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: "stock:read".into(),
            data_classification: DataClassification::Internal,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec![],
            domains: vec!["warehouse".into()],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        };

        let tool = ToolDefinition::from(&cap);
        assert_eq!(tool.name, "check_stock");
        assert_eq!(tool.description, "Check availability");
        assert_eq!(tool.side_effect, SideEffect::Read);
        assert_eq!(tool.capability_id.as_str(), "cap-stock-check");
        assert_eq!(tool.input_schema, serde_json::json!({"type": "object"}));
    }
}
