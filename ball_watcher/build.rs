fn main() -> std::io::Result<()> {
    prost_build::compile_protos(
        &["../protocol/struckout.proto", "../protocol/collision.proto"],
        &["../protocol/"],
    )?;
    Ok(())
}
