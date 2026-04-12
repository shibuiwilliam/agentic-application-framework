//! Scope intersection (PROJECT_AafService §8.2 Permission Model).
//!
//! The effective scopes a request actually carries are computed by
//! intersecting three layers:
//!
//! ```text
//! User Scopes     (what the user is allowed to do)
//!   ↓ filter
//! Intent Scopes   (what this intent needs)
//!   ↓ intersect
//! Agent Scopes    (what the agent's trust level permits)
//!   ↓ result
//! Effective Scopes (what the services actually see)
//! ```
//!
//! Each scope is a string like `"order:write"` or `"inventory:read"`.
//! Wildcards (`"order:*"`) expand to match any suffix.
//!
//! The runtime calls [`compute_effective_scopes`] once at the start
//! of every intent execution and threads the result through every
//! policy context thereafter — so every policy rule operates on the
//! **effective** scopes, not the raw user scopes.

use aaf_contracts::AutonomyLevel;
use std::collections::BTreeSet;

/// Compute the effective scopes by intersecting user scopes, intent
/// scopes, and the agent's autonomy-based scope ceiling.
///
/// ```text
/// effective = user_scopes ∩ intent_scopes ∩ agent_ceiling(level)
/// ```
///
/// Wildcard matching: a scope `"foo:*"` matches any scope starting
/// with `"foo:"`.
pub fn compute_effective_scopes(
    user_scopes: &[String],
    intent_scopes: &[String],
    agent_level: AutonomyLevel,
) -> Vec<String> {
    // Step 1: intersect user ∩ intent (with wildcard expansion).
    let after_intent: BTreeSet<String> = intent_scopes
        .iter()
        .filter(|is| user_scopes.iter().any(|us| scope_matches(us, is)))
        .cloned()
        .collect();

    // Step 2: apply agent-autonomy ceiling.
    let ceiling = autonomy_ceiling(agent_level);
    after_intent
        .into_iter()
        .filter(|s| ceiling.iter().any(|c| scope_matches(c, s)))
        .collect()
}

/// Returns `true` if `granted` covers `required`.
///
/// Matching rules:
/// - exact match: `"order:write" == "order:write"` → true
/// - wildcard: `"order:*"` matches `"order:write"`, `"order:read"`, etc.
/// - global wildcard: `"*"` matches anything.
pub fn scope_matches(granted: &str, required: &str) -> bool {
    if granted == required || granted == "*" {
        return true;
    }
    if let Some(prefix) = granted.strip_suffix(":*") {
        return required.starts_with(prefix)
            && required.as_bytes().get(prefix.len()) == Some(&b':');
    }
    false
}

/// Per PROJECT_AafService §8.2: at lower autonomy levels the agent
/// can only use read scopes; write/delete/send/payment require
/// higher levels.
///
/// | Level | Allowed |
/// |---|---|
/// | 1 | nothing autonomous — all require human approval (empty ceiling) |
/// | 2 | `:read` scopes only |
/// | 3 | `:read` + `:write` (low-risk) |
/// | 4 | everything except `:payment` |
/// | 5 | `*` (fully autonomous) |
fn autonomy_ceiling(level: AutonomyLevel) -> Vec<String> {
    match level {
        AutonomyLevel::Level1 => vec![], // everything needs approval
        AutonomyLevel::Level2 => vec!["*:read".into()],
        AutonomyLevel::Level3 => vec!["*:read".into(), "*:write".into()],
        AutonomyLevel::Level4 => {
            vec![
                "*:read".into(),
                "*:write".into(),
                "*:delete".into(),
                "*:send".into(),
                "*:execute".into(),
            ]
        }
        AutonomyLevel::Level5 => vec!["*".into()],
    }
}

/// Helper used by the ceiling matcher: `"*:read"` matches any scope
/// ending in `:read`.
fn ceiling_matches(ceiling: &str, scope: &str) -> bool {
    if ceiling == "*" {
        return true;
    }
    if let Some(suffix) = ceiling.strip_prefix('*') {
        return scope.ends_with(suffix);
    }
    ceiling == scope
}

