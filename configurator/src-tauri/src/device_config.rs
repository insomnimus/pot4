use std::fmt::{
	self,
	Write,
};

use serde::{
	Deserialize,
	Serialize,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigParseError {
	#[error("Config is missing one or more fields: {0}")]
	Incomplete(String),

	#[error("Missing '=': {0}")]
	MissingEquals(String),

	#[error("Unknown key: {0}")]
	UnknownKey(String),

	#[error("Invalid pot number: {0}")]
	InvalidPotNumber(String),

	#[error("Value too big: {0}")]
	ValueTooBig(String),

	// #[error("Data isn't UTF8: {0:?}")]
	// NonUtf8(Vec<u8>),
	#[error("Invalid value: {0}")]
	InvalidValue(String),

	#[error("Invalid button: {0}")]
	InvalidButton(String),

	#[error("Invalid button action: {0}")]
	InvalidButtonAction(String),
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct PotConfig {
	pub cc: u8,
	pub channel: u8,
	pub triggers: [bool; 4],
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct ButtonConfig {
	clicks: [ButtonAction; 3],
	hold: ButtonAction,
}

#[derive(Copy, Clone, Default)]
struct OptionalPotConfig {
	cc: Option<u8>,
	channel: Option<u8>,
	triggers: Option<[bool; 4]>,
}

#[derive(Copy, Clone, Default)]
struct OptionalButtonConfig {
	clicks: [Option<ButtonAction>; 4],
	hold: Option<ButtonAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
	pub name: String,
	pub pots: [PotConfig; 4],
	pub buttons: [ButtonConfig; 4],
}

impl Preset {
	pub fn parse(s: &str) -> Result<Self, ConfigParseError> {
		let mut pots = [OptionalPotConfig::default(); 4];
		let mut buttons = [OptionalButtonConfig::default(); 4];
		let mut name = String::with_capacity(32);

		for key_value in s.split(';') {
			let parse_value = |s: &str, max: u8| -> Result<u8, ConfigParseError> {
				let n = s
					.parse()
					.map_err(|_| ConfigParseError::InvalidValue(key_value.into()))?;
				if n <= max {
					Ok(n)
				} else {
					Err(ConfigParseError::ValueTooBig(key_value.into()))
				}
			};

			let (key, value) = key_value
				.split_once('=')
				.ok_or_else(|| ConfigParseError::MissingEquals(key_value.into()))?;

			if key == "name" {
				name.clear();
				name += value;
				continue;
			}

			if let Some(button_and_subkey) = key.strip_prefix("btn") {
				let (button, subkey) = button_and_subkey
					.split_once('.')
					.ok_or_else(|| ConfigParseError::InvalidButton(key_value.into()))?;
				let button = parse_value(button, 3)?;
				let action = parse_button_action(value)
					.ok_or_else(|| ConfigParseError::InvalidButtonAction(key_value.into()))?;

				match subkey {
					"1" => buttons[button as usize].clicks[0] = Some(action),
					"2" => buttons[button as usize].clicks[1] = Some(action),
					"3" => buttons[button as usize].clicks[2] = Some(action),
					"hold" => buttons[button as usize].hold = Some(action),
					_ => return Err(ConfigParseError::InvalidButtonAction(key_value.into())),
				}

				continue;
			}

			let pot_number_subkey = key
				.strip_prefix("pot")
				.ok_or_else(|| ConfigParseError::UnknownKey(key_value.into()))?;

			let (pot_number, subkey) = pot_number_subkey
				.split_once('.')
				.ok_or_else(|| ConfigParseError::UnknownKey(key_value.into()))?;

			let pot_number = pot_number
				.parse::<u8>()
				.map_err(|_| ConfigParseError::InvalidPotNumber(key_value.into()))?;

			if pot_number > 3 {
				return Err(ConfigParseError::InvalidPotNumber(key_value.into()));
			}

			match subkey {
				"cc" => {
					pots[pot_number as usize].cc = Some(parse_value(value, 127)?);
				}

				"chan" => {
					pots[pot_number as usize].channel = Some(parse_value(value, 15)?);
				}
				"triggers" => {
					let mut triggers = [false; 4];

					for s in value.split(',') {
						let n = parse_value(s, 3)?;
						triggers[n as usize] = true;
					}

					pots[pot_number as usize].triggers = Some(triggers);
				}

				_ => return Err(ConfigParseError::UnknownKey(key_value.into())),
			}
		}

		let mut ps = [PotConfig {
			cc: 0,
			channel: 0,
			triggers: [false; 4],
		}; 4];
		let mut bs = [ButtonConfig {
			clicks: [ButtonAction::None; 3],
			hold: ButtonAction::None,
		}; 4];

		for (optional, real) in pots.into_iter().zip(ps.iter_mut()) {
			*real = PotConfig {
				cc: optional
					.cc
					.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?,
				channel: optional
					.channel
					.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?,
				triggers: optional
					.triggers
					.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?,
			};
		}

		for (optional, real) in buttons.iter().zip(&mut bs) {
			for (optional_click, real_click) in optional.clicks.iter().zip(&mut real.clicks) {
				*real_click =
					optional_click.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?;
			}

			real.hold = optional
				.hold
				.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?;
		}

		Ok(Self {
			name,
			pots: ps,
			buttons: bs,
		})
	}
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceConfig {
	pub active_preset: u8,
	pub presets: [Preset; 4],
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConfigChange {
	ActivePreset(u8),
	Preset { preset: u8, change: PresetChange },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PresetChange {
	Name(String),
	Pot {
		pot: u8,
		cc: u8,
		channel: u8,
		triggers: [bool; 4],
	},
	Button {
		button: u8,
		clicks: [ButtonAction; 3],
		hold: ButtonAction,
	},
}

impl ConfigChange {
	pub fn to_command_string(&self) -> String {
		let s = match self {
			Self::ActivePreset(preset) => format!("config.set preset={preset}"),

			Self::Preset { preset, change } => match change {
				PresetChange::Name(name) => format!("preset.set {preset} name={name}"),

				PresetChange::Pot {
					pot,
					channel,
					cc,
					triggers,
				} => {
					let mut s = format!("preset.set {preset} pot{pot}.cc={cc};pot{pot}.chan={channel};pot{pot}.triggers=");

					for (i, (pot, _)) in triggers
						.iter()
						.enumerate()
						.filter(|(_pot, &yes)| yes)
						.enumerate()
					{
						if i > 0 {
							s += ",";
						}

						write!(s, "{pot}").unwrap();
					}

					s
				}

				PresetChange::Button {
					button,
					clicks,
					hold,
				} => {
					let mut s = String::with_capacity(128);

					write!(s, "btn{button}.hold={hold}").unwrap();
					for (i, action) in clicks.iter().enumerate() {
						let click = i + 1;
						write!(s, ";btn{button}.{click}={action}").unwrap();
					}

					s
				}
			},
		};

		println!("change: {s}");

		s
	}
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ButtonAction {
	None,
	NextPreset,
	PreviousPreset,
	Preset { preset: u8 },
	Cc { cc: u8, channel: u8 },
	Note { note: u8, channel: u8 },
}

impl fmt::Display for ButtonAction {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::None => f.write_str("none"),
			Self::NextPreset => f.write_str("next-preset"),
			Self::PreviousPreset => f.write_str("prev-preset"),
			Self::Preset { preset } => write!(f, "preset|{preset}"),
			Self::Cc { cc, channel } => write!(f, "cc|{channel}|{cc}"),
			Self::Note { note, channel } => write!(f, "note|{channel}|{note}"),
		}
	}
}

fn parse_button_action(s: &str) -> Option<ButtonAction> {
	fn last<'a>(mut iter: core::str::Split<'a, char>) -> Option<&'a str> {
		let s = iter.next()?;
		if iter.next().is_some() {
			None
		} else {
			Some(s)
		}
	}

	let parse_value = |s: &str, max: u8| -> Option<u8> { s.parse().ok().filter(|&n| n <= max) };

	let x = match s {
		"none" => ButtonAction::None,
		"next-preset" => ButtonAction::NextPreset,
		"prev-preset" => ButtonAction::PreviousPreset,
		_ => {
			let mut args = s.split('|');

			match args.next()? {
				"preset" => {
					let preset = last(args)?;
					ButtonAction::Preset {
						preset: parse_value(preset, 3)?,
					}
				}

				"cc" => {
					let channel = args.next()?;
					let cc = last(args)?;

					ButtonAction::Cc {
						cc: parse_value(cc, 127)?,
						channel: parse_value(channel, 15)?,
					}
				}

				"note" => {
					let channel = args.next()?;
					let note = last(args)?;

					ButtonAction::Note {
						channel: parse_value(channel, 15)?,
						note: parse_value(note, 127)?,
					}
				}

				_ => return None,
			}
		}
	};

	Some(x)
}
