use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Write};
use std::mem;
use std::slice;

use chrono::{DateTime, Utc};
use egraphics_core::{LevelOfDetail, MeshKey, Object, Scene};
use gltf_json as json;
use json::validation::Checked::Valid;
use tracing::info;

use crate::egraphics::document::{
    AabbSerde, EgraphicsInfoDocument, GeoreferenceSerde, MeshInfoRefSerde, MeshInfoSerde,
    ObjectInfoSerde, property_value_to_json, to_json_bytes,
};
use crate::egraphics::write::MeshGrouping;
use crate::egraphics::{
    FILE_NAME_INFO_COMPRESSED, FILE_NAME_INFO_UNCOMPRESSED, FILE_NAME_MODEL_DATA_BIN_UNCOMPRESSED,
    FILE_NAME_MODEL_DATA_COMPRESSED, FILE_NAME_MODEL_DATA_UNCOMPRESSED,
};
use crate::error::Error;
use crate::util::zstd_backend;

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

fn as_bytes<T>(data: &[T]) -> &[u8] {
    // Safety: T is a plain-data type (f32 / u32); all bit patterns are valid u8.
    unsafe { slice::from_raw_parts(data.as_ptr() as *const u8, mem::size_of_val(data)) }
}

fn write_padded<W: Write>(w: &mut W, data: &[u8]) -> Result<usize, std::io::Error> {
    w.write_all(data)?;
    let pad = (4 - data.len() % 4) % 4;
    if pad > 0 {
        w.write_all(&[0u8; 3][..pad])?;
    }
    Ok(data.len() + pad)
}

fn aabb_of(verts: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in verts {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }
    (min, max)
}

/// A name to use for a node/mesh tied to one specific object: `object_name` if set, otherwise
/// the (always-present, always-unique) `object_id` — never a class name, which collides across
/// every object of that class and gets auto-suffixed (`Railway.001`, `Railway.002`, ...) by
/// glTF-importing tools.
fn object_display_name(obj: &Object) -> String {
    obj.object_name
        .clone()
        .unwrap_or_else(|| obj.object_id.clone().into())
}

// ---------------------------------------------------------------------------
// Scene analysis
// ---------------------------------------------------------------------------

/// Geometry of a combined group: `(name, lod, vertices, flat_indices)`.
type CombinedGeometry = (String, LevelOfDetail, Vec<[f32; 3]>, Vec<u32>);

/// A group of non-shared `(object index, mesh_ref index)` pairs to bake and merge into one
/// glTF mesh — one group per class (`PerClass`) or one group per object (`PerObject`).
struct CombinedGroup {
    /// Name used for the glTF mesh/node (class name, or the object's own name in `PerObject`
    /// grouping).
    name: String,
    /// `(object index, mesh_ref index)` pairs baked into this group's merged geometry.
    refs: Vec<(usize, usize)>,
    /// `Some(obj.transform)` when every ref in this group belongs to one object and geometry
    /// should be baked in that object's own local space (`PerObject` grouping) — the glTF node for
    /// this group then carries this as its transform. `None` when refs are merged across
    /// multiple objects and must be pre-baked directly into world space (`PerClass`
    /// grouping) — the node then carries no transform.
    object_transform: Option<glam::Mat4>,
}

struct SceneAnalysis {
    /// Unique instanced mesh keys in stable insertion order.
    instanced_meshes: Vec<MeshKey>,
    /// LOD of each instanced mesh, taken from the first `MeshRef` that references it.
    instanced_lods: Vec<LevelOfDetail>,
    /// Mesh key → index into `instanced_meshes`.
    key_to_inst_idx: HashMap<MeshKey, usize>,
    /// Groups of non-shared mesh_refs to bake/merge together.
    combined_groups: Vec<CombinedGroup>,
    /// Object index → index into `combined_groups`, for objects with a non-shared remainder.
    obj_to_combined_group: HashMap<usize, usize>,
}

