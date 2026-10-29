use std::io::Cursor;

use egraphics_core::{Georeference, LevelOfDetail, MeshRef, Object, Scene, TriangleMesh};
use egraphics_io::EgraphicsWriter;
use glam::{DVec3, Mat4, Vec3};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn triangle(offset: Vec3) -> TriangleMesh {
    TriangleMesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0) + offset,
            Vec3::new(1.0, 0.0, 0.0) + offset,
            Vec3::new(0.0, 1.0, 0.0) + offset,
        ],
        vec![[0, 1, 2]],
    )
}

/// Write the scene as an uncompressed egraphics archive, then extract `info.json`.
fn write_and_read_info(scene: Scene) -> Value {
    let mut buf = Vec::new();
    EgraphicsWriter::new(&mut buf)
        .with_compressed(false)
        .finish(scene)
        .unwrap();

    let mut archive = tar::Archive::new(Cursor::new(buf));
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().into_owned();
        if path == "info.json" {
            let mut content = String::new();
            std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
            return serde_json::from_str(&content).unwrap();
        }
    }
    panic!("info.json not found in archive");
}

fn simple_obj(scene: &mut Scene, id: u32, transform: Mat4, mesh: MeshRef, class: &str) -> Object {
    let class_key = scene.get_or_insert_class_key(class);
    Object::new(
        vec![mesh.with_transform(transform)],
        class_key,
        format!("obj-{id}").into(),
        None,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The archive contains a parseable `info.json`.
#[test]
fn info_document_is_created() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let obj = simple_obj(&mut scene, 0, Mat4::IDENTITY, m, "Building");
    scene.add_object(obj);

    let doc = write_and_read_info(scene);
    assert!(doc["meshes"].is_array());
    assert!(doc["objects"].is_array());
}

/// A combined object's info entry has no transform and references the class mesh.
#[test]
fn combined_object_has_no_transform() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let class = scene.get_or_insert_class_key("Building");
    let obj = Object::new(
        vec![m],
        class,
        "bldg-001".to_string().into(),
        Some("HQ".into()),
    );
    scene.add_object(obj);

    let doc = write_and_read_info(scene);
    let obj = &doc["objects"][0];
    assert_eq!(obj["object_id"], "bldg-001");
    assert_eq!(obj["object_name"], "HQ");
    assert_eq!(obj["class_name"], "Building");
    assert!(
        obj.get("transform").is_none(),
        "combined objects have no transform"
    );
    assert_eq!(obj["meshes"].as_array().unwrap().len(), 1);
}

/// Two combined objects in the same class share the same mesh entry.
#[test]
fn combined_objects_share_class_mesh_entry() {
    let mut scene = Scene::new();
    let key1 = scene.add_mesh(triangle(Vec3::ZERO));
    let key2 = scene.add_mesh(triangle(Vec3::ONE * 5.0));
    let m1 = MeshRef::new(key1, LevelOfDetail::Two);
    let m2 = MeshRef::new(key2, LevelOfDetail::Two);
    let obj0 = simple_obj(&mut scene, 0, Mat4::IDENTITY, m1, "Building");
    let obj1 = simple_obj(
        &mut scene,
        1,
        Mat4::from_translation(Vec3::X * 3.0),
        m2,
        "Building",
    );
    scene.add_object(obj0);
    scene.add_object(obj1);

    let doc = write_and_read_info(scene);
    assert_eq!(doc["meshes"].as_array().unwrap().len(), 1);

    let ref0 = doc["objects"][0]["meshes"][0]["mesh"].as_u64().unwrap();
    let ref1 = doc["objects"][1]["meshes"][0]["mesh"].as_u64().unwrap();
    assert_eq!(ref0, ref1, "both combined objects reference the same mesh");
}

/// An instanced object has a transform in the info and references an instanced mesh.
#[test]
fn instanced_object_has_transform() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let shared = MeshRef::new(key, LevelOfDetail::Two);

    let t = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
    let obj0 = simple_obj(&mut scene, 0, Mat4::IDENTITY, shared, "Tree");
    let obj1 = simple_obj(&mut scene, 1, t, shared, "Tree");
    scene.add_object(obj0);
    scene.add_object(obj1);

    let doc = write_and_read_info(scene);
    for obj in doc["objects"].as_array().unwrap() {
        let transform = obj
            .get("transform")
            .expect("instanced objects must have a transform");
        assert_eq!(transform.as_array().unwrap().len(), 16);
    }
}

/// Instanced objects reference an instanced mesh (not a combined class entry).
#[test]
fn instanced_objects_reference_instanced_mesh() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let shared = MeshRef::new(key, LevelOfDetail::Two);

    let obj0 = simple_obj(&mut scene, 0, Mat4::IDENTITY, shared, "Tree");
    let obj1 = simple_obj(
        &mut scene,
        1,
        Mat4::from_translation(Vec3::X * 5.0),
        shared,
        "Tree",
    );
    scene.add_object(obj0);
    scene.add_object(obj1);

    let doc = write_and_read_info(scene);
    assert_eq!(doc["meshes"].as_array().unwrap().len(), 1);

    let ref0 = doc["objects"][0]["meshes"][0]["mesh"].as_u64().unwrap();
    let ref1 = doc["objects"][1]["meshes"][0]["mesh"].as_u64().unwrap();
    assert_eq!(ref0, 0);
    assert_eq!(ref1, 0);
}

