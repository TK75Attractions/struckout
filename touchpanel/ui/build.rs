use slint_build::CompilerConfiguration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CompilerConfiguration::new()
        .with_style("struckout".to_string())
        .attribute(
            |typ_name| typ_name.starts_with("Inner") && typ_name.ends_with("Adopter"),
            "#[stern::adopter]",
        );
    let profile = std::env::var("PROFILE").expect("'PROFILE' should be set by cargo");
    let config = (profile == "debug")
        .then_some(config.with_debug_info(true))
        .unwrap_or(config);

    slint_build::compile_with_config("./app-window.slint", config)?;

    Ok(())
}
