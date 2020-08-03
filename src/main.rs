use anyhow::{Result, Context};

mod cli;
mod ops;

fn main() -> Result<()> {
    let app = cli::new().get_matches();

    let cmd = match app.subcommand {
        None => {
            let _ = cli::new().print_long_help();
            std::process::exit(1);
        }
        Some(sub_cmd) => sub_cmd.matches,
    };

    let container = ops::CratesInfoContainer::new()?;

    // if we have more that two flags, we need to change this
    if cmd.is_present("list") || !cmd.is_present("list") && !cmd.is_present("update") {
        let _ = container.pretty_print_stats().context("Unable to list installed binaries.");
    }

    if cmd.is_present("update") {
        let _ = container.update_upgradable().context("Unable to run updater.");
    }

    Ok(())
}
