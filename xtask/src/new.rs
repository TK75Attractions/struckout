use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail, ensure};
use clap::Args;
use git2::Repository;
use heck::{ToKebabCase, ToSnakeCase, ToUpperCamelCase};
use tempfile::NamedTempFile;

macro_rules! verbose {
    ($verbose:expr, $($arg:tt)*) => {
        if $verbose {
            ::std::println!($($arg)*);
        }
    };
}

#[derive(Args)]
pub struct NewArgs {
    #[arg(
        help = "Name of the screen you are going to create (e.g. 'Home' if you'd create 'component HomeScreen')"
    )]
    name: String,
    #[arg(
        long,
        default_value_t = false,
        help = "Allow modification when the working directory is dirty or has unstaged changes"
    )]
    allow_dirty: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Allow modification when a VCS was not detected"
    )]
    allow_no_vcs: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Allow modification when the working directory has staged changes"
    )]
    allow_staged: bool,
}

impl NewArgs {
    pub fn run(self, verbose: bool) -> Result<(), anyhow::Error> {
        let cwd = std::env::current_dir().unwrap();
        let Some(std::path::Component::Normal(v)) = cwd.components().next_back() else {
            return Err(anyhow::anyhow!("invalid cwd"));
        };
        ensure!(
            v == "touchpanel",
            "the 'xtask new' command should be run in '<repo root>/touchpanel' directory"
        );

        let cache_dir = dirs::cache_dir()
            .with_context(|| "can't determine cache directory")?
            .join("struckout_xtask");
        if !cache_dir.exists() {
            verbose!(
                verbose,
                "cache dir does not exist; creating one at {}",
                cache_dir.display()
            );
            std::fs::create_dir_all(&cache_dir)?;
        }
        let ctx = Context {
            cwd,
            cache_dir,
            args: self,
            verbose,
        };
        let name = &ctx.args.name;

        ctx.check_version_control()?;
        verbose!(verbose, "checked git status");

        verbose!(verbose, "processing ui/app-window.slint");
        ctx.process_app_window_slint()?;

        verbose!(
            verbose,
            "processing ui/screens/{}-screen.slint",
            name.to_kebab_case()
        );
        process_screen_slint(&ctx.args.name)?;

        verbose!(verbose, "processing src/presentation/mod.rs");
        ctx.process_presentation_mod_rs()?;

        verbose!(verbose, "processing ui/lib.rs");
        ctx.process_ui_lib_rs()?;

        verbose!(
            verbose,
            "processing src/presentation/{}.rs",
            name.to_snake_case()
        );
        process_impl_rs(name)?;

        Ok(())
    }
}

struct Context {
    cwd: PathBuf,
    /// e.g. '~/.cache/struckout_xtask/'
    cache_dir: PathBuf,
    verbose: bool,
    args: NewArgs,
}

impl Context {
    /// Processes `ui/app-window.slint`.
    fn process_app_window_slint(&self) -> anyhow::Result<()> {
        let file = File::open("ui/app-window.slint")
            .with_context(|| "failed to open ui/app-window.slint")?;
        let reader = BufReader::new(file);

        let mut out = self.tempfile()?;

        let upper = self.args.name.to_upper_camel_case();
        let kebab = self.args.name.to_kebab_case();
        for line in reader.lines() {
            let line = line.with_context(|| "reading ui/app-window.slint: failed to read line")?;
            match line.as_str().trim() {
                "//@xtask-import" => {
                    writeln!(
                        out,
                        "import {{ {upper}Screen }} from \"./screens/{kebab}-screen.slint\";",
                    )
                    .with_context(|| "writing tempfile")?;
                }
                "//@xtask-export" => {
                    writeln!(
                        out,
                        "export {{ {upper}Adopter }}from \"./screens/{kebab}-screen.slint\";",
                    )?;
                }
                "//@xtask-enum" => {
                    writeln!(out, "\t{upper}")?;
                }
                "//@xtask-screen" => writeln!(
                    out,
                    "\t\tif nav-route == UiNavRoute.{upper}: {upper}Screen {{ }}",
                )?,
                _ => (),
            }
            writeln!(out, "{}", line)?;
        }
        std::fs::rename(out.path(), "ui/app-window.slint")
            .with_context(|| "failed to move tempfile")?;
        Ok(())
    }

