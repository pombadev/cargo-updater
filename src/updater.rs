use std::{
    env::consts,
    fmt,
    process::{self, Command},
    sync::mpsc::channel,
    thread,
};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use semver::Version;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};
use time::{format_description::well_known::Iso8601, OffsetDateTime};
use ureq::serde_json::Value;

pub enum CrateKind {
    Cratesio(String),
    Git(String),
    Local(String),
}

impl CrateKind {
    const fn full_string(&self) -> &String {
        match self {
            Self::Cratesio(p) | Self::Git(p) | Self::Local(p) => p,
        }
    }
}

impl fmt::Display for CrateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cratesio(_) => write!(f, "crates.io"),
            Self::Git(_) => write!(f, "git"),
            Self::Local(_) => write!(f, "local"),
        }
    }
}

pub struct CrateInfo {
    name: String,
    current: String,
    online: String,
    updated_at: String,
    kind: CrateKind,
}

impl CrateInfo {
    pub(crate) fn is_upgradable(&self) -> bool {
        let inner = || -> Result<bool> {
            let max = Version::parse(self.online.as_str())?;

            let current = Version::parse(self.current.as_str())?;

            Ok(current < max)
        };

        inner().unwrap_or(false) && self.is_from_cratesio()
    }

    pub(crate) fn is_from_cratesio(&self) -> bool {
        self.kind.to_string() == "crates.io"
    }
}

pub struct CratesInfoContainer {
    crates: Vec<CrateInfo>,
}

impl CratesInfoContainer {
    pub(crate) fn parse() -> Result<Self> {
        let output = Command::new("cargo")
            .args(["install", "--list"])
            .output()
            .context("cargo was not found in $PATH")?;

        if !output.status.success() {
            bail!("`cargo install --list` not successful");
        }

        let crates = std::str::from_utf8(&output.stdout[..])?
            .lines()
            .filter(|line| !line.starts_with(char::is_whitespace))
            .map(|line| {
                let krate = line.split(' ').enumerate().fold(
                    ("", "", CrateKind::Cratesio("".into())),
                    |mut total, (index, item)| {
                        match index {
                            // crate's name
                            0 => {
                                total.0 = item;
                            }
                            // crate's version
                            1 => {
                                let version =
                                    item.trim_end_matches(|c| c == ':').trim_start_matches('v');

                                total.1 = version;
                            }
                            // crate installation source
                            2 => {
                                let path = item.trim_matches(|c| c == '(' || c == ')' || c == ':');

                                let kind = if path.starts_with("http") {
                                    CrateKind::Git(path.to_string())
                                } else {
                                    CrateKind::Local(path.to_string())
                                };

                                total.2 = kind;
                            }
                            _ => {}
                        };

                        total
                    },
                );

                let (name, current, kind) = krate;

                CrateInfo {
                    kind,
                    name: name.into(),
                    current: current.into(),
                    updated_at: String::with_capacity(0),
                    online: String::with_capacity(0),
                }
            })
            .collect::<Vec<CrateInfo>>();

        Ok(Self { crates })
    }

    pub(crate) fn get_upgradable() -> Result<Self> {
        let (tx, rx) = channel();

        for item in Self::parse()?.crates {
            let tx = tx.clone();

            thread::spawn(move || -> Result<()> {
                let krate = if item.is_from_cratesio() {
                    let url = format!("https://crates.io/api/v1/crates/{}", item.name);

                    let response = ureq::get(&url)
                        .set(
                            "User-Agent",
                            &format!(
                                "{}/{} ({}, {})",
                                env!("CARGO_PKG_NAME"),
                                env!("CARGO_PKG_VERSION"),
                                consts::OS,
                                consts::ARCH,
                            ),
                        )
                        .call()?
                        .into_json::<ureq::serde_json::Value>()?;

                    let response = response
                        .get("crate")
                        .context("field `<response>.crate` not found")?;

                    // NOTE: `newest_version` is guranteed to exist
                    let online = response["newest_version"]
                        .as_str()
                        .context("field `<response>.crate.newest_version` not found")?;

                    let repository = match response.get("repository") {
                        // Some crates (e.g. mdbook-katex) have `Null` repositories.
                        Some(Value::String(v)) => v,
                        _ => "-",
                    };

                    let updated_at = match response.get("updated_at") {
                        Some(Value::String(val)) => {
                            match OffsetDateTime::parse(val, &Iso8601::DEFAULT) {
                                Ok(d) => {
                                    let (year, month, day) = d.to_calendar_date();

                                    format!("{day} {month} {year}")
                                }
                                Err(_) => "-".into(),
                            }
                        }
                        _ => "-".into(),
                    };

                    CrateInfo {
                        online: online.into(),
                        kind: CrateKind::Cratesio(repository.into()),
                        updated_at,
                        ..item
                    }
                } else {
                    CrateInfo {
                        online: "-".into(),
                        updated_at: "-".into(),
                        ..item
                    }
                };

                tx.send(krate)?;

                Ok(())
            });
        }

        drop(tx); // let know that loop is done.

        let crates = rx.iter().collect();

        Ok(Self { crates })
    }