/// Mesh entries carry non-zero vertex / index counts and a valid AABB.
#[test]
fn mesh_entry_has_counts_and_aabb() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let obj = simple_obj(&mut scene, 0, Mat4::IDENTITY, m, "Building");
    scene.add_object(obj);

    let doc = write_and_read_info(scene);
    let mesh = &doc["meshes"][0];
    assert!(mesh["vertex_count"].as_u64().unwrap() > 0);
    assert!(mesh["index_count"].as_u64().unwrap() > 0);
    assert!(mesh["vertex_offset"].as_u64().is_some());
    assert!(mesh["index_offset"].as_u64().is_some());
    let aabb = &mesh["aabb"];
    assert_eq!(aabb["min"].as_array().unwrap().len(), 3);
    assert_eq!(aabb["max"].as_array().unwrap().len(), 3);
}

/// Instanced mesh entries appear before combined class entries.
#[test]
fn mixed_scene_mesh_ordering() {
    let mut scene = Scene::new();

    let shared_key = scene.add_mesh(triangle(Vec3::ZERO));
    let unique_key = scene.add_mesh(triangle(Vec3::ONE * 10.0));
    let shared = MeshRef::new(shared_key, LevelOfDetail::Two);
    let unique = MeshRef::new(unique_key, LevelOfDetail::One);

    let obj0 = simple_obj(&mut scene, 0, Mat4::IDENTITY, shared, "Tree");
    let obj1 = simple_obj(
        &mut scene,
        1,
        Mat4::from_translation(Vec3::X * 5.0),
        shared,
        "Tree",
    );
    let obj2 = simple_obj(&mut scene, 2, Mat4::IDENTITY, unique, "Building");
    scene.add_object(obj0);
    scene.add_object(obj1);
    scene.add_object(obj2);

    let doc = write_and_read_info(scene);
    assert_eq!(
        doc["meshes"].as_array().unwrap().len(),
        2,
        "1 instanced + 1 combined"
    );

    let building_obj = doc["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["class_name"] == "Building")
        .unwrap();
    let mesh_ref = building_obj["meshes"][0]["mesh"].as_u64().unwrap();
    assert_eq!(mesh_ref, 1, "combined class mesh is at index 1");
}

/// `object_id` is preserved in the info document.
#[test]
fn object_id_preserved() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let class = scene.get_or_insert_class_key("Road");
    let obj = Object::new(vec![m], class, "de.tum.road.main".to_string().into(), None);
    scene.add_object(obj);

    let doc = write_and_read_info(scene);
    assert_eq!(doc["objects"][0]["object_id"], "de.tum.road.main");
    assert!(doc["objects"][0].get("object_name").is_none());
}

/// Generic properties round-trip into `info.json` with their JSON-native type, and are
/// omitted entirely when an object has none.
#[test]
fn properties_round_trip_with_correct_json_types() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let class = scene.get_or_insert_class_key("Building");

    let with_props = Object::new(vec![m], class, "bldg-with-props".to_string().into(), None)
        .with_property("height", 12.5)
        .with_property("floors", 4i64)
        .with_property("is_landmark", true)
        .with_property("material", "concrete");
    scene.add_object(with_props);

    let without_props = simple_obj(&mut scene, 1, Mat4::IDENTITY, m, "Building");
    scene.add_object(without_props);

    let doc = write_and_read_info(scene);
    let objects = doc["objects"].as_array().unwrap();

    let with_props_doc = objects
        .iter()
        .find(|o| o["object_id"] == "bldg-with-props")
        .unwrap();
    assert_eq!(with_props_doc["properties"]["height"], 12.5);
    assert_eq!(with_props_doc["properties"]["floors"], 4);
    assert_eq!(with_props_doc["properties"]["is_landmark"], true);
    assert_eq!(with_props_doc["properties"]["material"], "concrete");

    let without_props_doc = objects.iter().find(|o| o["object_id"] == "obj-1").unwrap();
    assert!(
        without_props_doc.get("properties").is_none(),
        "objects with no properties don't carry an empty properties object"
    );
}

/// A scene's georeferencing round-trips into `info.json` at full `f64` precision, and is
/// omitted entirely when not set.
#[test]
fn georeference_round_trips_at_full_precision() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let obj = simple_obj(&mut scene, 0, Mat4::IDENTITY, m, "Building");
    scene.add_object(obj);

    let offset = DVec3::new(600_000.123_456_789, 5_400_000.987_654_321, 512.5);
    scene.set_georeference(Some(
        Georeference::new()
            .with_crs("EPSG:25832")
            .with_offset(offset),
    ));

    let doc = write_and_read_info(scene);
    assert_eq!(doc["georeference"]["crs"], "EPSG:25832");
    assert_eq!(doc["georeference"]["offset"][0].as_f64().unwrap(), offset.x);
    assert_eq!(doc["georeference"]["offset"][1].as_f64().unwrap(), offset.y);
    assert_eq!(doc["georeference"]["offset"][2].as_f64().unwrap(), offset.z);
}

/// Without an explicit georeference, `info.json` omits the field entirely.
#[test]
fn georeference_omitted_when_not_set() {
    let mut scene = Scene::new();
    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let m = MeshRef::new(key, LevelOfDetail::Two);
    let obj = simple_obj(&mut scene, 0, Mat4::IDENTITY, m, "Building");
    scene.add_object(obj);

    let doc = write_and_read_info(scene);
    assert!(doc.get("georeference").is_none());
}