fn analyse_scene(objects: &[&Object], scene: &Scene, mesh_grouping: MeshGrouping) -> SceneAnalysis {
    let mut key_count: HashMap<MeshKey, usize> = HashMap::new();
    for obj in objects {
        for m in &obj.mesh_refs {
            *key_count.entry(m.key).or_insert(0) += 1;
        }
    }

    let mut instanced_meshes: Vec<MeshKey> = Vec::new();
    let mut instanced_lods: Vec<LevelOfDetail> = Vec::new();
    let mut key_to_inst_idx: HashMap<MeshKey, usize> = HashMap::new();
    for obj in objects {
        for m in &obj.mesh_refs {
            if *key_count.get(&m.key).unwrap_or(&0) > 1 && !key_to_inst_idx.contains_key(&m.key) {
                key_to_inst_idx.insert(m.key, instanced_meshes.len());
                instanced_meshes.push(m.key);
                instanced_lods.push(m.level_of_detail);
            }
        }
    }

    let mut combined_groups: Vec<CombinedGroup> = Vec::new();
    let mut obj_to_combined_group: HashMap<usize, usize> = HashMap::new();

    match mesh_grouping {
        MeshGrouping::PerClass => {
            let mut by_class: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
            for (oi, obj) in objects.iter().enumerate() {
                let class_name = scene.class_name(obj.class_key).unwrap_or_default();
                for (ri, m) in obj.mesh_refs.iter().enumerate() {
                    if !key_to_inst_idx.contains_key(&m.key) {
                        by_class
                            .entry(class_name.to_string())
                            .or_default()
                            .push((oi, ri));
                    }
                }
            }
            for (name, refs) in by_class {
                let group_idx = combined_groups.len();
                for &(oi, _) in &refs {
                    obj_to_combined_group.insert(oi, group_idx);
                }
                combined_groups.push(CombinedGroup {
                    name,
                    refs,
                    object_transform: None,
                });
            }
        }
        MeshGrouping::PerObject => {
            for (oi, obj) in objects.iter().enumerate() {
                let refs: Vec<(usize, usize)> = obj
                    .mesh_refs
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| !key_to_inst_idx.contains_key(&m.key))
                    .map(|(ri, _)| (oi, ri))
                    .collect();
                if refs.is_empty() {
                    continue;
                }
                let name = object_display_name(obj);
                obj_to_combined_group.insert(oi, combined_groups.len());
                combined_groups.push(CombinedGroup {
                    name,
                    refs,
                    object_transform: Some(obj.transform),
                });
            }
        }
    }

    SceneAnalysis {
        instanced_meshes,
        instanced_lods,
        key_to_inst_idx,
        combined_groups,
        obj_to_combined_group,
    }
}

// ---------------------------------------------------------------------------
// Binary mesh entry (one per unique buffer region)
// ---------------------------------------------------------------------------

struct BinMesh {
    vertex_offset: u64,
    vertex_byte_len: usize,
    vertex_count: u64,
    index_offset: u64,
    index_byte_len: usize,
    index_count: u64,
    aabb_min: [f32; 3],
    aabb_max: [f32; 3],
}

fn write_bin_mesh(
    buf: &mut Vec<u8>,
    total: &mut usize,
    verts: &[[f32; 3]],
    indices: &[u32],
) -> Result<BinMesh, Error> {
    let pos_bytes = as_bytes(verts);
    let idx_bytes = as_bytes(indices);

    let vertex_offset = *total as u64;
    let vertex_byte_len = pos_bytes.len();
    *total += write_padded(buf, pos_bytes)?;

    let index_offset = *total as u64;
    let index_byte_len = idx_bytes.len();
    *total += write_padded(buf, idx_bytes)?;

    let (aabb_min, aabb_max) = aabb_of(verts);
    Ok(BinMesh {
        vertex_offset,
        vertex_byte_len,
        vertex_count: verts.len() as u64,
        index_offset,
        index_byte_len,
        index_count: indices.len() as u64,
        aabb_min,
        aabb_max,
    })
}

// ---------------------------------------------------------------------------
// glTF helpers
// ---------------------------------------------------------------------------

/// Returns a JSON array of 3 finite floats, or `None` if any value is non-finite.
///
/// serde_json serializes `f32::INFINITY` / `NaN` as JSON `null`, which is
/// illegal in glTF accessor `min`/`max` fields and causes parsers to reject the file.
fn finite_vec3(v: [f32; 3]) -> Option<json::Value> {
    if v.iter().all(|x| x.is_finite()) {
        Some(json::Value::from(v.to_vec()))
    } else {
        None
    }
}

