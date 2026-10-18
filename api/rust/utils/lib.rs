pub mod yaml {
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
}

/// copy & pasted from <https://github.com/rust-lang/rust-analyzer/blob/70e0c46be740a471bcc95f2be290f4bbfdcffd87/crates/rust-analyzer/build.rs>
pub mod buildrs {
    use std::{path::PathBuf, process::Command};

    fn set_rerun() {
        println!("cargo:rerun-if-env-changed=CFG_RELEASE");

        let mut manifest_dir = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR")
                .expect("`CARGO_MANIFEST_DIR` is always set by cargo."),
        );

        while manifest_dir.parent().is_some() {
            let head_ref = manifest_dir.join(".git/HEAD");
            if head_ref.exists() {
                println!("cargo:rerun-if-changed={}", head_ref.display());
                return;
            }

            manifest_dir.pop();
        }

        println!("cargo:warning=Could not find `.git/HEAD` from manifest dir!");
    }

    /// Sets 'ST_COMMIT_HASH', 'ST_COMMIT_SHORT_HASH' and 'ST_COMMIT_DATE' environment variables
    /// which is accesible by using `env!("<ENV_NAME>")`.
    ///
    /// The function will be rerun when <project-root>/.git/HEAD is modified.
    pub fn set_commit_info() {
        set_rerun();

        #[allow(clippy::disallowed_methods)]
        let output = match Command::new("git")
            .arg("log")
            .arg("-1")
            .arg("--date=short")
            .arg("--format=%H %h %cd")
            .output()
        {
            Ok(output) if output.status.success() => output,
            _ => panic!("failed to get commit info"),
        };
        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut parts = stdout.split_whitespace();
        let mut next = || parts.next().unwrap();
        let _long_hash = next();
        println!("cargo:rustc-env=ST_COMMIT_SHORT_HASH={}", next());
        println!("cargo:rustc-env=ST_COMMIT_DATE={}", next())
    }
}
