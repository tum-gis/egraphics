//! `egraphics` is a library for processing graphics formats in 3D space.
//!
//! It builds on [glTF 2.0](https://github.com/KhronosGroup/glTF/tree/main/specification/2.0) and
//! adds a sidecar document with per-object metadata (identifiers, classes, transforms, bounding
//! boxes, properties) and an optional georeference.
//!
//! # Overview
//!
//! A [`Scene`] holds the triangle meshes and the objects referencing them. It is exported with
//! [`io::EgraphicsWriter`]:
//!
//! ```no_run
//! use egraphics::Scene;
//! use egraphics::io::EgraphicsWriter;
//!
//! # fn main() -> Result<(), egraphics::io::Error> {
//! let scene = Scene::new();
//! EgraphicsWriter::from_path("model.egraphics")?.finish(scene)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## File format
//!
//! A model is stored as a single [tar](https://en.wikipedia.org/wiki/Tar_(computing)) archive.
//! The file extension selects the variant:
//!
//! - `model_name.egraphics` (compact, the default)
//!     - `model_data.glb`: the geometry as binary glTF
//!     - `info.json.zst`: the sidecar document, compressed with [Zstandard](https://facebook.github.io/zstd/)
//! - `model_name.egraphics.tar` (uncompressed, human-readable)
//!     - `model_data.gltf`: the glTF document (pretty-printed JSON)
//!     - `model_data.bin`: the vertex and index buffer referenced by `model_data.gltf`
//!     - `info.json`: the sidecar document (pretty-printed JSON)
//!
//! The sidecar document contains:
//!
//! - `georeference` (optional): the coordinate reference system (`crs`) and a
//!   double-precision `offset` to add to the single-precision vertex coordinates
//! - `meshes`: one entry per mesh written to the buffer, with its level of detail, vertex and
//!   index ranges and bounding box
//! - `objects`: one entry per object, with its identifier, name, class, bounding box,
//!   referenced meshes (with an optional world transform each) and properties
//!
//! How the geometry is grouped into glTF meshes is controlled by [`io::MeshGrouping`].
//!
//! ## References
//!
//! - [glTF sample models](https://github.com/KhronosGroup/glTF-Sample-Models)

pub use egraphics_core::{
    Aabb, BuildStrategy, Bvh, Error, Frustum, Georeference, LevelOfDetail, MeshKey, MeshRef,
    Object, ObjectKey, PropertyValue, Ray, Scene, Triangle, TriangleMesh,
};

pub use egraphics_io as io;
