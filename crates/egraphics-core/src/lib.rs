mod accel;
mod error;
mod math;
mod scene;

#[doc(inline)]
pub use error::Error;

#[doc(inline)]
pub use accel::{BuildStrategy, Bvh};

#[doc(inline)]
pub use math::{Aabb, Frustum, Ray, Triangle};

#[doc(inline)]
pub use scene::{
    ClassKey, Georeference, LevelOfDetail, MeshKey, MeshRef, Object, ObjectKey, PropertyValue,
    Scene, TriangleMesh,
};
