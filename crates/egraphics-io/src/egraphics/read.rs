use crate::Error;
use crate::Error::{InvalidFileExtension, NoFileName};
use crate::egraphics::{FILE_EXTENSION_EGRAPHICS_FORMAT, FILE_EXTENSION_EGRAPHICS_TAR_FORMAT};
use egraphics_core::Scene;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tar::Archive;

/// `EgraphicsReader` sets up a reader for the custom reader data structure.
///
#[derive(Debug, Clone)]
pub struct EgraphicsReader<R: Read> {
    reader: R,
}

impl<R: Read> EgraphicsReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn finish(self) -> Result<Scene, Error> {
        let _archive = Archive::new(self.reader);

        let scene = Scene::new();
        Ok(scene)
    }
}

impl EgraphicsReader<File> {
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

        let file = std::fs::File::open(path)?;
        Ok(Self::new(file))
    }
}
