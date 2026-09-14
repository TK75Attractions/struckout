fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "../../proto/ball-tracker__projector.proto",
        "../../proto/camera__ball-tracker.proto",
        "../../proto/game-master.proto",
        "../../proto/xtask_sync.proto",
    ];
    let include = "../../proto/";
    println!("cargo::rerun-if-changed={}", include);

    tonic_prost_build::configure().compile_protos(&proto_files, &[include])?;
    Ok(())
}
