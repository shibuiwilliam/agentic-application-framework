//! AAF reference server.
//!
//! Subcommands:
//!
//! ```text
//! aaf-server                         # alias for `run`
//! aaf-server run    [path]           # full pipeline: compile → plan → execute
//! aaf-server validate <path>         # validate a YAML config (no execution)
//! aaf-server discover <q>            # ad-hoc capability discovery against the seeded registry
//! aaf-server compile <q>             # compile a goal string into an envelope and dump JSON
//! aaf-server ontology lint <dir>     # lint capability YAMLs for entity declarations
//! aaf-server ontology import <path>  # import an OpenAPI document into proposed ontology YAML
//! aaf-server help                    # show this list
//! ```
//!
//! All subcommands consult `aaf.yaml` (or a path passed as the second
//! arg) for capability seeds and budgets.

mod config;
pub mod identity;
pub mod import;
pub mod lint;

use crate::config::{CapabilitySeed, ConfigError, ServerConfig};
use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, DataClassification, EndpointKind, NodeId, Requester, SideEffect,
};
use aaf_intent::{compiler::CompileOutcome, IntentCompiler};
use aaf_planner::{BoundedAutonomy, CompositionChecker, RegistryPlanner};
use aaf_policy::PolicyEngine;
use aaf_registry::{DiscoveryQuery, Registry};
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceRecorder};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn seed_to_capability(seed: &CapabilitySeed) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(seed.id.as_str()),
        name: seed.name.clone(),
        description: seed.description.clone(),
        version: "1.0.0".into(),
        provider_agent: "seed".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::InProcess,
            address: seed.id.clone(),
            method: None,
        },
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        side_effect: SideEffect::Read,
        idempotent: true,
        reversible: true,
        deterministic: true,
        compensation: None,
        sla: CapabilitySla::default(),
        cost: CapabilityCost::default(),
        required_scope: seed.required_scope.clone(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: seed.domains.clone(),
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn load_config_at(path: &Path) -> Result<ServerConfig, ConfigError> {
    if path.exists() {
        ServerConfig::from_path(path)
    } else {
        eprintln!("(no config file at {:?}, using built-in defaults)", path);
        Ok(ServerConfig::default())
    }
}

async fn seed_registry(cfg: &ServerConfig) -> Result<Arc<Registry>, Box<dyn std::error::Error>> {
    let registry = Arc::new(Registry::in_memory());
    let seeds = if cfg.capabilities.is_empty() {
        vec![CapabilitySeed {
            id: "cap-sales-monthly".into(),
            name: "monthly sales report".into(),
            description: "monthly sales report grouped by region".into(),
            domains: vec!["sales".into()],
            required_scope: "sales:read".into(),
        }]
    } else {
        cfg.capabilities.clone()
    };
    for seed in &seeds {
        registry.register(seed_to_capability(seed)).await?;
    }
    Ok(registry)
}

fn print_help() {
    println!("aaf-server — AAF reference server\n");
    println!("USAGE:");
    println!("  aaf-server                         run pipeline using ./aaf.yaml");
    println!(
        "  aaf-server run    [path]           run pipeline using `path` (default: ./aaf.yaml)"
    );
    println!("  aaf-server validate <path>         parse and validate a YAML config");
    println!("  aaf-server discover <query>        semantic discovery over the seeded registry");
    println!("  aaf-server compile  <text>         compile a goal string and dump the envelope");
    println!("  aaf-server ontology lint <dir>     lint capability YAMLs for entity declarations (E2 Slice C)");
    println!("  aaf-server ontology import <file>  import an OpenAPI document into proposed ontology YAML");
    println!();
    println!(
        "  aaf-server identity generate-did <seed>                        (Wave 2 X1 Slice C)"
    );
    println!(
        "  aaf-server identity sign-manifest <manifest.yaml>              (print signed manifest)"
    );
    println!("  aaf-server identity verify <manifest.yaml>                     (verify signature)");
    println!("  aaf-server identity export-sbom <sbom.yaml> [--format spdx|cyclonedx]");
    println!("  aaf-server identity revoke <did> <reason>                      (issue + print revocation entry)");
    println!();
    println!("  aaf-server help                    show this message");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = match args.first().map(String::as_str) {
        None => ("run", &[][..]),
        Some("run") => ("run", &args[1..]),
        Some("validate") => ("validate", &args[1..]),
        Some("discover") => ("discover", &args[1..]),
        Some("compile") => ("compile", &args[1..]),
        Some("ontology") => ("ontology", &args[1..]),
        Some("identity") => ("identity", &args[1..]),
        Some("help" | "--help" | "-h") => {
            print_help();
            return Ok(());
        }
        // Backwards-compat: bare path → run
        Some(path) if Path::new(path).exists() => ("run", &args[..]),
        Some(other) => {
            eprintln!("unknown subcommand `{other}`\n");
            print_help();
            return Ok(());
        }
    };
    match cmd {
        "validate" => cmd_validate(rest),
        "discover" => cmd_discover(rest).await,
        "compile" => cmd_compile(rest),
        "ontology" => cmd_ontology(rest),
        "identity" => crate::identity::dispatch(rest),
        _ => cmd_run(rest).await,
    }
}

fn cmd_ontology(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("lint") => cmd_ontology_lint(&args[1..]),
        Some("import") => cmd_ontology_import(&args[1..]),
        _ => {
            eprintln!("usage: aaf-server ontology <lint|import> [args]");
            Ok(())
        }
    }
}

