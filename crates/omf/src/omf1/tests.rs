//! Conversion of OMF 1 files built here, record by record, covering every
//! element, geometry and data type the reference writer produced.

use std::io::{Cursor, Write};

use flate2::{Compression, write::ZlibEncoder};
use serde_json::{Value, json};

use super::Converter;
use crate::{AttributeData, Geometry, Grid2, Grid3, Location, file::Reader};

/// Writes an OMF 1 file: the 60-byte header, zlib-compressed binary arrays,
/// then the JSON record table.
struct Omf1Builder {
    binary: Vec<u8>,
    records: serde_json::Map<String, Value>,
    next: u32,
}

const HEADER_LEN: usize = 60;
const PROJECT_UID: [u8; 16] = [0x11; 16];

impl Omf1Builder {
    fn new() -> Self {
        Self {
            binary: Vec::new(),
            records: serde_json::Map::new(),
            next: 1,
        }
    }

    fn uid(&mut self) -> String {
        self.next += 1;
        format!("00000000-0000-4000-8000-{:012x}", self.next)
    }

    fn record(&mut self, class: &str, mut fields: Value) -> String {
        let uid = self.uid();
        let object = fields.as_object_mut().expect("record fields are an object");
        object.insert("__class__".to_owned(), class.into());
        object.insert("date_created".to_owned(), "2017-10-04T21:46:12Z".into());
        object.insert("date_modified".to_owned(), "2017-10-04T21:46:12Z".into());
        self.records.insert(uid.clone(), fields);
        uid
    }

    fn binary(&mut self, bytes: &[u8], dtype: &str) -> Value {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        let compressed = encoder.finish().unwrap();
        let start = HEADER_LEN + self.binary.len();
        self.binary.extend_from_slice(&compressed);
        json!({ "start": start, "length": compressed.len(), "dtype": dtype })
    }

    fn floats(&mut self, class: &str, values: &[f64]) -> String {
        let bytes = values.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>();
        let array = self.binary(&bytes, "<f8");
        self.record(class, json!({ "array": array }))
    }

    fn ints(&mut self, class: &str, values: &[i64]) -> String {
        let bytes = values.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>();
        let array = self.binary(&bytes, "<i8");
        self.record(class, json!({ "array": array }))
    }

    fn png(&mut self) -> Value {
        // A 1x1 RGB PNG.
        const PNG: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d,
            0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let mut image = self.binary(PNG, "image/png");
        image["dtype"] = "image/png".into();
        image
    }

    fn finish(mut self, elements: Vec<String>) -> Vec<u8> {
        let project = json!({
            "__class__": "Project",
            "name": "Old project",
            "description": "",
            "author": null,
            "revision": "",
            "date": "2017/10/04 21:46:12",
            "units": "m",
            "origin": [10.0, 20.0, 0.0],
            "elements": elements,
            "date_created": "2017-10-04T21:46:12Z",
            "date_modified": "2017-10-04T21:46:12Z",
        });
        self.records.insert(project_key(), project);
        let json = serde_json::to_vec(&Value::Object(self.records)).unwrap();
        let mut file = Vec::new();
        file.extend_from_slice(&[0x84, 0x83, 0x82, 0x81]);
        let mut version = [0_u8; 32];
        version[..10].copy_from_slice(b"OMF-v0.9.0");
        file.extend_from_slice(&version);
        file.extend_from_slice(&PROJECT_UID);
        file.extend_from_slice(&((HEADER_LEN + self.binary.len()) as u64).to_le_bytes());
        file.extend_from_slice(&self.binary);
        file.extend_from_slice(&json);
        file
    }
}

fn project_key() -> String {
    super::objects::Key::from_bytes(PROJECT_UID).value
}

fn content(name: &str) -> Value {
    json!({ "name": name, "description": "" })
}

fn merge(mut a: Value, b: Value) -> Value {
    for (key, value) in b.as_object().unwrap() {
        a[key] = value.clone();
    }
    a
}

