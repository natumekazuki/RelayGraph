use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use relaygraph::export::to_export;
use relaygraph::model::{BuildResult, Diagnostic, Plugin, ResolvedLink, Resource, Traversal};

#[test]
fn json_schema_documents_are_parseable_and_strict() {
    for schema in [
        include_str!("../docs/schema/config.schema.json"),
        include_str!("../docs/schema/sidecar.schema.json"),
        include_str!("../docs/schema/plugin.schema.json"),
        include_str!("../docs/schema/export.schema.json"),
    ] {
        let value: Value = serde_json::from_str(schema).unwrap();
        assert_eq!(
            value["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(value["type"], "object");
        assert_eq!(value["additionalProperties"], false);
    }
}

#[test]
fn cache_schema_documents_versioned_tables_and_indexes() {
    let schema = include_str!("../docs/schema/cache-schema.sql");

    assert!(schema.contains("cacheSchemaVersion"));
    assert!(schema.contains("CREATE TABLE metadata"));
    assert!(schema.contains("CREATE TABLE plugins"));
    assert!(schema.contains("CREATE TABLE resources"));
    assert!(schema.contains("CREATE TABLE links"));
    assert!(schema.contains("CREATE TABLE diagnostics"));
    assert!(schema.contains("CREATE INDEX resources_id_idx"));
    assert!(schema.contains("relation_rank INTEGER"));
}

#[test]
fn export_output_matches_documented_contract() {
    let schema: Value =
        serde_json::from_str(include_str!("../docs/schema/export.schema.json")).unwrap();
    let export = to_export(BuildResult {
        resources: vec![
            Resource {
                path: "source.md".to_string(),
                id: Some("source".to_string()),
                kind: Some("source".to_string()),
                sidecar: Some("source.md.relaygraph.yaml".to_string()),
                metadata: BTreeMap::new(),
                links: vec![
                    ResolvedLink {
                        rel: "x".to_string(),
                        to: "path:target.md".to_string(),
                        target_path: Some("target.md".to_string()),
                        target_id: None,
                        order: Some(1),
                    },
                    ResolvedLink {
                        rel: "x".to_string(),
                        to: "id:target".to_string(),
                        target_path: Some("target.md".to_string()),
                        target_id: Some("target".to_string()),
                        order: None,
                    },
                ],
            },
            Resource {
                path: "target.md".to_string(),
                id: Some("target".to_string()),
                kind: None,
                sidecar: None,
                metadata: BTreeMap::new(),
                links: Vec::new(),
            },
        ],
        diagnostics: vec![
            Diagnostic {
                code: "schema-error",
                path: Some("source.md.relaygraph.yaml".to_string()),
                message: "example".to_string(),
            },
            Diagnostic {
                code: "repo-error",
                path: None,
                message: "example without path".to_string(),
            },
        ],
        plugins: vec![
            Plugin {
                schema_version: Some(1),
                name: "contract".to_string(),
                resource_kinds: Vec::new(),
                relations: Vec::new(),
                rules: Vec::new(),
                traversal: Some(Traversal {
                    start_kinds: vec!["source".to_string()],
                    relation_order: vec!["x".to_string()],
                }),
            },
            Plugin {
                schema_version: Some(1),
                name: "without-traversal".to_string(),
                resource_kinds: Vec::new(),
                relations: Vec::new(),
                rules: Vec::new(),
                traversal: None,
            },
        ],
    });
    let output = serde_json::to_value(export).unwrap();

    assert_matches_schema(&output, &schema);
    assert_matches_schema(
        &output["resources"][0],
        &schema["properties"]["resources"]["items"],
    );
    assert_matches_schema(
        &output["resources"][0]["links"][0],
        &schema["$defs"]["link"],
    );
    assert_matches_schema(
        &output["resources"][0]["links"][1],
        &schema["$defs"]["link"],
    );
    assert_matches_schema(
        &output["resources"][1]["incomingLinks"][0],
        &schema["$defs"]["incomingLink"],
    );
    assert_matches_schema(&output["diagnostics"][0], &schema["$defs"]["diagnostic"]);
    assert_matches_schema(&output["diagnostics"][1], &schema["$defs"]["diagnostic"]);
    assert_matches_schema(
        &output["plugins"][0],
        &schema["properties"]["plugins"]["items"],
    );
    assert_matches_schema(
        &output["plugins"][1],
        &schema["properties"]["plugins"]["items"],
    );
    assert_matches_schema(
        &output["plugins"][0]["traversal"],
        &schema["properties"]["plugins"]["items"]["properties"]["traversal"],
    );
    assert!(output["resources"][0]["links"][0]["targetId"].is_null());
    assert!(output["resources"][0]["links"][1]["order"].is_null());
    assert!(output["diagnostics"][1]["path"].is_null());
    assert!(output["plugins"][1]["traversal"].is_null());
}

#[test]
fn config_schema_documents_runtime_contract() {
    let schema: Value =
        serde_json::from_str(include_str!("../docs/schema/config.schema.json")).unwrap();
    let valid = serde_json::json!({
        "schemaVersion": 1,
        "useGitIgnore": true,
        "sidecarSuffix": ".relaygraph.yaml",
        "plugins": ["relaygraph/plugins/feature-trace.yaml"],
        "exclude": ["._relaygraph/**"],
        "requireSidecar": ["docs/**"]
    });

    assert_matches_schema(&valid, &schema);
    assert_schema_rejects(&serde_json::json!({"unknownField": true}), &schema);
    assert_schema_rejects(&serde_json::json!({"schemaVersion": null}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": " "}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": "dir/.rg"}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": "dir\\.rg"}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": ".rg..x"}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": ":rg.yaml"}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": "*.yaml"}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": ".rg."}), &schema);
    assert_schema_rejects(&serde_json::json!({"sidecarSuffix": ".rg "}), &schema);
    assert_schema_rejects(&serde_json::json!({"plugins": [null]}), &schema);
    assert_schema_rejects(&serde_json::json!({"plugins": [" "]}), &schema);
    assert_schema_rejects(
        &serde_json::json!({"plugins": ["../outside.yaml"]}),
        &schema,
    );
    assert_schema_rejects(
        &serde_json::json!({"plugins": ["relaygraph/../outside.yaml"]}),
        &schema,
    );
    assert_schema_rejects(&serde_json::json!({"plugins": ["/outside.yaml"]}), &schema);
    assert_schema_rejects(&serde_json::json!({"plugins": ["\\outside.yaml"]}), &schema);
    assert_schema_rejects(
        &serde_json::json!({"plugins": ["relaygraph\\..\\outside.yaml"]}),
        &schema,
    );
    assert_schema_rejects(
        &serde_json::json!({"plugins": ["C:\\outside.yaml"]}),
        &schema,
    );
    assert_schema_rejects(&serde_json::json!({"exclude": [" "]}), &schema);
    assert_schema_rejects(&serde_json::json!({"requireSidecar": [" "]}), &schema);
}

#[test]
fn sidecar_and_plugin_schema_reject_whitespace_only_names() {
    let sidecar: Value =
        serde_json::from_str(include_str!("../docs/schema/sidecar.schema.json")).unwrap();
    let plugin: Value =
        serde_json::from_str(include_str!("../docs/schema/plugin.schema.json")).unwrap();

    assert_schema_rejects(&serde_json::json!({"id": " "}), &sidecar);
    assert_schema_rejects(&serde_json::json!({"kind": " "}), &sidecar);
    assert_schema_rejects(
        &serde_json::json!({"links": [{"rel": " ", "to": "path:a.md"}]}),
        &sidecar,
    );
    assert_schema_rejects(
        &serde_json::json!({"links": [{"rel": "x", "to": "id:   "}]}),
        &sidecar,
    );
    assert_schema_rejects(
        &serde_json::json!({"links": [{"rel": "x", "to": "path:   "}]}),
        &sidecar,
    );
    assert_schema_rejects(&serde_json::json!({"name": " "}), &plugin);
    assert_schema_rejects(
        &serde_json::json!({"name": "ok", "resourceKinds": [" "]}),
        &plugin,
    );
    assert_schema_rejects(
        &serde_json::json!({"name": "ok", "relations": [" "]}),
        &plugin,
    );
}

fn assert_matches_schema(output: &Value, schema: &Value) {
    assert_type_matches(output, schema);
    if output.is_object() {
        assert_object_contract(output, schema);
    }
}

fn assert_object_contract(output: &Value, schema: &Value) {
    let required = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for field in &required {
        assert!(output.get(*field).is_some(), "missing field {field}");
    }

    let properties = schema["properties"].as_object().unwrap();
    let actual = output.as_object().unwrap();
    for field in actual.keys() {
        assert!(
            properties.contains_key(field),
            "undocumented export field {field}"
        );
    }

    for (field, value) in actual {
        if let Some(field_schema) = properties.get(field) {
            assert_type_matches(value, field_schema);
        }
    }
}

fn assert_type_matches(value: &Value, schema: &Value) {
    let Some(schema_type) = schema.get("type") else {
        return;
    };
    let types = schema_type
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![schema_type.clone()]);
    let matches = types.iter().any(|schema_type| match schema_type.as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.as_i64().is_some(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => false,
    });
    assert!(
        matches,
        "value {value:?} did not match schema type {schema_type:?}"
    );
}

