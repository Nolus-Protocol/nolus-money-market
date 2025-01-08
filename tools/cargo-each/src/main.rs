use std::{collections::BTreeSet, env::current_dir, path::PathBuf};

use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand};
use clap::Parser as _;

use crate::subcommands::{Arguments, Parser, SubcommandArguments};

use self::{pipe::Pipe as _, subcommands::Tags};

mod check;
mod combinations_iter;
mod config;
mod iter_or_else_iter;
mod pipe;
mod subcommands;

fn main() -> Result<()> {
    let arguments: Parser = Parser::parse();

    let Arguments {
        cargo_path,
        manifest_path,
        mode,
        tags: groups,
        github_actions_logging,
        subcommand: subcommand_args,
    } = arguments.process();

    let metadata = get_metadata(cargo_path.clone(), manifest_path)
        .context("Error occurred while fetching the workspaces' metadata!")?;

    let groups: BTreeSet<&str> = groups.iter().map(String::as_str).collect();

    let groups: Tags<'_> = (!groups.is_empty()).then_some(&groups);

    let current_dir = current_dir().context("Error occurred while resolving current directory!")?;

    match subcommand_args {
        SubcommandArguments::Run(arguments) => subcommands::run_subcommand(
            cargo_path,
            &metadata,
            current_dir,
            mode,
            groups,
            github_actions_logging,
            arguments,
        ),
        SubcommandArguments::List(arguments) => subcommands::list_subcommand(
            &metadata,
            current_dir,
            mode,
            groups,
            github_actions_logging,
            arguments,
        ),
    }
}

fn get_metadata(cargo_path: PathBuf, manifest_path: Option<PathBuf>) -> Result<Metadata> {
    MetadataCommand::new()
        .cargo_path(cargo_path)
        .pipe_if_some(manifest_path, MetadataCommand::manifest_path)
        .exec()
        .context("Executing `cargo metadata` failed!")
}
