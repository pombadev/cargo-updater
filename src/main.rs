mod cargo;
mod cli;

#[tokio::main]
async fn main() {
    let app = cli::new().get_matches();

    let cmd = match app.subcommand {
        None => {
            eprintln!("Unexpected error occurred in subcommand.");
            std::process::exit(1);
        }
        Some(sub_cmd) => sub_cmd.matches,
    };

    if cmd.is_present("check") {
        let container = cargo::check_for_updates().await;
        return cargo::pretty_print_stats(container);
    }

    if (!cmd.is_present("check") && !cmd.is_present("update")) || cmd.is_present("update") {
        return cargo::update_upgradable_crates().await;
    }
}