fn assert_schema_rejects(value: &Value, schema: &Value) {
    assert!(
        !schema_accepts(value, schema),
        "schema unexpectedly accepted {value:?}"
    );
}

fn schema_accepts(value: &Value, schema: &Value) -> bool {
    if !schema_type_accepts(value, schema) {
        return false;
    }
    if let Some(const_value) = schema.get("const") {
        if value != const_value {
            return false;
        }
    }
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        if value
            .as_str()
            .is_some_and(|text| text.len() < min_length as usize)
        {
            return false;
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        match pattern {
            "\\S"
                if value
                    .as_str()
                    .is_some_and(|text| !text.chars().any(|c| !c.is_whitespace())) =>
            {
                return false;
            }
            "^(id|path):.*\\S.*$" => {
                let Some(text) = value.as_str() else {
                    return false;
                };
                let Some(locator_value) = text
                    .strip_prefix("id:")
                    .or_else(|| text.strip_prefix("path:"))
                else {
                    return false;
                };
                if !locator_value.chars().any(|c| !c.is_whitespace()) {
                    return false;
                }
            }
            "^(?!.*[<>:\\\"/\\\\|?*])(?!.*\\.\\.)(?!.*[.\\s]$).*\\S.*$" => {
                let Some(text) = value.as_str() else {
                    return false;
                };
                if text.chars().any(|character| {
                    ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&character)
                }) || text.contains("..")
                    || text
                        .chars()
                        .last()
                        .is_some_and(|character| character == '.' || character.is_whitespace())
                    || !text.chars().any(|c| !c.is_whitespace())
                {
                    return false;
                }
            }
            "^(?![A-Za-z]:)(?![\\\\/])(?!.*(^|[\\\\/])\\.\\.([\\\\/]|$)).*\\S.*$" => {
                let Some(text) = value.as_str() else {
                    return false;
                };
                let has_parent = text.split(['/', '\\']).any(|component| component == "..");
                if text.starts_with('/')
                    || text.starts_with('\\')
                    || text.get(1..2) == Some(":")
                    || has_parent
                    || !text.chars().any(|c| !c.is_whitespace())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            return value.as_object().is_none_or(|object| object.is_empty());
        };
        if value
            .as_object()
            .is_some_and(|object| object.keys().any(|key| !properties.contains_key(key)))
        {
            return false;
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        if let Some(object) = value.as_object() {
            for (key, child) in object {
                if let Some(child_schema) = properties.get(key) {
                    if !schema_accepts(child, child_schema) {
                        return false;
                    }
                }
            }
        }
    }
    if let Some(items) = schema.get("items") {
        if value
            .as_array()
            .is_some_and(|items_value| items_value.iter().any(|item| !schema_accepts(item, items)))
        {
            return false;
        }
    }
    true
}

fn schema_type_accepts(value: &Value, schema: &Value) -> bool {
    let Some(schema_type) = schema.get("type") else {
        return true;
    };
    let types = schema_type
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![schema_type.clone()]);
    types.iter().any(|schema_type| match schema_type.as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.as_i64().is_some(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => false,
    })
}

#[test]
fn cache_schema_applies_to_sqlite() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(include_str!("../docs/schema/cache-schema.sql"))
        .unwrap();

    for table in ["metadata", "plugins", "resources", "links", "diagnostics"] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }
}
