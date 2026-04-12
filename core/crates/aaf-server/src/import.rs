//! OpenAPI → ontology import (E2 Slice C).
//!
//! Reads an OpenAPI 3.x document (YAML or JSON) and produces one
//! `EntityProposal` per entry in `#/components/schemas/*`. The output
//! is explicitly a *proposal*: every entity carries a reminder
//! comment that classification defaults to `Internal` and must be
//! audited before merging.
//!
//! The import is intentionally permissive — unknown OpenAPI
//! constructs are mapped to `FieldType::Object` with a loss note
//! rather than rejected — because the whole point is to hand a
//! human reviewer a starting point, not to produce a
//! wire-format-perfect ontology in one shot.

use serde::Deserialize;
use std::collections::BTreeMap;

/// One proposed entity derived from an OpenAPI schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityProposal {
    /// Dot-qualified entity id, e.g. `"commerce.Order"`. Derived
    /// from an `x-aaf-entity-id` extension when present, otherwise
    /// the schema key prefixed with the document's title slug.
    pub id: String,
    /// Human-readable description pulled from `description:`.
    pub description: String,
    /// Field name → type string. Types are the primitive-ish set the
    /// ontology module supports (`string`, `integer`, `float`,
    /// `boolean`, `timestamp`, `object`).
    pub fields: BTreeMap<String, String>,
}

/// Import errors surfaced to the caller. The CLI prints the error
/// and exits non-zero.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Input is neither valid YAML nor valid JSON at the top level.
    #[error("input is not valid YAML/JSON: {0}")]
    Parse(String),
    /// Input does not look like an OpenAPI document at all
    /// (missing `components.schemas`).
    #[error("not an OpenAPI document: missing components.schemas")]
    NotOpenApi,
}

#[derive(Debug, Deserialize)]
struct OpenApiDoc {
    #[serde(default)]
    info: OpenApiInfo,
    components: Option<OpenApiComponents>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenApiInfo {
    #[serde(default)]
    title: String,
}

#[derive(Debug, Deserialize)]
struct OpenApiComponents {
    #[serde(default)]
    schemas: BTreeMap<String, OpenApiSchema>,
}

#[derive(Debug, Deserialize)]
struct OpenApiSchema {
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    properties: BTreeMap<String, OpenApiProperty>,
    #[serde(default, rename = "x-aaf-entity-id")]
    entity_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenApiProperty {
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

fn slugify(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn map_type(prop: &OpenApiProperty) -> &'static str {
    match prop.type_.as_deref() {
        Some("string") => match prop.format.as_deref() {
            Some("date-time") => "timestamp",
            _ => "string",
        },
        Some("integer") => "integer",
        Some("number") => "float",
        Some("boolean") => "boolean",
        Some("object" | "array") => "object",
        _ => "object",
    }
}

/// Parse an OpenAPI document (YAML or JSON) and return one proposal
/// per `components.schemas` entry.
pub fn import_openapi(raw: &str) -> Result<Vec<EntityProposal>, ImportError> {
    let doc: OpenApiDoc = match serde_yaml::from_str(raw) {
        Ok(d) => d,
        Err(_) => match serde_json::from_str::<OpenApiDoc>(raw) {
            Ok(d) => d,
            Err(e) => return Err(ImportError::Parse(e.to_string())),
        },
    };
    let components = doc.components.ok_or(ImportError::NotOpenApi)?;
    if components.schemas.is_empty() {
        return Err(ImportError::NotOpenApi);
    }

    let title_slug = if doc.info.title.is_empty() {
        "imported".to_string()
    } else {
        slugify(&doc.info.title)
    };

    let mut out = vec![];
    for (schema_name, schema) in components.schemas {
        let id = schema
            .entity_id
            .clone()
            .unwrap_or_else(|| format!("{title_slug}.{schema_name}"));
        let mut fields = BTreeMap::new();
        for (fname, prop) in &schema.properties {
            fields.insert(fname.clone(), map_type(prop).to_string());
        }
        // If the schema has no declared properties (or uses `allOf` /
        // `oneOf` / `$ref` which we don't traverse), still emit a
        // shell so the human can fill it in.
        if fields.is_empty() && schema.type_.as_deref() != Some("string") {
            fields.insert("(todo)".into(), "object".into());
        }
        out.push(EntityProposal {
            id,
            description: schema.description,
            fields,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Render a slice of proposals as a single YAML document (with
/// review-required comments) matching the shape of
/// `spec/examples/ontology-commerce.yaml`.
pub fn render_yaml(proposals: &[EntityProposal]) -> String {
    let mut out = String::new();
    out.push_str("# AUTO-GENERATED by `aaf-server ontology import` — HUMAN REVIEW REQUIRED.\n");
    out.push_str("# Classification defaults to `internal` for every entity. You MUST audit\n");
    out.push_str("# and downgrade / upgrade per data-handling policy before merging.\n");
    out.push_str("entities:\n");
    for p in proposals {
        out.push_str(&format!("  - id: {}\n", p.id));
        out.push_str("    version: 0.1.0\n");
        let desc_line = if p.description.is_empty() {
            "(imported — add a description)".into()
        } else {
            p.description.replace('\n', " ")
        };
        out.push_str(&format!("    description: {desc_line}\n"));
        out.push_str(
            "    classification: internal   # review: may need to be confidential or pii\n",
        );
        out.push_str("    fields:\n");
        for (fname, ftype) in &p.fields {
            out.push_str(&format!(
                "      - name: {fname}\n        field_type: {ftype}\n        required: false\n"
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENAPI: &str = r#"
openapi: 3.0.0
info:
  title: Commerce API
  version: 1.0.0
components:
  schemas:
    Order:
      type: object
      description: A customer order
      properties:
        id: { type: string }
        placed_at: { type: string, format: date-time }
        total_jpy: { type: integer }
    Customer:
      type: object
      description: A customer
      x-aaf-entity-id: commerce.Customer
      properties:
        email: { type: string }
        vip: { type: boolean }
"#;

    #[test]
    fn imports_two_schemas_and_honours_x_aaf_entity_id() {
        let props = import_openapi(OPENAPI).unwrap();
        assert_eq!(props.len(), 2);
        // Sorted by id → commerce.Customer first, commerce_api.Order second.
        let ids: Vec<&str> = props.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["commerce.Customer", "commerce_api.Order"]);
        let order = &props[1];
        assert_eq!(order.description, "A customer order");
        assert_eq!(order.fields.get("id"), Some(&"string".to_string()));
        assert_eq!(
            order.fields.get("placed_at"),
            Some(&"timestamp".to_string())
        );
        assert_eq!(order.fields.get("total_jpy"), Some(&"integer".to_string()));
    }

    #[test]
    fn render_yaml_has_review_header() {
        let props = import_openapi(OPENAPI).unwrap();
        let out = render_yaml(&props);
        assert!(out.contains("HUMAN REVIEW REQUIRED"));
        assert!(out.contains("id: commerce.Customer"));
    }

    #[test]
    fn rejects_non_openapi_doc() {
        let err = import_openapi("just: a: map:").unwrap_err();
        assert!(matches!(
            err,
            ImportError::Parse(_) | ImportError::NotOpenApi
        ));
    }

    #[test]
    fn missing_components_is_not_openapi() {
        let err = import_openapi("openapi: 3.0.0\ninfo: {title: X}\n").unwrap_err();
        assert!(matches!(err, ImportError::NotOpenApi));
    }
}
