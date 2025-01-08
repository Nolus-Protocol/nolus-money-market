use std::{collections::BTreeSet, path::PathBuf};

use anyhow::{anyhow, Result};
use cargo_metadata::{Metadata, Package};

use either::Either;

pub(crate) use self::{
    args::{Arguments, Parser, SubcommandArguments},
    list::subcommand as list_subcommand,
    run::subcommand as run_subcommand,
};

mod args;
mod list;
mod run;

pub(crate) type Tags<'r> = Option<&'r BTreeSet<&'r str>>;

pub(crate) enum Mode {
    Subdirectories,
    Workspace,
    Package(String),
}

fn get_packages_iter(
    metadata: &Metadata,
    current_dir: PathBuf,
    mode: Mode,
) -> Result<impl Iterator<Item = &Package>> {
    match mode {
        Mode::Subdirectories => Ok(Either::Left(filtered_workspace_members(
            metadata,
            current_dir,
        ))),
        Mode::Workspace => Ok(Either::Right(Either::Left(
            metadata.workspace_packages().into_iter(),
        ))),
        Mode::Package(package) => find_package_by_name(metadata, &package)
            .map(|package| Either::Right(Either::Right(Some(package).into_iter()))),
    }
}

fn find_package_by_name<'r>(metadata: &'r Metadata, package_name: &str) -> Result<&'r Package> {
    metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package.name == package_name)
        .ok_or_else(move || {
            anyhow!(r#"No package named "{package_name}" exists within the workspace members!"#)
        })
}

fn filtered_workspace_members(
    metadata: &Metadata,
    current_dir: PathBuf,
) -> impl Iterator<Item = &Package> {
    metadata
        .workspace_packages()
        .into_iter()
        .filter(move |package| package.manifest_path.starts_with(&current_dir))
}
