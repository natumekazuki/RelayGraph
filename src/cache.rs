use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::locator::parse_locator;
use crate::model::{BuildResult, Direction, Locator, CACHE_SCHEMA_VERSION};
use crate::plugin::build_relation_rank;
use crate::util::{display_path, normalize_repo_path_strict};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheResource {
    path: String,
    id: Option<String>,
    kind: Option<String>,
    sidecar: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CacheLink {
    source_path: String,
    rel: String,
    target_locator: String,
    target_path: Option<String>,
    target_id: Option<String>,
    #[serde(skip_serializing)]
    relation_rank: Option<i64>,
    order: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheDiagnostic {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
}

pub fn default_cache_path() -> PathBuf {
    PathBuf::from("._relaygraph")
        .join("cache")
        .join("relaygraph.sqlite")
}

pub fn write_cache(path: &Path, graph: &BuildResult) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", display_path(parent)))?;
    }

    let temp_path = tempfile::Builder::new()
        .prefix(".relaygraph-cache-")
        .suffix(".sqlite")
        .tempfile_in(parent.unwrap_or_else(|| Path::new(".")))
        .with_context(|| {
            format!(
                "failed to create temporary sqlite cache near {}",
                display_path(path)
            )
        })?
        .into_temp_path();

    write_cache_file(temp_path.as_ref(), graph)?;

    replace_completed_cache(temp_path.as_ref(), path)?;

    Ok(())
}

