use anyhow::{Context, Result};
use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};

mod updater;

fn main() -> Result<()> {
    let app = App::new(crate_name!())
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("updater")
                .setting(AppSettings::ColorAuto)
                .version(crate_version!())
                .about(crate_description!())
                .args(&[
                    Arg::from_usage("-u --update 'Update upgradable crates'")
                        .conflicts_with("list"),
                    Arg::from_usage("-l --list 'List latest available version'")
                        .conflicts_with("update"),
                ]),
        )
        .setting(AppSettings::ArgRequiredElseHelp)
        .get_matches();

    if let Some(cmd) = app.subcommand {
        let cmd = cmd.matches;

        let container = updater::CratesInfoContainer::new()?;

        if cmd.is_present("list") || cmd.args.is_empty() {
            container
                .list()
                .context("Unable to list installed binaries.")?;
        }

        if cmd.is_present("update") {
            container.update().context("Unable to run updater.")?;
        }
    }

    Ok(())
}
