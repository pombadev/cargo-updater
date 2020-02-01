use colored::Colorize;
use futures::{stream, StreamExt};
use miniserde::{json, Deserialize, Serialize};
use regex::Regex;
use reqwest::{Client, header::USER_AGENT};
use semver;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

#[derive(Debug)]
pub(crate) struct CrateInfo {
    pub(crate) name: String,
    pub(crate) current_version: String,
    pub(crate) max_version: String,
}

impl CrateInfo {
    pub(crate) fn is_upgradable(&self) -> bool {
        let max = semver::Version::parse(self.max_version.as_str()).unwrap();
        let curr = semver::Version::parse(self.current_version.as_str()).unwrap();

        curr < max
    }
}

#[derive(Debug)]
pub(crate) struct CratesInfoContainer {
    crates: Vec<CrateInfo>,
}

impl CratesInfoContainer {
    pub(crate) fn new() -> Self {
        Self::get_info_from_stdio().unwrap()
    }

    pub(crate) fn get_info_from_stdio() -> Result<CratesInfoContainer, Box<dyn std::error::Error>> {
        // spits output to stdio that looks like this:
        // cargo v0.38.0:
        //     cargo
        let output = std::process::Command::new("cargo")
            .args(&["install", "--list"])
            .output()?;

        // matches pattern: some-crate v0.0.1: from the output.
        let re = Regex::new(r"\S+.\sv\d+.*:")?;
        // matches pattern: `some-crate ` from the output.
        let name_re = Regex::new(r"\S+.\s")?;
        // matches pattern: `v0.0.1` from the output.
        let version_re = Regex::new(r"v\d.+\d")?;
        // matches any pattern that starts with: `v`
        // we could use `.starts_with` but we need this to strip `v` later.
        let v_prefix = Regex::new(r"^v")?;

        let crates_name_info = re
            .captures_iter(String::from_utf8(output.stdout)?.as_str())
            .map(|item| {
                // extract first line only as it's the only thing we are interested in.
                let line = item[0].to_string();

                let name_capture = name_re.captures(line.as_str()).unwrap();
                let name = name_capture[0].to_string();

                let version_capture = version_re.captures(line.as_str()).unwrap();
                let version = v_prefix.replace(&version_capture[0], "").to_string();

                CrateInfo {
                    name,
                    current_version: version,
                    max_version: "".to_string(),
                }
            })
            .collect::<Vec<CrateInfo>>();

        Ok(CratesInfoContainer {
            crates: crates_name_info,
        })
    }

    pub(crate) async fn check_for_updates(mut self) -> CratesInfoContainer {
        #[derive(Serialize, Deserialize, Debug)]
        struct MaxVersion {
            max_version: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        pub struct InfoJson {
            #[serde(rename = "crate")]
            crate_name: MaxVersion,
        }

        let limit = self.crates.len();

        let tasks =
            stream::iter(self.crates.iter_mut()).for_each_concurrent(limit, |item| async move {
                let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

                let response = client
                    .get(format!("https://crates.io/api/v1/crates/{}", item.name).as_str())
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap();

                let response: InfoJson = json::from_str(response.as_str()).unwrap();

                item.max_version = response.crate_name.max_version;
            });

        tasks.await;

        self
    }

    pub(crate) fn render_info_as_table(self) {
        let mut table = Table::new();

        table.style = TableStyle::blank();

        table.separate_rows = false;

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment("Crate".bold().underline(), 1, Alignment::Left),
            TableCell::new_with_alignment("Current".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Latest".bold().underline(), 1, Alignment::Center),
        ]));

        for item in self.crates {
            let crate_name = if item.is_upgradable() {
                item.name.as_str().red()
            } else {
                item.name.as_str().yellow()
            };

            table.add_row(Row::new(vec![
                TableCell::new_with_alignment(crate_name, 1, Alignment::Left),
                TableCell::new_with_alignment(item.current_version.as_str(), 1, Alignment::Center),
                TableCell::new_with_alignment(
                    item.max_version.as_str().magenta(),
                    1,
                    Alignment::Center,
                ),
            ]))
        }

        print!("{}", table.render());
    }
}

// pub(crate) async fn update_all_crates() -> Result<(), Box<dyn std::error::Error>> {
//     use tokio::process::Command;

//     let crates = check_for_updates(&get_crates_name_version()?).await;

//     for item in crates {
//         if item.3 == true {
//             let _ = Command::new("cargo")
//                 .args(&["install", "--force", format!("{}", item.0).as_str().trim()])
//                 .spawn()
//                 .expect("Failed to spawn `cargo` command.")
//                 .await;
//         }
//     }

//     /*
//     let upgradable = crates
//         .iter()
//         .filter(|item| item.3)
//         .map(|item| item.clone())
//         .collect::<CratesNameAndVersion>();

//     futures::stream::iter(upgradable.iter().map(|item| {
//         println!("updating item: {} ", item.0);

//         async move {
//             let _ = Command::new("cargo")
//                 .args(&["install", "--force", format!("{}", item.0).as_str().trim()])
//                 .spawn()
//                 .expect("Failed to spawn `cargo` command.")
//                 .await;
//         }
//     })).buffered(upgradable.len())
//         .collect::<()>().await;
//      */
//     Ok(())
// }

// #[allow(dead_code)]
// pub(crate) fn progress_demo() {
//     use pbr::ProgressBar;
//     use std::thread;
//     use std::time::Duration;

//     let items = get_crates_name_version().unwrap();

//     let mut pb = ProgressBar::new(items.len() as u64);

//     pb.set_max_refresh_rate(Some(Duration::from_secs(1)));

//     pb.show_tick = true;
//     pb.show_speed = false;
//     pb.show_percent = true;
//     pb.show_counter = true;
//     pb.show_time_left = true;

//     for item in items {
//         pb.message(format!("Downloading: {}", item.0).as_str());
//         pb.inc();
//         thread::sleep(Duration::from_millis(100));
//     }

//     pb.finish_print("Downloaded!");
// }
