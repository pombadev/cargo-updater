use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};

pub fn new() -> App<'static, 'static> {
    App::new(crate_name!())
        .bin_name("cargo")
        .version(crate_version!())
        .subcommand(
            SubCommand::with_name("global")
                .setting(AppSettings::ColorAuto)
                .version(crate_version!())
                .about(crate_description!())
                .args(&[
                    Arg::from_usage("-u --update 'Update upgradable crates'"),
                    Arg::from_usage("-l --list 'List latest available version'"),
                ]),
        )
}
