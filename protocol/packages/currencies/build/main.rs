use std::{
    env,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context as _, Result};
use either::Either;

use topology::{CurrencyDefinitions, Topology};

use self::{currencies_tree::CurrenciesTree, protocol::Protocol};

mod convert_case;
mod currencies_tree;
mod protocol;
mod sources;
mod subtype_lifetime;
mod swap_pairs;

const PROTOCOL_JSON: &str = "../../../build-configuration/protocol.json";

const TOPOLOGY_JSON: &str = "../../../build-configuration/topology.json";

const BUILD_REPORT: &str = "CURRENCIES_BUILD_REPORT";

fn main() -> Result<()> {
    for path in ["build/", PROTOCOL_JSON, TOPOLOGY_JSON] {
        println!("cargo::rerun-if-changed={path}");
    }

    println!("cargo::rerun-if-env-changed={BUILD_REPORT}");

    if env::var_os("CARGO_FEATURE_TESTING").is_some() {
        Ok(())
    } else if check_for_definitions()? {
        let build_report = build_report_writer()?;

        let output_directory: &Path = &PathBuf::from(
            env::var_os("OUT_DIR").context("Cargo did not set `OUT_DIR` environment variable!")?,
        );

        let topology = File::open(TOPOLOGY_JSON)
            .context(r#"Failed to open "topology.json"!"#)
            .and_then(|file| {
                serde_json::from_reader(file).context("Failed to parse topology JSON!")
            })?;

        let protocol = File::open(PROTOCOL_JSON)
            .context(r#"Failed to open "protocol.json"!"#)
            .and_then(|file| {
                serde_json::from_reader(file).context("Failed to parse protocol JSON!")
            })?;

        generate_currencies(build_report, output_directory, topology, protocol)
    } else {
        Err(anyhow!(
            "Topology and protocol definitions don't exist while `testing` \
            feature is not selected!"
        ))
    }
}

fn check_for_definitions() -> Result<bool> {
    IntoIterator::into_iter([PROTOCOL_JSON, TOPOLOGY_JSON])
        .map(Path::new)
        .map(Path::try_exists)
        .try_fold(true, |all_exist, result| {
            result
                .map(|exists| all_exist && exists)
                .context("Failed to check whether JSON descriptor file exists!")
        })
}

fn build_report_writer() -> Result<impl Write> {
    if let Some(report_file) = env::var_os(BUILD_REPORT) {
        File::create(report_file)
            .context("Failed to open build report for writing!")
            .map(Either::Left)
    } else {
        Ok(Either::Right(io::stderr()))
    }
}

fn generate_currencies<BuildReport>(
    build_report: BuildReport,
    output_directory: &Path,
    topology: Topology,
    protocol: Protocol,
) -> Result<()>
where
    BuildReport: Write,
{
    let CurrencyDefinitions {
        host_currency,
        dex_currencies,
    } = topology.currency_definitions(&protocol.dex_network)?;

    if protocol.lpn_ticker == host_currency.ticker() {
        bail!(
            "Liquidity provider's currency cannot be the same as the host \
                network's native currency!",
        );
    }

    if protocol.stable_currency_ticker == host_currency.ticker() {
        bail!(
            "Stable currency cannot be the same as the host network's native \
                currency!",
        );
    }

    sources::write(
        build_report,
        output_directory,
        &protocol,
        &host_currency,
        &protocol.dex_currencies(&host_currency, &dex_currencies),
        &CurrenciesTree::new(&protocol, &host_currency)?,
    )
}
