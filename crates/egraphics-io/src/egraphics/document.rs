use std::collections::HashMap;

use egraphics_core::PropertyValue;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EgraphicsInfoDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub georeference: Option<GeoreferenceSerde>,
    pub meshes: Vec<MeshInfoSerde>,
    pub objects: Vec<ObjectInfoSerde>,
}

/// Sidecar mirror of `egraphics_core::Georeference`. `crs` and `offset` are independently
/// optional, omitted from JSON when unset. `offset` is `[f64; 3]` to preserve full precision
/// (unlike every `[f32; ...]` transform elsewhere in this schema).
#[derive(Serialize, Deserialize)]
pub struct GeoreferenceSerde {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f64; 3]>,
}

/// Geometry entry. One per unique mesh written to the binary buffer.
///
/// `id` is the position of this entry in the `meshes` array. `geometry_id` is
/// omitted from JSON when `None`.
#[derive(Serialize, Deserialize)]
pub struct MeshInfoSerde {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry_id: Option<String>,
    pub lod: u8,
    pub vertex_offset: u64,
    pub vertex_count: u64,
    pub index_offset: u64,
    pub index_count: u64,
    pub aabb: AabbSerde,
}

/// Object entry. `transform` present only for instanced objects (local-space geometry);
/// omitted for combined objects (world-space). `object_name` omitted when `None`.
#[derive(Serialize, Deserialize)]
pub struct ObjectInfoSerde {
    pub object_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,
    pub class_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f32; 16]>,
    pub aabb: AabbSerde,
    pub meshes: Vec<MeshInfoRefSerde>,
    /// Generic business attributes, omitted from JSON when empty.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, serde_json::Value>,
}

/// Reference from an object to a mesh entry by index into `EgraphicsInfoDocument::meshes`.
///
/// `transform`, when present, is the **fully composed, self-sufficient world transform** for
/// this reference — apply it directly, no need to also consult `ObjectInfoSerde.transform`.
/// When absent, the referenced mesh's vertex data is already in world space. An object's
/// `meshes` list can freely mix both kinds of entries.
#[derive(Serialize, Deserialize)]
pub struct MeshInfoRefSerde {
    pub mesh: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f32; 16]>,
}

/// Converts a core [`PropertyValue`] to the JSON value stored in [`ObjectInfoSerde::properties`].
pub fn property_value_to_json(value: &PropertyValue) -> serde_json::Value {
    match value {
        PropertyValue::String(s) => serde_json::Value::String(s.clone()),
        PropertyValue::Int(n) => serde_json::json!(n),
        PropertyValue::Float(n) => serde_json::json!(n),
        PropertyValue::Bool(b) => serde_json::Value::Bool(*b),
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct AabbSerde {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

pub fn to_json_bytes(
    doc: &EgraphicsInfoDocument,
    pretty: bool,
) -> Result<Vec<u8>, serde_json::Error> {
    if pretty {
        serde_json::to_vec_pretty(doc)
    } else {
        serde_json::to_vec(doc)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn empty_aabb() -> AabbSerde {
        AabbSerde {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        }
    }

    fn round_trip(doc: &EgraphicsInfoDocument) -> serde_json::Value {
        let mut buf = Vec::new();
        buf.write_all(&to_json_bytes(doc, true).unwrap()).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    #[test]
    fn geometry_id_omitted_when_none() {
        let doc = EgraphicsInfoDocument {
            georeference: None,
            meshes: vec![MeshInfoSerde {
                id: 0,
                geometry_id: None,
                lod: 0,
                vertex_offset: 0,
                vertex_count: 3,
                index_offset: 36,
                index_count: 3,
                aabb: empty_aabb(),
            }],
            objects: vec![ObjectInfoSerde {
                object_id: String::new(),
                object_name: None,
                class_name: "Tree".into(),
                transform: None,
                aabb: empty_aabb(),
                meshes: vec![MeshInfoRefSerde {
                    mesh: 0,
                    transform: None,
                }],
                properties: HashMap::new(),
            }],
        };
        let v = round_trip(&doc);
        assert!(v["meshes"][0].get("geometry_id").is_none());
        assert_eq!(v["meshes"][0]["lod"], 0);
        assert!(v["objects"][0].get("object_name").is_none());
        assert!(v["objects"][0].get("transform").is_none());
        assert!(v["objects"][0].get("properties").is_none());
        assert!(v.get("georeference").is_none());
    }

    #[test]
    fn all_fields_present_when_set() {
        let doc = EgraphicsInfoDocument {
            georeference: Some(GeoreferenceSerde {
                crs: Some("EPSG:25832".into()),
                offset: Some([600_000.123_456_789, 5_400_000.0, 0.0]),
            }),
            meshes: vec![MeshInfoSerde {
                id: 7,
                geometry_id: Some("550e8400-uuid".into()),
                lod: 2,
                vertex_offset: 0,
                vertex_count: 3,
                index_offset: 36,
                index_count: 3,
                aabb: empty_aabb(),
            }],
            objects: vec![ObjectInfoSerde {
                object_id: "de.tum.bldg.main-hall".into(),
                object_name: Some("Main Hall".into()),
                class_name: "Building".into(),
                transform: Some([
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 0.0, 5.0, 1.0,
                ]),
                aabb: empty_aabb(),
                meshes: vec![MeshInfoRefSerde {
                    mesh: 0,
                    transform: None,
                }],
                properties: HashMap::from([
                    ("height".to_string(), serde_json::json!(12.5)),
                    ("is_landmark".to_string(), serde_json::json!(true)),
                ]),
            }],
        };
        let v = round_trip(&doc);
        assert_eq!(v["meshes"][0]["id"], 7);
        assert_eq!(v["meshes"][0]["geometry_id"], "550e8400-uuid");
        assert_eq!(v["meshes"][0]["lod"], 2);
        assert_eq!(v["objects"][0]["object_id"], "de.tum.bldg.main-hall");
        assert_eq!(v["objects"][0]["object_name"], "Main Hall");
        assert_eq!(v["objects"][0]["class_name"], "Building");
        assert_eq!(v["objects"][0]["transform"].as_array().unwrap().len(), 16);
        assert_eq!(v["objects"][0]["meshes"][0]["mesh"], 0);
        assert_eq!(v["objects"][0]["properties"]["height"], 12.5);
        assert_eq!(v["objects"][0]["properties"]["is_landmark"], true);
        assert_eq!(v["georeference"]["crs"], "EPSG:25832");
        assert_eq!(v["georeference"]["offset"][0], 600_000.123_456_789);
        assert_eq!(v["georeference"]["offset"][1], 5_400_000.0);
    }

    #[test]
    fn two_refs_same_mesh_index() {
        let doc = EgraphicsInfoDocument {
            georeference: None,
            meshes: vec![MeshInfoSerde {
                id: 3,
                geometry_id: None,
                lod: 0,
                vertex_offset: 0,
                vertex_count: 3,
                index_offset: 36,
                index_count: 3,
                aabb: empty_aabb(),
            }],
            objects: vec![ObjectInfoSerde {
                object_id: String::new(),
                object_name: None,
                class_name: "Tree".into(),
                transform: Some([1.0; 16]),
                aabb: empty_aabb(),
                meshes: vec![
                    MeshInfoRefSerde {
                        mesh: 0,
                        transform: None,
                    },
                    MeshInfoRefSerde {
                        mesh: 0,
                        transform: None,
                    },
                ],
                properties: HashMap::new(),
            }],
        };
        let v = round_trip(&doc);
        let refs = v["objects"][0]["meshes"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["mesh"], 0);
        assert_eq!(refs[1]["mesh"], 0);
    }
}
