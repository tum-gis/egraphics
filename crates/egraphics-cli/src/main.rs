use egraphics_io::EgraphicsExporter;
use std::path::PathBuf;
use tracing::info;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    let p = PathBuf::from("~/model.gltf");
    //let e = EgraphicsExporter::new(p, "test".to_string());
    let _x = EgraphicsExporter::new(p).with_derive_obj_file(true);

    info!("ok");
}
