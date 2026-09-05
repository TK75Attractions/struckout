use std::{path::PathBuf, str::FromStr};

fn main() -> std::io::Result<()> {
    let proto_files = [
        "../proto/ball-tracker__projector.proto",
        "../proto/camera__ball-tracker.proto",
        "../proto/game-master.proto",
        "../proto/xtask_sync.proto",
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
