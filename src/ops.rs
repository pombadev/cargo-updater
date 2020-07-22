use anyhow::{Context, Result};
use colored::Colorize;
use futures::{stream, StreamExt};
use miniserde::{json, Deserialize, Serialize};
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
    pub(crate) current: String,
    pub(crate) online: String,
}

impl CrateInfo {
    pub(crate) fn is_upgradable(&self) -> bool {
        let max = match Version::parse(self.online.as_str()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e.to_string());
                std::process::exit(1);
            }
        };

        let curr = match Version::parse(self.current.as_str()) {
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
    pub(crate) fn new() -> Result<Self> {
        Self::parse().context("Unable to parse installed version from stdio.")
    }

    pub(crate) fn parse() -> Result<CratesInfoContainer> {
        let output = Command::new("cargo")
            .args(&["install", "--list"])
            .output()?;

        let crates = std::str::from_utf8(&output.stdout[..])?
            .lines()
            .filter(|line| {
                // https://github.com/rust-lang/cargo/blob/f84f3f8c630c75a1ec01b818ff469d3496228c6b/src/cargo/ops/cargo_install.rs#L687
                !line.starts_with("    ")
            })
            .map(|line| {
                // https://github.com/rust-lang/cargo/blob/f84f3f8c630c75a1ec01b818ff469d3496228c6b/src/cargo/ops/cargo_install.rs#L689
                let m = line.trim_end_matches(|c| c == ':');
                let mut m = m.split(" v");
                let name = m.next().unwrap_or("");
                let version = m.next().unwrap_or("");

                CrateInfo {
                    name: name.into(),
                    current: version.into(),
                    online: String::new(),
                }
            })
            .collect::<Vec<CrateInfo>>();

        Ok(CratesInfoContainer { crates })
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

pub(crate) async fn update_upgradable_crates() -> Result<()> {
    let container = get_upgradable_crates().await?;

    let crates: Vec<String> = container
        .crates
        .iter()
        .filter(|item| item.is_upgradable())
        .map(|item| item.name.clone())
        .collect();

    if crates.len() == 0 {
        println!(
            "Nothing to update, run `cargo updater --list` to view installed version and available version."
        );

        return Ok(());
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
        };
    }

    Ok(())
}

pub(crate) async fn get_upgradable_crates() -> Result<CratesInfoContainer> {
    let mut container = CratesInfoContainer::new()?;

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

            item.online = response.crate_name.max_version;
        });

    tasks.await;

    Ok(container)
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
                item.online.as_str().bright_yellow(),
            )
        } else {
            (item.name.as_str().green(), item.online.as_str().green())
        };

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment(name, 1, Alignment::Left),
            TableCell::new_with_alignment(item.current.as_str(), 1, Alignment::Center),
            TableCell::new_with_alignment(max, 1, Alignment::Center),
        ]))
    }

    print!("{}", table.render());
}
