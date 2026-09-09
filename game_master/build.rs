fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=../api/proto/");

    tonic_prost_build::configure()
        .compile_protos(&["../api/proto/game-master.proto"], &["../api/proto/"])?;

    let port = struckout_yaml::get_port_def("../api/spec/servers.yaml", "game-master", "grpc")?;
    println!("cargo::rustc-env=GAME_MASTER_GRPC_PORT={}", port);

    Ok(())
}
