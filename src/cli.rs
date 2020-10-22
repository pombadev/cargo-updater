use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};

pub fn new() -> App<'static, 'static> {
    App::new(crate_name!()).bin_name("cargo").subcommand(
        SubCommand::with_name("updater")
            .setting(AppSettings::ColorAuto)
            .version(crate_version!())
            .about(crate_description!())
            .args(&[
                Arg::from_usage("-u --update 'Update upgradable crates'").conflicts_with("list"),
                Arg::from_usage("-l --list 'List latest available version'")
                    .conflicts_with("update"),
            ]),
    )
}
