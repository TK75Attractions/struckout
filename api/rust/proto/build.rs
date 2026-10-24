fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = [
        "../../proto/ball-tracker__projector.proto",
        "../../proto/camera__ball-tracker.proto",
        "../../proto/xtask_sync.proto",
        "../../proto/game_master/session.proto",
        "../../proto/game_master/player.proto",
    ];
    let include = "../../proto/";
    println!("cargo::rerun-if-changed={}", include);

    const SERDE_ATTR: &str = "#[derive(serde::Serialize, serde::Deserialize)]";
    let mut config = tonic_prost_build::Config::new();
    config.type_attribute(".tk75attractions.struckout.v1.DetectionsPacket", SERDE_ATTR);
    config.type_attribute(".tk75attractions.struckout.v1.Detection", SERDE_ATTR);
    config.type_attribute(".tk75attractions.struckout.v1.CameraPose", SERDE_ATTR);
    config.type_attribute(
        ".tk75attractions.struckout.game_master.v1.GameMode",
        "#[derive(::strum::EnumIs)]",
    );

    tonic_prost_build::configure().compile_with_config(config, &proto_files, &[include])?;
    Ok(())
}
