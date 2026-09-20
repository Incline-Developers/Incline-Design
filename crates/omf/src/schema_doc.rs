// Note: this outputs Python Markdown for mkdocs rather than the CommonMark that rustdoc uses.
use core::panic;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::Path,
    sync::OnceLock,
};

use schemars::{
    Schema,
    transform::{Transform, transform_subschemas},
};
use serde_json::{Map, Value};

use crate::schema::{project_schema, schema_for};

pub(crate) fn update_schema_docs() {
    let schema = project_schema(false);
    let base_dir = Path::new("docs/schema");
    create_dir_all(Path::new(base_dir)).unwrap();
    let mut project = schema.clone();
    project.remove("$schema");
    project.remove("definitions");
    object(base_dir, "Project", project.as_value()).unwrap();
    for (name, schema) in schema
        .get("definitions")
        .and_then(Value::as_object)
        .unwrap()
    {
        if name == "Geometry" {
            geometry(base_dir, schema).unwrap();
        } else if name == "NumberRange" {
            number_colormap_range(base_dir, name, schema).unwrap();
        } else {
            object(base_dir, name, schema).unwrap();
        }
    }
}

fn geometry(base_dir: &Path, schema: &Value) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(base_dir.join("Geometry.md"))?;
    write!(f, "<!-- Generated documentation, do not edit -->\n\n")?;
    // Title.
    write!(f, "# Geometry\n\n")?;
    // Description paragraph.
    let descr = description(&schema);
    write!(f, "{descr}\n\n")?;
    // List of options.
    write!(f, "## Options\n\n")?;
    let variants = schema
        .get("oneOf")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("unknown geometry = {schema:#?}"));
    for item in variants {
        let child_schema = item;
        let name = variant_name(child_schema);
        write!(f, "- [{name}]({name}.md)\n")?;
        object(base_dir, name, child_schema)?;
    }
    write!(f, "\n")?;
    // Code.
    schema_object_code(&mut f, schema)
}

fn object(base_dir: &Path, name: &str, schema: &Value) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(base_dir.join(format!("{name}.md")))?;
    write!(f, "<!-- Generated documentation, do not edit -->\n\n")?;
    // Title.
    write!(f, "# {name}\n\n")?;
    // Description paragraph.
    let descr = description(&schema);
    write!(f, "{descr}\n\n")?;
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        write!(f, "## Fields\n\n")?;
        struct_fields(
            &mut f,
            properties,
            schema.get("additionalProperties").is_some(),
        )?;
    } else if let Some(variants) = schema.get("oneOf").and_then(Value::as_array) {
        enum_variants(&mut f, variants)?;
    } else if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !descr.contains("## Values") {
            simple_enum_values(&mut f, values)?;
        }
    } else {
        panic!("unknown schema: {schema:#?}");
    }
    // Code.
    schema_object_code(&mut f, schema)
}

fn number_colormap_range(base_dir: &Path, name: &str, schema: &Value) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(base_dir.join(format!("{name}.md")))?;
    write!(f, "<!-- Generated documentation, do not edit -->\n\n")?;
    // Title.
    write!(f, "# {name}\n\n")?;
    // Description paragraph.
    let descr = description(&schema);
    write!(f, "{descr}\n\n")?;
    ty_defn_list_item(
        &mut f,
        "min",
        "double, integer, date, or date-time",
        "Minimum value of the range. The type should match the associated number array type.",
    )?;
    ty_defn_list_item(
        &mut f,
        "max",
        "double, integer, date, or date-time",
        "Maximum value of the range. Must have the same type as `min`.",
    )?;
    // Code.
    schema_object_code(&mut f, schema)
}

macro_rules! re {
    ($text:literal) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($text).unwrap())
    }};
}

