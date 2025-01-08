use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::{Metadata, Package};

use either::Either;

use crate::{combinations_iter, config::deserialize_config_if_any, pipe::Pipe as _};

use super::{get_packages_iter, Mode, Tags};

pub(crate) struct Arguments {
    pub(super) exact: bool,
    pub(super) external_command: bool,
    pub(super) print_command: bool,
    pub(super) pass_package_manifest: bool,
    pub(super) pass_package_name: bool,
    pub(super) subcommand: OsString,
    pub(super) args: Vec<OsString>,
}

pub(crate) fn subcommand(
    cargo_path: PathBuf,
    metadata: &Metadata,
    current_dir: PathBuf,
    mode: Mode,
    groups: Tags<'_>,
    github_actions_logging: bool,
    Arguments {
        exact,
        external_command,
        print_command,
        pass_package_manifest,
        pass_package_name,
        subcommand,
        args,
    }: Arguments,
) -> Result<()> {
    execute_command_builder(
        if external_command {
            CommandType::ExternalCommand
        } else {
            CommandType::CargoSubcommand { cargo_path }
        },
        print_command,
        pass_package_manifest,
        pass_package_name,
        subcommand,
        args,
    )
    .and_then(|execute_command_builder| {
        get_packages_iter(metadata, current_dir, mode).and_then(|mut packages| {
            packages.try_for_each(move |package| {
                execute_for_package(
                    groups,
                    github_actions_logging,
                    exact,
                    &execute_command_builder,
                    package,
                )
            })
        })
    })
}

fn execute_for_package<'r, ExecuteCommandFunctor>(
    tags: Tags<'_>,
    github_actions_logging: bool,
    exact: bool,
    execute_command_functor: ExecuteCommandFunctor,
    package: &'r Package,
) -> Result<()>
where
    ExecuteCommandFunctor: for<'t> Fn(&'t Path, &'t str, &'t str) -> Result<ExitStatus>,
{
    let maybe_config = deserialize_config_if_any(package)?;

    let mut features_combinations =
        combinations_iter::package_combinations(package, maybe_config.as_ref(), tags)
            .context("Error occurred while constructing combinations!")?;

    let mut features_combinations = if exact {
        let features_combination = features_combinations.next();

        if features_combination.is_some() && features_combinations.next().is_some() {
            bail!(
                r#"More than one combination found for package "{}" when current filters are applied, which contradicts the "exact" flag!"#,
                package.name
            );
        }

        Either::Left(features_combination.into_iter())
    } else {
        Either::Right(features_combinations)
    };

    features_combinations.try_for_each(move |features_combination| {
        execute_for_features_combination(
            &execute_command_functor,
            package,
            &features_combination,
            github_actions_logging,
        )
    })
}

fn execute_for_features_combination<ExecuteCommandFunctor>(
    execute_command_functor: ExecuteCommandFunctor,
    package: &Package,
    features_combination: &str,
    github_actions_logging: bool,
) -> Result<()>
where
    ExecuteCommandFunctor: for<'r> Fn(&'r Path, &'r str, &'r str) -> Result<ExitStatus>,
{
    println!(
        "{}Running for `{}` with the following features: `{features_combination}`",
        if github_actions_logging {
            "##[group]"
        } else {
            ""
        },
        package.name
    );

    let result = execute_command_functor(
        package.manifest_path.as_ref(),
        package.name.as_ref(),
        features_combination,
    )
        .with_context(|| {
            format!(
                "Error occurred while running command for package `{}` with features set `{features_combination}`!",
                package.name
            )
        });

    if github_actions_logging {
        println!("##[endgroup]");
    }

    result.and_then(|exit_status| {
        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Command execution exited with failure for package `{}` with features set `{features_combination}`!",
                package.name
            ))
        }
    })
}

enum CommandType {
    CargoSubcommand { cargo_path: PathBuf },
    ExternalCommand,
}

fn execute_command_builder(
    command_type: CommandType,
    print_command: bool,
    pass_package_manifest: bool,
    pass_package_name: bool,
    subcommand: OsString,
    extra_args: Vec<OsString>,
) -> Result<impl for<'r> Fn(&'r Path, &'r str, &'r str) -> Result<ExitStatus>> {
    resolve_subcommand(&command_type, subcommand)
        .context("Failed to resolve subcommand!")
        .map(|ResolveSubcommandOutput { subcommand }| {
            let build_base = move || match command_type {
                CommandType::CargoSubcommand { ref cargo_path } => Command::new(cargo_path)
                    .pipe_mut(|command| _ = command.arg(subcommand.as_os_str())),
                CommandType::ExternalCommand => Command::new(subcommand.as_os_str()),
            };

            move |manifest_path: &Path, package: &str, features: &str| {
                directory_from_manifest_path(manifest_path).and_then(|working_directory: &Path| {
                    execute_command(
                        build_base().pipe_mut(|command| _ = command.current_dir(working_directory)),
                        print_command,
                        pass_package_manifest.then_some(manifest_path),
                        pass_package_name.then_some(package),
                        &extra_args,
                        features,
                    )
                })
            }
        })
}

fn execute_command(
    mut command: Command,
    print_command: bool,
    package_manifest: Option<&Path>,
    package_name: Option<&str>,
    extra_args: &[OsString],
    features: &str,
) -> Result<ExitStatus> {
    command
        .args(extra_args)
        .pipe_if_some(package_manifest, |command, package_manifest| {
            command.arg("--manifest-path").arg(package_manifest)
        })
        .pipe_if_some(package_name, |command, package_name| {
            command.args(["--package", package_name])
        })
        .args(["--features", features])
        .pipe_if(print_command, |command| {
            print!("\t");

            if let Some(path) = command.get_current_dir() {
                print!("{path:?} ");
            }

            print!(">>> {:?}", command.get_program());

            command.get_args().for_each(|arg| print!(" {:?}", arg));

            println!();

            command
        })
        .status()
        .context("Failed to execute command!")
}

fn directory_from_manifest_path(manifest_path: &Path) -> Result<&Path> {
    manifest_path
        .parent()
        .and_then(|manifest_path| manifest_path.is_dir().then_some(manifest_path))
        .ok_or_else(
            #[cold]
                || anyhow!("Failed to build command!\n Package's manifest path doesn't have a parent directory!"),
        )
}

struct ResolveSubcommandOutput {
    subcommand: OsString,
}

fn resolve_subcommand(
    command_type: &CommandType,
    mut subcommand: OsString,
) -> Result<ResolveSubcommandOutput> {
    if matches!(command_type, CommandType::ExternalCommand) {
        let subcommand_as_path = Path::new(&subcommand);

        if subcommand_as_path
            .parent()
            .map_or(false, |parent| !parent.as_os_str().is_empty())
        {
            subcommand = Path::new(&subcommand)
                .canonicalize()
                .context("Failed to canonicalize external command's path!")?
                .into_os_string();
        }
    };

    Ok(ResolveSubcommandOutput { subcommand })
}
