
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = struckout_yaml::get_port_def("../api/spec/servers.yaml", "game-master", "grpc")?;
    println!("cargo::rustc-env=TOUCHPANEL_GAME_MASTER_GRPC_PORT={}", port);
    Ok(())
}
