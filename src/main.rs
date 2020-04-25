mod cargo;
mod cli;

#[tokio::main]
async fn main() {
    let app = cli::new().get_matches();

    let cmd = match app.subcommand {
        None => {
            let _ = cli::new().print_long_help();
            std::process::exit(1);
        }
        Some(sub_cmd) => sub_cmd.matches,
    };

    // if we have more that two flags, we need to change this
    if cmd.is_present("list") || !cmd.is_present("list") && !cmd.is_present("update") {
        let container = cargo::get_upgradable_crates().await;
        cargo::pretty_print_stats(container);
    }

    if cmd.is_present("update") {
        cargo::update_upgradable_crates().await
    }
}
