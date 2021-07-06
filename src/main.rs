use anyhow::{Context, Result};

mod cli;
mod core;

fn main() -> Result<()> {
    let app = cli::new().get_matches();

    let cmd = match app.subcommand {
        None => {
            let _ = cli::new().print_long_help();
            std::process::exit(1);
        }
        Some(sub_cmd) => sub_cmd.matches,
    };

    let container = core::CratesInfoContainer::new()?;

    if cmd.is_present("list") || cmd.args.is_empty() {
        container
            .list()
            .context("Unable to list installed binaries.")?;
    }

    if cmd.is_present("update") {
        container.update().context("Unable to run updater.")?;
    }

    Ok(())
}
