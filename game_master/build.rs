fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=../api/proto/");

    tonic_prost_build::configure()
        .compile_protos(&["../api/proto/game-master.proto"], &["../api/proto/"])?;
    Ok(())
}
