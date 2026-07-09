use std::{path::PathBuf, str::FromStr};

fn main() -> std::io::Result<()> {
    let proto_files = [
        "../proto/struckout.proto",
        "../proto/collision.proto",
        "../proto/xtask_sync.proto",
        "../proto/master_and_projector.proto",
    ];
    let includes = ["../proto/"];
    proto_files
        .iter()
        .chain(includes.iter())
        .map(|s| PathBuf::from_str(s).unwrap())
        .for_each(|path| println!("cargo::rerun-if-changed={}", path.display()));

    prost_build::compile_protos(&proto_files, &includes)?;
    Ok(())
}
