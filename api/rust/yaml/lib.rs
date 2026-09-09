use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("no such entity found in yaml file")]
struct ErrNoSuchEntity;

/// Gets port definition from specified file.
pub fn get_port_def(
    file_path: impl AsRef<Path>,
    module_name: &str,
    server_name: &str,
) -> Result<u16, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file_path)?;
    let config: Config = serde_saphyr::from_str(&content)?;
    let server = config
        .servers
        .get(module_name)
        .ok_or(ErrNoSuchEntity)?
        .get(server_name)
        .ok_or(ErrNoSuchEntity)?;
    Ok(server.port)
}

#[derive(Debug, Deserialize)]
pub struct Config {
    servers: HashMap<String, HashMap<String, Server>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Server {
    #[serde(rename = "type")]
    protocol_type: ProtocolType,
    port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Grpc,
    Tcp,
}
