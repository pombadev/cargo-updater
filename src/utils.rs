/* brings traits in to scope */
use colored::Colorize;
use futures::stream::StreamExt;

use semver;

pub(crate) type CratesNameAndVersion = Vec<(String, String, String, bool)>;

//#[derive(Debug, Clone, Copy)]
//pub(crate) struct Crate<'a> {
//    pub(crate) name: &'a str,
//    pub(crate) current_version: &'a str,
//    pub(crate) max_version: &'a str,
//}
//
//impl <'a> Crate<'a> {
//    pub(crate) fn new(name: &'a str, curr: &'a str, max: &'a str) -> Self {
//        Self {
//            name,
//            current_version: curr,
//            max_version: max
//        }
//    }
//
//    pub(crate)  fn is_upgradable(&self) -> bool {
//        let max = semver::Version::parse(self.max_version).unwrap();
//        let curr = semver::Version::parse(self.current_version).unwrap();
//
//        curr < max
//    }
//
//    pub(crate) fn has_max_version(&self) -> bool {
//        !self.max_version.is_empty()
//    }
//}
//
//pub(crate) type Crates<'a> = Vec<Crate<'a>>;

pub(crate) fn render_table_to_std(c: &CratesNameAndVersion) {
    use term_table::{
        row::Row,
        table_cell::{Alignment, TableCell},
        Table, TableStyle,
    };

    let mut table = Table::new();

    table.style = TableStyle::blank();

    table.separate_rows = false;

    table.add_row(Row::new(vec![
        TableCell::new_with_alignment("Crate".bold().underline(), 1, Alignment::Left),
        TableCell::new_with_alignment("Current".bold().underline(), 1, Alignment::Center),
        TableCell::new_with_alignment("Latest".bold().underline(), 1, Alignment::Center),
        // TableCell::new_with_alignment("Upgradable".bold().underline(), 1, Alignment::Right),
    ]));

    for item in c {
        let crate_name = if item.3 {
            item.0.as_str().red()
        } else {
            item.0.as_str().yellow()
        };

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment(crate_name, 1, Alignment::Left),
            TableCell::new_with_alignment(item.1.as_str(), 1, Alignment::Center),
            TableCell::new_with_alignment(item.2.as_str().magenta(), 1, Alignment::Center),
            // TableCell::new_with_alignment(item.3, 1, Alignment::Center),
        ]))
    }

    println!("{}", table.render());
}

/// get_crates_name_version
pub(crate) fn get_crates_name_version() -> Result<CratesNameAndVersion, Box<dyn std::error::Error>> {
    use regex::Regex;
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

            (
               name,
               version,
               "".to_string(),
               false
           )
        })
        .collect::<CratesNameAndVersion>();

    Ok(crates_name_info)
}

pub(crate) async fn update_all_crates() -> Result<(), Box<dyn std::error::Error>> {
   use tokio::process::Command;

   let crates = check_for_updates(&get_crates_name_version()?).await;

   for item in crates {
       if item.3 == true {
           let _ = Command::new("cargo")
               .args(&["install", "--force", format!("{}", item.0).as_str().trim()])
               .spawn()
               .expect("Failed to spawn `cargo` command.")
               .await;
       }
   }

   /*
   let upgradable = crates
       .iter()
       .filter(|item| item.3)
       .map(|item| item.clone())
       .collect::<CratesNameAndVersion>();

   futures::stream::iter(upgradable.iter().map(|item| {
       println!("updating item: {} ", item.0);

       async move {
           let _ = Command::new("cargo")
               .args(&["install", "--force", format!("{}", item.0).as_str().trim()])
               .spawn()
               .expect("Failed to spawn `cargo` command.")
               .await;
       }
   })).buffered(upgradable.len())
       .collect::<()>().await;
    */

   Ok(())
}

pub(crate) async fn check_for_updates(crate_info: &CratesNameAndVersion) -> CratesNameAndVersion {
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    // use `join` instead of stream?
    // https://docs.rs/futures/0.3.1/futures/macro.join.html

    #[derive(Serialize, Deserialize)]
    struct CrateJson {
        max_version: String
    }

    #[derive(Serialize, Deserialize)]
    struct CrateContainer {
        #[serde(rename = "crate")]
        crate_name: CrateJson
    }

    let tasks = futures::stream::iter(crate_info.iter().map(|item| {
        async move {
            let client = Client::builder().user_agent("Mozilla/5.0").build().unwrap();

            let response: CrateContainer = client
                .get(format!("https://crates.io/api/v1/crates/{}", item.0).as_str())
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();

            let max = response.crate_name.max_version;

            (
                item.0.clone(),
                item.1.clone(),
                max.clone(),
                semver::Version::parse(max.clone().as_str()).unwrap() > semver::Version::parse(
                    item.1.as_str()
                ).unwrap()
            )
        }
    }))
    .buffer_unordered(crate_info.len());

    let mut data = tasks.collect::<CratesNameAndVersion>().await;

   data.sort();

    data
}

#[allow(dead_code)]
pub(crate) fn progress_demo() {
    use pbr::ProgressBar;
    use std::thread;
    use std::time::Duration;

    let items = get_crates_name_version().unwrap();

    let mut pb = ProgressBar::new(items.len() as u64);

    pb.set_max_refresh_rate(Some(Duration::from_secs(1)));

    pb.show_tick = true;
    pb.show_speed = false;
    pb.show_percent = true;
    pb.show_counter = true;
    pb.show_time_left = true;

    for item in items {
        pb.message(format!("Downloading: {}", item.0).as_str());
        pb.inc();
        thread::sleep(Duration::from_millis(100));
    }

    pb.finish_print("Downloaded!");
}
