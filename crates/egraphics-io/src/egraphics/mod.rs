mod document;
#[expect(dead_code, reason = "reader is not exposed yet")]
mod read;
mod read_impl;
pub mod write;
mod write_impl;

pub const FILE_EXTENSION_EGRAPHICS_FORMAT: &str = "egraphics";
pub const FILE_EXTENSION_EGRAPHICS_TAR_FORMAT: &str = "egraphics.tar";

pub const FILE_NAME_MODEL_DATA_COMPRESSED: &str = "model_data.glb";
pub const FILE_NAME_MODEL_DATA_UNCOMPRESSED: &str = "model_data.gltf";
pub const FILE_NAME_MODEL_DATA_BIN_UNCOMPRESSED: &str = "model_data.bin";
pub const FILE_NAME_INFO_COMPRESSED: &str = "info.json.zst";
pub const FILE_NAME_INFO_UNCOMPRESSED: &str = "info.json";
