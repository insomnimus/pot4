use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigParseError {
	#[error("Missing '=': {0}")]
	MissingEquals(String),

	#[error("Unknown key: {0}")]
	UnknownKey(String),

	#[error("Invalid pot number: {0}")]
	InvalidPotNumber(String),

	#[error("Value too big: {0}")]
	ValueTooBig(String),

	#[error("Invalid value: {0}")]
	InvalidValue(String),
}

#[derive(Copy, Clone)]
pub enum PotKey {
	Cc,
	Chan,
}

#[derive(Copy, Clone)]
pub struct PotChange {
	pub pot: u8,
	pub key: PotKey,
	pub value: u8,
}

impl PotChange {
	pub fn parse(s: &str) -> Result<Self, ConfigParseError> {
		// example: pot0.cc=1

		let key_value = s;
		let (key, value) = key_value
			.split_once('=')
			.ok_or_else(|| ConfigParseError::MissingEquals(key_value.into()))?;

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

				Ok(Self {
					pot: pot_number,
					key: PotKey::Cc,
					value,
				})
			}

			"chan" => {
				if value > 15 {
					return Err(ConfigParseError::ValueTooBig(key_value.into()));
				}

				Ok(Self {
					pot: pot_number,
					key: PotKey::Chan,
					value,
				})
			}

			_ => Err(ConfigParseError::UnknownKey(key_value.into())),
		}
	}

	pub fn serialize(&self) -> String {
		let key = match self.key {
			PotKey::Cc => "cc",
			PotKey::Chan => "chan",
		};

		format!("pot{}.{}={}", self.pot, key, self.value)
	}
}
