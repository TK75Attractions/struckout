fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "../../proto/ball-tracker__projector.proto",
        "../../proto/camera__ball-tracker.proto",
        "../../proto/game-master.proto",
        "../../proto/xtask_sync.proto",
    ];
    let include = "../../proto/";
    println!("cargo::rerun-if-changed={}", include);

    const SERDE_ATTR: &str = "#[derive(serde::Serialize, serde::Deserialize)]";
    let mut config = tonic_prost_build::Config::new();
    config.type_attribute(".tk75attractions.struckout.v1.DetectionsPacket", SERDE_ATTR);
    config.type_attribute(".tk75attractions.struckout.v1.Detection", SERDE_ATTR);

    tonic_prost_build::configure().compile_with_config(config, &proto_files, &[include])?;
    Ok(())
}