    /// Processes `src/presentation/mod.rs`.
    fn process_presentation_mod_rs(&self) -> anyhow::Result<()> {
        let file = File::open("src/presentation/mod.rs")
            .with_context(|| "failed to open 'src/presentation/mod.rs'")?;
        let reader = BufReader::new(file);
        let mut out = self.tempfile()?;
        for line in reader.lines() {
            let line = line?;
            match line.as_str().trim() {
                "//@xtask-pub-mod" => {
                    writeln!(out, "pub mod {};", self.args.name.to_snake_case())?;
                }
                "//@xtask-register" => {
                    writeln!(
                        out,
                        "\t\t.register({}Destination::new(&application))",
                        self.args.name.to_upper_camel_case()
                    )?;
                }
                _ => (),
            }
            writeln!(out, "{}", line)?;
        }
        std::fs::rename(out.path(), "src/presentation/mod.rs")
            .with_context(|| "failed to move tempfile")?;
        Ok(())
    }

    /// Processes `ui/lib.rs`.
    fn process_ui_lib_rs(&self) -> anyhow::Result<()> {
        let file = File::open("ui/lib.rs").with_context(|| "failed to open ui/lib.rs")?;
        let reader = BufReader::new(file);

        let mut out = self.tempfile()?;

        for line in reader.lines() {
            let line = line.with_context(|| "reading ui/lib.rs: failed to read line")?;
            match line.as_str().trim() {
                "//@xtask-route" => writeln!(out, "\t\t{}", self.args.name.to_upper_camel_case())?,
                _ => (),
            }
            writeln!(out, "{}", line)?;
        }
        std::fs::rename(out.path(), "ui/lib.rs").with_context(|| "failed to move tempfile")?;
        Ok(())
    }

    // Originally copy & pasted from https://github.com/rust-lang/cargo/blob/e6faaba5c4c461c818212756bea8681555019b4d/src/ops/cargo_fix/mod.rs#L223
    fn check_version_control(&self) -> anyhow::Result<()> {
        let args = &self.args;
        verbose!(self.verbose, "checking git status");
        if args.allow_no_vcs {
            return Ok(());
        }
        if !existing_vcs_repo(&self.cwd) {
            bail!(
                "no VCS found for this package and `xtask new` can potentially \
             perform destructive changes; if you'd like to suppress this \
             error pass `--allow-no-vcs`"
            )
        }

        if args.allow_dirty && args.allow_staged {
            return Ok(());
        }

        let mut dirty_files = Vec::new();
        let mut staged_files = Vec::new();
        if let Ok(repo) = git2::Repository::discover(&self.cwd) {
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
                            if !args.allow_staged {
                                staged_files.push(path.to_string())
                            }
                        }
                        _ => {
                            if !args.allow_dirty {
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

    fn tempfile(&self) -> anyhow::Result<NamedTempFile> {
        tempfile::Builder::new()
            .tempfile_in(&self.cache_dir)
            .with_context(|| "failed to create temp file")
    }
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

/// Processes `ui/screens/<name>-screen.slint`.
fn process_screen_slint(name: &str) -> anyhow::Result<()> {
    let fname = format!("ui/screens/{}-screen.slint", name.to_kebab_case());
    let mut file =
        File::create_new(&fname).with_context(|| format!("failed to create {}", fname))?;
    write!(
        file,
        "
global Adopter {{  }}

export component {}Screen {{
    Text {{
        text: \"{}\";
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
use touchpanel_ui::{{{upper}States, {upper}ViewModelTrait, {upper}PropertyMappers, NavRoute, NavRouteKind}};
use stern::nav::NavDestination;

use crate::Application;

touchpanel_ui::define_{snake}_mapper! {{}}

struct {upper}ViewModel {{
    state: {upper}States<Mapper>,
}}

impl {upper}ViewModel {{
    fn new(application: &Application) -> Self {{
        use slint::{{ComponentHandle, Global}};
        Self {{
            state: {upper}States::<Mapper>::new(
                application.ui.global::<touchpanel_ui::{upper}Adopter>().as_weak(),
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
