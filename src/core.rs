use std::{
    process::{self, Command},
    sync::mpsc::channel,
    thread,
};

use anyhow::{Context, Result};
use colored::Colorize;
use semver::Version;
use serde::{Deserialize, Serialize};
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
    pub(crate) fn is_upgradable(&self) -> Result<bool> {
        let max = Version::parse(self.online.as_str())?;

        let curr = Version::parse(self.current.as_str())?;

        Ok(curr < max)
    }
}

#[derive(Debug)]
pub(crate) struct CratesInfoContainer {
    pub(crate) crates: Vec<CrateInfo>,
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
                let line = line.trim_end_matches(|c| c == ':');
                let mut name_version = line.split(" ");
                let name = name_version.next().unwrap_or("");
                let version = name_version.next().unwrap_or("");

                let version = if version.starts_with('v') {
                    version.strip_prefix('v').unwrap_or("")
                } else {
                    version
                };

                CrateInfo {
                    name: name.into(),
                    current: version.into(),
                    online: String::new(),
                }
            })
            .collect::<Vec<CrateInfo>>();

        Ok(CratesInfoContainer { crates })
    }

    pub(crate) fn get_upgradable(&self) -> Result<Self> {
        #[derive(Serialize, Deserialize, Debug)]
        struct MaxVersion {
            newest_version: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        pub struct InfoJson {
            #[serde(rename = "crate")]
            crate_name: MaxVersion,
        }
        let (tx, rx) = channel();

        for item in Self::new()?.crates {
            let tx = tx.clone();

            thread::spawn(move || -> Result<()> {
                let response =
                    attohttpc::get(format!("https://crates.io/api/v1/crates/{}", item.name))
                        .send()?;

                let response = response.json::<InfoJson>()?;

                tx.send(CrateInfo {
                    name: item.name,
                    current: item.current,
                    online: response.crate_name.newest_version,
                })?;

                Ok(())
            });
        }

        drop(tx); // let know that loop is done.

        let response = rx.iter().collect::<Vec<CrateInfo>>();

        Ok(Self { crates: response })
    }

    pub(crate) fn update_upgradable(&self) -> Result<()> {
        let container = self.get_upgradable()?;

        let crates: Vec<String> = container
            .crates
            .iter()
            .filter(|item| match item.is_upgradable() {
                Ok(res) => res,
                Err(_) => false,
            })
            .map(|item| item.name.clone())
            .collect();

        if crates.is_empty() {
            println!(
                "Nothing to update, run `cargo updater --list` to view installed and available version."
            );

            return Ok(());
        }

        let mut cmd = Command::new("cargo");

        let cmd = cmd.args(&["install", "--force"]).args(&crates);

        let mut child = cmd.spawn().unwrap_or_else(|_| {
            eprintln!("`cargo install --force {:?}` failed to start", &crates);
            process::exit(1);
        });

        let status = child.wait().unwrap_or_else(|_| {
            eprintln!("failed to wait process status.");
            process::exit(1);
        });

        if !status.success() {
            match status.code() {
                Some(code) => {
                    eprintln!("Exited with status code: {}", code);
                    process::exit(code);
                }
                None => {
                    eprintln!("Running `cargo install` was not successful.");
                    process::exit(1);
                }
            };
        }

        Ok(())
    }

    pub(crate) fn pretty_print(&self) -> Result<()> {
        let mut table = Table::new();

        table.style = TableStyle::blank();

        table.separate_rows = false;

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment("Crate".bold().underline(), 1, Alignment::Left),
            TableCell::new_with_alignment("Current".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Latest".bold().underline(), 1, Alignment::Center),
        ]));

        let mut container = self.get_upgradable()?;

        // sort by name
        container.crates.sort_by(|a, b| a.name.cmp(&b.name));

        for item in container.crates {
            let (name, max) = if item.is_upgradable().unwrap_or(false) {
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

        Ok(())
    }
}
