use std::io::Cursor;

use egraphics_core::{LevelOfDetail, MeshKey, MeshRef, Object, Scene, TriangleMesh};
use egraphics_io::{EgraphicsWriter, MeshGrouping};
use glam::{Mat4, Vec3};

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

fn mesh_ref(key: MeshKey) -> MeshRef {
    MeshRef::new(key, LevelOfDetail::Zero)
}

fn obj_with_class(scene: &mut Scene, meshes: Vec<MeshRef>, class: &str) -> Object {
    let class_key = scene.get_or_insert_class_key(class);
    Object::new(meshes, class_key, String::new().into(), None)
}

/// Write an uncompressed archive and extract the glTF JSON bytes.
fn write_and_read_gltf(scene: Scene) -> gltf::Gltf {
    write_and_read_gltf_with_grouping(scene, MeshGrouping::PerClass)
}

/// Same as [`write_and_read_gltf`], but with an explicit [`MeshGrouping`].
fn write_and_read_gltf_with_grouping(scene: Scene, mesh_grouping: MeshGrouping) -> gltf::Gltf {
    let mut buf = Vec::new();
    EgraphicsWriter::new(&mut buf)
        .with_compressed(false)
        .with_mesh_grouping(mesh_grouping)
        .finish(scene)
        .unwrap();

    let mut archive = tar::Archive::new(Cursor::new(buf));
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().into_owned();
        if path == "model_data.gltf" {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            // from_slice does not resolve external URIs, which is fine for structural tests.
            return gltf::Gltf::from_slice(&bytes).unwrap();
        }
    }
    panic!("model_data.gltf not found in archive");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Two objects in different classes produce two root nodes and two glTF meshes.
#[test]
fn one_mesh_per_class() {
    let mut scene = Scene::new();

    let mesh_a = scene.add_mesh(triangle(Vec3::ZERO));
    let mesh_b = scene.add_mesh(triangle(Vec3::ONE));

    let building = scene.get_or_insert_class_key("Building");
    let road = scene.get_or_insert_class_key("Road");
    let obj_a = Object::new(
        vec![mesh_ref(mesh_a)],
        building,
        String::new().into(),
        Some("A".into()),
    );
    let obj_b = Object::new(
        vec![mesh_ref(mesh_b)],
        road,
        String::new().into(),
        Some("B".into()),
    );

    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    assert_eq!(doc.meshes().count(), 2, "one mesh per class");

    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();
    assert_eq!(root_nodes.len(), 2, "one root node per class");

    for node in &root_nodes {
        assert!(
            node.mesh().is_some(),
            "each class node must reference a mesh"
        );
        assert!(
            node.children().next().is_none(),
            "class nodes have no children"
        );
    }
}

/// Two objects with distinct meshes in the same class are combined into one world-space mesh.
#[test]
fn two_objects_same_class_combined() {
    let mut scene = Scene::new();

    let mesh_a = scene.add_mesh(triangle(Vec3::ZERO));
    let mesh_b = scene.add_mesh(triangle(Vec3::ONE * 5.0));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(mesh_a)], "Building");
    let obj_b = obj_with_class(
        &mut scene,
        vec![mesh_ref(mesh_b).with_transform(Mat4::from_translation(Vec3::X * 3.0))],
        "Building",
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    assert_eq!(
        doc.meshes().count(),
        1,
        "distinct meshes in the same class combine into one"
    );

    let gltf_scene = doc.scenes().next().unwrap();
    assert_eq!(
        gltf_scene.nodes().count(),
        1,
        "one root node for the combined class"
    );
}

