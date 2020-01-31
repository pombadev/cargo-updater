use clap::{
	Arg,
	crate_description,
	App,
	AppSettings,
};

pub fn new() -> App<'static, 'static> {
    App::new("cargo")
        .about(crate_description!())
		.setting(AppSettings::ColorAuto)
		.arg(
			Arg::from_usage("-c --check 'Checks for available updates'"),
		)
		.arg(
			Arg::from_usage("-u --update 'Update any executables that has updates'"),
		)
}
