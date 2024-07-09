//! `egraphics` is a library for processing graphics formats in 3D space.
//!
//! It is mainly wrapping [glTF 2.0](https://github.com/KhronosGroup/glTF/tree/main/specification/2.0) and adding an ecoords.
//!
//! # Overview
//!
//!
//! ## Data structure
//!
//! For serializing and annotating a graphical format, this data structure is used:
//!
//! - `model_name` (directory) or `model_name.egraphics` (single file as [tarball](https://en.wikipedia.org/wiki/Tar_(computing)))
//!     - `model.gltf` (uncompressed) or `model.glb` (compressed)
//!         - using: [gltf-rs](https://github.com/gltf-rs/gltf)
//!     - `model.bin`
//!         - contains vector data
//!     - `ecoord.json`
//!         - contains a transform tree with validity durations
//!         - information: srid
//!         - purpose: translate and rotate the point cloud without reading/writing the point data
//!
//! For deriving further graphical format variants, another directory is used:
//!
//! - `model_name_out` (directory)
//!     - `model.obj`
//!     - ...
//!
//!  ## References
//!
//! - [glTF sample models](https://github.com/KhronosGroup/glTF-Sample-Models)
//!
//!

pub use egraphics_core::{Error, Triangle, TriangleMesh, Vertex};

pub use egraphics_io as io;
