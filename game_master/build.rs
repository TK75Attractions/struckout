use slint_build::CompilerConfiguration;

fn main() {
    let config = CompilerConfiguration::new()
        .with_style("material".to_string())
        .attribute(
            |typ_name| typ_name.starts_with("Inner") && typ_name.ends_with("Adopter"),
            "#[slint_fw::adopter]",
        );
    slint_build::compile_with_config("src/presentation/app-window.slint", config)
        .expect("slint build failed");
}
