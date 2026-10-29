use std::collections::HashMap;

use glam::{Mat4, Vec3};
use slotmap::SlotMap;

use super::{ClassKey, LevelOfDetail, MeshKey, PropertyValue, TriangleMesh};
use crate::math::{Aabb, Ray};
use crate::scene::id::ObjectId;

/// A reference from an [`Object`] to a mesh stored in [`Scene::meshes`](super::Scene::meshes),
/// tagged with the level of detail the object is using it at, and this reference's placement
/// within the object's own frame (`IDENTITY` for geometry authored directly in that frame;
/// non-identity for instanced/implicit geometry placed within the object).
#[derive(Debug, Clone, Copy)]
pub struct MeshRef {
    pub key: MeshKey,
    pub level_of_detail: LevelOfDetail,
    pub transform: Mat4,
}

impl MeshRef {
    pub fn new(key: MeshKey, level_of_detail: LevelOfDetail) -> Self {
        Self {
            key,
            level_of_detail,
            transform: Mat4::IDENTITY,
        }
    }

    /// Sets this reference's placement within the object's frame.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }
}

/// A scene object with zero or more shared triangle meshes and a world transform.
#[derive(Debug, Clone)]
pub struct Object {
    /// Object-space → world-space transform.
    ///
    /// Starts as `IDENTITY`; [`Object::rebase`] (called automatically by
    /// [`super::Scene::add_object`]) derives a precision-friendly value from the object's
    /// geometry and adjusts every [`MeshRef::transform`] to compensate, so the object's actual
    /// world placement doesn't change.
    pub transform: Mat4,

    /// AABB in object (local) space — union of all submesh AABBs.
    pub aabb_local: Aabb,

    /// References to triangle meshes for precise ray intersection.
    ///
    /// Multiple objects can reference the same [`MeshKey`] to support
    /// instancing without duplicating geometry.
    pub mesh_refs: Vec<MeshRef>,

    /// Interned class name of the object — see [`super::Scene::get_or_insert_class_key`].
    pub class_key: ClassKey,
    /// ID of the object
    pub object_id: ObjectId,
    /// Name of the object
    pub object_name: Option<String>,

    /// Generic business attributes (e.g. CityGML-style generic attributes) keyed by name.
    pub properties: HashMap<String, PropertyValue>,
}

impl Object {
    /// Constructs a new object.
    ///
    /// `transform` starts as `IDENTITY` and `aabb_local` as a zero-size AABB at the origin;
    /// call [`Object::rebase`] (or [`super::Scene::add_object`], which does this automatically)
    /// to derive both from the referenced meshes.
    pub fn new(
        meshes: Vec<MeshRef>,
        class_key: ClassKey,
        object_id: ObjectId,
        object_name: Option<String>,
    ) -> Self {
        Self {
            transform: Mat4::IDENTITY,
            aabb_local: Aabb::new(Vec3::ZERO, Vec3::ZERO),
            mesh_refs: meshes,
            class_key,
            object_id,
            object_name,
            properties: HashMap::new(),
        }
    }

    /// Sets a generic property, overwriting any existing value for `key`.
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<PropertyValue>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Appends a [`MeshRef`] to this object.
    ///
    /// Doesn't recompute `aabb_local`/`transform` itself (this method has no access to the mesh
    /// registry needed to do that) — call [`Object::rebase`] (or
    /// [`super::Scene::rebuild_object_aabbs`]) afterward, same as after any other mesh mutation.
    pub fn add_mesh_ref(&mut self, mesh_ref: MeshRef) {
        self.mesh_refs.push(mesh_ref);
    }

    pub fn extend_mesh_refs(&mut self, mesh_refs: impl IntoIterator<Item = MeshRef>) {
        self.mesh_refs.extend(mesh_refs);
    }

