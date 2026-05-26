use crate::model::Locator;

pub fn parse_locator(value: &str) -> std::result::Result<Locator, String> {
    if let Some(id) = value.strip_prefix("id:") {
        if id.trim().is_empty() {
            return Err("empty id locator".to_string());
        }
        return Ok(Locator::Id(id.to_string()));
    }
    if let Some(path) = value.strip_prefix("path:") {
        if path.trim().is_empty() {
            return Err("empty path locator".to_string());
        }
        return Ok(Locator::Path(path.to_string()));
    }
    Err(format!("unsupported locator prefix: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_locators() {
        assert!(matches!(
            parse_locator("id:doc.auth"),
            Ok(Locator::Id(id)) if id == "doc.auth"
        ));
        assert!(matches!(
            parse_locator("path:docs/auth.md"),
            Ok(Locator::Path(path)) if path == "docs/auth.md"
        ));
    }

    #[test]
    fn rejects_unknown_locator_prefix() {
        assert!(parse_locator("symbol:Auth.Login").is_err());
    }

    #[test]
    fn rejects_whitespace_only_locator_values() {
        assert!(parse_locator("id:   ").is_err());
        assert!(parse_locator("path:   ").is_err());
    }
}
