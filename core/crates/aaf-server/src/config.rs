//! YAML-driven server configuration.
//!
//! Loaded from `aaf.yaml` (or a path passed on the CLI). The schema is
//! intentionally minimal in v0.1 — it covers what the demo binary
//! actually consumes — and forward-compatible because every section is
//! `#[serde(default)]`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level config document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// Project metadata.
    #[serde(default)]
    pub project: ProjectConfig,
    /// Default budget applied to demo runs.
    #[serde(default)]
    pub budget: BudgetSection,
    /// Capabilities to seed into the registry on startup.
    #[serde(default)]
    pub capabilities: Vec<CapabilitySeed>,
    /// Goal text used by the demo run.
    #[serde(default)]
    pub demo: DemoConfig,
}

/// Project metadata block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Display name.
    pub name: String,
    /// Version string.
    pub version: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "aaf-demo".into(),
            version: "0.1.0".into(),
        }
    }
}

/// Budget block consumed by the demo runner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BudgetSection {
    /// Token cap.
    pub max_tokens: u64,
    /// USD cap.
    pub max_cost_usd: f64,
    /// Latency cap (ms).
    pub max_latency_ms: u64,
}

impl Default for BudgetSection {
    fn default() -> Self {
        Self {
            max_tokens: 5_000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
        }
    }
}

/// One seeded capability — minimal subset of the full
/// [`aaf_contracts::CapabilityContract`] needed to bootstrap the demo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySeed {
    /// Capability id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description used by discovery.
    pub description: String,
    /// Domain tags.
    #[serde(default)]
    pub domains: Vec<String>,
    /// Required scope.
    #[serde(default = "default_scope")]
    pub required_scope: String,
}

fn default_scope() -> String {
    "*".into()
}

/// Demo run block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemoConfig {
    /// Goal string handed to the intent compiler.
    pub goal: String,
    /// Domain.
    pub domain: String,
    /// Requester role.
    pub role: String,
    /// Requester scopes.
    pub scopes: Vec<String>,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            goal: "show last month sales by region".into(),
            domain: "sales".into(),
            role: "analyst".into(),
            scopes: vec!["sales:read".into()],
        }
    }
}

/// Errors raised when loading config from disk.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// I/O failure.
    #[error("config io error: {0}")]
    Io(#[from] std::io::Error),
    /// YAML parse failure.
    #[error("config parse error: {0}")]
    Parse(#[from] serde_yaml::Error),
}

impl ServerConfig {
    /// Load YAML config from a file path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path)?;
        let cfg: Self = serde_yaml::from_str(&raw)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r"
project:
  name: aaf-prod
  version: 0.2.0
budget:
  max_tokens: 8000
  max_cost_usd: 2.0
  max_latency_ms: 60000
capabilities:
  - id: cap-sales-monthly
    name: monthly sales
    description: monthly sales report
    domains: [sales]
    required_scope: sales:read
demo:
  goal: show me last month revenue
  domain: sales
  role: analyst
  scopes: [sales:read]
";

    #[test]
    fn parses_full_config() {
        let cfg: ServerConfig = serde_yaml::from_str(SAMPLE).unwrap();
        assert_eq!(cfg.project.name, "aaf-prod");
        assert_eq!(cfg.budget.max_cost_usd, 2.0);
        assert_eq!(cfg.capabilities.len(), 1);
        assert_eq!(cfg.demo.scopes, vec!["sales:read"]);
    }

    #[test]
    fn defaults_round_trip() {
        let cfg = ServerConfig::default();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let back: ServerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(cfg, back);
    }
}