/// Two objects sharing the same `MeshKey` are instanced: the mesh is written once
/// in local space and each object gets its own node with a transform.
#[test]
fn shared_key_triggers_instancing() {
    let mut scene = Scene::new();

    let shared = scene.add_mesh(triangle(Vec3::ZERO));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared)], "Tree");
    let obj_b = obj_with_class(
        &mut scene,
        vec![mesh_ref(shared).with_transform(Mat4::from_translation(Vec3::X * 3.0))],
        "Tree",
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    assert_eq!(
        doc.meshes().count(),
        1,
        "shared mesh is written once in local space"
    );

    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();
    assert_eq!(
        root_nodes.len(),
        1,
        "the two instanced nodes are grouped under one \"Tree\" class parent"
    );
    let children: Vec<_> = root_nodes[0].children().collect();
    assert_eq!(children.len(), 2, "one instanced node per object");
    for node in &children {
        assert!(
            node.mesh().is_some(),
            "each instanced node references the shared mesh"
        );
    }
    assert_eq!(
        doc.nodes().count(),
        3,
        "2 instanced leaves + 1 class group parent"
    );
}

/// Accessor vertex and index counts match the combined geometry of all objects.
#[test]
fn accessor_counts_match_combined() {
    let mut scene = Scene::new();

    let m1 = scene.add_mesh(triangle(Vec3::ZERO));
    let m2 = scene.add_mesh(triangle(Vec3::ONE));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(m1)], "Test");
    let obj_b = obj_with_class(&mut scene, vec![mesh_ref(m2)], "Test");
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    let accessors: Vec<_> = doc.accessors().collect();
    assert_eq!(accessors.len(), 2);

    let pos_acc = accessors
        .iter()
        .find(|a| a.data_type() == gltf::accessor::DataType::F32)
        .unwrap();
    let idx_acc = accessors
        .iter()
        .find(|a| a.data_type() == gltf::accessor::DataType::U32)
        .unwrap();

    assert_eq!(pos_acc.count(), 6, "6 vertices from two combined triangles");
    assert_eq!(idx_acc.count(), 6, "6 indices from two combined triangles");
}