fn cmd_ontology_lint(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = args
        .first()
        .map_or_else(|| PathBuf::from("spec/examples"), PathBuf::from);
    if !dir.is_dir() {
        eprintln!("ontology lint: {dir:?} is not a directory");
        return Ok(());
    }
    let report = lint::lint_directory(&dir)?;
    lint::print_report(&report);
    if report.has_errors() {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_ontology_import(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: aaf-server ontology import <openapi.yaml|openapi.json>");
            return Ok(());
        }
    };
    let raw = std::fs::read_to_string(path.as_path())?;
    let proposals = import::import_openapi(&raw)?;
    let out = import::render_yaml(&proposals);
    if let Some(out_path) = args.get(1) {
        std::fs::write(out_path, out)?;
        println!("wrote {} entity proposals to {out_path}", proposals.len());
    } else {
        print!("{out}");
    }
    Ok(())
}

fn cmd_validate(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = args
        .first()
        .map_or_else(|| PathBuf::from("aaf.yaml"), PathBuf::from);
    let cfg = ServerConfig::from_path(&path)?;
    println!("✓ {:?} parses cleanly", path);
    println!(
        "  project       : {} v{}",
        cfg.project.name, cfg.project.version
    );
    println!("  capabilities  : {}", cfg.capabilities.len());
    println!("  budget USD    : {}", cfg.budget.max_cost_usd);
    println!("  budget tokens : {}", cfg.budget.max_tokens);
    Ok(())
}

async fn cmd_discover(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let query = args.join(" ");
    if query.trim().is_empty() {
        eprintln!("usage: aaf-server discover <query>");
        return Ok(());
    }
    let cfg = load_config_at(Path::new("aaf.yaml"))?;
    let registry = seed_registry(&cfg).await?;
    let hits = registry
        .discover(&DiscoveryQuery::new(query.clone()))
        .await?;
    println!("query: {query}");
    if hits.is_empty() {
        println!("(no matches)");
    } else {
        for h in hits {
            println!(
                "  {:.2}  {}  ({})",
                h.score, h.capability.id, h.capability.name
            );
        }
    }
    Ok(())
}