fn add_to_gltf(
    buffer_views: &mut Vec<json::buffer::View>,
    accessors: &mut Vec<json::Accessor>,
    gltf_meshes: &mut Vec<json::Mesh>,
    bin: &BinMesh,
    name: Option<String>,
) -> u32 {
    let view_pos = buffer_views.len() as u32;
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: bin.vertex_byte_len.into(),
        byte_offset: Some(bin.vertex_offset.into()),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    });

    let view_idx = buffer_views.len() as u32;
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: bin.index_byte_len.into(),
        byte_offset: Some(bin.index_offset.into()),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ElementArrayBuffer)),
    });

    let acc_pos = accessors.len() as u32;
    accessors.push(json::Accessor {
        buffer_view: Some(json::Index::new(view_pos)),
        byte_offset: None,
        count: bin.vertex_count.into(),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec3),
        min: finite_vec3(bin.aabb_min),
        max: finite_vec3(bin.aabb_max),
        name: None,
        normalized: false,
        sparse: None,
    });

    let acc_idx = accessors.len() as u32;
    accessors.push(json::Accessor {
        buffer_view: Some(json::Index::new(view_idx)),
        byte_offset: None,
        count: bin.index_count.into(),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::U32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Scalar),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    let mesh_idx = gltf_meshes.len() as u32;
    gltf_meshes.push(json::Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        name,
        primitives: vec![json::mesh::Primitive {
            attributes: {
                let mut map = BTreeMap::new();
                map.insert(
                    Valid(json::mesh::Semantic::Positions),
                    json::Index::new(acc_pos),
                );
                map
            },
            extensions: Default::default(),
            extras: Default::default(),
            indices: Some(json::Index::new(acc_idx)),
            material: None,
            mode: Valid(json::mesh::Mode::Triangles),
            targets: None,
        }],
        weights: None,
    });
    mesh_idx
}

fn make_node(
    name: Option<String>,
    mesh: Option<json::Index<json::Mesh>>,
    transform: Option<glam::Mat4>,
) -> json::Node {
    json::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: transform.map(|m| m.to_cols_array()),
        mesh,
        name,
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    }
}

/// A parent node with no mesh/transform of its own — used to group child nodes (e.g. by
/// class, in `PerObject` grouping) for easier selection/hiding in DCC tools. Carrying no
/// transform (identity) keeps children's already-computed world transforms valid, since glTF
/// node transforms are relative to their immediate parent.
fn make_group_node(name: Option<String>, children: Vec<json::Index<json::Node>>) -> json::Node {
    json::Node {
        children: Some(children),
        ..make_node(name, None, None)
    }
}

// ---------------------------------------------------------------------------
// GLB binary container builder
// ---------------------------------------------------------------------------

/// Encodes a glTF JSON + binary buffer into the GLB binary container format.
///
/// The JSON chunk is padded with spaces (0x20); the binary chunk with zeros.
/// If `bin` is empty the BIN chunk is omitted entirely.
fn build_glb(json_bytes: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_pad = (4 - json_bytes.len() % 4) % 4;
    let json_chunk_len = json_bytes.len() + json_pad;

    let bin_chunk_total = if bin.is_empty() {
        0
    } else {
        let bin_pad = (4 - bin.len() % 4) % 4;
        8 + bin.len() + bin_pad
    };

    let total_len = 12 + 8 + json_chunk_len + bin_chunk_total;
    let mut out = Vec::with_capacity(total_len);

    // File header
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    // Chunk 0: JSON
    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(json_bytes);
    out.extend(std::iter::repeat_n(0x20u8, json_pad));

    // Chunk 1: BIN (omitted when empty)
    if !bin.is_empty() {
        let bin_pad = (4 - bin.len() % 4) % 4;
        let bin_chunk_data_len = bin.len() + bin_pad;
        out.extend_from_slice(&(bin_chunk_data_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes()); // "BIN\0"
        out.extend_from_slice(bin);
        out.extend(std::iter::repeat_n(0u8, bin_pad));
    }

    out
}

// ---------------------------------------------------------------------------
// Tar helper
// ---------------------------------------------------------------------------

fn append_tar_entry<W: Write>(
    archive: &mut tar::Builder<W>,
    name: &str,
    data: &[u8],
    time: Option<DateTime<Utc>>,
) -> Result<(), Error> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o664);
    if let Some(t) = time {
        header.set_mtime(t.timestamp() as u64);
    }
    header.set_cksum();
    archive.append_data(&mut header, name, Cursor::new(data))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

