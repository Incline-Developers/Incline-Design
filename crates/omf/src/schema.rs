use std::fmt::Write;

use schemars::{
    JsonSchema, Schema,
    generate::SchemaSettings,
    transform::{Transform, transform_subschemas},
};
use serde_json::{Value, json};

use crate::{Project, format_full_name};

fn simple_enum_variant(schema: &Value) -> Option<(String, String)> {
    let variant = schema
        .get("const")
        .or_else(|| {
            let values = schema.get("enum")?.as_array()?;
            (values.len() == 1).then(|| &values[0])
        })?
        .as_str()?;
    let description = schema.get("description")?.as_str()?;
    Some((variant.to_owned(), description.to_owned()))
}

#[derive(Debug, Clone, Default)]
struct TweakSchema {
    remove_descr: bool,
}

impl Transform for TweakSchema {
    fn transform(&mut self, schema: &mut Schema) {
        // Preserve the published schema's ordering and numeric representation.
        if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
            required.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        }
        for key in ["minimum", "maximum"] {
            if let Some(value) = schema.get_mut(key)
                && let Some(number) = value.as_f64()
            {
                *value = json!(number);
            }
        }
        // Keep the published OMF schema's single-value enums.
        if let Some(value) = schema.remove("const") {
            schema.insert("enum".into(), json!([value]));
        }
        if schema.get("format").and_then(Value::as_str) == Some("uint8") {
            schema.insert("maximum".into(), json!(255.0));
        }
        // Move descriptions of simple enum values into the parent.
        if let Some(variants) = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .and_then(|variants| {
                variants
                    .iter()
                    .map(simple_enum_variant)
                    .collect::<Option<Vec<_>>>()
            })
        {
            schema.remove("oneOf");
            schema.insert(
                "enum".into(),
                variants
                    .iter()
                    .map(|(name, _)| Value::from(name.clone()))
                    .collect(),
            );
            schema.insert("type".into(), json!("string"));
            let mut descr = schema
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            descr += "\n\n### Values\n\n";
            for (name, description) in variants {
                let body = description.replace("\n", "\n    ");
                write!(&mut descr, "`{name}`\n:   {body}\n\n").unwrap();
            }
            schema.insert("description".into(), descr.into());
        }
        if self.remove_descr {
            schema.remove("description");
        }
        transform_subschemas(self, schema);
    }
}

// Keep the OMF 2 schema dialect and reference paths stable across Schemars
// upgrades. Geometry payloads opt into inline schemas to retain tagged variants.
pub(crate) fn schema_for<T: JsonSchema>(remove_descr: bool) -> Schema {
    SchemaSettings::draft2019_09()
        .with(|settings| settings.definitions_path = "/definitions".into())
        .with_transform(TweakSchema { remove_descr })
        .into_generator()
        .into_root_schema_for::<T>()
}

pub(crate) fn project_schema(remove_descr: bool) -> Schema {
    let mut root = schema_for::<Project>(remove_descr);
    root.insert("title".into(), format_full_name().into());
    root.insert(
        "$id".into(),
        "https://github.com/gmggroup/omf-rust/blob/main/omf.schema.json".into(),
    );
    root
}

pub fn json_schema() -> Schema {
    project_schema(true)
}

#[cfg(test)]
pub(crate) mod tests {
    use schemars::Schema;

    use crate::schema::json_schema;

    const SCHEMA: &str = "omf.schema.json";

    #[ignore = "used to get schema"]
    #[test]
    fn update_schema() {
        std::fs::write(
            SCHEMA,
            serde_json::to_string_pretty(&json_schema())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        crate::schema_doc::update_schema_docs();
    }

    #[ignore = "used to get schema docs"]
    #[test]
    fn update_schema_docs() {
        crate::schema_doc::update_schema_docs();
        #[cfg(feature = "parquet")]
        crate::file::parquet::schemas::dump_parquet_schemas();
    }

    #[test]
    fn schema() {
        let schema = json_schema();
        let expected: Schema =
            serde_json::from_reader(std::fs::File::open(SCHEMA).unwrap()).unwrap();
        assert!(schema == expected, "schema has changed");
    }
}