fn cmd_compile(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let goal = args.join(" ");
    if goal.trim().is_empty() {
        eprintln!("usage: aaf-server compile <text>");
        return Ok(());
    }
    let compiler = IntentCompiler::default();
    let outcome = compiler.compile(
        &goal,
        Requester {
            user_id: "cli".into(),
            role: "analyst".into(),
            scopes: vec!["sales:read".into()],
            tenant: None,
        },
        "sales",
        BudgetContract {
            max_tokens: 5_000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
        },
    )?;
    match outcome {
        CompileOutcome::Compiled(env) => {
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        CompileOutcome::NeedsRefinement(qs) => {
            println!("needs refinement:");
            for q in qs {
                println!("  - [{}] {}", q.field, q.prompt);
            }
        }
    }
    Ok(())
}

async fn cmd_run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = args
        .first()
        .map_or_else(|| PathBuf::from("aaf.yaml"), PathBuf::from);
    let cfg = load_config_at(&path)?;
    println!("{} v{} starting", cfg.project.name, cfg.project.version);

    let registry = seed_registry(&cfg).await?;
    println!(
        "registered {} capabilities",
        if cfg.capabilities.is_empty() {
            1
        } else {
            cfg.capabilities.len()
        }
    );

    // ── Compile intent ───────────────────────────────────────────────
    let budget = BudgetContract {
        max_tokens: cfg.budget.max_tokens,
        max_cost_usd: cfg.budget.max_cost_usd,
        max_latency_ms: cfg.budget.max_latency_ms,
    };
    let compiler = IntentCompiler::default();
    let outcome = compiler.compile(
        &cfg.demo.goal,
        Requester {
            user_id: "demo".into(),
            role: cfg.demo.role.clone(),
            scopes: cfg.demo.scopes.clone(),
            tenant: None,
        },
        cfg.demo.domain.clone(),
        budget,
    )?;
    let intent = match outcome {
        CompileOutcome::Compiled(env) => env,
        CompileOutcome::NeedsRefinement(qs) => {
            println!("intent needs refinement:");
            for q in qs {
                println!("  - [{}] {}", q.field, q.prompt);
            }
            return Ok(());
        }
    };
    println!(
        "compiled intent {} ({:?})",
        intent.intent_id, intent.intent_type
    );

    // ── Plan ─────────────────────────────────────────────────────────
    // The bounded-autonomy cap inherits from the configured budget so
    // the planner's check matches the runtime's actual envelope.
    let bounds = BoundedAutonomy {
        max_steps: 10,
        max_depth: 5,
        max_cost_usd: cfg.budget.max_cost_usd,
        max_latency_ms: cfg.budget.max_latency_ms,
    };
    let planner = RegistryPlanner::new(registry.clone(), bounds, CompositionChecker::default());
    let plan = planner.plan(&intent).await?;
    println!("plan: {} step(s)", plan.steps.len());

    // ── Materialise + execute ────────────────────────────────────────
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let mut builder = GraphBuilder::new();
    let mut prev: Option<NodeId> = None;
    for step in &plan.steps {
        let node_id = NodeId::from(step.capability.as_str());
        let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            node_id.clone(),
            SideEffect::Read,
            Arc::new(move |_, _| Ok(serde_json::json!({"rows": 47}))),
        ));
        builder = builder.add_node(node);
        if let Some(p) = prev.take() {
            builder = builder.add_edge(p, node_id.clone());
        }
        prev = Some(node_id);
    }
    let graph = builder.build()?;

    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await?;
    match outcome {
        ExecutionOutcome::Completed { steps, .. } => {
            println!("✓ completed {steps} steps");
        }
        ExecutionOutcome::Partial { steps, reason, .. } => {
            println!("△ partial: {steps} steps, reason: {reason}");
        }
        ExecutionOutcome::PendingApproval { at_step, .. } => {
            println!("⏸ paused for approval at step {at_step}");
        }
        ExecutionOutcome::RolledBack {
            failed_at,
            reason,
            compensated,
        } => {
            println!(
                "↩ rolled back at step {failed_at} ({reason}); compensated {} step(s)",
                compensated.len()
            );
        }
    }

    let trace = recorder.get(&intent.trace_id).await?;
    println!(
        "trace status = {:?}, steps recorded = {}",
        trace.status,
        trace.steps.len()
    );
    println!("done");
    Ok(())
}
