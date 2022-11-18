use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};

mod updater;

fn main() -> Result<()> {
    let app = Command::new("cargo")
        .bin_name("cargo")
        .subcommand(
            Command::new("updater")
                .version(env!("CARGO_PKG_VERSION"))
                .about(env!("CARGO_PKG_DESCRIPTION"))
                .args(&[
                    Arg::new("update")
                        .short('u')
                        .long("update")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("list")
                        .help("Update upgradable crates"),
                    Arg::new("list")
                        .short('l')
                        .long("list")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("update")
                        .help("List latest available version"),
                    Arg::new("locked")
                        .short('L')
                        .long("locked")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("list")
                        .requires("update")
                        .help("When updating upgradable crates, use their Cargo.lock if packaged"),
                ])
                .arg_required_else_help(true),
        )
        .get_matches();

    match app.subcommand() {
        Some(("updater", cmd)) => {
            if let Some(list) = cmd.get_one::<bool>("list") {
                if *list {
                    updater::CratesInfoContainer::list()
                        .context("Unable to list installed binaries")?;
                }
            }

            if let Some(update) = cmd.get_one::<bool>("update") {
                if *update {
                    let use_locked = cmd.get_one::<bool>("locked").unwrap_or(&false);

                    updater::CratesInfoContainer::update(use_locked).context("Unable to update")?;
                }
            }
        }
        _ => {
            unreachable!()
        }
    }

    Ok(())
}
