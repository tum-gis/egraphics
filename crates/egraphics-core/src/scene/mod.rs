mod georeference;
mod id;
mod level_of_detail;
mod mesh;
mod object;
mod property;
#[expect(
    clippy::module_inception,
    reason = "`scene::scene` holds the `Scene` type alongside its sibling scene modules"
)]
mod scene;

pub use georeference::Georeference;
pub use id::{ClassKey, MeshKey, ObjectKey};
pub use level_of_detail::LevelOfDetail;
pub use mesh::TriangleMesh;
pub use object::{MeshRef, Object};
pub use property::PropertyValue;
pub use scene::Scene;
