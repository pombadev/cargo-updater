use anyhow::{Context, Result};
use clap::{App, Arg, Command};

mod updater;

fn main() -> Result<()> {
    let app = App::new(env!("CARGO_PKG_NAME"))
        .bin_name("cargo")
        .subcommand(
            Command::new("updater")
                .version(env!("CARGO_PKG_VERSION"))
                .about(env!("CARGO_PKG_DESCRIPTION"))
                .args(&[
                    Arg::from_usage("-u --update 'Update upgradable crates'")
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with("list"),
                    Arg::from_usage("-l --list 'List latest available version'")
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with("update"),
                ])
                .arg_required_else_help(true),
        )
        .get_matches();

    if let Some(("updater", cmd)) = app.subcommand() {
        if let Some(list) = cmd.get_one::<bool>("list") {
            if *list {
                updater::CratesInfoContainer::list()
                    .context("Unable to list installed binaries.")?;
            }
        }

        if let Some(update) = cmd.get_one::<bool>("update") {
            if *update {
                updater::CratesInfoContainer::update().context("Unable to run updater.")?;
            }
        }
    }

    Ok(())
}
