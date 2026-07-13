use slint_build::CompilerConfiguration;

fn main() {
    let config = CompilerConfiguration::new().with_style("material".to_string());
    slint_build::compile_with_config("src/presentation/app-window.slint", config)
        .expect("slint build failed");
}