    /// Derives a precision-friendly `transform` from the union world-space bounding box of the
    /// referenced meshes (looked up in `mesh_store`), and rewrites every [`MeshRef::transform`]
    /// to compensate — each mesh's actual world placement is unchanged. Also recomputes
    /// `aabb_local` to match.
    ///
    /// Safe to call repeatedly (e.g. after mutating a mesh this object references — see
    /// [`super::Scene::rebuild_object_aabbs`]): it composes with the *current* `transform`
    /// rather than assuming `IDENTITY`, so re-running it doesn't drift the object's placement.
    /// Objects with no meshes (or only degenerate ones) fall back to a zero-size `aabb_local`
    /// at the origin and leave `transform` untouched.
    pub fn rebase(&mut self, mesh_store: &SlotMap<MeshKey, TriangleMesh>) {
        let world_transforms: Vec<Mat4> = self
            .mesh_refs
            .iter()
            .map(|mesh_ref| self.transform * mesh_ref.transform)
            .collect();

        let world_bbox = self
            .mesh_refs
            .iter()
            .zip(&world_transforms)
            .map(|(mesh_ref, &w)| mesh_store[mesh_ref.key].compute_aabb().transform(w))
            .filter(|aabb| aabb.min.is_finite() && aabb.max.is_finite())
            .reduce(|a, b| a.union(&b));

        let Some(world_bbox) = world_bbox else {
            self.aabb_local = Aabb::new(Vec3::ZERO, Vec3::ZERO);
            return;
        };

        let center = world_bbox.center();
        let new_transform = Mat4::from_translation(center);
        let inv_new_transform = new_transform.inverse();

        for (mesh_ref, &w) in self.mesh_refs.iter_mut().zip(&world_transforms) {
            mesh_ref.transform = inv_new_transform * w;
        }
        self.transform = new_transform;
        self.aabb_local = Aabb::new(world_bbox.min - center, world_bbox.max - center);
    }

    /// World-space AABB obtained by transforming `aabb_local`.
    pub fn world_aabb(&self) -> Aabb {
        self.aabb_local.transform(self.transform)
    }