fn build_everything() -> Vec<u8> {
    let mut b = Omf1Builder::new();
    let mut elements = Vec::new();

    // Points, with a texture, a vector and a date-time attribute.
    let vertices = b.floats("Vector3Array", &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let geometry = b.record("PointSetGeometry", json!({ "origin": [0.0, 0.0, 0.0], "vertices": vertices }));
    let vectors = b.floats("Vector3Array", &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, f64::NAN, 0.0, 0.0]);
    let vector_data = b.record("Vector3Data", merge(content("dip"), json!({ "location": "vertices", "array": vectors })));
    let dates = b.record("DateTimeArray", json!({ "array": ["2017-10-04T21:46:12Z", "", null] }));
    let date_data = b.record(
        "DateTimeData",
        merge(content("sampled"), json!({ "location": "vertices", "array": dates, "colormap": null })),
    );
    let image = b.png();
    let texture = b.record(
        "ImageTexture",
        merge(
            content("photo"),
            json!({ "origin": [0.0, 0.0, 0.0], "axis_u": [2.0, 0.0, 0.0], "axis_v": [0.0, 2.0, 0.0], "image": image }),
        ),
    );
    elements.push(b.record(
        "PointSetElement",
        merge(
            content("collars"),
            json!({ "color": "#ff8000", "subtype": "collar", "geometry": geometry, "data": [vector_data, date_data], "textures": [texture] }),
        ),
    ));

    // Lines, with string data.
    let vertices = b.floats("Vector3Array", &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0]);
    let segments = b.ints("Int2Array", &[0, 1, 1, 2]);
    let geometry = b.record("LineSetGeometry", json!({ "vertices": vertices, "segments": segments }));
    let strings = b.record("StringArray", json!({ "array": ["a", ""] }));
    let string_data = b.record("StringData", merge(content("label"), json!({ "location": "segments", "array": strings })));
    elements.push(b.record(
        "LineSetElement",
        merge(
            content("strings"),
            json!({ "color": [0, 0, 255], "subtype": "borehole", "geometry": geometry, "data": [string_data] }),
        ),
    ));

    // A triangulated surface, with mapped data that has no legends and
    // uses the largest index, colors as an Int3Array, and a colormap.
    let vertices = b.floats("Vector3Array", &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0]);
    let triangles = b.ints("Int3Array", &[0, 1, 2, 1, 3, 2]);
    let geometry = b.record("SurfaceGeometry", json!({ "vertices": vertices, "triangles": triangles }));
    let indices = b.ints("ScalarArray", &[0, 1]);
    let mapped = b.record("MappedData", merge(content("domain"), json!({ "location": "faces", "array": indices, "legends": [] })));
    let colors = b.ints("Int3Array", &[255, 0, 0, 0, 255, 0]);
    let color_data = b.record("ColorData", merge(content("paint"), json!({ "location": "faces", "array": colors })));
    let gradient = b.record("ColorArray", json!({ "array": [[0, 0, 0], [255, 255, 255]] }));
    let colormap = b.record("ScalarColormap", merge(content(""), json!({ "gradient": gradient, "limits": [0.0, 10.0] })));
    let heights = b.floats("ScalarArray", &[0.0, 1.0, f64::NAN, 3.0]);
    let scalar = b.record(
        "ScalarData",
        merge(content("height"), json!({ "location": "vertices", "array": heights, "colormap": colormap })),
    );
    elements.push(b.record(
        "SurfaceElement",
        merge(
            content("topo"),
            json!({ "color": null, "subtype": "surface", "geometry": geometry, "data": [mapped, color_data, scalar], "textures": [] }),
        ),
    ));

    // A grid surface, with mapped data whose legends are shorter than the
    // index range, a vector-2 attribute and a texture.
    let offsets = b.floats("ScalarArray", &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    let geometry = b.record(
        "SurfaceGridGeometry",
        json!({ "origin": [0.0, 0.0, 0.0], "tensor_u": [1.0, 1.0], "tensor_v": [2.0], "axis_u": [1.0, 0.0, 0.0], "axis_v": [0.0, 1.0, 0.0], "offset_w": offsets }),
    );
    let indices = b.ints("ScalarArray", &[0, 2]);
    let names = b.record("StringArray", json!({ "array": ["waste", "ore"] }));
    let names_legend = b.record("Legend", merge(content("names"), json!({ "values": names })));
    let legend_colors = b.record("ColorArray", json!({ "array": [[1, 2, 3], "#ffffff"] }));
    let colors_legend = b.record("Legend", merge(content("colors"), json!({ "values": legend_colors })));
    let densities = b.floats("ScalarArray", &[2.7]);
    let density_legend = b.record("Legend", merge(content("density"), json!({ "values": densities })));
    let mapped = b.record(
        "MappedData",
        merge(
            content("rock"),
            json!({ "location": "faces", "array": indices, "legends": [names_legend, colors_legend, density_legend] }),
        ),
    );
    let flows = b.floats("Vector2Array", &[1.0, 0.0, 0.0, 1.0]);
    let flow = b.record("Vector2Data", merge(content("flow"), json!({ "location": "faces", "array": flows })));
    let image = b.png();
    let texture = b.record("ImageTexture", merge(content("map"), json!({ "image": image })));
    elements.push(b.record(
        "SurfaceElement",
        merge(
            content("grid"),
            json!({ "geometry": geometry, "data": [mapped, flow], "textures": [texture], "subtype": "surface" }),
        ),
    ));

    // A block model, with data whose cells are all null categories.
    let geometry = b.record("VolumeGridGeometry", json!({ "tensor_u": [5.0, 5.0], "tensor_v": [5.0], "tensor_w": [2.0, 2.0, 2.0] }));
    let grades = b.floats("ScalarArray", &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
    let grade = b.record("ScalarData", merge(content("grade"), json!({ "location": "cells", "array": grades, "colormap": null })));
    let nothing = b.ints("ScalarArray", &[-1, -1, -1, -1, -1, -1]);
    let empty = b.record("MappedData", merge(content("unassigned"), json!({ "location": "cells", "array": nothing })));
    elements.push(b.record(
        "VolumeElement",
        merge(content("blocks"), json!({ "geometry": geometry, "data": [grade, empty], "subtype": "volume" })),
    ));

    // A record type no OMF 1 reader knows, referenced by nothing.
    b.record("SomeoneElsesRecord", json!({ "anything": [1, 2, 3] }));

    b.finish(elements)
}

fn convert(bytes: Vec<u8>) -> crate::Project {
    let mut output = Cursor::new(Vec::new());
    let problems = Converter::new().convert(bytes, &mut output).expect("OMF 1 converts");
    assert!(problems.iter().all(|problem| !problem.is_error()), "{problems}");
    let reader = Reader::new(output.into_inner()).expect("converted archive opens");
    let (project, problems) = reader.project().expect("converted project reads");
    assert!(problems.iter().all(|problem| !problem.is_error()), "{problems}");
    project
}

#[test]
fn detects_omf1() {
    let bytes = build_everything();
    assert!(super::detect(&mut Cursor::new(&bytes)).unwrap());
    assert!(!super::detect(&mut Cursor::new(b"PK\x03\x04 not omf 1")).unwrap());
}

#[test]
fn converts_every_element_type() {
    let project = convert(build_everything());
    assert_eq!(project.name, "Old project");
    assert_eq!(project.units, "m");
    assert_eq!(project.origin, [10.0, 20.0, 0.0]);
    let names = project.elements.iter().map(|element| element.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, ["collars", "strings", "topo", "grid", "blocks"]);

    let collars = &project.elements[0];
    assert!(matches!(collars.geometry, Geometry::PointSet(_)));
    assert_eq!(collars.color, Some([255, 128, 0, 255]));
    assert!(collars.attributes.iter().any(|attribute| attribute.location == Location::Projected));

    let grid = &project.elements[3];
    let Geometry::GridSurface(surface) = &grid.geometry else { panic!("grid surface") };
    assert!(matches!(surface.grid, Grid2::Tensor { .. }));
    assert!(
        grid.attributes.iter().any(|attribute| attribute.location == Location::Projected),
        "grid surface keeps its texture"
    );
    let rock = grid.attributes.iter().find(|attribute| attribute.name == "rock").unwrap();
    let AttributeData::Category { names, gradient, attributes, .. } = &rock.data else {
        panic!("category")
    };
    // Index 2 is used, so there are three categories though the legends
    // only name two.
    assert_eq!(names.item_count(), 3);
    assert_eq!(gradient.as_ref().map(|gradient| gradient.item_count()), Some(3));
    assert_eq!(attributes.len(), 1);

    let topo = &project.elements[2];
    let domain = topo.attributes.iter().find(|attribute| attribute.name == "domain").unwrap();
    let AttributeData::Category { names, .. } = &domain.data else { panic!("category") };
    assert_eq!(names.item_count(), 2, "indices 0 and 1 with no legend make two categories");

    let blocks = &project.elements[4];
    let Geometry::BlockModel(model) = &blocks.geometry else { panic!("block model") };
    assert!(matches!(model.grid, Grid3::Tensor { .. }));
    let empty = blocks.attributes.iter().find(|attribute| attribute.name == "unassigned").unwrap();
    let AttributeData::Category { names, .. } = &empty.data else { panic!("category") };
    assert_eq!(names.item_count(), 0);
}

#[test]
fn converts_the_reference_writers_file() {
    use base64::Engine as _;
    let text = include_str!("fixtures/omf1_test_proj.b64");
    let bytes = base64::engine::general_purpose::STANDARD.decode(text.split_whitespace().collect::<String>()).unwrap();
    let project = convert(bytes);
    assert_eq!(project.elements.len(), 2);
}

#[test]
fn unreadable_record_fails_only_where_used() {
    let mut b = Omf1Builder::new();
    let geometry = b.record("PointSetGeometry", json!({ "vertices": 42 }));
    let element = b.record("PointSetElement", merge(content("broken"), json!({ "geometry": geometry })));
    let error = Converter::new().convert(b.finish(vec![element]), Cursor::new(Vec::new())).unwrap_err();
    assert!(error.to_string().contains("could not be read"), "{error}");
}

/// A file from the reference OMF 1 writer (Python `omf` 1.0.1) holding every
/// element type, texture and data type it can write.
#[test]
fn converts_every_type_from_the_reference_writer() {
    use base64::Engine as _;
    let text = include_str!("fixtures/every_type_v1.b64");
    let bytes = base64::engine::general_purpose::STANDARD.decode(text.split_whitespace().collect::<String>()).unwrap();
    let project = convert(bytes);
    assert_eq!(project.name, "Reference writer");
    assert_eq!(project.origin, [100.0, 200.0, 0.0]);
    let names = project.elements.iter().map(|element| element.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, ["collars", "strings", "topo", "grid", "blocks"]);
    let topo = &project.elements[2];
    assert!(topo.attributes.iter().any(|attribute| attribute.location == Location::Projected));
    let rock = topo.attributes.iter().find(|attribute| attribute.name == "rock").unwrap();
    let AttributeData::Category { names, gradient, .. } = &rock.data else {
        panic!("category")
    };
    assert_eq!(names.item_count(), 2);
    assert!(gradient.is_some());
}
