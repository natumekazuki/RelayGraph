use crate::model::{Diagnostic, SUPPORTED_SCHEMA_VERSION};

pub fn print_diagnostics(diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        println!("ok");
        return;
    }

    for diagnostic in diagnostics {
        match diagnostic.path.as_deref() {
            Some(path) => println!("{} {}: {}", diagnostic.code, path, diagnostic.message),
            None => println!("{}: {}", diagnostic.code, diagnostic.message),
        }
    }
}

pub fn diagnostics_to_message(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| match diagnostic.path.as_deref() {
            Some(path) => format!("{} {}: {}", diagnostic.code, path, diagnostic.message),
            None => format!("{}: {}", diagnostic.code, diagnostic.message),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn validate_schema_version(
    version: Option<u32>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let version = version.unwrap_or(SUPPORTED_SCHEMA_VERSION);
    if version != SUPPORTED_SCHEMA_VERSION {
        diagnostics.push(Diagnostic {
            code: "schema-error",
            path: Some(path.to_string()),
            message: format!(
                "unsupported schemaVersion {version}; expected {SUPPORTED_SCHEMA_VERSION}"
            ),
        });
    }
}