pub fn write_egraphics_format<W: Write>(
    scene: Scene,
    writer: W,
    mesh_grouping: MeshGrouping,
    compression_level: Option<i32>,
    time: Option<DateTime<Utc>>,
    pretty: bool,
) -> Result<(), Error> {
    let objects: Vec<&Object> = scene.objects().values().collect();
    let analysis = analyse_scene(&objects, &scene, mesh_grouping);
    let n_inst = analysis.instanced_meshes.len();

    // -----------------------------------------------------------------------
    // Collect combined-group geometry (skip groups with no renderable geometry).
    // -----------------------------------------------------------------------
    // (name, lod, verts, flat_indices) — lod taken from the first mesh_ref in the group.
    let combined_geom: Vec<CombinedGeometry> = analysis
        .combined_groups
        .iter()
        .filter_map(|group| {
            let mut verts: Vec<[f32; 3]> = Vec::new();
            let mut flat: Vec<u32> = Vec::new();
            let mut lod: LevelOfDetail = LevelOfDetail::One;
            let mut lod_set = false;
            for &(oi, ri) in &group.refs {
                let obj = objects[oi];
                let m = &obj.mesh_refs[ri];
                if !lod_set {
                    lod = m.level_of_detail;
                    lod_set = true;
                }
                let mesh = &scene.meshes()[m.key];
                let base = verts.len() as u32;
                // `PerObject` groups bake in the (single) owning object's local space, since
                // the node itself carries `object_transform`; `PerClass` groups merge
                // multiple objects together and must be pre-baked directly into world space.
                let mesh_to_target = match group.object_transform {
                    Some(_) => m.transform,
                    None => obj.transform * m.transform,
                };
                for &v in &mesh.vertices {
                    let p = mesh_to_target.transform_point3(v);
                    verts.push([p.x, p.y, p.z]);
                }
                for &[a, b, c] in &mesh.indices {
                    flat.extend_from_slice(&[a + base, b + base, c + base]);
                }
            }
            if verts.is_empty() {
                None
            } else {
                Some((group.name.clone(), lod, verts, flat))
            }
        })
        .collect();

    // -----------------------------------------------------------------------
    // Binary buffer (in memory): instanced meshes then combined class meshes.
    // -----------------------------------------------------------------------
    let mut bin_buf: Vec<u8> = Vec::new();
    let mut bin_total: usize = 0;
    let mut instanced_bins: Vec<BinMesh> = Vec::with_capacity(n_inst);
    let mut combined_bins: Vec<BinMesh> = Vec::with_capacity(combined_geom.len());

    for &key in &analysis.instanced_meshes {
        let mesh = &scene.meshes()[key];
        let verts: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
        let flat: Vec<u32> = mesh
            .indices
            .iter()
            .flat_map(|&[a, b, c]| [a, b, c])
            .collect();
        instanced_bins.push(write_bin_mesh(&mut bin_buf, &mut bin_total, &verts, &flat)?);
    }

    for (_, _lod, verts, flat) in &combined_geom {
        combined_bins.push(write_bin_mesh(&mut bin_buf, &mut bin_total, verts, flat)?);
    }

    // -----------------------------------------------------------------------
    // glTF structure
    // -----------------------------------------------------------------------
    let mut buffer_views: Vec<json::buffer::View> = Vec::new();
    let mut accessors: Vec<json::Accessor> = Vec::new();
    let mut gltf_meshes: Vec<json::Mesh> = Vec::new();
    let mut nodes: Vec<json::Node> = Vec::new();

    let mut inst_gltf: Vec<u32> = Vec::with_capacity(n_inst);
    for (i, bin) in instanced_bins.iter().enumerate() {
        // In `PerObject` grouping, name every mesh after its info.json `MeshInfoSerde.id` (this
        // loop index) so a mesh in a DCC tool can be looked back up in the sidecar JSON.
        let name = if mesh_grouping == MeshGrouping::PerObject {
            Some(format!("Mesh_{i}"))
        } else {
            scene.meshes()[analysis.instanced_meshes[i]]
                .geometry_id
                .clone()
        };
        inst_gltf.push(add_to_gltf(
            &mut buffer_views,
            &mut accessors,
            &mut gltf_meshes,
            bin,
            name,
        ));
    }

    // Every leaf node is grouped by its owning object's class (easier to select/hide/move
    // whole classes in DCC tools like Blender) — in both modes. A class that ends up with
    // only one node (the common case with no shared geometry) stays flat at the root instead
    // of being wrapped in a redundant, otherwise-empty same-named parent.
    let mut class_group_children: BTreeMap<String, Vec<u32>> = BTreeMap::new();

    for (j, bin) in combined_bins.iter().enumerate() {
        let group = &analysis.combined_groups[j];
        let mesh_name = if mesh_grouping == MeshGrouping::PerObject {
            format!("Mesh_{}", n_inst + j)
        } else {
            group.name.clone()
        };
        let mesh_idx = add_to_gltf(
            &mut buffer_views,
            &mut accessors,
            &mut gltf_meshes,
            bin,
            Some(mesh_name),
        );
        let node_idx = nodes.len() as u32;
        nodes.push(make_node(
            Some(group.name.clone()),
            Some(json::Index::new(mesh_idx)),
            group.object_transform,
        ));
        let (obj_idx, _) = group.refs[0];
        let class_name = scene
            .class_name(objects[obj_idx].class_key)
            .unwrap_or_default();
        class_group_children
            .entry(class_name.to_string())
            .or_default()
            .push(node_idx);
    }

    for &obj in &objects {
        for m in &obj.mesh_refs {
            if let Some(&inst_idx) = analysis.key_to_inst_idx.get(&m.key) {
                let node_idx = nodes.len() as u32;
                nodes.push(make_node(
                    Some(object_display_name(obj)),
                    Some(json::Index::new(inst_gltf[inst_idx])),
                    Some(obj.transform * m.transform),
                ));
                let class_name = scene.class_name(obj.class_key).unwrap_or_default();
                class_group_children
                    .entry(class_name.to_string())
                    .or_default()
                    .push(node_idx);
            }
        }
    }

    let mut scene_nodes: Vec<json::Index<json::Node>> = Vec::new();
    for (class_name, children) in class_group_children {
        // Skipping the group parent is only redundant when the lone child's own name is
        // already the class name (a `PerClass` merged-class node) — an instanced or
        // `PerObject` node is named after its object, so wrapping it is the only place the
        // class name would otherwise show up at all.
        let redundant_singleton = children.len() == 1
            && nodes[children[0] as usize].name.as_deref() == Some(class_name.as_str());
        if redundant_singleton {
            scene_nodes.push(json::Index::new(children[0]));
        } else {
            let group_node_idx = nodes.len() as u32;
            nodes.push(make_group_node(
                Some(class_name),
                children.into_iter().map(json::Index::new).collect(),
            ));
            scene_nodes.push(json::Index::new(group_node_idx));
        }
    }

    // -----------------------------------------------------------------------
    // Sidecar document
    // -----------------------------------------------------------------------
    let mut info_meshes: Vec<MeshInfoSerde> = Vec::new();

    for (i, bin) in instanced_bins.iter().enumerate() {
        let mesh = &scene.meshes()[analysis.instanced_meshes[i]];
        info_meshes.push(MeshInfoSerde {
            id: i as u32,
            geometry_id: mesh.geometry_id.clone(),
            lod: analysis.instanced_lods[i].as_index() as u8,
            vertex_offset: bin.vertex_offset,
            vertex_count: bin.vertex_count,
            index_offset: bin.index_offset,
            index_count: bin.index_count,
            aabb: AabbSerde {
                min: bin.aabb_min,
                max: bin.aabb_max,
            },
        });
    }

    for (j, bin) in combined_bins.iter().enumerate() {
        let lod = combined_geom[j].1; // index 1: lod
        info_meshes.push(MeshInfoSerde {
            id: (n_inst + j) as u32,
            geometry_id: None,
            lod: lod.as_index() as u8,
            vertex_offset: bin.vertex_offset,
            vertex_count: bin.vertex_count,
            index_offset: bin.index_offset,
            index_count: bin.index_count,
            aabb: AabbSerde {
                min: bin.aabb_min,
                max: bin.aabb_max,
            },
        });
    }

    let mut info_objects: Vec<ObjectInfoSerde> = Vec::new();
    for (oi, &obj) in objects.iter().enumerate() {
        let world_aabb = obj.world_aabb();
        let aabb = AabbSerde {
            min: world_aabb.min.to_array(),
            max: world_aabb.max.to_array(),
        };
        let class_name = scene.class_name(obj.class_key).unwrap_or_default();

        // Instanced-referencing entries: fully composed, self-sufficient world transform —
        // a consumer never needs to know about `ObjectInfoSerde.transform` to place these.
        let mut mesh_refs: Vec<MeshInfoRefSerde> = obj
            .mesh_refs
            .iter()
            .filter_map(|m| {
                analysis
                    .key_to_inst_idx
                    .get(&m.key)
                    .map(|&idx| MeshInfoRefSerde {
                        mesh: idx as u32,
                        transform: Some((obj.transform * m.transform).to_cols_array()),
                    })
            })
            .collect();
        let has_instanced_ref = !mesh_refs.is_empty();

        // Combined-group-referencing entry (at most one). `PerClass` groups are
        // pre-baked directly into world space (no transform needed); `PerObject` groups are
        // baked in the object's own local space, so the group's `object_transform` — this very
        // object's transform — must be carried here to stay self-sufficient.
        if let Some(&group_idx) = analysis.obj_to_combined_group.get(&oi) {
            let group = &analysis.combined_groups[group_idx];
            mesh_refs.push(MeshInfoRefSerde {
                mesh: (n_inst + group_idx) as u32,
                transform: group.object_transform.map(|t| t.to_cols_array()),
            });
        }

        info_objects.push(ObjectInfoSerde {
            object_id: obj.object_id.clone().into(),
            object_name: obj.object_name.clone(),
            class_name: class_name.to_string(),
            transform: has_instanced_ref.then(|| obj.transform.to_cols_array()),
            aabb,
            meshes: mesh_refs,
            properties: obj
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), property_value_to_json(v)))
                .collect(),
        });
    }

    let info_doc = EgraphicsInfoDocument {
        georeference: scene.georeference().map(|g| GeoreferenceSerde {
            crs: g.crs.clone(),
            offset: g.offset.map(|o| o.to_array()),
        }),
        meshes: info_meshes,
        objects: info_objects,
    };

    // -----------------------------------------------------------------------
    // Pack into tar archive
    // -----------------------------------------------------------------------
    info!("Writing egraphics archive");
    let mut archive = tar::Builder::new(writer);

    if let Some(level) = compression_level {
        // Compressed path: GLB (self-contained binary glTF) + zstd info document.
        let gltf_root = json::Root {
            accessors,
            buffers: vec![json::Buffer {
                byte_length: bin_total.into(),
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                uri: None, // embedded in GLB BIN chunk
            }],
            buffer_views,
            meshes: gltf_meshes,
            nodes,
            scenes: vec![json::Scene {
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                nodes: scene_nodes,
            }],
            ..Default::default()
        };
        let mut gltf_json_bytes = Vec::new();
        json::serialize::to_writer(&mut gltf_json_bytes, &gltf_root)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let glb = build_glb(&gltf_json_bytes, &bin_buf);
        append_tar_entry(&mut archive, FILE_NAME_MODEL_DATA_COMPRESSED, &glb, time)?;

        let info_bytes = to_json_bytes(&info_doc, pretty)?;
        let mut zst_bytes: Vec<u8> = Vec::new();
        {
            let mut encoder = zstd_backend::encoder(&mut zst_bytes, level)?;
            encoder.write_all(&info_bytes)?;
            // the encoder finishes the frame when dropped at the end of this scope
        }
        append_tar_entry(&mut archive, FILE_NAME_INFO_COMPRESSED, &zst_bytes, time)?;
    } else {
        // Uncompressed path: separate gltf + bin + info.json.
        let gltf_root = json::Root {
            accessors,
            buffers: vec![json::Buffer {
                byte_length: bin_total.into(),
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                uri: Some(FILE_NAME_MODEL_DATA_BIN_UNCOMPRESSED.to_string()),
            }],
            buffer_views,
            meshes: gltf_meshes,
            nodes,
            scenes: vec![json::Scene {
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                nodes: scene_nodes,
            }],
            ..Default::default()
        };
        let mut gltf_json_bytes = Vec::new();
        json::serialize::to_writer(&mut gltf_json_bytes, &gltf_root)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        append_tar_entry(
            &mut archive,
            FILE_NAME_MODEL_DATA_UNCOMPRESSED,
            &gltf_json_bytes,
            time,
        )?;
        append_tar_entry(
            &mut archive,
            FILE_NAME_MODEL_DATA_BIN_UNCOMPRESSED,
            &bin_buf,
            time,
        )?;

        let info_bytes = to_json_bytes(&info_doc, pretty)?;
        append_tar_entry(&mut archive, FILE_NAME_INFO_UNCOMPRESSED, &info_bytes, time)?;
    }

    Ok(())
}
