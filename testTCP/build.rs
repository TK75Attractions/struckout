fn main() {
    println!("cargo:rerun-if-changed=proto/");
    prost_build::compile_protos(
    &[
        "../protocol/collision.proto",
        "../protocol/debug.proto",
    ],
    &["../protocol/"],
    ).unwrap();
}