    /// Intersects a world-space ray with this object.
    ///
    /// The ray is transformed into each mesh's own local space (composing `transform` with
    /// that mesh's [`MeshRef::transform`]) and tested in turn. Returns the nearest hit `t`
    /// across all meshes. Falls back to the world AABB when no meshes are present.
    pub fn intersect_ray(
        &self,
        ray: &Ray,
        mesh_store: &SlotMap<MeshKey, TriangleMesh>,
    ) -> Option<f32> {
        if self.mesh_refs.is_empty() {
            return self.world_aabb().intersect_ray(ray);
        }

        // Quick AABB cull in world space before touching any mesh data.
        self.world_aabb().intersect_ray(ray)?;

        let mut nearest: Option<f32> = None;
        for mesh_ref in &self.mesh_refs {
            let inv = (self.transform * mesh_ref.transform).inverse();
            let origin_local = inv.transform_point3(ray.origin);
            let dir_local_raw = inv.transform_vector3(ray.dir);
            let scale = dir_local_raw.length();
            if scale < f32::EPSILON {
                continue;
            }
            let ray_local = Ray::new(origin_local, dir_local_raw / scale);

            let mesh = &mesh_store[mesh_ref.key];
            if let Some((t, _)) = mesh.intersect_ray(&ray_local) {
                let t_world = t / scale;
                if nearest.is_none_or(|n| t_world < n) {
                    nearest = Some(t_world);
                }
            }
        }
        nearest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_at(offset: Vec3) -> TriangleMesh {
        TriangleMesh::new(
            vec![
                Vec3::new(-1.0, -1.0, 0.0) + offset,
                Vec3::new(1.0, -1.0, 0.0) + offset,
                Vec3::new(0.0, 1.0, 0.0) + offset,
            ],
            vec![[0, 1, 2]],
        )
    }

    fn test_object(meshes: Vec<MeshRef>) -> Object {
        let mut classes: SlotMap<ClassKey, String> = SlotMap::default();
        let class_key = classes.insert("Test".to_string());
        Object::new(meshes, class_key, String::new().into(), None)
    }

    #[test]
    fn with_property_sets_and_overwrites() {
        let obj = test_object(vec![])
            .with_property("height", 12.5)
            .with_property("class", "Building")
            .with_property("class", "Road"); // overwrite

        assert_eq!(
            obj.properties.get("height"),
            Some(&PropertyValue::Float(12.5))
        );
        assert_eq!(
            obj.properties.get("class"),
            Some(&PropertyValue::String("Road".into()))
        );
    }

    #[test]
    fn add_mesh_ref_appends_and_is_reflected_after_rebase() {
        let mut meshes: SlotMap<MeshKey, TriangleMesh> = SlotMap::default();
        let key_a = meshes.insert(triangle_at(Vec3::ZERO));
        let mut obj = test_object(vec![MeshRef::new(key_a, LevelOfDetail::Zero)]);
        obj.rebase(&meshes);

        let key_b = meshes.insert(triangle_at(Vec3::new(100.0, 0.0, 0.0)));
        obj.add_mesh_ref(MeshRef::new(key_b, LevelOfDetail::Zero));
        assert_eq!(obj.mesh_refs.len(), 2);

        obj.rebase(&meshes);
        assert!(obj.world_aabb().max.x >= 100.0);
    }

    #[test]
    fn rebase_recenters_single_mesh_and_preserves_world_position() {
        let mut meshes: SlotMap<MeshKey, TriangleMesh> = SlotMap::default();
        let offset = Vec3::new(1000.0, 2000.0, 0.0);
        let key = meshes.insert(triangle_at(offset));

        let mut obj = test_object(vec![MeshRef::new(key, LevelOfDetail::Zero)]);
        obj.rebase(&meshes);

        assert_eq!(obj.transform, Mat4::from_translation(offset));
        let world = obj.world_aabb();
        assert!((world.min - (Vec3::new(-1.0, -1.0, 0.0) + offset)).length() < 1e-3);
        assert!((world.max - (Vec3::new(1.0, 1.0, 0.0) + offset)).length() < 1e-3);
    }

    #[test]
    fn rebase_is_idempotent() {
        let mut meshes: SlotMap<MeshKey, TriangleMesh> = SlotMap::default();
        let key = meshes.insert(triangle_at(Vec3::new(5.0, 0.0, 0.0)));
        let mut obj = test_object(vec![MeshRef::new(key, LevelOfDetail::Zero)]);

        obj.rebase(&meshes);
        let (transform_1, mesh_transform_1) = (obj.transform, obj.mesh_refs[0].transform);
        obj.rebase(&meshes);

        assert_eq!(obj.transform, transform_1);
        assert_eq!(obj.mesh_refs[0].transform, mesh_transform_1);
    }

    #[test]
    fn rebase_preserves_per_mesh_placement_and_ray_hits() {
        let mut meshes: SlotMap<MeshKey, TriangleMesh> = SlotMap::default();
        let key = meshes.insert(triangle_at(Vec3::ZERO));

        let mut obj = test_object(vec![
            MeshRef::new(key, LevelOfDetail::Zero),
            MeshRef::new(key, LevelOfDetail::Zero)
                .with_transform(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0))),
        ]);
        obj.rebase(&meshes);

        // Union AABB must cover both the original and the translated placement.
        let world = obj.world_aabb();
        assert!(world.min.x <= -1.0 + 1e-3);
        assert!(world.max.x >= 11.0 - 1e-3);

        // A ray through the translated copy should hit it, unaffected by rebase.
        let ray = Ray::new(Vec3::new(10.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0));
        let t = obj.intersect_ray(&ray, &meshes).expect("ray should hit");
        assert!((t - 5.0).abs() < 1e-3);
    }

    #[test]
    fn rebase_on_empty_object_falls_back_to_zero_box() {
        let meshes: SlotMap<MeshKey, TriangleMesh> = SlotMap::default();
        let mut obj = test_object(vec![]);

        obj.rebase(&meshes);

        assert_eq!(obj.transform, Mat4::IDENTITY);
        assert_eq!(obj.aabb_local, Aabb::new(Vec3::ZERO, Vec3::ZERO));
    }
}
