use std::collections::BTreeMap;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct KirMetadataAnnotation {
    pub type_name: Option<String>,
    pub properties: Value,
    pub raw: Value,
}

pub fn metadata_annotations(properties: &BTreeMap<String, Value>) -> Vec<KirMetadataAnnotation> {
    properties
        .get("metadata")
        .map(metadata_value_annotations)
        .unwrap_or_default()
}

pub fn metadata_annotations_named(
    properties: &BTreeMap<String, Value>,
    type_name: &str,
) -> Vec<KirMetadataAnnotation> {
    metadata_annotations(properties)
        .into_iter()
        .filter(|annotation| {
            annotation
                .type_name
                .as_deref()
                .is_some_and(|candidate| metadata_type_matches(candidate, type_name))
        })
        .collect()
}

pub fn metadata_string_property(annotation: &KirMetadataAnnotation, key: &str) -> Option<String> {
    annotation
        .properties
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn metadata_value_annotations(value: &Value) -> Vec<KirMetadataAnnotation> {
    let mut annotations = Vec::new();
    collect_metadata_annotations(value, None, &mut annotations);
    annotations
}

fn collect_metadata_annotations(
    value: &Value,
    implied_type: Option<&str>,
    annotations: &mut Vec<KirMetadataAnnotation>,
) {
    if let Some(items) = value.as_array() {
        for item in items {
            collect_metadata_annotations(item, None, annotations);
        }
        return;
    }

    let Some(object) = value.as_object() else {
        return;
    };

    let explicit_type = object
        .get("type")
        .or_else(|| object.get("metatype"))
        .or_else(|| object.get("kind"))
        .and_then(Value::as_str);
    let type_name = explicit_type.or(implied_type).map(ToOwned::to_owned);

    if type_name.is_some() {
        annotations.push(KirMetadataAnnotation {
            type_name,
            properties: object
                .get("properties")
                .cloned()
                .unwrap_or_else(|| value.clone()),
            raw: value.clone(),
        });
    } else {
        annotations.push(KirMetadataAnnotation {
            type_name: None,
            properties: value.clone(),
            raw: value.clone(),
        });
    }

    for (key, nested) in object {
        if matches!(key.as_str(), "type" | "metatype" | "kind" | "properties") {
            continue;
        }
        if nested.is_object() || nested.is_array() {
            collect_metadata_annotations(nested, Some(key), annotations);
        }
    }
}

fn metadata_type_matches(candidate: &str, expected: &str) -> bool {
    candidate == expected
        || candidate
            .rsplit([':', '.', '/'])
            .find(|part| !part.is_empty())
            == Some(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_array_metadata_annotations() {
        let properties = BTreeMap::from([(
            "metadata".to_string(),
            json!([
                {
                    "type": "ContractRole",
                    "properties": {
                        "role": "assumption"
                    }
                }
            ]),
        )]);

        let annotations = metadata_annotations_named(&properties, "ContractRole");

        assert_eq!(annotations.len(), 1);
        assert_eq!(
            metadata_string_property(&annotations[0], "role").as_deref(),
            Some("assumption")
        );
    }

    #[test]
    fn extracts_object_keyed_metadata_annotations() {
        let properties = BTreeMap::from([(
            "metadata".to_string(),
            json!({
                "ContractRole": {
                    "properties": {
                        "role": "guarantee"
                    }
                }
            }),
        )]);

        let annotations = metadata_annotations_named(&properties, "ContractRole");

        assert_eq!(annotations.len(), 1);
        assert_eq!(
            metadata_string_property(&annotations[0], "role").as_deref(),
            Some("guarantee")
        );
    }
}
