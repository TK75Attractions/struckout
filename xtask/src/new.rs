use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use anyhow::{Context, bail, ensure};
use clap::Args;
use git2::Repository;
use heck::{ToKebabCase, ToSnakeCase, ToUpperCamelCase};
use tempfile::NamedTempFile;

#[derive(Args)]
pub struct NewArgs {
    #[arg(
        help = "Name of the screen you are going to create (e.g. 'Home' if you'd create 'component HomeScreen')"
    )]
    name: String,
    #[arg(
        long,
        default_value_t = false,
        help = "Whether to allow modification when the working directory is dirty or has unstaged changes"
    )]
    allow_dirty: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Whether to allow modification when a VCS was not detected"
    )]
    allow_no_vcs: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Whether to allow modification when the working directory has staged changes"
    )]
    allow_staged: bool,
}

impl NewArgs {
    pub fn run(self) -> Result<(), anyhow::Error> {
        let cwd = std::env::current_dir().unwrap();
        let Some(std::path::Component::Normal(v)) = cwd.components().next_back() else {
            return Err(anyhow::anyhow!("invalid cwd"));
        };
        ensure!(
            v == "touchpanel",
            "the 'xtask new' command should be run in '<repo root>/touchpanel' directory"
        );

        check_version_control(cwd, self.allow_no_vcs, self.allow_dirty, self.allow_staged)?;

        process_app_window_slint(&self.name)?;
        process_screen_slint(&self.name)?;
        process_presentation_mod_rs(&self.name)?;
        process_impl_rs(&self.name)?;

        Ok(())
    }
}

/// Processes `ui/app-window.slint`.
fn process_app_window_slint(name: &str) -> anyhow::Result<()> {
    let file = File::open("ui/app-window.slint").with_context(|| "failed to open slint file")?;
    let reader = BufReader::new(file);
    let mut out = tempfile::NamedTempFile::new()?;

    let name_upper_camel = name.to_upper_camel_case();
    let name_kebab = name.to_kebab_case();
    for line in reader.lines() {
        let line = line?;
        match line.as_str() {
            "//@xtask-import" => {
                writeln!(
                    out,
                    "import {{ {}Screen }} from \"./screens/{}-screen.slint\";",
                    name_upper_camel, name_kebab,
                )?;
            }
            "//@xtask-export" => {
                writeln!(
                    out,
                    "export {{ {}Adopter }}from \"./screens/{}-screen.slint\"",
                    name_upper_camel, name_kebab
                )?;
            }
            "//@xtask-enum" => {
                writeln!(out, "    {}", name_upper_camel)?;
            }
            "//@xtask-screen" => writeln!(
                out,
                "    if nav-route == UiNavRoute.{}: {}Screen",
                name_upper_camel, name_upper_camel,
            )?,
            _ => (),
        }
        writeln!(out, "{}", line)?;
    }
    std::fs::rename(out.path(), "ui/app-window.slint")?;
    Ok(())
}

/// Processes `ui/screens/<name>-screen.slint`.
fn process_screen_slint(name: &str) -> anyhow::Result<()> {
    let mut file = File::create_new(format!("ui/screens/{}-screen.slint", name.to_kebab_case()))?;
    write!(
        file,
        "
global Adopter {{  }}

export component {}Screen {{
    Text {{
        text: \"{}\"
    }}
}}

export {{ Adopter as {}Adopter }}
",
        name.to_upper_camel_case(),
        name.to_kebab_case(),
        name.to_upper_camel_case()
    )?;
    Ok(())
}

/// Processes `src/presentation/mod.rs`.
fn process_presentation_mod_rs(name: &str) -> anyhow::Result<()> {
    let file = File::create("src/presentation/mod.rs")?;
    let reader = BufReader::new(file);
    let mut out = NamedTempFile::new()?;
    for line in reader.lines() {
        let line = line?;
        match line.as_str() {
            "//@xtask-pub-mod" => {
                writeln!(out, "pub mod {}", name.to_snake_case())?;
            }
            "//@xtask-register" => {
                writeln!(
                    out,
                    ".register({}Destination::new(&application))",
                    name.to_upper_camel_case()
                )?;
            }
            _ => (),
        }
        writeln!(out, "{}", line)?;
    }
    std::fs::rename(out.path(), "src/presentation/mod.rs")?;
    Ok(())
}

