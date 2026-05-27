use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn relaygraph() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_relaygraph"))
}

#[test]
fn cli_validates_exports_caches_and_traces_fixture_repo() {
    let root = temp_root("relaygraph-cli");
    create_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    assert_success(run(&root, ["validate", "--json"]));

    let export_path = root.join("graph.json");
    assert_success(run(
        &root,
        ["export", "--output", export_path.to_str().unwrap()],
    ));
    assert!(export_path.exists());

    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));
    assert!(cache_path.exists());

    let resources = run(
        &root,
        [
            "cache",
            "resources",
            "--db",
            cache_path.to_str().unwrap(),
            "--kind",
            "source",
        ],
    );
    assert_success(resources);

    let trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:src/main.rs",
        ],
    );
    assert_success_with_stdout(trace, "docs/root.md");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_feature_trace_plugin_is_embedded_for_fresh_repo() {
    let root = temp_root("relaygraph-embedded-plugin");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("docs/root.md"), "# Root\n").unwrap();
    fs::write(
        root.join("docs/root.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: root\nkind: feature-root\nlinks:\n  - rel: decomposes-to\n    to: path:docs/module.md\n  - rel: decomposes-to\n    to: path:docs/design.md\n",
    )
    .unwrap();
    fs::write(root.join("docs/design.md"), "# Design\n").unwrap();
    fs::write(
        root.join("docs/design.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: design\nkind: design-doc\nlinks: []\n",
    )
    .unwrap();
    fs::write(root.join("docs/module.md"), "# Module\n").unwrap();
    fs::write(
        root.join("docs/module.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: module\nkind: module\nlinks:\n  - rel: realized-by\n    to: path:src/main.rs\n  - rel: verified-by\n    to: path:tests/main.rs\n",
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("src/main.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nkind: source\nlinks: []\n",
    )
    .unwrap();
    fs::write(root.join("tests/main.rs"), "#[test] fn main_test() {}\n").unwrap();
    fs::write(
        root.join("tests/main.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: test\nkind: test\nlinks: []\n",
    )
    .unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["validate", "--json"]);
    assert_success_with_stdout(output, "[]");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_read_commands_do_not_parse_broken_config() {
    let root = temp_root("relaygraph-cache-read-broken-config");
    create_fixture_repo(&root);

    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));
    fs::write(root.join(".relaygraph.yaml"), "unknownField: true\n").unwrap();

    let diagnostics = run(
        &root,
        ["cache", "diagnostics", "--db", cache_path.to_str().unwrap()],
    );
    assert_success_with_stdout(diagnostics, "ok");
    let resources = run(
        &root,
        [
            "cache",
            "resources",
            "--db",
            cache_path.to_str().unwrap(),
            "--kind",
            "source",
        ],
    );
    assert_success_with_stdout(resources, "src/main.rs");
    let links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--to",
            "path:src/main.rs",
        ],
    );
    assert_success_with_stdout(links, "docs/root.md --realized-by--> path:src/main.rs");
    let trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:src/main.rs",
        ],
    );
    assert_success_with_stdout(trace, "docs/root.md");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_trace_matches_relation_order_used_by_trace() {
    let root = temp_root("relaygraph-cache-order");
    create_relation_order_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));

    let trace = run(&root, ["trace", "path:root.md", "--direction", "outgoing"]);
    assert_success_with_lines(trace, &["root.md", "b.md", "a.md"]);

    let cache_trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:root.md",
            "--direction",
            "outgoing",
        ],
    );
    assert_success_with_lines(cache_trace, &["root.md", "b.md", "a.md"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn trace_rejects_parent_traversal_start_locator() {
    let root = temp_root("relaygraph-trace-parent-start");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("target.md"), "# Target\n").unwrap();

    let output = run(&root, ["trace", "path:../target.md"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("parent traversal"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn trace_and_cache_trace_tie_break_by_target_locator() {
    let root = temp_root("relaygraph-target-locator-order");
    create_target_locator_order_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));

    let trace = run(&root, ["trace", "path:root.md", "--direction", "outgoing"]);
    assert_success_with_lines(trace, &["root.md", "z.md", "a.md"]);

    let cache_trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:root.md",
            "--direction",
            "outgoing",
        ],
    );
    assert_success_with_lines(cache_trace, &["root.md", "z.md", "a.md"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn incoming_trace_and_cache_links_respect_relation_order_and_target_ids() {
    let root = temp_root("relaygraph-incoming-order");
    create_incoming_order_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));

    let trace = run(
        &root,
        ["trace", "path:target.md", "--direction", "incoming"],
    );
    assert_success_with_lines(trace, &["target.md", "b.md", "a.md"]);

    let cache_trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:target.md",
            "--direction",
            "incoming",
        ],
    );
    assert_success_with_lines(cache_trace, &["target.md", "b.md", "a.md"]);

    let cache_links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--to",
            "id:target",
        ],
    );
    assert_success_with_lines(
        cache_links,
        &[
            "b.md --zrel--> path:target.md",
            "a.md --arel--> path:target.md",
        ],
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn incoming_trace_tie_breaks_by_source_path_in_cache() {
    let root = temp_root("relaygraph-incoming-tie");
    create_incoming_tie_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));

    let trace = run(
        &root,
        ["trace", "path:target.md", "--direction", "incoming"],
    );
    assert_success_with_lines(trace, &["target.md", "a.md", "z.md"]);

    let cache_trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:target.md",
            "--direction",
            "incoming",
        ],
    );
    assert_success_with_lines(cache_trace, &["target.md", "a.md", "z.md"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_trace_both_matches_trace_after_mixing_directions() {
    let root = temp_root("relaygraph-both-order");
    create_both_direction_order_fixture_repo(&root);

    assert_success(run(&root, ["validate"]));
    let cache_path = root.join("relaygraph.sqlite");
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    ));

    let trace = run(&root, ["trace", "path:a.md", "--direction", "both"]);
    assert_success_with_lines(trace, &["a.md", "b.md", "z.md"]);

    let cache_trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "path:a.md",
            "--direction",
            "both",
        ],
    );
    assert_success_with_lines(cache_trace, &["a.md", "b.md", "z.md"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_links_to_missing_id_filters_by_unresolved_id() {
    let root = temp_root("relaygraph-missing-id-filter");
    create_missing_id_links_fixture_repo(&root);

    let cache_path = root.join("relaygraph.sqlite");
    let rebuild = run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    );
    assert!(!rebuild.status.success());
    assert!(cache_path.exists());

    let links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--to",
            "id:missing-a",
        ],
    );
    assert_success_with_lines(links, &["a.md --arel--> id:missing-a"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn duplicate_plugin_names_are_validate_diagnostics() {
    let root = temp_root("relaygraph-duplicate-plugin");
    create_duplicate_plugin_fixture_repo(&root);

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"duplicate-plugin\""));
    assert!(stdout.contains("plugin name duplicate is already used by"));

    let cache_path = root.join("relaygraph.sqlite");
    let rebuild = run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    );
    assert!(!rebuild.status.success());
    let stderr = String::from_utf8_lossy(&rebuild.stderr);
    assert!(
        !stderr.contains("UNIQUE constraint failed"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn plugin_paths_must_stay_inside_repository() {
    let root = temp_root("relaygraph-outside-plugin");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - ../outside-plugin.yaml\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("plugin path must be repo-relative"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn absolute_plugin_paths_are_rejected() {
    let root = temp_root("relaygraph-absolute-plugin");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    let plugin_path = root.join("relaygraph/plugins/order.yaml");
    fs::write(
        &plugin_path,
        "schemaVersion: 1\nname: order\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    let plugin_path = plugin_path.to_string_lossy().replace('\\', "/");
    fs::write(
        root.join(".relaygraph.yaml"),
        format!("schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - '{plugin_path}'\n"),
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("plugin path must be repo-relative"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn subdirectory_execution_uses_repository_root_config() {
    let root = temp_root("relaygraph-subdir-root");
    create_fixture_repo(&root);

    let output = run(&root.join("docs"), ["validate", "--json"]);

    assert_success_with_stdout(output, "[]");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn subdirectory_cache_commands_use_repository_root_default_cache() {
    let root = temp_root("relaygraph-subdir-cache");
    create_fixture_repo(&root);

    assert_success(run(&root.join("docs"), ["cache", "rebuild"]));
    assert!(root.join("._relaygraph/cache/relaygraph.sqlite").exists());
    assert!(!root
        .join("docs/._relaygraph/cache/relaygraph.sqlite")
        .exists());
    assert_success(run(&root.join("docs"), ["cache", "diagnostics"]));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn output_file_names_without_parent_directory_are_supported() {
    let root = temp_root("relaygraph-output-file-name");
    create_fixture_repo(&root);

    assert_success(run(&root, ["export", "--output", "graph.json"]));
    assert!(root.join("graph.json").exists());
    assert_success(run(
        &root,
        ["cache", "rebuild", "--output", "relaygraph.sqlite"],
    ));
    assert!(root.join("relaygraph.sqlite").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_outputs_refuse_to_overwrite_declarations_without_force() {
    let root = temp_root("relaygraph-output-protect");
    create_fixture_repo(&root);

    let export = run(&root, ["export", "--output", ".relaygraph.yaml"]);
    assert!(!export.status.success());
    let config = fs::read_to_string(root.join(".relaygraph.yaml")).unwrap();
    assert!(config.contains("schemaVersion: 1"));
    assert!(config.contains("plugins:"));

    let cache = run(
        &root,
        [
            "cache",
            "rebuild",
            "--output",
            "docs/root.md.relaygraph.yaml",
        ],
    );
    assert!(!cache.status.success());
    let sidecar = fs::read_to_string(root.join("docs/root.md.relaygraph.yaml")).unwrap();
    assert!(sidecar.contains("id: docs.root"));

    let forced_export = run(&root, ["export", "--output", ".relaygraph.yaml", "--force"]);
    assert!(!forced_export.status.success());
    let config = fs::read_to_string(root.join(".relaygraph.yaml")).unwrap();
    assert!(config.contains("plugins:"));

    let traversal_export = run(
        &root,
        ["export", "--output", "docs/../.relaygraph.yaml", "--force"],
    );
    assert!(!traversal_export.status.success());
    let config = fs::read_to_string(root.join(".relaygraph.yaml")).unwrap();
    assert!(config.contains("plugins:"));

    let uppercase_root = PathBuf::from(root.to_string_lossy().to_string().to_uppercase());
    let case_mismatch_export = run(
        &root,
        [
            "export",
            "--output",
            uppercase_root.join(".relaygraph.yaml").to_str().unwrap(),
            "--force",
        ],
    );
    assert!(!case_mismatch_export.status.success());
    let config = fs::read_to_string(root.join(".relaygraph.yaml")).unwrap();
    assert!(config.contains("plugins:"));

    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
    let forced_workflow_export = run(
        &root,
        ["export", "--output", ".github/workflows/ci.yml", "--force"],
    );
    assert!(!forced_workflow_export.status.success());
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert_eq!(workflow, "name: ci\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_outputs_protect_orphan_sidecars_even_with_force() {
    let root = temp_root("relaygraph-output-orphan-sidecar");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(
        root.join("missing.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: orphan\nlinks: []\n",
    )
    .unwrap();

    let export = run(
        &root,
        [
            "export",
            "--output",
            "missing.md.relaygraph.yaml",
            "--force",
        ],
    );

    assert!(!export.status.success());
    let sidecar = fs::read_to_string(root.join("missing.md.relaygraph.yaml")).unwrap();
    assert!(sidecar.contains("id: orphan"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_outputs_can_be_rebuilt_when_generated_dir_is_not_excluded() {
    let root = temp_root("relaygraph-default-output-self-resource");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nexclude: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    assert_success(run(&root, ["export"]));
    assert_success(run(&root, ["export"]));
    assert!(root.join("._relaygraph/generated/relaygraph.json").exists());
    let export_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("._relaygraph/generated/relaygraph.json")).unwrap(),
    )
    .unwrap();
    assert!(!export_json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"]
            .as_str()
            .is_some_and(|path| path.starts_with("._relaygraph/"))));

    assert_success(run(&root, ["cache", "rebuild"]));
    assert_success(run(&root, ["cache", "rebuild"]));
    assert!(root.join("._relaygraph/cache/relaygraph.sqlite").exists());
    let resources = run(&root, ["cache", "resources"]);
    let stdout = String::from_utf8_lossy(&resources.stdout);
    let stderr = String::from_utf8_lossy(&resources.stderr);
    assert!(
        resources.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(!stdout.contains("._relaygraph/"));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn outputs_allow_symlink_ancestors_above_repository_root() {
    use std::os::unix::fs::symlink;

    let base = temp_root("relaygraph-output-ancestor-symlink");
    let real = base.join("real");
    let alias = base.join("alias");
    fs::create_dir_all(&real).unwrap();
    symlink(&real, &alias).unwrap();

    let root = alias.join("repo");
    create_fixture_repo(&root);
    let export_path = root.join("graph.json");

    assert_success(run(
        &root,
        ["export", "--output", export_path.to_str().unwrap()],
    ));
    assert!(real.join("repo/graph.json").exists());

    let _ = fs::remove_dir_all(base);
}

#[test]
fn configured_plugins_are_declarations_not_resources() {
    let root = temp_root("relaygraph-plugin-declaration-not-resource");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - relaygraph/plugins/./custom.yaml\nexclude: []\nrequireSidecar:\n  - relaygraph/**\n",
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let init = run(&root, ["init"]);
    assert_success(init);
    assert!(!root
        .join("relaygraph/plugins/custom.yaml.relaygraph.yaml")
        .exists());

    let export_path = root.join("graph.json");
    assert_success(run(
        &root,
        ["export", "--output", export_path.to_str().unwrap()],
    ));
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert!(!export_json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "relaygraph/plugins/custom.yaml"));
    assert!(!export_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "missing-sidecar"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn reserved_generated_plugin_paths_are_rejected() {
    let root = temp_root("relaygraph-reserved-plugin");
    fs::create_dir_all(root.join("._relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - ._relaygraph/plugins/custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("._relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("reserved generated directory"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_plugin_paths_do_not_hide_resources() {
    let root = temp_root("relaygraph-invalid-plugin-does-not-hide-resource");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - docs/../a.md\nexclude: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);

    assert!(!export.status.success());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert!(export_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "schema-error"));
    assert!(export_json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "a.md"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn backslash_parent_plugin_paths_do_not_hide_resources() {
    let root = temp_root("relaygraph-backslash-plugin-does-not-hide-resource");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - 'docs\\..\\a.md'\nexclude: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);

    assert!(!export.status.success());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert!(export_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "schema-error"));
    assert!(export_json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"] == "a.md"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn reserved_generated_directory_is_case_insensitive() {
    let root = temp_root("relaygraph-reserved-case-insensitive");
    fs::create_dir_all(root.join("._RelayGraph/generated")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nexclude: []\n",
    )
    .unwrap();
    fs::write(
        root.join("._RelayGraph/generated/relaygraph.json"),
        "{\"generated\":true}\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let export_path = root.join("graph.json");
    assert_success(run(
        &root,
        ["export", "--output", export_path.to_str().unwrap()],
    ));
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert!(!export_json["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["path"]
            .as_str()
            .is_some_and(|path| path.to_ascii_lowercase().starts_with("._relaygraph/"))));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_relation_contract_rejects_undeclared_relations() {
    let root = temp_root("relaygraph-empty-relations-contract");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - relaygraph/plugins/custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds:\n  - source\nrelations: []\nrules:\n  - when: source\n    requireAnyOutgoing:\n      - x\ntraversal:\n  relationOrder:\n    - x\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: x\n    to: path:a.md\n",
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"unknown-relation\""));
    assert!(stdout.contains("rule references unknown relation: x"));
    assert!(stdout.contains("traversal references unknown relation: x"));
    assert!(stdout.contains("unknown relation: x"));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn plugin_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = temp_root("relaygraph-plugin-symlink");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::create_dir_all(root.join("._relaygraph/generated")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - relaygraph/plugins/custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("._relaygraph/generated/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    symlink(
        root.join("._relaygraph/generated/custom.yaml"),
        root.join("relaygraph/plugins/custom.yaml"),
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"plugin-load-error\""));
    assert!(stdout.contains("plugin file must not be a symlink"));

    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn plugin_symlinks_are_rejected() {
    let root = temp_root("relaygraph-plugin-symlink");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::create_dir_all(root.join("._relaygraph/generated")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - relaygraph/plugins/custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("._relaygraph/generated/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    let link = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            root.join("relaygraph/plugins/custom.yaml")
                .to_str()
                .unwrap(),
            root.join("._relaygraph/generated/custom.yaml")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();
    if !link.status.success() {
        let _ = fs::remove_dir_all(root);
        return;
    }

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"plugin-load-error\""));
    assert!(stdout.contains("plugin file must not be a symlink"));

    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn default_outputs_reject_boundary_link_parents() {
    let root = temp_root("relaygraph-default-output-boundary");
    let outside = temp_root("relaygraph-default-output-outside");
    create_fixture_repo(&root);
    fs::create_dir_all(&outside).unwrap();
    let relaygraph_dir = root.join("._relaygraph");
    let junction = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            relaygraph_dir.to_str().unwrap(),
            outside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(junction.status.success());

    let export = run(&root, ["export"]);
    assert!(!export.status.success());
    assert!(!outside.join("generated/relaygraph.json").exists());

    let cache = run(&root, ["cache", "rebuild"]);
    assert!(!cache.status.success());
    assert!(!outside.join("cache/relaygraph.sqlite").exists());

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn explicit_outputs_allow_force_overwrite() {
    let root = temp_root("relaygraph-output-force");
    create_fixture_repo(&root);
    fs::create_dir_all(root.join("._relaygraph/generated")).unwrap();
    let output = root.join("._relaygraph/generated/graph.json");
    fs::write(&output, "old\n").unwrap();

    assert_success(run(
        &root,
        ["export", "--output", output.to_str().unwrap(), "--force"],
    ));
    let graph = fs::read_to_string(output).unwrap();
    assert!(graph.contains("\"resources\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_outputs_protect_normalized_plugin_paths_even_with_force() {
    let root = temp_root("relaygraph-output-normalized-plugin-protect");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - relaygraph/./plugins/custom.yaml\nexclude:\n  - relaygraph/plugins/**\n",
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();

    let export = run(
        &root,
        [
            "export",
            "--output",
            "relaygraph/plugins/custom.yaml",
            "--force",
        ],
    );
    assert!(!export.status.success());
    let plugin = fs::read_to_string(root.join("relaygraph/plugins/custom.yaml")).unwrap();
    assert!(plugin.contains("name: custom"));

    let cache = run(
        &root,
        [
            "cache",
            "rebuild",
            "--output",
            "relaygraph/plugins/custom.yaml",
            "--force",
        ],
    );
    assert!(!cache.status.success());
    let plugin = fs::read_to_string(root.join("relaygraph/plugins/custom.yaml")).unwrap();
    assert!(plugin.contains("name: custom"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn duplicate_ids_are_not_resolved_to_arbitrary_targets() {
    let root = temp_root("relaygraph-duplicate-id-resolution");
    create_duplicate_id_fixture_repo(&root);

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);
    assert!(!export.status.success());
    let graph: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    let root_resource = graph["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["path"] == "root.md")
        .unwrap();
    assert!(root_resource["links"][0]["targetPath"].is_null());
    assert!(graph["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "ambiguous-id"));

    let cache_path = root.join("relaygraph.sqlite");
    let rebuild = run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    );
    assert!(!rebuild.status.success());
    let trace = run(
        &root,
        [
            "cache",
            "trace",
            "--db",
            cache_path.to_str().unwrap(),
            "id:dup",
        ],
    );
    assert!(!trace.status.success());
    let stderr = String::from_utf8_lossy(&trace.stderr);
    assert!(stderr.contains("ambiguous cache resource id"));

    let links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--to",
            "id:dup",
        ],
    );
    assert!(!links.status.success());
    let stderr = String::from_utf8_lossy(&links.stderr);
    assert!(stderr.contains("ambiguous cache resource id"));

    let links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--from",
            "id:dup",
        ],
    );
    assert!(!links.status.success());
    let stderr = String::from_utf8_lossy(&links.stderr);
    assert!(stderr.contains("ambiguous cache resource id"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ignored_custom_plugins_are_diagnostics() {
    let root = temp_root("relaygraph-ignored-plugin");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(root.join(".gitignore"), "relaygraph/plugins/custom.yaml\n").unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nplugins:\n  - relaygraph/plugins/custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"plugin-load-error\""));
    assert!(stdout.contains("not part of Git-backed repository discovery"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn discovered_custom_plugins_allow_normalized_config_paths() {
    let root = temp_root("relaygraph-normalized-plugin");
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nplugins:\n  - relaygraph/plugins/./custom.yaml\n",
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/custom.yaml"),
        "schemaVersion: 1\nname: custom\nresourceKinds: []\nrelations: []\nrules: []\n",
    )
    .unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["validate", "--json"]);

    assert_success_with_stdout(output, "[]");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn embedded_default_plugin_allows_normalized_config_path() {
    let root = temp_root("relaygraph-normalized-default-plugin");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nplugins:\n  - ./relaygraph/plugins/feature-trace.yaml\n",
    )
    .unwrap();
    fs::write(root.join("docs/root.md"), "# Root\n").unwrap();
    fs::write(
        root.join("docs/root.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: root\nkind: feature-root\nlinks:\n  - rel: decomposes-to\n    to: path:docs/module.md\n  - rel: decomposes-to\n    to: path:docs/design.md\n",
    )
    .unwrap();
    fs::write(root.join("docs/design.md"), "# Design\n").unwrap();
    fs::write(
        root.join("docs/design.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: design\nkind: design-doc\nlinks: []\n",
    )
    .unwrap();
    fs::write(root.join("docs/module.md"), "# Module\n").unwrap();
    fs::write(
        root.join("docs/module.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: module\nkind: module\nlinks:\n  - rel: realized-by\n    to: path:src/main.rs\n  - rel: verified-by\n    to: path:tests/main.rs\n",
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("src/main.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nkind: source\nlinks: []\n",
    )
    .unwrap();
    fs::write(root.join("tests/main.rs"), "#[test] fn main_test() {}\n").unwrap();
    fs::write(
        root.join("tests/main.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: test\nkind: test\nlinks: []\n",
    )
    .unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["validate", "--json"]);

    assert_success_with_stdout(output, "[]");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn validate_json_reports_graph_build_errors_as_json_diagnostics() {
    let root = temp_root("relaygraph-json-build-error");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: true\nplugins: []\n",
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(diagnostics[0]["code"], "repo-error");
    assert!(diagnostics[0]["message"]
        .as_str()
        .unwrap()
        .contains("git ls-files failed"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_invalid_plugin_paths_before_writing_sidecars() {
    let root = temp_root("relaygraph-init-invalid-plugin");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nplugins:\n  - ../outside.yaml\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!root.join("a.md.relaygraph.yaml").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_gitignored_generated_sidecars() {
    let root = temp_root("relaygraph-init-ignored-sidecar");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(".gitignore"), "*.md.relaygraph.yaml\n").unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nplugins: []\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("generated sidecar would be ignored by Git discovery"));
    assert!(!root.join("a.md.relaygraph.yaml").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ignored_root_config_is_not_accepted_in_git_ignore_mode() {
    let root = temp_root("relaygraph-ignored-config");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(".gitignore"), ".relaygraph.yaml\n").unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    let init = Command::new("git")
        .arg("init")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(init.status.success());

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(".relaygraph.yaml must be part of Git-backed repository discovery"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_unsupported_config_schema_version() {
    let root = temp_root("relaygraph-init-config-version");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 2\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let dry_run = run(&root, ["init", "--dry-run"]);
    assert!(!dry_run.status.success());
    assert!(!root.join("a.md.relaygraph.yaml").exists());

    let output = run(&root, ["init"]);
    assert!(!output.status.success());
    assert!(!root.join("a.md.relaygraph.yaml").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn plugin_duplicate_kind_and_relation_are_schema_errors() {
    let root = temp_root("relaygraph-duplicate-plugin-items");
    create_duplicate_plugin_items_fixture_repo(&root);

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("resourceKinds contains duplicate value: source"));
    assert!(stdout.contains("relations contains duplicate value: x"));
    assert!(!stdout.contains("\"code\": \"unknown-kind\""));
    assert!(!stdout.contains("\"code\": \"unknown-relation\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_generates_valid_unique_ids_for_dotfiles_and_extensions() {
    let root = temp_root("relaygraph-init-ids");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    fs::write(root.join("123"), "number-like id\n").unwrap();
    fs::write(root.join("true"), "bool-like id\n").unwrap();
    fs::write(root.join("foo.md"), "# Foo MD\n").unwrap();
    fs::write(root.join("foo.rs"), "fn foo() {}\n").unwrap();

    assert_success(run(&root, ["init"]));
    assert!(root.join("123.relaygraph.yaml").exists());
    assert!(root.join(".gitignore.relaygraph.yaml").exists());
    assert!(root.join("foo.md.relaygraph.yaml").exists());
    assert!(root.join("foo.rs.relaygraph.yaml").exists());
    assert!(root.join("true.relaygraph.yaml").exists());
    assert_success(run(&root, ["validate"]));

    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn init_rejects_dangling_sidecar_symlink_before_writing() {
    let root = temp_root("relaygraph-init-dangling-sidecar");
    let outside_root = temp_root("relaygraph-init-dangling-target");
    let outside = outside_root.join("outside.yaml");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    let link = root.join("a.md.relaygraph.yaml");
    let symlink = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            link.to_str().unwrap(),
            outside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(symlink.status.success());

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!outside.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("sidecar must not be a symlink"));

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside_root);
}

#[cfg(windows)]
#[test]
fn init_rejects_boundary_link_resources_before_writing_sidecars() {
    let root = temp_root("relaygraph-init-boundary-resource");
    let outside = temp_root("relaygraph-init-boundary-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\n",
    )
    .unwrap();
    let link = root.join("linked");
    let junction = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            link.to_str().unwrap(),
            outside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(junction.status.success());

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!root.join("linked.relaygraph.yaml").exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("resource must not be a symlink"));

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn validate_json_reports_config_parse_errors_as_diagnostics() {
    let root = temp_root("relaygraph-config-json-error");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(".relaygraph.yaml"), "unknownField: true\n").unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("\"path\": \".relaygraph.yaml\""));
    assert!(stdout.contains("failed to parse .relaygraph.yaml"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_metadata_is_diagnostic_and_does_not_break_export_or_cache() {
    let root = temp_root("relaygraph-invalid-metadata");
    create_invalid_metadata_fixture_repo(&root);

    let validate = run(&root, ["validate", "--json"]);
    assert!(!validate.status.success());
    let stdout = String::from_utf8_lossy(&validate.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("metadata must be JSON-compatible"));

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);
    assert!(!export.status.success());
    assert!(export_path.exists());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert_eq!(
        export_json["resources"][0]["metadata"],
        serde_json::json!({})
    );

    let cache_path = root.join("relaygraph.sqlite");
    let rebuild = run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    );
    assert!(!rebuild.status.success());
    let stderr = String::from_utf8_lossy(&rebuild.stderr);
    assert!(
        !stderr.contains("key must be a string"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_path_locator_does_not_resolve_target_path() {
    let root = temp_root("relaygraph-missing-path");
    create_missing_path_fixture_repo(&root);

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);
    assert!(!export.status.success());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert!(export_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "missing-path"));
    assert!(export_json["resources"][0]["links"][0]["targetPath"].is_null());

    let cache_path = root.join("relaygraph.sqlite");
    let rebuild = run(
        &root,
        ["cache", "rebuild", "--output", cache_path.to_str().unwrap()],
    );
    assert!(!rebuild.status.success());
    let links = run(
        &root,
        [
            "cache",
            "links",
            "--db",
            cache_path.to_str().unwrap(),
            "--to",
            "path:missing.md",
        ],
    );
    assert_success_with_lines(links, &["a.md --x--> path:missing.md"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn path_locators_normalize_current_directory_components() {
    let root = temp_root("relaygraph-path-dot-normalize");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("docs/source.md"), "# Source\n").unwrap();
    fs::write(root.join("docs/target.md"), "# Target\n").unwrap();
    fs::write(
        root.join("docs/source.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nlinks:\n  - rel: x\n    to: path:docs/./target.md\n",
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert_success_with_stdout(output, "[]");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn path_locators_reject_parent_traversal() {
    let root = temp_root("relaygraph-path-parent-reject");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("docs/source.md"), "# Source\n").unwrap();
    fs::write(
        root.join("docs/source.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nlinks:\n  - rel: x\n    to: path:../target.md\n",
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("parent traversal"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_sidecar_suffix_is_schema_error() {
    let root = temp_root("relaygraph-empty-suffix");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nsidecarSuffix: \"\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("sidecarSuffix must be a non-empty filename suffix"));

    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);
    assert!(!export.status.success());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    assert_eq!(export_json["resources"].as_array().unwrap().len(), 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn whitespace_only_config_strings_are_schema_errors() {
    let root = temp_root("relaygraph-whitespace-config");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins:\n  - \" \"\nexclude:\n  - \" \"\nrequireSidecar:\n  - \" \"\nsidecarSuffix: \" \"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sidecarSuffix must be a non-empty filename suffix"));
    assert!(stdout.contains("plugins entries must not be empty or whitespace"));
    assert!(stdout.contains("exclude entries must not be empty or whitespace"));
    assert!(stdout.contains("requireSidecar entries must not be empty or whitespace"));

    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nlinks: []\n",
    )
    .unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nsidecarSuffix: \" \"\n",
    )
    .unwrap();
    let export_path = root.join("graph.json");
    let export = run(&root, ["export", "--output", export_path.to_str().unwrap()]);
    assert!(!export.status.success());
    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).unwrap()).unwrap();
    let resources = export_json["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["path"], "a.md");
    assert_eq!(resources[0]["sidecar"], "a.md.relaygraph.yaml");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn sidecar_suffix_rejects_path_like_values_before_init_writes() {
    let root = temp_root("relaygraph-path-like-suffix");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\nsidecarSuffix: \".rg/../../outside\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!root.join("outside").exists());
    assert!(!root.parent().unwrap().join("outside").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn sidecar_suffix_rejects_windows_reserved_characters_before_init_writes() {
    let root = temp_root("relaygraph-reserved-suffix");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\nsidecarSuffix: \":rg.yaml\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!root.join("a.md:rg.yaml").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn sidecar_suffix_rejects_windows_normalized_trailing_characters() {
    let root = temp_root("relaygraph-trailing-suffix");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\nrequireSidecar:\n  - \"**\"\nsidecarSuffix: \".rg.\"\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();

    let output = run(&root, ["init"]);

    assert!(!output.status.success());
    assert!(!root.join("a.md.rg").exists());
    assert!(!root.join("a.md.rg.").exists());

    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn filesystem_scan_reports_boundary_link_resources() {
    let root = temp_root("relaygraph-fs-boundary-link");
    let outside = temp_root("relaygraph-fs-boundary-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    let link = root.join("linked");
    let junction = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            link.to_str().unwrap(),
            outside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(junction.status.success());

    let output = run(&root, ["validate", "--json"]);

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("resource must not be a symlink"));

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn require_reachable_kinds_requires_each_listed_kind() {
    let root = temp_root("relaygraph-require-all-reachable");
    create_require_all_reachable_fixture_repo(&root);

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"missing-required-relation\""));
    assert!(stdout.contains("requires reachable resource kind test"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_sidecar_reports_schema_error_once() {
    let root = temp_root("relaygraph-invalid-sidecar");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("broken.md"), "# Broken\n").unwrap();
    fs::write(root.join("broken.md.relaygraph.yaml"), "schemaVersion: [").unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.matches("\"code\": \"schema-error\"").count(), 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_null_sidecar_fields_are_schema_errors() {
    let root = temp_root("relaygraph-null-sidecar");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("broken.md"), "# Broken\n").unwrap();
    fs::write(
        root.join("broken.md.relaygraph.yaml"),
        "schemaVersion: null\nid: null\nkind: null\nlinks: []\n",
    )
    .unwrap();

    let output = run(&root, ["validate", "--json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\": \"schema-error\""));
    assert!(stdout.contains("explicit null is not allowed"));

    let _ = fs::remove_dir_all(root);
}

fn create_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();

    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/feature-trace.yaml
exclude:
  - ._relaygraph/**
  - relaygraph/plugins/**
requireSidecar:
  - docs/**
  - src/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/feature-trace.yaml"),
        r#"
schemaVersion: 1
name: feature-trace
resourceKinds:
  - feature-root
  - source
relations:
  - realized-by
rules:
  - when: feature-root
    requireAnyOutgoing:
      - realized-by
    requireReachableKinds:
      - source
traversal:
  startKinds:
    - feature-root
  relationOrder:
    - realized-by
"#,
    )
    .unwrap();
    fs::write(root.join("docs/root.md"), "# Root\n").unwrap();
    fs::write(
        root.join("docs/root.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: docs.root\nkind: feature-root\nlinks:\n  - rel: realized-by\n    to: path:src/main.rs\n",
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("src/main.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: src.main\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_relation_order_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - feature-root
  - source
relations:
  - arel
  - zrel
rules: []
traversal:
  relationOrder:
    - zrel
    - arel
"#,
    )
    .unwrap();
    fs::write(root.join("root.md"), "# Root\n").unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    fs::write(
        root.join("root.md.relaygraph.yaml"),
        r#"
schemaVersion: 1
id: root
kind: feature-root
links:
  - rel: arel
    to: path:a.md
  - rel: zrel
    to: path:b.md
"#,
    )
    .unwrap();
}

fn create_target_locator_order_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - feature-root
  - source
relations:
  - arel
rules: []
traversal:
  relationOrder:
    - arel
"#,
    )
    .unwrap();
    fs::write(root.join("root.md"), "# Root\n").unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("z.md"), "# Z\n").unwrap();
    fs::write(
        root.join("root.md.relaygraph.yaml"),
        r#"
schemaVersion: 1
id: root
kind: feature-root
links:
  - rel: arel
    to: path:a.md
  - rel: arel
    to: id:z
"#,
    )
    .unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks: []\n",
    )
    .unwrap();
    fs::write(
        root.join("z.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: z\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_incoming_order_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - source
relations:
  - arel
  - zrel
rules: []
traversal:
  relationOrder:
    - zrel
    - arel
"#,
    )
    .unwrap();
    fs::write(root.join("target.md"), "# Target\n").unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    fs::write(
        root.join("target.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: target\nkind: source\nlinks: []\n",
    )
    .unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: arel\n    to: path:target.md\n",
    )
    .unwrap();
    fs::write(
        root.join("b.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: b\nkind: source\nlinks:\n  - rel: zrel\n    to: path:target.md\n",
    )
    .unwrap();
}

fn create_incoming_tie_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - source
relations:
  - arel
rules: []
traversal:
  relationOrder:
    - arel
"#,
    )
    .unwrap();
    fs::write(root.join("target.md"), "# Target\n").unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("z.md"), "# Z\n").unwrap();
    fs::write(
        root.join("target.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: target\nkind: source\nlinks: []\n",
    )
    .unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: arel\n    to: path:target.md\n",
    )
    .unwrap();
    fs::write(
        root.join("z.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: z\nkind: source\nlinks:\n  - rel: arel\n    to: id:target\n",
    )
    .unwrap();
}

fn create_both_direction_order_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - source
relations:
  - arel
rules: []
traversal:
  relationOrder:
    - arel
"#,
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    fs::write(root.join("z.md"), "# Z\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: arel\n    to: path:z.md\n",
    )
    .unwrap();
    fs::write(
        root.join("b.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: b\nkind: source\nlinks:\n  - rel: arel\n    to: path:a.md\n",
    )
    .unwrap();
    fs::write(
        root.join("z.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: z\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_missing_id_links_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - source
relations:
  - arel
rules: []
"#,
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: arel\n    to: id:missing-a\n",
    )
    .unwrap();
    fs::write(
        root.join("b.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: b\nkind: source\nlinks:\n  - rel: arel\n    to: id:missing-b\n",
    )
    .unwrap();
}

fn create_duplicate_plugin_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/one.yaml
  - relaygraph/plugins/two.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/one.yaml"),
        r#"
schemaVersion: 1
name: duplicate
resourceKinds:
  - source
relations: []
rules: []
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/two.yaml"),
        r#"
schemaVersion: 1
name: duplicate
resourceKinds:
  - source
relations: []
rules: []
"#,
    )
    .unwrap();
    fs::write(root.join("source.md"), "# Source\n").unwrap();
    fs::write(
        root.join("source.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_duplicate_plugin_items_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/duplicates.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/duplicates.yaml"),
        r#"
schemaVersion: 1
name: duplicates
resourceKinds:
  - source
  - source
relations:
  - x
  - x
rules: []
"#,
    )
    .unwrap();
    fs::write(root.join("source.md"), "# Source\n").unwrap();
    fs::write(
        root.join("source.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_invalid_metadata_fixture_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        r#"
schemaVersion: 1
id: a
metadata:
  nested:
    ? [a, b]
    : value
links: []
"#,
    )
    .unwrap();
}

fn create_missing_path_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/order.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/order.yaml"),
        r#"
schemaVersion: 1
name: order
resourceKinds:
  - source
relations:
  - x
rules: []
"#,
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: a\nkind: source\nlinks:\n  - rel: x\n    to: path:missing.md\n",
    )
    .unwrap();
}

fn create_require_all_reachable_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("relaygraph/plugins")).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        r#"
schemaVersion: 1
useGitIgnore: false
plugins:
  - relaygraph/plugins/feature-trace.yaml
exclude:
  - relaygraph/plugins/**
"#,
    )
    .unwrap();
    fs::write(
        root.join("relaygraph/plugins/feature-trace.yaml"),
        r#"
schemaVersion: 1
name: feature-trace
resourceKinds:
  - feature-root
  - source
  - test
relations:
  - realized-by
rules:
  - when: feature-root
    requireAnyOutgoing:
      - realized-by
    requireReachableKinds:
      - source
      - test
"#,
    )
    .unwrap();
    fs::write(root.join("root.md"), "# Root\n").unwrap();
    fs::write(root.join("source.rs"), "fn source() {}\n").unwrap();
    fs::write(
        root.join("root.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: root\nkind: feature-root\nlinks:\n  - rel: realized-by\n    to: path:source.rs\n",
    )
    .unwrap();
    fs::write(
        root.join("source.rs.relaygraph.yaml"),
        "schemaVersion: 1\nid: source\nkind: source\nlinks: []\n",
    )
    .unwrap();
}

fn create_duplicate_id_fixture_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join(".relaygraph.yaml"),
        "schemaVersion: 1\nuseGitIgnore: false\nplugins: []\n",
    )
    .unwrap();
    fs::write(root.join("root.md"), "# Root\n").unwrap();
    fs::write(
        root.join("root.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: root\nlinks:\n  - rel: x\n    to: id:dup\n",
    )
    .unwrap();
    fs::write(root.join("a.md"), "# A\n").unwrap();
    fs::write(
        root.join("a.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: dup\nlinks: []\n",
    )
    .unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    fs::write(
        root.join("b.md.relaygraph.yaml"),
        "schemaVersion: 1\nid: dup\nlinks: []\n",
    )
    .unwrap();
}

fn run<const N: usize>(root: &Path, args: [&str; N]) -> Output {
    Command::new(relaygraph())
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert_success_with_stdout(output, "");
}

fn assert_success_with_stdout(output: Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains(expected),
        "stdout did not contain {expected:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

fn assert_success_with_lines(output: Output, expected: &[&str]) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let actual = stdout
        .lines()
        .filter(|line| !line.starts_with("wrote "))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "stderr:\n{stderr}");
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
}
