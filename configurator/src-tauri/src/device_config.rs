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
}

#[derive(Copy, Clone, Default)]
struct OptionalPotConfig {
	cc: Option<u8>,
	channel: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
	name: String,
	pub pots: [PotConfig; 4],
}

impl Preset {
	pub fn parse(s: &str) -> Result<Self, ConfigParseError> {
		// example: name=Preset 1;pot0.cc=1;pot0.chan=0;pot1.cc=10;pot1.chan=10 ...
		let mut pots = [OptionalPotConfig::default(); 4];
		let mut name = String::with_capacity(32);

		for key_value in s.split(';') {
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

			let value = value
				.parse::<u8>()
				.map_err(|_| ConfigParseError::InvalidValue(key_value.into()))?;

			match subkey {
				"cc" => {
					if value > 127 {
						return Err(ConfigParseError::ValueTooBig(key_value.into()));
					}

					pots[pot_number as usize].cc = Some(value);
				}

				"chan" => {
					if value > 15 {
						return Err(ConfigParseError::ValueTooBig(key_value.into()));
					}

					pots[pot_number as usize].channel = Some(value);
				}

				_ => return Err(ConfigParseError::UnknownKey(key_value.into())),
			}
		}

		let mut ps = [PotConfig { cc: 0, channel: 0 }; 4];
		for (optional, real) in pots.into_iter().zip(ps.iter_mut()) {
			*real = PotConfig {
				cc: optional
					.cc
					.ok_or_else(|| ConfigParseError::Incomplete(s.into()))?,
				channel: optional
					.channel
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
#[serde(tag = "type")]
pub enum ConfigChange {
	ActivePreset(u8),
	Preset(PresetChange),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
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
	},
}

impl ConfigChange {
	pub fn to_command_string(&self) -> String {
		match self {
			Self::ActivePreset(preset) => format!("config.set preset={preset}"),
			Self::Preset(preset_change) => match preset_change {
				PresetChange::Name { preset, name } => format!("preset.set {preset} name={name}"),
				PresetChange::Pot {
					preset,
					pot,
					channel,
					cc,
				} => format!("preset.set {preset} pot{pot}.cc={cc};pot{pot}.chan={channel}"),
			},
		}
	}
}
