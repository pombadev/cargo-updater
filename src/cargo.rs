use colored::Colorize;
use futures::{stream, StreamExt};
use miniserde::{json, Deserialize, Serialize};
use regex::Regex;
use reqwest::{header::USER_AGENT, Client};
use semver::Version;
use std::process::Command;

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
        let max = match Version::parse(self.max_version.as_str()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e.to_string());
                std::process::exit(1);
            }
        };

        let curr = match Version::parse(self.current_version.as_str()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e.to_string());
                std::process::exit(1);
            }
        };

        curr < max
    }
}

#[derive(Debug)]
pub(crate) struct CratesInfoContainer {
    crates: Vec<CrateInfo>,
}

impl CratesInfoContainer {
    pub(crate) fn maybe_new() -> Self {
        Self::parse_from_stdio().expect("Unable to parse installed version from stdio.")
    }

    pub(crate) fn parse_from_stdio() -> Result<CratesInfoContainer, Box<dyn std::error::Error>> {
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
        #[allow(clippy::trivial_regex)]
        let v_prefix = Regex::new(r"^v")?;

        let crates_name_info = re
            .captures_iter(String::from_utf8(output.stdout)?.as_str())
            .map(|item| {
                // extract first line only as it's the only thing we are interested in.
                let line = item[0].to_string();

                let name_capture = name_re
                    .captures(line.as_str())
                    .expect("Unable to capture regex by name.");
                let name = name_capture[0].trim().to_string();

                let version_capture = version_re
                    .captures(line.as_str())
                    .expect("Unable to capture regex by version.");
                let version = v_prefix.replace(&version_capture[0], "").trim().to_string();

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
}

#[derive(Serialize, Deserialize, Debug)]
struct MaxVersion {
    max_version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InfoJson {
    #[serde(rename = "crate")]
    crate_name: MaxVersion,
}

pub(crate) async fn update_upgradable_crates() {
    let container = get_upgradable_crates().await;

    let crates: Vec<String> = container
        .crates
        .iter()
        .filter(|item| item.is_upgradable())
        .map(|item| item.name.clone())
        .collect();

    if crates.len() == 0 {
        return println!(
            "Nothing to update, run `cargo updater --list` to view installed version and latest version."
        );
    }

    let mut cmd = Command::new("cargo");

    let cmd = cmd.args(&["install", "--force"]).args(&crates);

    let mut child = cmd
        .spawn()
        .expect(format!("`cargo install --force {:?}` failed to start", &crates).as_str());

    let status = child.wait().expect("failed to wait process status.");

    if !status.success() {
        match status.code() {
            Some(code) => println!("Exited with status code: {}", code),
            None => eprintln!("Unknown error"),
        }
    }
}

pub(crate) async fn get_upgradable_crates() -> CratesInfoContainer {
    let mut container = CratesInfoContainer::maybe_new();

    let limit = container.crates.len();

    let tasks =
        stream::iter(container.crates.iter_mut()).for_each_concurrent(limit, |item| async move {
            let client = Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("Unable to build `reqwest` client.");

            let response = client
                .get(format!("https://crates.io/api/v1/crates/{}", item.name).as_str())
                .send()
                .await
                .expect("Unable to `send` request.")
                .text()
                .await
                .expect("Unable to parse response to text.");

            let response: InfoJson =
                json::from_str(response.as_str()).expect("Unable to parse response to json.");

            item.max_version = response.crate_name.max_version;
        });

    tasks.await;

    container
}

pub(crate) fn pretty_print_stats(container: CratesInfoContainer) {
    let mut table = Table::new();

    table.style = TableStyle::blank();

    table.separate_rows = false;

    table.add_row(Row::new(vec![
        TableCell::new_with_alignment("Crate".bold().underline(), 1, Alignment::Left),
        TableCell::new_with_alignment("Current".bold().underline(), 1, Alignment::Center),
        TableCell::new_with_alignment("Latest".bold().underline(), 1, Alignment::Center),
    ]));

    for item in container.crates {
        let (name, max) = if item.is_upgradable() {
            (
                item.name.as_str().bright_yellow(),
                item.max_version.as_str().bright_yellow(),
            )
        } else {
            (
                item.name.as_str().green(),
                item.max_version.as_str().green(),
            )
        };

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment(name, 1, Alignment::Left),
            TableCell::new_with_alignment(item.current_version.as_str(), 1, Alignment::Center),
            TableCell::new_with_alignment(max, 1, Alignment::Center),
        ]))
    }

    print!("{}", table.render());
}