fn write_cache_file(path: &Path, graph: &BuildResult) -> Result<()> {
    let mut connection = Connection::open(path)
        .with_context(|| format!("failed to open sqlite cache {}", display_path(path)))?;
    let transaction = connection.transaction()?;

    transaction.execute_batch(include_str!("../docs/schema/cache-schema.sql"))?;
    transaction.pragma_update(None, "user_version", CACHE_SCHEMA_VERSION)?;
    transaction.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        params!["cacheSchemaVersion", CACHE_SCHEMA_VERSION.to_string()],
    )?;

    for plugin in &graph.plugins {
        transaction.execute(
            "INSERT INTO plugins (name, traversal_json) VALUES (?1, ?2)",
            params![
                plugin.name.as_str(),
                serde_json::to_string(&plugin.traversal)
                    .context("failed to serialize plugin traversal")?
            ],
        )?;
    }

    let relation_rank = build_relation_rank(&graph.plugins);

    for resource in &graph.resources {
        transaction.execute(
            r#"
            INSERT INTO resources (path, id, kind, sidecar, metadata_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                resource.path.as_str(),
                resource.id.as_deref(),
                resource.kind.as_deref(),
                resource.sidecar.as_deref(),
                serde_json::to_string(&resource.metadata)
                    .context("failed to serialize resource metadata")?
            ],
        )?;

        for link in &resource.links {
            transaction.execute(
                r#"
                INSERT INTO links (
                    source_path,
                    rel,
                    target_locator,
                    target_path,
                    target_id,
                    relation_rank,
                    link_order
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    resource.path.as_str(),
                    link.rel.as_str(),
                    link.to.as_str(),
                    link.target_path.as_deref(),
                    link.target_id.as_deref(),
                    relation_rank
                        .get(link.rel.as_str())
                        .map(|rank| *rank as i64),
                    link.order
                ],
            )?;
        }
    }

    for diagnostic in &graph.diagnostics {
        transaction.execute(
            "INSERT INTO diagnostics (code, path, message) VALUES (?1, ?2, ?3)",
            params![
                diagnostic.code,
                diagnostic.path.as_deref(),
                diagnostic.message.as_str()
            ],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

#[cfg(windows)]
fn replace_completed_cache(temp_path: &Path, path: &Path) -> Result<()> {
    if !path.exists() {
        fs::rename(temp_path, path).with_context(|| {
            format!(
                "failed to move sqlite cache {} to {}",
                display_path(temp_path),
                display_path(path)
            )
        })?;
        return Ok(());
    }

    let replaced_wide = wide_path(path);
    let replacement_wide = wide_path(temp_path);
    let ok = unsafe {
        ReplaceFileW(
            replaced_wide.as_ptr(),
            replacement_wide.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "failed to replace sqlite cache {} with {}",
                display_path(path),
                display_path(temp_path)
            )
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_completed_cache(temp_path: &Path, path: &Path) -> Result<()> {
    fs::rename(temp_path, path).with_context(|| {
        format!(
            "failed to replace sqlite cache {} with {}",
            display_path(path),
            display_path(temp_path)
        )
    })
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
#[link(name = "Kernel32")]
extern "system" {
    fn ReplaceFileW(
        lp_replaced_file_name: *const u16,
        lp_replacement_file_name: *const u16,
        lp_backup_file_name: *const u16,
        dw_replace_flags: u32,
        lp_exclude: *mut std::ffi::c_void,
        lp_reserved: *mut std::ffi::c_void,
    ) -> i32;
}

pub fn cache_resources(path: &Path, kind: Option<&str>) -> Result<Vec<CacheResource>> {
    let connection = open_cache(path)?;
    let mut statement = connection.prepare(
        r#"
        SELECT path, id, kind, sidecar
        FROM resources
        WHERE (?1 IS NULL OR kind = ?1)
        ORDER BY path
        "#,
    )?;
    let rows = statement.query_map(params![kind], |row| {
        Ok(CacheResource {
            path: row.get(0)?,
            id: row.get(1)?,
            kind: row.get(2)?,
            sidecar: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read cache resources")
}

pub fn cache_links(
    path: &Path,
    from: Option<&str>,
    to: Option<&str>,
    rel: Option<&str>,
) -> Result<Vec<CacheLink>> {
    let connection = open_cache(path)?;
    let from_path = from
        .map(|locator| resolve_cache_resource_path(&connection, locator))
        .transpose()?;
    let target_locator = to
        .map(parse_locator)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let target_path_from_locator = to
        .map(|locator| resolve_cache_resource_path_optional(&connection, locator))
        .transpose()?
        .flatten();
    let mut links = read_cache_links(&connection)?;

    links.retain(|link| {
        if from_path
            .as_deref()
            .is_some_and(|path| link.source_path != path)
        {
            return false;
        }
        if rel.is_some_and(|rel| link.rel != rel) {
            return false;
        }
        match &target_locator {
            Some(Locator::Id(id)) => {
                link.target_id.as_deref() == Some(id.as_str())
                    || target_path_from_locator
                        .as_deref()
                        .is_some_and(|path| link.target_path.as_deref() == Some(path))
            }
            Some(Locator::Path(path)) => {
                let Ok(normalized) = normalize_repo_path_strict(path) else {
                    return false;
                };
                link.target_path.as_deref() == Some(normalized.as_str())
                    || cache_link_target_path(link).as_deref() == Some(normalized.as_str())
            }
            None => true,
        }
    });

    Ok(links)
}

fn cache_link_target_path(link: &CacheLink) -> Option<String> {
    match parse_locator(&link.target_locator).ok()? {
        Locator::Path(path) => normalize_repo_path_strict(&path).ok(),
        Locator::Id(_) => None,
    }
}

pub fn cache_trace(path: &Path, from: &str, direction: Direction) -> Result<Vec<String>> {
    let connection = open_cache(path)?;
    let start_path = resolve_cache_resource_path(&connection, from)?;
    let links = read_cache_links(&connection)?;
    let mut by_source = BTreeMap::<String, Vec<CacheLink>>::new();
    for link in links {
        if matches!(direction, Direction::Outgoing | Direction::Both) {
            by_source
                .entry(link.source_path.clone())
                .or_default()
                .push(link.clone());
        }
        if matches!(direction, Direction::Incoming | Direction::Both) {
            if let Some(target_path) = &link.target_path {
                by_source
                    .entry(target_path.clone())
                    .or_default()
                    .push(CacheLink {
                        source_path: target_path.clone(),
                        rel: link.rel.clone(),
                        target_locator: format!("path:{}", link.source_path),
                        target_path: Some(link.source_path.clone()),
                        target_id: None,
                        relation_rank: link.relation_rank,
                        order: link.order,
                    });
            }
        }
    }
    for links in by_source.values_mut() {
        sort_cache_links(links);
    }

    let mut visited = BTreeSet::new();
    let mut pending = vec![start_path];
    let mut ordered = Vec::new();

    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }
        ordered.push(path.clone());

        let mut next = by_source
            .get(path.as_str())
            .into_iter()
            .flat_map(|links| links.iter())
            .filter_map(|link| link.target_path.clone())
            .collect::<Vec<_>>();
        next.reverse();
        pending.extend(next);
    }

    Ok(ordered)
}

fn sort_cache_links(links: &mut [CacheLink]) {
    links.sort_by(|left, right| {
        (
            left.order.unwrap_or(i64::MAX),
            left.relation_rank.unwrap_or(i64::MAX),
            &left.rel,
            &left.target_locator,
        )
            .cmp(&(
                right.order.unwrap_or(i64::MAX),
                right.relation_rank.unwrap_or(i64::MAX),
                &right.rel,
                &right.target_locator,
            ))
    });
}

pub fn cache_diagnostics(path: &Path) -> Result<Vec<CacheDiagnostic>> {
    let connection = open_cache(path)?;
    let mut statement = connection.prepare(
        r#"
        SELECT code, path, message
        FROM diagnostics
        ORDER BY code, path, message
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(CacheDiagnostic {
            code: row.get(0)?,
            path: row.get(1)?,
            message: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read cache diagnostics")
}

pub fn print_cache_resources(rows: &[CacheResource], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
        return Ok(());
    }

    for row in rows {
        println!(
            "{} id={} kind={}",
            row.path,
            row.id.as_deref().unwrap_or("-"),
            row.kind.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

pub fn print_cache_links(rows: &[CacheLink], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
        return Ok(());
    }

    for row in rows {
        println!(
            "{} --{}--> {}",
            row.source_path, row.rel, row.target_locator
        );
    }
    Ok(())
}

pub fn print_cache_diagnostics(rows: &[CacheDiagnostic], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("ok");
        return Ok(());
    }

    for row in rows {
        match row.path.as_deref() {
            Some(path) => println!("{} {}: {}", row.code, path, row.message),
            None => println!("{}: {}", row.code, row.message),
        }
    }
    Ok(())
}

fn open_cache(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| {
            format!(
                "failed to open sqlite cache {}; run `relaygraph cache rebuild` first",
                display_path(path)
            )
        })?;
    validate_cache_schema(&connection, path)?;
    Ok(connection)
}

fn validate_cache_schema(connection: &Connection, path: &Path) -> Result<()> {
    ensure_cache_integrity(connection, path)?;
    ensure_cache_user_version(connection, path)?;
    ensure_cache_tables(connection, path)?;
    ensure_cache_indexes(connection, path)?;
    ensure_cache_foreign_keys(connection, path)?;

    let version = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'cacheSchemaVersion'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .with_context(|| {
            format!(
                "failed to read cache metadata from {}; run `relaygraph cache rebuild`",
                display_path(path)
            )
        })?;

    let Some(version) = version else {
        anyhow::bail!(
            "cache metadata is missing in {}; run `relaygraph cache rebuild`",
            display_path(path)
        );
    };
    let parsed = version.parse::<u32>().with_context(|| {
        format!(
            "invalid cacheSchemaVersion {version:?} in {}; run `relaygraph cache rebuild`",
            display_path(path)
        )
    })?;
    if parsed != CACHE_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported cacheSchemaVersion {parsed}; expected {CACHE_SCHEMA_VERSION}; run `relaygraph cache rebuild`"
        );
    }
    Ok(())
}

fn ensure_cache_integrity(connection: &Connection, path: &Path) -> Result<()> {
    let integrity = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .with_context(|| {
            format!(
                "failed to verify sqlite cache integrity for {}; run `relaygraph cache rebuild`",
                display_path(path)
            )
        })?;
    if integrity != "ok" {
        anyhow::bail!(
            "sqlite cache integrity check failed for {}: {}; run `relaygraph cache rebuild`",
            display_path(path),
            integrity
        );
    }
    Ok(())
}

fn ensure_cache_user_version(connection: &Connection, path: &Path) -> Result<()> {
    let user_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
        .with_context(|| {
            format!(
                "failed to read sqlite cache user_version from {}; run `relaygraph cache rebuild`",
                display_path(path)
            )
        })?;
    if user_version != CACHE_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported sqlite cache user_version {user_version}; expected {CACHE_SCHEMA_VERSION}; run `relaygraph cache rebuild`"
        );
    }
    Ok(())
}

fn ensure_cache_tables(connection: &Connection, path: &Path) -> Result<()> {
    const TABLES: &[(&str, &[&str])] = &[
        ("metadata", &["key", "value"]),
        ("plugins", &["name", "traversal_json"]),
        (
            "resources",
            &["path", "id", "kind", "sidecar", "metadata_json"],
        ),
        (
            "links",
            &[
                "source_path",
                "rel",
                "target_locator",
                "target_path",
                "target_id",
                "relation_rank",
                "link_order",
            ],
        ),
        ("diagnostics", &["code", "path", "message"]),
    ];

    for (table, columns) in TABLES {
        let found = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get::<_, i64>(0),
            )
            .with_context(|| {
                format!(
                    "failed to inspect sqlite cache tables in {}; run `relaygraph cache rebuild`",
                    display_path(path)
                )
            })?;
        if found != 1 {
            anyhow::bail!(
                "sqlite cache table {table} is missing in {}; run `relaygraph cache rebuild`",
                display_path(path)
            );
        }

        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .with_context(|| {
                format!(
                    "failed to inspect sqlite cache table {table} in {}; run `relaygraph cache rebuild`",
                    display_path(path)
                )
            })?;
        let existing = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()
            .with_context(|| {
                format!(
                    "failed to read sqlite cache columns for {table} in {}; run `relaygraph cache rebuild`",
                    display_path(path)
                )
            })?;
        for column in *columns {
            if !existing.contains(*column) {
                anyhow::bail!(
                    "sqlite cache column {table}.{column} is missing in {}; run `relaygraph cache rebuild`",
                    display_path(path)
                );
            }
        }
    }
    Ok(())
}

fn ensure_cache_indexes(connection: &Connection, path: &Path) -> Result<()> {
    const INDEXES: &[&str] = &[
        "links_source_path_idx",
        "links_target_path_idx",
        "links_target_id_idx",
        "resources_id_idx",
        "resources_kind_idx",
    ];

    for index in INDEXES {
        let found = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                params![index],
                |row| row.get::<_, i64>(0),
            )
            .with_context(|| {
                format!(
                    "failed to inspect sqlite cache indexes in {}; run `relaygraph cache rebuild`",
                    display_path(path)
                )
            })?;
        if found != 1 {
            anyhow::bail!(
                "sqlite cache index {index} is missing in {}; run `relaygraph cache rebuild`",
                display_path(path)
            );
        }
    }
    Ok(())
}

fn ensure_cache_foreign_keys(connection: &Connection, path: &Path) -> Result<()> {
    let links_foreign_key = connection
        .prepare("PRAGMA foreign_key_list(links)")
        .and_then(|mut statement| {
            let keys = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(keys.into_iter().any(|(table, from, to)| {
                table == "resources" && from == "source_path" && to == "path"
            }))
        })
        .with_context(|| {
            format!(
                "failed to inspect sqlite cache foreign key definitions in {}; run `relaygraph cache rebuild`",
                display_path(path)
            )
        })?;
    if !links_foreign_key {
        anyhow::bail!(
            "sqlite cache foreign key links.source_path -> resources.path is missing in {}; run `relaygraph cache rebuild`",
            display_path(path)
        );
    }

    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .with_context(|| {
            format!(
                "failed to inspect sqlite cache foreign keys in {}; run `relaygraph cache rebuild`",
                display_path(path)
            )
        })?;
    let mut rows = statement.query([])?;
    if rows.next()?.is_some() {
        anyhow::bail!(
            "sqlite cache foreign key check failed in {}; run `relaygraph cache rebuild`",
            display_path(path)
        );
    }
    Ok(())
}

fn resolve_cache_resource_path(connection: &Connection, locator: &str) -> Result<String> {
    resolve_cache_resource_path_optional(connection, locator)?
        .with_context(|| format!("unknown cache resource locator: {locator}"))
}

fn resolve_cache_resource_path_optional(
    connection: &Connection,
    locator: &str,
) -> Result<Option<String>> {
    match parse_locator(locator).map_err(anyhow::Error::msg)? {
        Locator::Id(id) => {
            let mut statement =
                connection.prepare("SELECT path FROM resources WHERE id = ?1 ORDER BY path")?;
            let paths = statement
                .query_map(params![id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            match paths.as_slice() {
                [path] => Ok(Some(path.clone())),
                [] => Ok(None),
                _ => anyhow::bail!("ambiguous cache resource id: {locator}"),
            }
        }
        Locator::Path(path) => {
            let path = normalize_repo_path_strict(&path).map_err(anyhow::Error::msg)?;
            match connection.query_row(
                "SELECT path FROM resources WHERE path = ?1",
                params![path],
                |row| row.get(0),
            ) {
                Ok(path) => Ok(Some(path)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error)
                    .with_context(|| format!("failed to resolve cache resource path: {locator}")),
            }
        }
    }
}

fn read_cache_links(connection: &Connection) -> Result<Vec<CacheLink>> {
    let mut statement = connection.prepare(
        r#"
        SELECT source_path, rel, target_locator, target_path, target_id, relation_rank, link_order
        FROM links
        ORDER BY COALESCE(link_order, 9223372036854775807),
                 COALESCE(relation_rank, 9223372036854775807),
                 rel,
                 source_path,
                 target_locator
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(CacheLink {
            source_path: row.get(0)?,
            rel: row.get(1)?,
            target_locator: row.get(2)?,
            target_path: row.get(3)?,
            target_id: row.get(4)?,
            relation_rank: row.get(5)?,
            order: row.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read cache links")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BuildResult, Resource};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn failed_cache_write_preserves_existing_cache() {
        let root = temp_root("relaygraph-cache-atomic");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");

        write_cache(&cache_path, &empty_graph()).unwrap();
        assert!(cache_resources(&cache_path, None).unwrap().is_empty());

        let result = write_cache(&cache_path, &graph_with_non_json_metadata());

        assert!(result.is_err());
        assert!(cache_resources(&cache_path, None).unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_write_does_not_delete_existing_temp_like_files() {
        let root = temp_root("relaygraph-cache-temp-preserve");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");
        let existing_temp = root.join(".relaygraph-cache-existing.sqlite");
        fs::write(&existing_temp, "do not delete").unwrap();

        write_cache(&cache_path, &empty_graph()).unwrap();

        assert_eq!(fs::read_to_string(&existing_temp).unwrap(), "do not delete");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_schema_validation_rejects_incomplete_schema() {
        let root = temp_root("relaygraph-cache-incomplete-schema");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");
        let connection = Connection::open(&cache_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA user_version = 1;
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                );
                INSERT INTO metadata (key, value) VALUES ('cacheSchemaVersion', '1');
                "#,
            )
            .unwrap();
        drop(connection);

        let result = cache_resources(&cache_path, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("run `relaygraph cache rebuild`"));
        assert!(message.contains("sqlite cache table plugins is missing"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_schema_validation_rejects_wrong_user_version() {
        let root = temp_root("relaygraph-cache-user-version");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");

        write_cache(&cache_path, &empty_graph()).unwrap();
        let connection = Connection::open(&cache_path).unwrap();
        connection.pragma_update(None, "user_version", 999).unwrap();
        drop(connection);

        let result = cache_resources(&cache_path, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("unsupported sqlite cache user_version 999"));
        assert!(message.contains("run `relaygraph cache rebuild`"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_schema_validation_rejects_missing_index() {
        let root = temp_root("relaygraph-cache-missing-index");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");

        write_cache(&cache_path, &empty_graph()).unwrap();
        let connection = Connection::open(&cache_path).unwrap();
        connection
            .execute_batch("DROP INDEX links_source_path_idx;")
            .unwrap();
        drop(connection);

        let result = cache_resources(&cache_path, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("sqlite cache index links_source_path_idx is missing"));
        assert!(message.contains("run `relaygraph cache rebuild`"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_schema_validation_rejects_foreign_key_violations() {
        let root = temp_root("relaygraph-cache-foreign-key");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");

        write_cache(&cache_path, &empty_graph()).unwrap();
        let connection = Connection::open(&cache_path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        connection
            .execute(
                "INSERT INTO links (source_path, rel, target_locator) VALUES (?1, ?2, ?3)",
                params!["missing.md", "x", "path:target.md"],
            )
            .unwrap();
        drop(connection);

        let result = cache_resources(&cache_path, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("sqlite cache foreign key check failed"));
        assert!(message.contains("run `relaygraph cache rebuild`"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_schema_validation_rejects_missing_foreign_key_definition() {
        let root = temp_root("relaygraph-cache-missing-foreign-key");
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("relaygraph.sqlite");
        let connection = Connection::open(&cache_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA user_version = 1;
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                );
                INSERT INTO metadata (key, value) VALUES ('cacheSchemaVersion', '1');
                CREATE TABLE plugins (
                    name TEXT PRIMARY KEY NOT NULL,
                    traversal_json TEXT
                );
                CREATE TABLE resources (
                    path TEXT PRIMARY KEY NOT NULL,
                    id TEXT,
                    kind TEXT,
                    sidecar TEXT,
                    metadata_json TEXT NOT NULL
                );
                CREATE TABLE links (
                    source_path TEXT NOT NULL,
                    rel TEXT NOT NULL,
                    target_locator TEXT NOT NULL,
                    target_path TEXT,
                    target_id TEXT,
                    relation_rank INTEGER,
                    link_order INTEGER
                );
                CREATE INDEX links_source_path_idx ON links(source_path);
                CREATE INDEX links_target_path_idx ON links(target_path);
                CREATE INDEX links_target_id_idx ON links(target_id);
                CREATE INDEX resources_id_idx ON resources(id);
                CREATE INDEX resources_kind_idx ON resources(kind);
                CREATE TABLE diagnostics (
                    code TEXT NOT NULL,
                    path TEXT,
                    message TEXT NOT NULL
                );
                "#,
            )
            .unwrap();
        drop(connection);

        let result = cache_resources(&cache_path, None);

        assert!(result.is_err());
        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("foreign key links.source_path -> resources.path is missing"));
        assert!(message.contains("run `relaygraph cache rebuild`"));
        let _ = fs::remove_dir_all(root);
    }

    fn empty_graph() -> BuildResult {
        BuildResult {
            resources: Vec::new(),
            diagnostics: Vec::new(),
            plugins: Vec::new(),
        }
    }

    fn graph_with_non_json_metadata() -> BuildResult {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "nested".to_string(),
            serde_yaml::from_str("? [a, b]\n: value\n").unwrap(),
        );

        BuildResult {
            resources: vec![Resource {
                path: "a.md".to_string(),
                id: None,
                kind: None,
                sidecar: None,
                metadata,
                links: Vec::new(),
            }],
            diagnostics: Vec::new(),
            plugins: Vec::new(),
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
    }
}
