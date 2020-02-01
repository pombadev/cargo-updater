mod helpers;

#[tokio::main]
async fn main() {
    let crates_info = helpers::CratesInfoContainer::new();

    let crates_info = crates_info.check_for_updates().await;

    crates_info.render_info_as_table();
}
