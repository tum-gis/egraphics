slotmap::new_key_type! {
    /// Key identifying a [`TriangleMesh`](super::TriangleMesh) stored in [`Scene::meshes`](super::Scene::meshes).
    pub struct MeshKey;
    /// Key identifying an [`Object`](super::Object) stored in [`Scene::objects`](super::Scene::objects).
    pub struct ObjectKey;
    /// Key identifying a class name interned in [`Scene::classes`](super::Scene::classes).
    pub struct ClassKey;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ObjectId(String);

impl From<String> for ObjectId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<ObjectId> for String {
    fn from(id: ObjectId) -> Self {
        id.0
    }
}
