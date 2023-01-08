//! Library for using georeferencing [glTF 2.0](https://github.com/KhronosGroup/glTF/tree/main/specification/2.0).
//!
//! # Overview
//!
//!
//! ## Data structure
//!
//! For serializing and annotating a graphical format, this data structure is used:
//!
//! - `model_name` (directory) or `model_name.egra` (single file as [tarball](https://en.wikipedia.org/wiki/Tar_(computing)))
//!     - `model.gltf` (uncompressed) or `model.glb` (compressed)
//!         - using: [gltf-rs](https://github.com/gltf-rs/gltf)
//!     - `model.bin`
//!         - contains vector data
//!     - `frames.json`
//!         - contains a transformation tree with validity durations
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

pub use egraphics_core as core;

pub use egraphics_io as io;
