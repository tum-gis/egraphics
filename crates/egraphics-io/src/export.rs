use crate::error::Error;
use crate::export_impl::write_gltf_file;
use egraphics_core::TriangleMesh;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `EgraphicsExporter` exports to glTF 2.0 and derives to additional formats.
///
/// * `path` - Path to file with gltf extension.
/// * `derive_obj_file` - If true, also derive an obj file (dependency [assimp](https://github.com/assimp/assimp)).
#[derive(Debug, Clone)]
pub struct EgraphicsExporter {
    path: PathBuf,
    derive_obj_file: bool,
    create_parent_directories: bool,
}

impl EgraphicsExporter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            derive_obj_file: false,
            create_parent_directories: false,
        }
    }

    pub fn with_derive_obj_file(mut self, derive_obj_file: bool) -> Self {
        self.derive_obj_file = derive_obj_file;
        self
    }

    pub fn with_create_parent_directories(mut self, create_parent_directories: bool) -> Self {
        self.create_parent_directories = create_parent_directories;
        self
    }

    pub fn get_obj_file_path(&self) -> PathBuf {
        self.path.clone().with_extension("obj")
    }

    pub fn finish(&self, triangle_mesh: TriangleMesh) -> Result<(), Error> {
        if self.create_parent_directories {
            fs::create_dir_all(self.path.parent().unwrap())?;
        }

        write_gltf_file(triangle_mesh, &self.path);

        if self.derive_obj_file {
            let mut child = Command::new("assimp")
                .arg("export")
                .arg(&self.path)
                .arg(self.get_obj_file_path())
                .spawn()
                .expect("assimp command failed to start");

            let _result = child.wait().unwrap();
        }

        Ok(())
    }
}
