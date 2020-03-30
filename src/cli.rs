use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};

pub fn new() -> App<'static, 'static> {
    App::new(crate_name!())
        .bin_name("cargo")
        .version(crate_version!())
        .setting(AppSettings::ColorAuto)
        .subcommand(
            SubCommand::with_name("global")
                .version(crate_version!())
                .about(crate_description!())
                .args(&[
                    Arg::from_usage("-u --update 'Update any executables that has updates'"),
                    Arg::from_usage("-c --check 'Checks for available updates'"),
                ]),
        )
}
