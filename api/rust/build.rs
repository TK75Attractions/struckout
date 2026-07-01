fn main() -> std::io::Result<()> {
    prost_build::compile_protos(
        &["../proto/struckout.proto", "../proto/collision.proto"],
        &["../proto/"],
    )?;
    Ok(())
}
