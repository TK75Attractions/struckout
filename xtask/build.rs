fn main() -> std::io::Result<()> {
    prost_build::compile_protos(
        &["../protocol/struckout.proto", "../protocol/collision.proto"],
        &["../protocol/"],
    )?;
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations");
    Ok(())
}
