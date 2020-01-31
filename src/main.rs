mod utils;

mod cli;

#[tokio::main]
async fn main() {
    let app = cli::new().get_matches();

    if app.is_present("check") {
        println!("check!");
        std::process::exit(0);
    }

    if app.is_present("update") {
        println!("update!");
        std::process::exit(0);
    }

    let crates = utils::get_crates_name_version().unwrap();
    let crates = utils::check_for_updates(&crates).await;

    utils::render_table_to_std(&crates);

    // utils::progress_demo();
}
