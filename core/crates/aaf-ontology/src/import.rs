//! Best-effort import from external shapes (OpenAPI / JSON Schema).
//!
//! v0.1 ships a very small JSON-Schema-ish importer whose job is to
//! turn an object schema into a draft [`Entity`]. The output is
//! **always a proposal** — never auto-registered — per Rule O2.

use crate::entity::{Classification, Entity, EntityField, FieldType};

/// Import a flat object schema (`{ "type": "object", "properties":
/// {...} }`) into a draft entity. Nested objects become
/// `FieldType::Object`, arrays become `FieldType::List(inner)`.
pub fn import_object_schema(
    id: impl Into<String>,
    description: impl Into<String>,
    classification: Classification,
    schema: &serde_json::Value,
) -> Entity {
    let mut entity = Entity::new(id, description, classification);
    if let Some(obj) = schema.get("properties").and_then(|v| v.as_object()) {
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (name, field) in obj {
            let field_type = match field.get("type").and_then(|v| v.as_str()) {
                Some("string") => FieldType::String,
                Some("integer") => FieldType::Integer,
                Some("number") => FieldType::Float,
                Some("boolean") => FieldType::Boolean,
                Some("object") => FieldType::Object,
                Some("array") => {
                    let inner = field
                        .get("items")
                        .and_then(|i| i.get("type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("string")
                        .to_string();
                    FieldType::List(inner)
                }
                _ => FieldType::Object,
            };
            entity.fields.push(EntityField {
                name: name.clone(),
                field_type,
                required: required.contains(name),
                classification: None,
                description: field
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_flat_object_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["id", "total"],
            "properties": {
                "id": {"type": "string", "description": "order id"},
                "total": {"type": "number"},
                "items": {"type": "array", "items": {"type": "string"}},
                "customer": {"type": "object"}
            }
        });
        let e = import_object_schema(
            "commerce.Order",
            "an order",
            Classification::Internal,
            &schema,
        );
        assert_eq!(e.fields.len(), 4);
        let id_field = e.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id_field.required);
        assert_eq!(id_field.field_type, FieldType::String);
        let items_field = e.fields.iter().find(|f| f.name == "items").unwrap();
        assert_eq!(items_field.field_type, FieldType::List("string".into()));
    }
}
