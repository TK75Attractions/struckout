fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=../../api/proto/");
    println!("cargo::rerun-if-changed=../../api/spec/servers.yaml");

    tonic_prost_build::configure().compile_protos(
        &["../../api/proto/game-master.proto"],
        &["../../api/proto/"],
    )?;

    // 本物と同じ出どころからポートを取る。ここを別に持つと、
    // spec を直したときに偽物だけ古いポートで待つことになる。
    let port = struckout_yaml::get_port_def("../../api/spec/servers.yaml", "game-master", "grpc")?;
    println!("cargo::rustc-env=GAME_MASTER_GRPC_PORT={}", port);

    Ok(())
}