fn description(schema: &Value) -> String {
    use regex::{Captures, Regex};

    let link = re!(r#"\]\(crate::(?<page>\w+)(::(?<anchor>\w+))?\)"#);
    let svg = re!(r#"<!--\s*(.*?\.svg)\s*-->"#);
    let Some(descr) = schema.get("description").and_then(Value::as_str) else {
        return String::new();
    };
    // Schemars 1 includes doc attributes expanded by include_str!. The SVGs are
    // already linked by the marker above, so omit their embedded XML here.
    let embedded_svg = re!(r"(?s)<\?xml.*?</svg>");
    let descr = embedded_svg.replace_all(descr, "");
    let s = link.replace_all(&descr, |caps: &Captures| {
        let page = caps.name("page").unwrap().as_str();
        if let Some(anchor) = caps.name("anchor") {
            format!("]({page}.md#{anchor})", anchor = anchor.as_str())
        } else {
            format!("]({page}.md)")
        }
    });
    svg.replace_all(&s, |caps: &Captures| {
        format!(
            "![{svg}](../images/{svg})",
            svg = caps.get(1).unwrap().as_str()
        )
    })
    .into_owned()
}

fn struct_fields(
    f: &mut impl Write,
    properties: &Map<String, Value>,
    additional: bool,
) -> std::io::Result<()> {
    for (name, child) in properties {
        if name == "type" {
            continue;
        }
        if child.is_object() {
            let schema = child;
            ty_defn_list_item(f, name, &field_type(schema), &description(schema))?;
        } else {
            defn_list_item(f, name, "")?;
        }
    }
    if additional {
        write!(f, "- Plus any custom values.\n\n")?;
    }
    Ok(())
}

fn simple_enum_values(f: &mut impl Write, values: &Vec<Value>) -> std::io::Result<()> {
    write!(f, "## Values\n\n")?;
    for value in values {
        if let Value::String(s) = value {
            let fixed = s.replace("\n\n", "\n\n    ");
            write!(f, "- {fixed}\n")?;
        } else {
            panic!("enum variant not a string: {value:#?}")
        }
    }
    write!(f, "\n")
}

fn enum_variants(f: &mut impl Write, variants: &[Value]) -> std::io::Result<()> {
    for schema in variants {
        if schema.is_object() {
            let name = variant_name(schema);
            write!(f, "## {name}\n\n")?;
            write!(f, "{}\n\n", description(schema))?;
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                write!(f, "### Fields\n\n")?;
                defn_list_item(f, "type", &format!("`\"{name}\"`"))?;
                struct_fields(f, properties, schema.get("additionalProperties").is_some())?;
            }
        }
    }
    Ok(())
}

fn variant_name(schema: &Value) -> &str {
    schema
        .pointer("/properties/type/enum/0")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected enum variant: {schema:#?}"))
}

fn field_type(schema: &Value) -> String {
    if let Some(ty) = schema.get("$ref").and_then(Value::as_str) {
        let name = ty.strip_prefix("#/definitions/").unwrap_or(ty);
        format!("[`{name}`]({name}.md)")
    } else if let Some(name) = known_type(schema) {
        name
    } else if let Some(name) = optional_type(schema) {
        name
    } else if let Some(name) = array_type(schema) {
        name
    } else if let Some(ty) = schema.get("type").and_then(Value::as_str) {
        instance_type(ty)
            .unwrap_or_else(|| panic!("unknown array: {schema:#?}"))
            .to_owned()
    } else {
        panic!("unknown type: {schema:#?}");
    }
}

fn instance_type(t: &str) -> Option<&str> {
    match t {
        "boolean" => Some("bool"),
        "array" => None,
        _ => Some(t),
    }
}

fn known_type(schema: &Value) -> Option<String> {
    fn no_meta(schema: &Value) -> Value {
        let mut copy = schema.clone();
        if let Some(object) = copy.as_object_mut() {
            for key in [
                "$schema",
                "$id",
                "title",
                "description",
                "default",
                "deprecated",
                "readOnly",
                "writeOnly",
                "examples",
            ] {
                object.remove(key);
            }
        }
        copy
    }
    static TYPES: OnceLock<[(&str, Value); 3]> = OnceLock::new();
    let types = TYPES.get_or_init(|| {
        [
            (
                "RGB color (uint8, 3 items)",
                no_meta(schema_for::<[u8; 3]>(true).as_value()),
            ),
            (
                "RGB color (uint8, 3 items) or null",
                no_meta(schema_for::<Option<[u8; 3]>>(true).as_value()),
            ),
            (
                "3D vector",
                no_meta(schema_for::<[f64; 3]>(true).as_value()),
            ),
        ]
    });
    let schema = no_meta(schema);
    types
        .iter()
        .find_map(|(name, ty)| (ty == &schema).then(|| (*name).to_owned()))
}

fn optional_type(schema: &Value) -> Option<String> {
    if let Some(items) = schema.get("anyOf").and_then(Value::as_array)
        && let [first, second] = &items[..]
        && second == &serde_json::json!({"type": "null"})
    {
        return Some(format!("{} or null", field_type(first)));
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array)
        && let [first, second] = &types[..]
        && second == "null"
    {
        return Some(format!("{} or null", instance_type(first.as_str()?)?));
    }
    None
}

fn array_type(schema: &Value) -> Option<String> {
    let item = schema.get("items")?;
    item.as_object()?;
    if ["additionalItems", "uniqueItems", "contains"]
        .iter()
        .any(|key| schema.get(key).is_some())
    {
        return None;
    }
    let ty = field_type(item);
    let min = schema.get("minItems").and_then(Value::as_u64);
    let max = schema.get("maxItems").and_then(Value::as_u64);
    Some(match (min, max) {
        (None, None) => format!("array of {ty}"),
        (None, Some(m)) => format!("array of {ty}, up to {m} items"),
        (Some(n), None) => format!("array of {ty}, at least {n} items"),
        (Some(n), Some(m)) if n == m => format!("array of {ty}, {n} items"),
        (Some(n), Some(m)) => format!("array of {ty}, {n} to {m} items"),
    })
}

fn defn_list_item(f: &mut impl Write, title: &str, body: &str) -> std::io::Result<()> {
    let fixed_body = body.trim().replace("\n", "\n    ");
    write!(f, "`{title}`\n:   {fixed_body}\n\n")
}

fn ty_defn_list_item(f: &mut impl Write, title: &str, ty: &str, body: &str) -> std::io::Result<()> {
    let fixed_body = body.trim().replace("\n", "\n    ");
    write!(f, "`{title}`: {ty}\n:   {fixed_body}\n\n")
}

struct RemoveDescriptions;

impl Transform for RemoveDescriptions {
    fn transform(&mut self, schema: &mut Schema) {
        schema.remove("description");
        transform_subschemas(self, schema);
    }
}

fn schema_object_code(f: &mut impl Write, schema: &Value) -> std::io::Result<()> {
    write!(f, "## Schema\n\n")?;
    let mut no_descr = Schema::try_from(schema.clone()).expect("valid schema");
    RemoveDescriptions.transform(&mut no_descr);
    let code = serde_json::to_string_pretty(&no_descr).unwrap();
    write!(f, "```json\n{code}\n```\n")
}
