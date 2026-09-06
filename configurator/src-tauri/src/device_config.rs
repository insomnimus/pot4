use std::fmt::Write;

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
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct PotConfig {
	pub cc: u8,
	pub channel: u8,
	pub triggers: [bool; 4],
}

#[derive(Copy, Clone, Default)]
struct OptionalPotConfig {
	cc: Option<u8>,
	channel: Option<u8>,
	triggers: Option<[bool; 4]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
	name: String,
	pub pots: [PotConfig; 4],
}

impl Preset {
	pub fn parse(s: &str) -> Result<Self, ConfigParseError> {
		let mut pots = [OptionalPotConfig::default(); 4];
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

		Ok(Self { name, pots: ps })
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
	Preset(PresetChange),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PresetChange {
	Name {
		preset: u8,
		name: String,
	},
	Pot {
		preset: u8,
		pot: u8,
		cc: u8,
		channel: u8,
		triggers: [bool; 4],
	},
}

impl ConfigChange {
	pub fn to_command_string(&self) -> String {
		let s = match self {
			Self::ActivePreset(preset) => format!("config.set preset={preset}"),
			Self::Preset(preset_change) => match preset_change {
				PresetChange::Name { preset, name } => format!("preset.set {preset} name={name}"),
				PresetChange::Pot {
					preset,
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
			},
		};

		println!("change: {s}");

		s
	}
}
