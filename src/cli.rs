use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::cache::{
    cache_diagnostics, cache_links, cache_resources, cache_trace, default_cache_path,
    print_cache_diagnostics, print_cache_links, print_cache_resources, write_cache,
};
use crate::config::load_config;
use crate::diagnostic::print_diagnostics;
use crate::export::to_export;
use crate::generate::{generate_sidecar, parse_generate_link, GenerateLink, GenerateOptions};
use crate::graph::build_graph;
use crate::init::init_missing_sidecars;
use crate::link_edit::{
    add_link, remove_link, update_link, AddLinkOptions, LinkEditOptions, RemoveLinkOptions,
    UpdateLinkOptions,
};
use crate::model::{BuildResult, Diagnostic, Direction, Locator, CONFIG_PATH};
use crate::plugin::configured_plugin_paths;
use crate::repo::list_repo_files;
use crate::skill::install_skill;
use crate::sync::sync_path_hints;
use crate::trace::{trace_from, TraceResult};
use crate::util::{display_path, is_repo_boundary_link, normalize_repo_path};

#[derive(Parser)]
#[command(name = "relaygraph")]
#[command(about = "Build, author, validate, and query Git-backed resource graphs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate sidecars, locators, plugins, and plugin rules.
    Validate {
        /// Print diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Export the resolved graph as JSON.
    Export {
        /// Output path. Defaults to ._relaygraph/generated/relaygraph.json.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Allow overwriting an existing explicit output path.
        #[arg(long)]
        force: bool,
    },
    /// Traverse related resources from an id: or path: locator.
    Trace {
        /// Start locator, for example id:docs.auth or path:docs/auth.md.
        from: String,
        /// Direction to traverse.
        #[arg(long, value_enum, default_value_t = Direction::Both)]
        direction: Direction,
        /// Print structured trace output for AI agents and tooling.
        #[arg(long, conflicts_with = "format")]
        json: bool,
        /// Output format for non-JSON trace output.
        #[arg(long, value_enum, default_value_t = TraceFormat::Relations)]
        format: TraceFormat,
    },
    /// Rebuild or inspect the local SQLite cache.
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    /// Generate missing sidecars for configured requireSidecar patterns.
    Init {
        /// Print files that would be created without writing them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate a sidecar for one discovered resource path.
    Generate {
        /// Existing target resource locator, for example path:src/main.rs.
        target: String,
        /// Resource kind to write into the generated sidecar.
        #[arg(long)]
        kind: Option<String>,
        /// Outgoing link in rel:locator form. May be repeated; targets must resolve before writing.
        #[arg(long, value_parser = parse_generate_link)]
        link: Vec<crate::generate::GenerateLink>,
        /// Print the sidecar path that would be created without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Add, remove, or update links in an existing sidecar.
    Link {
        #[command(subcommand)]
        command: LinkCommands,
    },
    /// Sync derived sidecar fields from canonical resource IDs.
    Sync {
        /// Print sidecars that would be updated without writing them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Install bundled AI-agent assets.
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },
}

#[derive(Subcommand)]
enum LinkCommands {
    /// Add an outgoing link to an existing resource sidecar.
    Add {
        /// Source resource id locator to edit, for example id:docs.root.
        source: String,
        /// Outgoing link in rel:id form.
        #[arg(value_parser = parse_id_link)]
        link: GenerateLink,
        /// Write pathHint resolved from the target id.
        #[arg(long)]
        path_hint: bool,
        /// Optional explicit traversal order.
        #[arg(long)]
        order: Option<i64>,
        /// Print the sidecar path that would be updated without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove an outgoing link from an existing resource sidecar.
    Remove {
        /// Source resource id locator to edit, for example id:docs.root.
        source: String,
        /// Existing outgoing link in rel:id form.
        #[arg(value_parser = parse_id_link)]
        link: GenerateLink,
        /// Print the sidecar path that would be updated without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Update an outgoing link in an existing resource sidecar.
    Update {
        /// Source resource id locator to edit, for example id:docs.root.
        source: String,
        /// Existing outgoing link in rel:id form.
        #[arg(value_parser = parse_id_link)]
        current: GenerateLink,
        /// Replacement outgoing link in rel:id form.
        #[arg(long = "new", value_parser = parse_id_link)]
        new_link: Option<GenerateLink>,
        /// Set or refresh pathHint from the target id.
        #[arg(long)]
        path_hint: bool,
        /// Remove pathHint.
        #[arg(long)]
        clear_path_hint: bool,
        /// Set explicit traversal order.
        #[arg(long)]
        order: Option<i64>,
        /// Remove explicit traversal order.
        #[arg(long)]
        clear_order: bool,
        /// Print the sidecar path that would be updated without writing it.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum CacheCommands {
    /// Rebuild the SQLite cache from Git-backed declarations.
    Rebuild {
        /// Output path. Defaults to ._relaygraph/cache/relaygraph.sqlite.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Allow overwriting an existing explicit output path.
        #[arg(long)]
        force: bool,
    },
    /// List resources from the SQLite cache.
    Resources {
        /// Cache path. Defaults to ._relaygraph/cache/relaygraph.sqlite.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Filter by resource kind.
        #[arg(long)]
        kind: Option<String>,
        /// Print rows as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List links from the SQLite cache.
    Links {
        /// Cache path. Defaults to ._relaygraph/cache/relaygraph.sqlite.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Filter by source locator.
        #[arg(long)]
        from: Option<String>,
        /// Filter by target locator.
        #[arg(long)]
        to: Option<String>,
        /// Filter by relation.
        #[arg(long)]
        rel: Option<String>,
        /// Print rows as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Traverse related resources using only the SQLite cache.
    Trace {
        /// Cache path. Defaults to ._relaygraph/cache/relaygraph.sqlite.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Start locator, for example id:docs.auth or path:docs/auth.md.
        from: String,
        /// Direction to traverse.
        #[arg(long, value_enum, default_value_t = Direction::Both)]
        direction: Direction,
        /// Print structured trace output for AI agents and tooling.
        #[arg(long, conflicts_with = "format")]
        json: bool,
        /// Output format for non-JSON trace output.
        #[arg(long, value_enum, default_value_t = TraceFormat::Relations)]
        format: TraceFormat,
    },
    /// List diagnostics stored in the SQLite cache.
    Diagnostics {
        /// Cache path. Defaults to ._relaygraph/cache/relaygraph.sqlite.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Print rows as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SkillCommands {
    /// Install the bundled RelayGraph Skill into a skills directory.
    Install {
        /// Skills directory where relaygraph/ will be recreated.
        #[arg(long, value_name = "DIR")]
        to: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TraceFormat {
    /// Print direction-aware relation rows.
    Relations,
    /// Print only reachable paths, matching the legacy trace output.
    Paths,
}

pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let command = match cli.command {
        Commands::Skill { command } => return skill_command(command),
        command => command,
    };

    let current_dir = std::env::current_dir().context("failed to resolve current directory")?;
    let root = resolve_repo_root(&current_dir)?;

    if matches!(
        &command,
        Commands::Cache {
            command: CacheCommands::Resources { .. }
                | CacheCommands::Links { .. }
                | CacheCommands::Trace { .. }
                | CacheCommands::Diagnostics { .. },
        }
    ) {
        let Commands::Cache { command } = command else {
            unreachable!();
        };
        return cache_read_command(&root, command);
    }

    let config = match load_config(&root) {
        Ok(config) => config,
        Err(error) => {
            if let Commands::Validate { json: true } = &command {
                let diagnostics = vec![Diagnostic {
                    code: "schema-error",
                    path: Some(CONFIG_PATH.to_string()),
                    message: format!("{error:#}"),
                }];
                println!("{}", serde_json::to_string_pretty(&diagnostics)?);
                return Ok(ExitCode::FAILURE);
            }
            return Err(error);
        }
    };

    match command {
        Commands::Validate { json } => validate_command(&root, &config, json),
        Commands::Export { output, force } => export_command(&root, &config, output, force),
        Commands::Trace {
            from,
            direction,
            json,
            format,
        } => trace_command(&root, &config, &from, direction, json, format),
        Commands::Cache { command } => cache_rebuild_command(&root, &config, command),
        Commands::Init { dry_run } => init_command(&root, &config, dry_run),
        Commands::Generate {
            target,
            kind,
            link,
            dry_run,
        } => generate_command(&root, &config, target, kind, link, dry_run),
        Commands::Link { command } => link_command(&root, &config, command),
        Commands::Sync { dry_run } => sync_command(&root, &config, dry_run),
        Commands::Skill { .. } => unreachable!("skill commands are handled before config loading"),
    }
}

fn skill_command(command: SkillCommands) -> Result<ExitCode> {
    match command {
        SkillCommands::Install { to } => {
            let target = install_skill(&to)?;
            println!("installed relaygraph skill to {}", display_path(&target));
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn validate_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    json: bool,
) -> Result<ExitCode> {
    let graph = match build_graph(root, config) {
        Ok(graph) => graph,
        Err(error) if json => {
            let diagnostics = vec![Diagnostic {
                code: "repo-error",
                path: None,
                message: format!("{error:#}"),
            }];
            println!("{}", serde_json::to_string_pretty(&diagnostics)?);
            return Ok(ExitCode::FAILURE);
        }
        Err(error) => return Err(error),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&graph.diagnostics)?);
    } else {
        print_diagnostics(&graph.diagnostics);
    }
    Ok(if graph.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn resolve_repo_root(current_dir: &std::path::Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(current_dir)
        .output();
    if let Ok(output) = output {
        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !root.is_empty() {
                return Ok(PathBuf::from(root));
            }
        }
    }

    for ancestor in current_dir.ancestors() {
        if ancestor.join(CONFIG_PATH).is_file() {
            return Ok(ancestor.to_path_buf());
        }
    }

    Ok(current_dir.to_path_buf())
}

fn export_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    output: Option<PathBuf>,
    force: bool,
) -> Result<ExitCode> {
    let graph = build_graph(root, config)?;
    let explicit_output = output.is_some();
    let output = output.unwrap_or_else(|| {
        root.join("._relaygraph")
            .join("generated")
            .join("relaygraph.json")
    });
    guard_output(root, config, &graph, &output, force, explicit_output)?;
    let export = to_export(graph);
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", display_path(parent)))?;
    }
    fs::write(&output, serde_json::to_string_pretty(&export)?)
        .with_context(|| format!("failed to write {}", display_path(&output)))?;
    println!("wrote {}", display_path(&output));
    Ok(if export.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn trace_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    from: &str,
    direction: Direction,
    json: bool,
    format: TraceFormat,
) -> Result<ExitCode> {
    let graph = build_graph(root, config)?;
    if !graph.diagnostics.is_empty() {
        print_diagnostics(&graph.diagnostics);
        return Ok(ExitCode::FAILURE);
    }
    let trace = trace_from(&graph.resources, &graph.plugins, from, direction)?;
    print_trace_result(&trace, json, format)?;
    Ok(ExitCode::SUCCESS)
}

fn print_trace_result(trace: &TraceResult, json: bool, format: TraceFormat) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(trace)?);
        return Ok(());
    }

    match format {
        TraceFormat::Paths => {
            for path in trace.paths() {
                println!("{path}");
            }
        }
        TraceFormat::Relations => {
            for node in &trace.nodes {
                let Some(via) = &node.via else {
                    println!("{}", node.path);
                    continue;
                };
                println!("{} --{}--> {}", via.from, via.rel, via.to);
            }
        }
    }
    Ok(())
}

fn guard_output(
    root: &std::path::Path,
    config: &crate::model::Config,
    graph: &BuildResult,
    output: &std::path::Path,
    force: bool,
    require_force_for_existing: bool,
) -> Result<()> {
    let raw_absolute_output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(output)
    };
    reject_boundary_output_path(root, &raw_absolute_output)?;

    let absolute_output = normalize_existing_path_for_comparison(&raw_absolute_output)?;
    let normalized_root = normalize_existing_path_for_comparison(root)?;

    if let Ok(relative) = absolute_output.strip_prefix(&normalized_root) {
        let repo_path = normalize_repo_path(relative.to_string_lossy());
        if protected_repo_paths(root, config, graph)?.contains(&repo_path) {
            anyhow::bail!(
                "refusing to write output into repository declaration or source path {}",
                repo_path
            );
        }
    }

    if require_force_for_existing && absolute_output.exists() && !force {
        anyhow::bail!(
            "refusing to overwrite existing output {}; pass --force to replace it",
            display_path(output)
        );
    }

    Ok(())
}

fn reject_boundary_output_path(root: &std::path::Path, output: &std::path::Path) -> Result<()> {
    let root = normalize_path_components(root);
    let output = normalize_path_components(output);
    let Ok(relative_output) = output.strip_prefix(&root) else {
        return Ok(());
    };

    let mut current = root;
    for component in relative_output.components() {
        current.push(component.as_os_str());
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if is_repo_boundary_link(&metadata) {
            anyhow::bail!(
                "refusing to write output through symlink or reparse point {}",
                display_path(&current)
            );
        }
    }
    Ok(())
}

fn normalize_path_components(path: &std::path::Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn normalize_existing_path_for_comparison(path: &std::path::Path) -> Result<PathBuf> {
    if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", display_path(path)))
    } else {
        Ok(normalize_path_components(path))
    }
}

fn protected_repo_paths(
    root: &std::path::Path,
    config: &crate::model::Config,
    graph: &BuildResult,
) -> Result<std::collections::BTreeSet<String>> {
    let mut paths = std::collections::BTreeSet::new();
    paths.insert(CONFIG_PATH.to_string());
    for plugin in configured_plugin_paths(config) {
        paths.insert(plugin);
    }
    let suffix = crate::config::sidecar_suffix(config);
    for path in list_repo_files(root, config.use_git_ignore.unwrap_or(true))? {
        if path.ends_with(&suffix) {
            paths.insert(path);
        }
    }
    for resource in &graph.resources {
        paths.insert(resource.path.clone());
        if let Some(sidecar) = &resource.sidecar {
            paths.insert(sidecar.clone());
        }
    }
    Ok(paths)
}

fn cache_rebuild_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    command: CacheCommands,
) -> Result<ExitCode> {
    match command {
        CacheCommands::Rebuild { output, force } => {
            let graph = build_graph(root, config)?;
            let explicit_output = output.is_some();
            let output = output.unwrap_or_else(|| root.join(default_cache_path()));
            guard_output(root, config, &graph, &output, force, explicit_output)?;
            write_cache(&output, &graph)?;
            println!("wrote {}", display_path(&output));
            Ok(if graph.diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                print_diagnostics(&graph.diagnostics);
                ExitCode::FAILURE
            })
        }
        _ => unreachable!("read-only cache commands are handled before config loading"),
    }
}

fn cache_read_command(root: &std::path::Path, command: CacheCommands) -> Result<ExitCode> {
    match command {
        CacheCommands::Resources { db, kind, json } => {
            let rows = cache_resources(
                &db.unwrap_or_else(|| root.join(default_cache_path())),
                kind.as_deref(),
            )?;
            print_cache_resources(&rows, json)?;
            Ok(ExitCode::SUCCESS)
        }
        CacheCommands::Links {
            db,
            from,
            to,
            rel,
            json,
        } => {
            let rows = cache_links(
                &db.unwrap_or_else(|| root.join(default_cache_path())),
                from.as_deref(),
                to.as_deref(),
                rel.as_deref(),
            )?;
            print_cache_links(&rows, json)?;
            Ok(ExitCode::SUCCESS)
        }
        CacheCommands::Trace {
            db,
            from,
            direction,
            json,
            format,
        } => {
            let trace = cache_trace(
                &db.unwrap_or_else(|| root.join(default_cache_path())),
                &from,
                direction,
            )?;
            print_trace_result(&trace, json, format)?;
            Ok(ExitCode::SUCCESS)
        }
        CacheCommands::Diagnostics { db, json } => {
            let rows = cache_diagnostics(&db.unwrap_or_else(|| root.join(default_cache_path())))?;
            print_cache_diagnostics(&rows, json)?;
            Ok(if rows.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        CacheCommands::Rebuild { .. } => unreachable!("cache rebuild requires config"),
    }
}

fn init_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    dry_run: bool,
) -> Result<ExitCode> {
    let created = init_missing_sidecars(root, config, dry_run)?;
    for path in &created {
        println!("{path}");
    }
    Ok(ExitCode::SUCCESS)
}

fn generate_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    target: String,
    kind: Option<String>,
    links: Vec<crate::generate::GenerateLink>,
    dry_run: bool,
) -> Result<ExitCode> {
    let created = generate_sidecar(
        root,
        config,
        GenerateOptions {
            target,
            kind,
            links,
            dry_run,
        },
    )?;
    println!("{created}");
    Ok(ExitCode::SUCCESS)
}

fn parse_id_link(value: &str) -> std::result::Result<GenerateLink, String> {
    let link = parse_generate_link(value)?;
    match crate::locator::parse_locator(&link.to)? {
        Locator::Id(_) => Ok(link),
        Locator::Path(_) => Err("link target must use id: locator".to_string()),
    }
}

fn link_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    command: LinkCommands,
) -> Result<ExitCode> {
    let changed = match command {
        LinkCommands::Add {
            source,
            link,
            path_hint,
            order,
            dry_run,
        } => add_link(
            root,
            config,
            AddLinkOptions {
                common: LinkEditOptions { source, dry_run },
                link,
                path_hint,
                order,
            },
        )?,
        LinkCommands::Remove {
            source,
            link,
            dry_run,
        } => remove_link(
            root,
            config,
            RemoveLinkOptions {
                common: LinkEditOptions { source, dry_run },
                link,
            },
        )?,
        LinkCommands::Update {
            source,
            current,
            new_link,
            path_hint,
            clear_path_hint,
            order,
            clear_order,
            dry_run,
        } => update_link(
            root,
            config,
            UpdateLinkOptions {
                common: LinkEditOptions { source, dry_run },
                current,
                new_link,
                path_hint,
                clear_path_hint,
                order,
                clear_order,
            },
        )?,
    };
    println!("{changed}");
    Ok(ExitCode::SUCCESS)
}

fn sync_command(
    root: &std::path::Path,
    config: &crate::model::Config,
    dry_run: bool,
) -> Result<ExitCode> {
    let changed = sync_path_hints(root, config, dry_run)?;
    for path in &changed {
        println!("{path}");
    }
    Ok(ExitCode::SUCCESS)
}
