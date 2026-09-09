use slint_build::CompilerConfiguration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CompilerConfiguration::new()
        .with_style("material".to_string())
        .attribute(
            |typ_name| typ_name.starts_with("Inner") && typ_name.ends_with("Adopter"),
            "#[stern::adopter]",
        );
    slint_build::compile_with_config("src/presentation/app-window.slint", config)?;

    println!("cargo::rerun-if-changed=../api/proto/");
    tonic_prost_build::configure()
        .compile_protos(&["../api/proto/game-master.proto"], &["../api/proto/"])?;

    let port = struckout_yaml::get_port_def("../api/spec/servers.yaml", "game-master", "grpc")?;
    println!("cargo::rustc-env=TOUCHPANEL_GAME_MASTER_GRPC_PORT={}", port);
    Ok(())
}
