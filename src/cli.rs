use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::cache::{
    cache_diagnostics, cache_links, cache_resources, cache_trace, default_cache_path,
    print_cache_diagnostics, print_cache_links, print_cache_resources, write_cache,
};
use crate::config::load_config;
use crate::diagnostic::print_diagnostics;
use crate::export::to_export;
use crate::graph::build_graph;
use crate::init::init_missing_sidecars;
use crate::model::{BuildResult, Diagnostic, Direction, CONFIG_PATH};
use crate::plugin::normalize_plugin_repo_path;
use crate::repo::list_repo_files;
use crate::trace::trace_from;
use crate::util::{display_path, is_repo_boundary_link, normalize_repo_path};

#[derive(Parser)]
#[command(name = "relaygraph")]
#[command(about = "Build, validate, and export Git-backed resource graphs")]
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

pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let current_dir = std::env::current_dir().context("failed to resolve current directory")?;
    let root = resolve_repo_root(&current_dir)?;
    let command = cli.command;

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
        Commands::Trace { from, direction } => trace_command(&root, &config, &from, direction),
        Commands::Cache { command } => cache_rebuild_command(&root, &config, command),
        Commands::Init { dry_run } => init_command(&root, &config, dry_run),
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
) -> Result<ExitCode> {
    let graph = build_graph(root, config)?;
    if !graph.diagnostics.is_empty() {
        print_diagnostics(&graph.diagnostics);
        return Ok(ExitCode::FAILURE);
    }
    for path in trace_from(&graph.resources, &graph.plugins, from, direction)? {
        println!("{path}");
    }
    Ok(ExitCode::SUCCESS)
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
    reject_boundary_output_path(&raw_absolute_output)?;

    let absolute_output = normalize_existing_path_for_comparison(&raw_absolute_output)?;
    let normalized_root = normalize_existing_path_for_comparison(root)?;
    reject_boundary_output_path(&absolute_output)?;

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

fn reject_boundary_output_path(output: &std::path::Path) -> Result<()> {
    for path in output.ancestors() {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            continue;
        };
        if is_repo_boundary_link(&metadata) {
            anyhow::bail!(
                "refusing to write output through symlink or reparse point {}",
                display_path(path)
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
    for plugin in config.plugins.as_deref().unwrap_or(&[]) {
        paths.insert(normalize_plugin_repo_path(plugin));
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
        } => {
            for path in cache_trace(
                &db.unwrap_or_else(|| root.join(default_cache_path())),
                &from,
                direction,
            )? {
                println!("{path}");
            }
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