fn process_impl_rs(name: &str) -> anyhow::Result<()> {
    let mut file = File::create_new(format!("src/presentation/{}.rs", name.to_snake_case()))?;
    write!(file, "{}", impl_rs_content(name))?;
    Ok(())
}

fn impl_rs_content(name: &str) -> String {
    let snake = name.to_snake_case();
    let upper = name.to_upper_camel_case();
    format!(
        "\
use touchpanel_ui::{{{upper}States, {upper}ViewModelTrait, NavRoute, NavRouteKind}};
use slint::Global;
use stern::nav::NavDestination;

use crate::Application;

touchpanel_ui::define_{snake}_mapper! {{}}

struct {upper}ViewModel {{
    state: {upper}States<Mapper>,
}}

impl {upper}ViewModel {{
    fn new(application: &Application) -> Self {{
        Self {{
            state: {upper}States::<Mapper>::new(
                application.ui.global::<touchpanel_ui::{upper}Adopter>.as_weak(),
            )
        }}
    }}
}}

impl {upper}ViewModelTrait for {upper}ViewModel {{}}

pub struct {upper}Destination({upper}ViewModel);

impl {upper}Destination {{
    pub fn new(application: &Application) -> Self {{
        Self({upper}ViewModel::new(application))
    }}
}}

impl NavDestination<NavRoute> for {upper}Destination {{
    fn load(&mut self, route: &NavRoute) {{
        let NavRoute::{upper} = route else {{
            panic!(\"given NavRoute has invalid variant value\");
        }};

        todo!()
    }}

    fn route(&self) -> NavRouteKind {{
        NavRouteKind::{upper}
    }}
}}
"
    )
}

// copy & pasted from https://github.com/rust-lang/cargo/blob/e6faaba5c4c461c818212756bea8681555019b4d/src/ops/cargo_fix/mod.rs#L223
fn check_version_control(
    cwd: impl AsRef<Path>,
    allow_no_vcs: bool,
    allow_dirty: bool,
    allow_staged: bool,
) -> anyhow::Result<()> {
    if allow_no_vcs {
        return Ok(());
    }
    if !existing_vcs_repo(&cwd) {
        bail!(
            "no VCS found for this package and `xtask new` can potentially \
             perform destructive changes; if you'd like to suppress this \
             error pass `--allow-no-vcs`"
        )
    }

    if allow_dirty && allow_staged {
        return Ok(());
    }

    let mut dirty_files = Vec::new();
    let mut staged_files = Vec::new();
    if let Ok(repo) = git2::Repository::discover(cwd) {
        let mut repo_opts = git2::StatusOptions::new();
        repo_opts.include_ignored(false);
        repo_opts.include_untracked(true);
        for status in repo.statuses(Some(&mut repo_opts))?.iter() {
            if let Ok(path) = status.path() {
                match status.status() {
                    git2::Status::CURRENT => (),
                    git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE => {
                        if !allow_staged {
                            staged_files.push(path.to_string())
                        }
                    }
                    _ => {
                        if !allow_dirty {
                            dirty_files.push(path.to_string())
                        }
                    }
                };
            }
        }
    }

    if dirty_files.is_empty() && staged_files.is_empty() {
        return Ok(());
    }

    let mut files_list = String::new();
    for file in dirty_files {
        files_list.push_str("  * ");
        files_list.push_str(&file);
        files_list.push_str(" (dirty)\n");
    }
    for file in staged_files {
        files_list.push_str("  * ");
        files_list.push_str(&file);
        files_list.push_str(" (staged)\n");
    }

    bail!(
        "the working directory has uncommitted changes, and \
         `xtask new` can potentially perform destructive changes; if you'd \
         like to suppress this error pass `--allow-dirty`, \
         or commit the changes to these files:\n\
         \n\
         {}\n\
         ",
        files_list
    );
}

fn existing_vcs_repo(cwd: impl AsRef<Path>) -> bool {
    let Ok(repo) = Repository::discover(&cwd) else {
        return false;
    };
    // Don't check if the working directory itself is ignored.
    if repo
        .workdir()
        .is_some_and(|workdir| workdir == std::env::current_dir().unwrap())
    {
        true
    } else {
        !repo.is_path_ignored(cwd).unwrap_or(false)
    }
}
