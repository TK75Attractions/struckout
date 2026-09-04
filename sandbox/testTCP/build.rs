fn main() {
    println!("cargo:rerun-if-changed=proto/");
    prost_build::compile_protos(
    &[
        "../protocol/debug.proto",
        "../protocol/collision.proto"
    ],
    &["../protocol/"],
    ).unwrap();
}