    pub(crate) fn update(use_locked: &bool) -> Result<()> {
        let standard_crates = Self::get_standard_crates()?;

        if standard_crates.is_empty() {
            println!("Nothing to update, run with `--list` to view available updates.");

            return Ok(());
        }

        let mut flags = Vec::with_capacity(2);

        flags.push("--force");

        if *use_locked {
            flags.push("--locked")
        }

        let mut cmd = Command::new("cargo");

        let cmd = cmd.arg("install").args(flags).args(standard_crates);

        let mut child = cmd.spawn()?;

        let status = child.wait()?;

        if !status.success() {
            match status.code() {
                Some(code) => {
                    eprintln!("Exited with status code: {}", code);
                    process::exit(code);
                }
                None => {
                    bail!("Running `cargo install` was not successful.");
                }
            };
        }

        Ok(())
    }

    fn get_standard_crates() -> Result<Vec<String>> {
        let container = Self::get_upgradable()?;

        let (standard_crates, non_standard_crates) =
            container
                .crates
                .iter()
                .fold((vec![], vec![]), |mut total, krate| {
                    if krate.is_upgradable() {
                        total.0.push(krate.name.clone());
                    } else if !krate.is_from_cratesio() {
                        total.1.push(krate.name.clone());
                    }
                    total
                });

        if !non_standard_crates.is_empty() {
            println!(
                "Skipped updating binaries not installed from crates.io: {}",
                non_standard_crates.join(", ").bold()
            );
        }
        Ok(standard_crates)
    }

    pub(crate) fn list() -> Result<()> {
        let mut table = Table::new();

        table.style = TableStyle::blank();

        table.separate_rows = false;

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment("Crate".bold().underline(), 1, Alignment::Left),
            TableCell::new_with_alignment("Current".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Latest".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Updated".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Source".bold().underline(), 1, Alignment::Center),
            TableCell::new_with_alignment("Repository".bold().underline(), 1, Alignment::Center),
        ]));

        // empty row
        // table.add_row(Row::new(vec![] as Vec<TableCell>));

        let mut container = Self::get_upgradable()?;

        // sort by name
        container.crates.sort_by(|a, b| a.name.cmp(&b.name));

        for krate in container.crates {
            let online = if krate.is_upgradable() {
                krate.online.bright_red()
            } else if krate.is_from_cratesio() {
                krate.online.bright_green()
            } else {
                krate.online.normal()
            };

            let kind = if krate.is_from_cratesio() {
                krate.kind.to_string().bright_cyan()
            } else {
                krate.kind.to_string().bright_yellow()
            };

            let (repo, repo_alignment) =
                (krate.kind.full_string().bright_purple(), Alignment::Center);

            table.add_row(Row::new(vec![
                TableCell::new_with_alignment(&krate.name.bright_blue(), 1, Alignment::Left),
                TableCell::new_with_alignment(&krate.current.bright_purple(), 1, Alignment::Center),
                TableCell::new_with_alignment(online, 1, Alignment::Center),
                TableCell::new_with_alignment(
                    if krate.updated_at == "-" {
                        krate.updated_at.normal()
                    } else {
                        krate.updated_at.bright_purple()
                    },
                    1,
                    Alignment::Center,
                ),
                TableCell::new_with_alignment(kind, 1, Alignment::Center),
                TableCell::new_with_alignment(repo, 1, repo_alignment),
            ]));
        }

        print!("{}", table.render().trim());

        Ok(())
    }
}