/// A "mixed" object (one shared mesh_ref used by another object too, plus one unique
/// mesh_ref) must not duplicate the shared geometry into the combined-class buffer.
#[test]
fn mixed_object_shared_mesh_is_not_duplicated() {
    let mut scene = Scene::new();

    let shared_key = scene.add_mesh(triangle(Vec3::ZERO));
    let unique_key = scene.add_mesh(triangle(Vec3::ONE * 10.0));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_b = obj_with_class(
        &mut scene,
        vec![mesh_ref(shared_key), mesh_ref(unique_key)],
        "Tree",
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    assert_eq!(
        doc.meshes().count(),
        2,
        "one instanced mesh (shared) + one combined mesh (just the unique remainder)"
    );

    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();
    assert_eq!(
        root_nodes.len(),
        1,
        "all of obj_a's and obj_b's nodes are grouped under one \"Tree\" class parent"
    );
    assert_eq!(
        root_nodes[0].children().count(),
        3,
        "obj_a: 1 instanced node; obj_b: 1 instanced node + 1 combined node"
    );
    assert_eq!(
        doc.nodes().count(),
        4,
        "3 leaves (2 instanced + 1 combined) + 1 class group parent"
    );

    let total_verts: u64 = doc
        .accessors()
        .filter(|a| a.data_type() == gltf::accessor::DataType::F32)
        .map(|a| a.count() as u64)
        .sum();
    assert_eq!(
        total_verts, 6,
        "shared triangle (3 verts) + unique triangle (3 verts), shared written only once"
    );
}

/// In `PerObject` grouping, objects of the same class are no longer merged into one glTF mesh,
/// but a mesh genuinely shared across objects is still written once and instanced.
#[test]
fn per_object_mode_keeps_objects_separate_but_still_shares_meshes() {
    let mut scene = Scene::new();

    let shared_key = scene.add_mesh(triangle(Vec3::ZERO));
    let m1 = scene.add_mesh(triangle(Vec3::ONE * 5.0));
    let m2 = scene.add_mesh(triangle(Vec3::ONE * 10.0));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_b = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_c = obj_with_class(&mut scene, vec![mesh_ref(m1)], "Building");
    let obj_d = obj_with_class(&mut scene, vec![mesh_ref(m2)], "Building");
    scene.add_object(obj_a);
    scene.add_object(obj_b);
    scene.add_object(obj_c);
    scene.add_object(obj_d);

    let doc = write_and_read_gltf_with_grouping(scene, MeshGrouping::PerObject);
    assert_eq!(
        doc.meshes().count(),
        3,
        "1 instanced (shared) + 2 per-object (c, d) — not merged into one Building mesh"
    );

    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();
    assert_eq!(
        root_nodes.len(),
        2,
        "one root group node per class (Tree, Building) — leaves are re-parented, not root"
    );
    for group in &root_nodes {
        assert_eq!(
            group.children().count(),
            2,
            "each class group has its class's 2 objects as children"
        );
    }
    assert_eq!(
        doc.nodes().count(),
        6,
        "4 leaf nodes (a, b, c, d) + 2 class group parents"
    );
}

/// A "mixed" object's multiple leaf nodes (one instanced + one dedicated) both land under
/// the same class group, alongside its plain sibling's single node.
#[test]
fn per_object_mode_groups_mixed_objects_leaf_nodes_under_one_class() {
    let mut scene = Scene::new();

    let shared_key = scene.add_mesh(triangle(Vec3::ZERO));
    let unique_key = scene.add_mesh(triangle(Vec3::ONE * 10.0));

    // obj_a: plain, shares `shared_key` with obj_b — 1 leaf node.
    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    // obj_b: mixed (shared + unique) — 2 leaf nodes (1 instanced + 1 dedicated).
    let obj_b = obj_with_class(
        &mut scene,
        vec![mesh_ref(shared_key), mesh_ref(unique_key)],
        "Tree",
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf_with_grouping(scene, MeshGrouping::PerObject);
    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();

    assert_eq!(root_nodes.len(), 1, "a single \"Tree\" class group");
    assert_eq!(
        root_nodes[0].children().count(),
        3,
        "obj_a's 1 node + obj_b's 2 nodes (instanced + dedicated), all under \"Tree\""
    );
}

/// Per-object node/mesh names use `object_id` (unique) rather than the class name, so
/// same-class objects with no `object_name` don't collide (`Railway.001`, `Railway.002`, ...).
#[test]
fn per_object_mode_names_nodes_by_object_id_not_class() {
    let mut scene = Scene::new();
    let class = scene.get_or_insert_class_key("Railway");

    let m1 = scene.add_mesh(triangle(Vec3::ZERO));
    let m2 = scene.add_mesh(triangle(Vec3::ONE * 5.0));

    let obj_a = Object::new(
        vec![mesh_ref(m1)],
        class,
        "railway-001".to_string().into(),
        None,
    );
    let obj_b = Object::new(
        vec![mesh_ref(m2)],
        class,
        "railway-002".to_string().into(),
        None,
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf_with_grouping(scene, MeshGrouping::PerObject);
    let names: std::collections::HashSet<&str> = doc
        .nodes()
        .filter_map(|n| n.name())
        .filter(|&n| n != "Railway")
        .collect();

    assert_eq!(
        names,
        std::collections::HashSet::from(["railway-001", "railway-002"]),
        "leaf node names must be the distinct object IDs, not a repeated class name"
    );
}

/// In `PerObject` grouping, every glTF mesh (instanced and per-object alike) is named after its
/// position — which is exactly its `info.json` `MeshInfoSerde.id` — so it can be looked up
/// there directly.
#[test]
fn per_object_mode_names_meshes_by_info_json_id() {
    let mut scene = Scene::new();

    let shared_key = scene.add_mesh(triangle(Vec3::ZERO));
    let m1 = scene.add_mesh(triangle(Vec3::ONE * 5.0));
    let m2 = scene.add_mesh(triangle(Vec3::ONE * 10.0));

    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_b = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_c = obj_with_class(&mut scene, vec![mesh_ref(m1)], "Building");
    let obj_d = obj_with_class(&mut scene, vec![mesh_ref(m2)], "Building");
    scene.add_object(obj_a);
    scene.add_object(obj_b);
    scene.add_object(obj_c);
    scene.add_object(obj_d);

    let doc = write_and_read_gltf_with_grouping(scene, MeshGrouping::PerObject);
    let names: Vec<Option<&str>> = doc.meshes().map(|m| m.name()).collect();

    assert_eq!(names.len(), 3, "1 instanced (shared) + 2 per-object (c, d)");
    for (i, name) in names.iter().enumerate() {
        assert_eq!(*name, Some(format!("Mesh_{i}")).as_deref());
    }
}

/// In `PerClass` grouping (also the default): a class with only one resulting node (no
/// shared geometry) stays flat at the root — no redundant same-named empty parent — while a
/// class that mixes combined and instanced geometry (2+ nodes) does get grouped.
#[test]
fn combined_by_class_mode_only_groups_classes_with_multiple_nodes() {
    let mut scene = Scene::new();

    // "Building": two distinct unique meshes merge into exactly one combined node.
    let m1 = scene.add_mesh(triangle(Vec3::ZERO));
    let m2 = scene.add_mesh(triangle(Vec3::ONE * 5.0));
    let obj_e = obj_with_class(&mut scene, vec![mesh_ref(m1)], "Building");
    let obj_f = obj_with_class(&mut scene, vec![mesh_ref(m2)], "Building");
    scene.add_object(obj_e);
    scene.add_object(obj_f);

    // "Tree": a mixed object (shared + unique) produces 3 nodes for this class.
    let shared_key = scene.add_mesh(triangle(Vec3::ONE * 10.0));
    let unique_key = scene.add_mesh(triangle(Vec3::ONE * 20.0));
    let obj_a = obj_with_class(&mut scene, vec![mesh_ref(shared_key)], "Tree");
    let obj_b = obj_with_class(
        &mut scene,
        vec![mesh_ref(shared_key), mesh_ref(unique_key)],
        "Tree",
    );
    scene.add_object(obj_a);
    scene.add_object(obj_b);

    let doc = write_and_read_gltf(scene);
    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();
    assert_eq!(root_nodes.len(), 2, "one root entry per class");

    let building = root_nodes
        .iter()
        .find(|n| n.name() == Some("Building"))
        .unwrap();
    assert!(
        building.mesh().is_some(),
        "\"Building\" has exactly one node, so it stays a flat mesh node, not a group"
    );
    assert_eq!(building.children().count(), 0);

    let tree = root_nodes
        .iter()
        .find(|n| n.name() == Some("Tree"))
        .unwrap();
    assert!(
        tree.mesh().is_none(),
        "\"Tree\" has multiple nodes, so it becomes an empty group parent"
    );
    assert_eq!(tree.children().count(), 3);
}

/// A class with exactly one object in `PerObject` grouping must still be grouped under its class
/// name — its lone node is named after the object, not the class, so unlike a singleton
/// `PerClass` merged-class node, skipping the group parent here would throw away the
/// only place the class name would appear at all.
#[test]
fn singleton_class_with_object_named_node_is_still_grouped() {
    let mut scene = Scene::new();

    let key = scene.add_mesh(triangle(Vec3::ZERO));
    let class = scene.get_or_insert_class_key("WaterSurface");
    let obj = Object::new(
        vec![mesh_ref(key)],
        class,
        "waterbody-042".to_string().into(),
        None,
    );
    scene.add_object(obj);

    let doc = write_and_read_gltf_with_grouping(scene, MeshGrouping::PerObject);
    let gltf_scene = doc.scenes().next().unwrap();
    let root_nodes: Vec<_> = gltf_scene.nodes().collect();

    assert_eq!(
        root_nodes.len(),
        1,
        "the single object is still wrapped in a \"WaterSurface\" class group"
    );
    assert_eq!(root_nodes[0].name(), Some("WaterSurface"));
    assert!(
        root_nodes[0].mesh().is_none(),
        "the root is a group parent, not the object's own mesh node"
    );

    let children: Vec<_> = root_nodes[0].children().collect();
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].name(),
        Some("waterbody-042"),
        "the class name doesn't get lost even though there's only one member"
    );
}
