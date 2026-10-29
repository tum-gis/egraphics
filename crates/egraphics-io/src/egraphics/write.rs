use crate::Error::{InvalidFileExtension, NoFileName};
use crate::egraphics::write_impl::write_egraphics_format;
use crate::egraphics::{FILE_EXTENSION_EGRAPHICS_FORMAT, FILE_EXTENSION_EGRAPHICS_TAR_FORMAT};
use crate::error::Error;
use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub const DEFAULT_COMPRESSION_LEVEL: i32 = 10;

/// Controls how objects' non-shared geometry is grouped into glTF meshes.
///
/// Either way, geometry genuinely shared by multiple objects (via a repeated `MeshKey`) is
/// always written once and referenced by multiple nodes — this only affects the remainder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshGrouping {
    /// Merge every non-shared object of a class into one glTF mesh. Fewer draw calls, but
    /// individual object geometry can't be picked back out of the merged buffer.
    #[default]
    PerClass,
    /// Give every object its own glTF node/mesh. Doesn't scale to large scenes, but geometry
    /// maps 1:1 to objects — helpful for inspecting/debugging small scenes.
    PerObject,
}

/// `EgraphicsWriter` exports a scene as a tar archive containing a glTF model
/// and an info sidecar.
///
/// Compressed (default): `model_data.glb` + `info.json.zst` inside the archive.
/// Uncompressed: `model_data.gltf` + `model_data.bin` + `info.json`.
#[derive(Debug, Clone)]
pub struct EgraphicsWriter<W: Write> {
    writer: W,
    compression_level: Option<i32>,
    time: Option<DateTime<Utc>>,
    pretty: bool,
    mesh_grouping: MeshGrouping,
}

impl<W: Write> EgraphicsWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            compression_level: Some(DEFAULT_COMPRESSION_LEVEL),
            time: None,
            pretty: false,
            mesh_grouping: MeshGrouping::default(),
        }
    }

    pub fn with_compressed(mut self, compressed: bool) -> Self {
        self.compression_level = if compressed {
            Some(DEFAULT_COMPRESSION_LEVEL)
        } else {
            None
        };
        self
    }

    pub fn with_time(mut self, time: Option<DateTime<Utc>>) -> Self {
        self.time = time;
        self
    }

    pub fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    pub fn with_mesh_grouping(mut self, mesh_grouping: MeshGrouping) -> Self {
        self.mesh_grouping = mesh_grouping;
        self
    }

    pub fn finish(self, scene: egraphics_core::Scene) -> Result<(), Error> {
        write_egraphics_format(
            scene,
            self.writer,
            self.mesh_grouping,
            self.compression_level,
            self.time,
            self.pretty,
        )
    }
}

impl EgraphicsWriter<File> {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file_name_str = path
            .as_ref()
            .file_name()
            .ok_or(NoFileName())?
            .to_string_lossy()
            .to_lowercase();
        if !file_name_str.ends_with(FILE_EXTENSION_EGRAPHICS_TAR_FORMAT)
            && !file_name_str.ends_with(FILE_EXTENSION_EGRAPHICS_FORMAT)
        {
            return Err(InvalidFileExtension(file_name_str.to_string()));
        }

        let compressed = !file_name_str.ends_with(FILE_EXTENSION_EGRAPHICS_TAR_FORMAT);
        let pretty = !compressed;

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self::new(file)
            .with_compressed(compressed)
            .with_pretty(pretty))
    }
}
