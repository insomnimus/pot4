mod cli;
mod device_config;
mod logger;
mod usb;

use std::{
	io,
	io::{
		BufRead,
		Write,
	},
	process,
};

use clap::Parser;
use log::*;

use self::{
	cli::Cli,
	usb::ConfigDevice,
};

fn shell(config: &mut ConfigDevice) -> Result<(), io::Error> {
	let stdin = io::stdin();
	let mut stdin = stdin.lock();
	let mut stdout = io::stdout();

	let mut line = String::new();

	loop {
		print!("> ");
		stdout.flush()?;

		line.clear();

		if stdin.read_line(&mut line)? == 0 {
			break;
		}

		let message = line.trim();
		if message.is_empty() {
			continue;
		}

		match message {
			"help" => {
				println!(
					"Commands:
ping: Send a ping
config.get: Retreive configuration
config.set <KEY1=VALUE1;KEY2=VALUE2>...: Set one or more configuration parameters; available parameters: preset=, pot0.cc=, pot0.chan=
config.save: Save configuration to persistent memory
config.reset: Load last saved configuration
preset.get <preset-no>: Get preset configuration
preset.set <no> <KEY1=VALUE1;KEY2=VALUE2...>: Change preset values. Available keys: name=, pot0.cc=, pot0.chan=
help: Show this message
exit: Exit the interactive shell",
				);
				continue;
			}
			"exit" => {
				return Ok(());
			}
			_ => (),
		}

		config.write_message(message)?;

		let response = config.read_message()?;

		println!("{}", String::from_utf8_lossy(&response));
	}

	Ok(())
}

fn main() {
	fn run() -> io::Result<()> {
		let args = Cli::parse();
		let log_level = args.verbosity.log_level().unwrap_or(log::Level::Warn);
		logger::init(log_level);

		let mut config = ConfigDevice::open()?;
		trace!("Sending command: ping");
		config.write_message("ping")?;
		let resp = config.read_message()?;
		if &resp != b"pong" {
			return Err(io::Error::other(format!(
				"Ping failed: got response {}",
				String::from_utf8_lossy_owned(resp)
			)));
		}

		if !args.changes.is_empty() {
			let mut cmd = String::with_capacity(128);
			cmd += "config.set ";

			for c in &args.changes {
				cmd += &c.serialize();
				cmd.push(';');
			}

			cmd.pop();

			trace!("Sending command {cmd}");
			config.write_message(&cmd)?;
			config.read_ok()?;
		}

		if args.interactive {
			shell(&mut config)?;
		} else {
			trace!("Sending command config.get");
			config.write_message("config.get")?;

			let resp = config.read_message()?;
			let resp = str::from_utf8(&resp).map_err(|_| {
				io::Error::other(format!(
					"Got a non-UTF8 response: {}",
					String::from_utf8_lossy(&resp)
				))
			})?;

			println!("{resp}");
		}
		Ok(())
	}

	if let Err(e) = run() {
		eprintln!("error: {e}");
		process::exit(1);
	}
}
