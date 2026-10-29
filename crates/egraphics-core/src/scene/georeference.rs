use glam::DVec3;

/// Maps a scene's small-magnitude local coordinates to true global coordinates.
///
/// `crs` and `offset` are independently optional — a scene may have one without the other
/// (e.g. a known CRS with the offset not yet determined, or vice versa).
///
/// `offset` is deliberately `f64` (not the `f32` used everywhere else in this crate for
/// object-local transforms/vertices) — its purpose is to hold the large-magnitude part
/// (UTM/ECEF-scale coordinates run into the millions), where `f32`'s ~7 significant digits
/// would destroy sub-meter precision.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Georeference {
    /// Coordinate reference system identifier (e.g. `"EPSG:25832"`). Not parsed/validated here
    /// — CRS string formats vary too much (EPSG codes, WKT1/2, PROJJSON) to pick one now.
    pub crs: Option<String>,
    /// Translation from scene-local origin to true `crs` coordinates.
    pub offset: Option<DVec3>,
}

impl Georeference {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_crs(mut self, crs: impl Into<String>) -> Self {
        self.crs = Some(crs.into());
        self
    }

    pub fn with_offset(mut self, offset: DVec3) -> Self {
        self.offset = Some(offset);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_crs_and_with_offset_set_the_respective_field() {
        let g = Georeference::new()
            .with_crs("EPSG:25832")
            .with_offset(DVec3::new(600_000.0, 5_400_000.0, 0.0));
        assert_eq!(g.crs.as_deref(), Some("EPSG:25832"));
        assert_eq!(g.offset, Some(DVec3::new(600_000.0, 5_400_000.0, 0.0)));
    }

    #[test]
    fn new_defaults_to_neither_set() {
        let g = Georeference::new();
        assert_eq!(g.crs, None);
        assert_eq!(g.offset, None);
    }
}