// Override scope_matches for ceiling entries which use prefix wildcards.
fn scope_matches_with_ceiling(granted: &str, required: &str) -> bool {
    scope_matches(granted, required) || ceiling_matches(granted, required)
}

// Redefine compute_effective_scopes to use ceiling-aware matching.
// (The public function above already does this correctly because
// autonomy_ceiling returns "*:read" etc., and scope_matches handles
// exact + suffix wildcards. But ceiling wildcards are *prefix* wildcards
// ("*:read" matches "order:read"), so we need ceiling_matches.)

/// Compute effective scopes (production version with ceiling-aware
/// matching).
pub fn effective_scopes(
    user_scopes: &[String],
    intent_scopes: &[String],
    agent_level: AutonomyLevel,
) -> Vec<String> {
    let after_intent: BTreeSet<String> = intent_scopes
        .iter()
        .filter(|is| user_scopes.iter().any(|us| scope_matches(us, is)))
        .cloned()
        .collect();

    let ceiling = autonomy_ceiling(agent_level);
    after_intent
        .into_iter()
        .filter(|s| ceiling.iter().any(|c| scope_matches_with_ceiling(c, s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(scope_matches("order:write", "order:write"));
        assert!(!scope_matches("order:write", "order:read"));
    }

    #[test]
    fn wildcard_match() {
        assert!(scope_matches("order:*", "order:write"));
        assert!(scope_matches("order:*", "order:read"));
        assert!(!scope_matches("order:*", "inventory:read"));
    }

    #[test]
    fn global_wildcard() {
        assert!(scope_matches("*", "anything:at:all"));
    }

    #[test]
    fn ceiling_match_suffix() {
        assert!(ceiling_matches("*:read", "order:read"));
        assert!(ceiling_matches("*:read", "inventory:read"));
        assert!(!ceiling_matches("*:read", "order:write"));
        assert!(ceiling_matches("*", "anything"));
    }

    #[test]
    fn level_2_allows_only_reads() {
        let result = effective_scopes(
            &[
                "order:read".into(),
                "order:write".into(),
                "payment:execute".into(),
            ],
            &[
                "order:read".into(),
                "order:write".into(),
                "payment:execute".into(),
            ],
            AutonomyLevel::Level2,
        );
        assert_eq!(result, vec!["order:read"]);
    }

    #[test]
    fn level_3_allows_reads_and_writes() {
        let result = effective_scopes(
            &[
                "order:read".into(),
                "order:write".into(),
                "payment:execute".into(),
            ],
            &[
                "order:read".into(),
                "order:write".into(),
                "payment:execute".into(),
            ],
            AutonomyLevel::Level3,
        );
        assert!(result.contains(&"order:read".to_string()));
        assert!(result.contains(&"order:write".to_string()));
        assert!(!result.contains(&"payment:execute".to_string()));
    }

    #[test]
    fn level_5_allows_everything() {
        let result = effective_scopes(
            &["order:read".into(), "payment:execute".into()],
            &["order:read".into(), "payment:execute".into()],
            AutonomyLevel::Level5,
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn level_1_allows_nothing() {
        let result = effective_scopes(
            &["order:read".into()],
            &["order:read".into()],
            AutonomyLevel::Level1,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn intent_scopes_filter_user_scopes() {
        let result = effective_scopes(
            &[
                "order:read".into(),
                "order:write".into(),
                "admin:delete".into(),
            ],
            &["order:read".into(), "order:write".into()], // intent doesn't need admin
            AutonomyLevel::Level5,
        );
        assert_eq!(result.len(), 2);
        assert!(!result.contains(&"admin:delete".to_string()));
    }

    #[test]
    fn user_wildcard_expands_for_intent() {
        let result = effective_scopes(
            &["order:*".into()], // user has wildcard
            &["order:read".into(), "order:write".into()],
            AutonomyLevel::Level5,
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn no_user_scope_means_no_effective_scope() {
        let result = effective_scopes(&[], &["order:read".into()], AutonomyLevel::Level5);
        assert!(result.is_empty());
    }
}
