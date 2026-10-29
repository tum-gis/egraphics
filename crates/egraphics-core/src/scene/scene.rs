use slotmap::SlotMap;
use std::collections::HashMap;

use super::{ClassKey, Georeference, MeshKey, ObjectKey};
use crate::scene::id::ObjectId;
use crate::{Bvh, Object, TriangleMesh};

/// A collection of objects with a shared mesh registry and an optional BVH.
#[derive(Debug, Default)]
pub struct Scene {
    objects: SlotMap<ObjectKey, Object>,

    object_key_by_id: HashMap<ObjectId, ObjectKey>,

    /// Shared mesh storage. Register a mesh here once and reference it from
    /// multiple objects to avoid duplicating geometry (instancing).
    meshes: SlotMap<MeshKey, TriangleMesh>,

    /// Interned class names. Unlike meshes, class names are auto-deduplicated by
    /// value — see [`Scene::get_or_insert_class_key`].
    classes: SlotMap<ClassKey, String>,

    class_key_by_name: HashMap<String, ClassKey>,

    georeference: Option<Georeference>,

    bvh: Option<Bvh>,
}

impl Scene {
    /// Constructs an empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn objects(&self) -> &SlotMap<ObjectKey, Object> {
        &self.objects
    }

    pub fn meshes(&self) -> &SlotMap<MeshKey, TriangleMesh> {
        &self.meshes
    }

    pub fn classes(&self) -> &SlotMap<ClassKey, String> {
        &self.classes
    }

    /// Maps this scene's local coordinates to true global coordinates, if set.
    pub fn georeference(&self) -> Option<&Georeference> {
        self.georeference.as_ref()
    }

    /// Sets (or, with `None`, clears) this scene's georeferencing.
    pub fn set_georeference(&mut self, georeference: Option<Georeference>) {
        self.georeference = georeference;
    }

    /// Resolves an interned class name.
    pub fn class_name(&self, key: ClassKey) -> Option<&str> {
        self.classes.get(key).map(String::as_str)
    }

    /// Returns the existing [`ClassKey`] for `name`, or interns it as a new one if this is the
    /// first time it's been seen — unlike [`Scene::add_mesh`], which always inserts a new entry,
    /// sharing is automatic here rather than the caller's choice.
    pub fn get_or_insert_class_key(&mut self, name: impl Into<String>) -> ClassKey {
        let name = name.into();
        if let Some(&key) = self.class_key_by_name.get(&name) {
            return key;
        }
        let key = self.classes.insert(name.clone());
        self.class_key_by_name.insert(name, key);
        key
    }

    /// Looks up an object by its storage key.
    pub fn get_object(&self, key: ObjectKey) -> Option<&Object> {
        self.objects.get(key)
    }

    /// Looks up an object by its storage key, for in-place mutation.
    ///
    /// Note: don't mutate the returned object's `object_id` — doing so desyncs
    /// [`Scene::get_object_by_id`]'s index, which is keyed on the value it had at insertion time.
    pub fn get_object_mut(&mut self, key: ObjectKey) -> Option<&mut Object> {
        self.objects.get_mut(key)
    }

    /// Looks up an object by its business `object_id`.
    pub fn get_object_by_id(&self, id: &ObjectId) -> Option<&Object> {
        let key = *self.object_key_by_id.get(id)?;
        self.objects.get(key)
    }

    /// Looks up an object by its business `object_id`, for in-place mutation.
    ///
    /// Note: don't mutate the returned object's `object_id` — doing so desyncs
    /// this same index.
    pub fn get_object_by_id_mut(&mut self, id: &ObjectId) -> Option<&mut Object> {
        let key = *self.object_key_by_id.get(id)?;
        self.objects.get_mut(key)
    }

    /// Looks up a mesh by its storage key.
    pub fn get_mesh(&self, key: MeshKey) -> Option<&TriangleMesh> {
        self.meshes.get(key)
    }

    /// Looks up a mesh by its storage key, for in-place mutation.
    pub fn get_mesh_mut(&mut self, key: MeshKey) -> Option<&mut TriangleMesh> {
        self.meshes.get_mut(key)
    }

    /// Registers a mesh in the shared registry and returns a key to reference it.
    ///
    /// Pass the returned [`MeshKey`] to one or more [`Object`]s (via [`crate::MeshRef`])
    /// to share the geometry.
    pub fn add_mesh(&mut self, mesh: TriangleMesh) -> MeshKey {
        self.meshes.insert(mesh)
    }

    /// Appends an object, deriving its `transform` and `aabb_local` from the current mesh
    /// registry (see [`Object::rebase`]). Invalidates the existing BVH.
    pub fn add_object(&mut self, mut object: Object) -> ObjectKey {
        object.rebase(&self.meshes);
        self.bvh = None;

        let object_id = object.object_id.clone();
        let object_key = self.objects.insert(object);
        self.object_key_by_id.insert(object_id, object_key);
        object_key
    }

    /// Re-derives every object's `transform`, `aabb_local`, and `MeshRef` placements from the
    /// current mesh registry (see [`Object::rebase`]).
    ///
    /// Call this after mutating a mesh's geometry through [`Scene::get_mesh_mut`] — cached
    /// AABBs otherwise go stale, since meshes are shared and mutation doesn't touch the
    /// objects that reference them. Invalidates the existing BVH.
    pub fn rebuild_object_aabbs(&mut self) {
        for object in self.objects.values_mut() {
            object.rebase(&self.meshes);
        }
        self.bvh = None;
    }

    /// Rebuilds the flat BVH over all current objects using the SAH strategy.
    ///
    /// Must be called after all objects are added (or after any mutation) before
    /// acceleration-structure queries can be made.
    pub fn rebuild_bvh(&mut self) {
        use crate::accel::BuildStrategy;
        let refs: Vec<&Object> = self.objects.values().collect();
        self.bvh = Some(Bvh::build(&refs, BuildStrategy::Sah));
    }

    /// Returns a reference to the BVH if it has been built.
    pub fn bvh(&self) -> Option<&Bvh> {
        self.bvh.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_key_interns_same_name_to_same_key() {
        let mut scene = Scene::new();
        let a = scene.get_or_insert_class_key("Building");
        let b = scene.get_or_insert_class_key("Building");
        assert_eq!(a, b);
        assert_eq!(scene.classes().len(), 1);
    }

    #[test]
    fn class_key_gives_distinct_names_distinct_keys() {
        let mut scene = Scene::new();
        let building = scene.get_or_insert_class_key("Building");
        let road = scene.get_or_insert_class_key("Road");
        assert_ne!(building, road);
        assert_eq!(scene.classes().len(), 2);
    }

    #[test]
    fn class_name_resolves_interned_key() {
        let mut scene = Scene::new();
        let key = scene.get_or_insert_class_key("Tree");
        assert_eq!(scene.class_name(key), Some("Tree"));
    }

    #[test]
    fn georeference_defaults_to_none() {
        let scene = Scene::new();
        assert_eq!(scene.georeference(), None);
    }

    #[test]
    fn set_georeference_round_trips_and_clears() {
        let mut scene = Scene::new();
        let geo = Georeference::new()
            .with_crs("EPSG:25832")
            .with_offset(glam::DVec3::new(600_000.0, 5_400_000.0, 0.0));

        scene.set_georeference(Some(geo.clone()));
        assert_eq!(scene.georeference(), Some(&geo));

        scene.set_georeference(None);
        assert_eq!(scene.georeference(), None);
    }
}
