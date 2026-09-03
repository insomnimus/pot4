use clap::Parser;
use clap_verbosity_flag::{
	Verbosity,
	WarnLevel,
};

use crate::device_config::PotChange;

#[derive(Parser)]
#[command(author, version)]
/// Insomnia Pot 4 Midi CC Controller live configuration utility.
pub struct Cli {
	#[command(flatten)]
	pub verbosity: Verbosity<WarnLevel>,

	/// Enter an interactive shell.
	#[arg(short, long)]
	pub interactive: bool,

	/// One or more config changes in the format KEY=VALUE.
	/// Syntax: pot<1..3>.<cc|chan>=<value>
	#[arg(value_parser = PotChange::parse, num_args = 0..=8)]
	pub changes: Vec<PotChange>,
}
