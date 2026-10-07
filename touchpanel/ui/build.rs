use slint_build::CompilerConfiguration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CompilerConfiguration::new()
        .with_style("material".to_string())
        .attribute(
            |typ_name| typ_name.starts_with("Inner") && typ_name.ends_with("Adopter"),
            "#[stern::adopter]",
        );
    slint_build::compile_with_config("./app-window.slint", config)?;

    Ok(())
}
