use core::str::FromStr;

use arrayvec::{
	ArrayString,
	ArrayVec,
};
use defmt::Format;

use super::{
	ButtonAction,
	ButtonGesture,
};
use crate::Response;

const MAX_CHANGES: usize = 13; // 3 per pot, 1 for preset
const MAX_PRESET_CHANGES: usize = 13; // 3 per pot, 1 for preset name

#[derive(Format, PartialEq, Eq, Copy, Clone)]
pub enum ParseError {
	InvalidUtf8,
	UnknownCommand,
	InvalidAssignment,
	UnknownKey,
	InvalidPreset,
	PresetNameTooLong,
	InvalidConfigKey,
	InvalidPot,
	InvalidValue,
	ValueOutOfRange,
	TooManyChanges,
	CommandTakesNoArgs,
	MissingArgs,
	InvalidButton,
	InvalidButtonGesture,
}

impl ParseError {
	pub fn to_error_message(self) -> Response {
		use ParseError::*;

		let msg = match self {
			InvalidUtf8 => "error UTF8 parse error",
			UnknownCommand => "error Unknown command",
			InvalidAssignment => "error Invalid assignment",
			UnknownKey => "error Unknown key",
			InvalidConfigKey => "error Invalid config key",
			InvalidPot => "error Invalid pot",
			InvalidValue => "error Invalid value",
			ValueOutOfRange => "error Value out of range",
			TooManyChanges => "error Too many config changes",
			InvalidPreset => "Invalid preset",
			PresetNameTooLong => "Preset name too long",
			CommandTakesNoArgs => "Command takes no args",
			MissingArgs => "error Missing one or more arguments",
			InvalidButton => "error Invalid button",
			InvalidButtonGesture => "error Invalid button gesture",
		};

		let mut buf = Response::new();
		buf.extend(msg.bytes());

		buf
	}
}

#[allow(clippy::large_enum_variant)]
pub enum Command {
	Ping,

	GetConfig {
		saved: bool,
		key: Option<GetConfigKey>,
	},
	SetConfig {
		changes: ArrayVec<ConfigChange, MAX_CHANGES>,
	},
	SaveConfig,
	ResetConfig,

	GetPreset {
		saved: bool,
		preset: u8,
	},
	SetPreset {
		preset: u8,
		changes: ArrayVec<PresetConfigChange, MAX_PRESET_CHANGES>,
	},

	FactoryReset,
	Beep {
		fq: u16,
		duration: u16,
		duty: Option<f32>,
	},

	// Internal commands (no protocol exists for these)
	NextPreset,
	PreviousPreset,
}

impl Command {
	pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
		let text = str::from_utf8(data).map_err(|_| ParseError::InvalidUtf8)?;
		let (cmd, args) = text.split_once(' ').unwrap_or((text, ""));
		let cmd = cmd.trim();
		let args = args.trim();

		let cmd = match cmd {
			// These commands have no arguments.
			"ping" | "config.save" | "config.reset" | "factory-reset" if !args.is_empty() => {
				return Err(ParseError::CommandTakesNoArgs);
			}

			"ping" => Self::Ping,

			"config.get" => {
				let mut saved = false;
				let key = if args.is_empty() {
					None
				} else {
					if let Some(args) = args.strip_prefix("saved.") {
						saved = true;
						Some(GetConfigKey::parse(args)?)
					} else if args == "saved" {
						saved = true;
						None
					} else {
						Some(GetConfigKey::parse(args)?)
					}
				};

				Self::GetConfig { saved, key }
			}
			"config.set" => {
				let mut changes = ArrayVec::<_, MAX_CHANGES>::new();

				for assignment in args.split(';') {
					let change = ConfigChange::parse(assignment)?;
					if changes.try_push(change).is_err() {
						return Err(ParseError::TooManyChanges);
					}
				}

				if changes.is_empty() {
					return Err(ParseError::InvalidAssignment);
				}

				Self::SetConfig { changes }
			}
			"config.save" => Self::SaveConfig,
			"config.reset" => Self::ResetConfig,

			"preset.get" => {
				let (prefix, args) = args.split_once('.').unwrap_or(("", args));
				let saved = prefix == "saved";
				if !saved && !prefix.is_empty() {
					return Err(ParseError::InvalidPreset);
				}

				let preset = parse_value(args, 3).map_err(|_| ParseError::InvalidPreset)?;
				Self::GetPreset { saved, preset }
			}
			"preset.set" => {
				let mut changes = ArrayVec::<_, MAX_PRESET_CHANGES>::new();
				let (preset, args) = args.split_once(' ').ok_or(ParseError::InvalidAssignment)?;
				let preset = parse_value(preset, 3).map_err(|_| ParseError::InvalidPreset)?;

				for assignment in args.trim().split(';') {
					let change = PresetConfigChange::parse(assignment)?;
					if changes.try_push(change).is_err() {
						return Err(ParseError::TooManyChanges);
					}
				}

				if changes.is_empty() {
					return Err(ParseError::InvalidAssignment);
				}

				Self::SetPreset { preset, changes }
			}

			"factory-reset" => Self::FactoryReset,

			"beep" => {
				let mut args = args.split_ascii_whitespace();

				let fq = parse_value(args.next().ok_or(ParseError::MissingArgs)?, 24000)?;
				let duration = parse_value(args.next().ok_or(ParseError::MissingArgs)?, 5000)?;

				let duty = args
					.next()
					.map(|s| {
						let val = parse_value(s.trim(), 1.0)?;
						if val < 0.0 {
							Err(ParseError::ValueOutOfRange)
						} else {
							Ok(val)
						}
					})
					.transpose()?;

				if args.next().is_some() {
					return Err(ParseError::InvalidValue);
				}

				Self::Beep { fq, duration, duty }
			}

			_ => return Err(ParseError::UnknownCommand),
		};

		Ok(cmd)
	}
}

#[derive(Clone, Copy)]
pub struct ConfigChange {
	pub key: ConfigKey,
	pub value: ConfigValue,
}

impl ConfigChange {
	pub fn parse(assignment: &str) -> Result<Self, ParseError> {
		let (key, value) = assignment
			.split_once('=')
			.ok_or(ParseError::InvalidAssignment)?;

		let key = ConfigKey::parse(key)?;
		let value = ConfigValue::parse(value, key)?;

		Ok(Self { key, value })
	}
}

#[derive(Clone, Copy)]
pub enum ConfigKey {
	PotCc(u8),
	PotChan(u8),
	PotTriggers(u8),
	Preset,
	Button { button: u8, gesture: ButtonGesture },
}

impl ConfigKey {
	fn parse(key: &str) -> Result<Self, ParseError> {
		if key == "preset" {
			return Ok(Self::Preset);
		}

		let rest = key.strip_prefix("pot").ok_or(ParseError::UnknownKey)?;

		let (pot, field) = rest.split_once('.').ok_or(ParseError::UnknownKey)?;

		let pot = pot.parse::<u8>().map_err(|_| ParseError::InvalidPot)?;

		if pot >= 4 {
			return Err(ParseError::InvalidPot);
		}

		match field {
			"cc" => Ok(Self::PotCc(pot)),
			"chan" => Ok(Self::PotChan(pot)),
			"triggers" => Ok(Self::PotTriggers(pot)),
			_ => Err(ParseError::UnknownKey),
		}
	}
}

#[derive(Clone, Copy)]
pub enum PresetConfigKey {
	PotCc(u8),
	PotChan(u8),
	PotTriggers(u8),
	Name,
	Button { button: u8, gesture: ButtonGesture },
}

impl PresetConfigKey {
	fn parse(key: &str) -> Result<Self, ParseError> {
		if key == "name" {
			return Ok(Self::Name);
		}

		if key.starts_with("btn") {
			let (button, gesture) = parse_button_key(key)?;
			return Ok(Self::Button { button, gesture });
		}

		let rest = key.strip_prefix("pot").ok_or(ParseError::UnknownKey)?;

		let (pot, field) = rest.split_once('.').ok_or(ParseError::UnknownKey)?;

		let pot = pot.parse::<u8>().map_err(|_| ParseError::InvalidPot)?;

		if pot >= 4 {
			return Err(ParseError::InvalidPot);
		}

		match field {
			"cc" => Ok(Self::PotCc(pot)),
			"chan" => Ok(Self::PotChan(pot)),
			"triggers" => Ok(Self::PotTriggers(pot)),
			_ => Err(ParseError::UnknownKey),
		}
	}
}

pub struct PresetConfigChange {
	pub key: PresetConfigKey,
	pub value: ConfigValue,
}

impl PresetConfigChange {
	fn parse(assignment: &str) -> Result<Self, ParseError> {
		let (key, value) = assignment
			.split_once('=')
			.ok_or(ParseError::InvalidAssignment)?;

		let key = PresetConfigKey::parse(key)?;
		let value = ConfigValue::preset_parse(value, key)?;

		Ok(Self { key, value })
	}
}

#[derive(Copy, Clone)]
pub enum ConfigValue {
	U8(u8),
	PresetName(ArrayString<32>),
	ButtonAction(ButtonAction),
}

impl ConfigValue {
	fn parse(s: &str, key: ConfigKey) -> Result<Self, ParseError> {
		let val = match key {
			ConfigKey::Preset => {
				Self::U8(parse_value(s, 3).map_err(|_| ParseError::InvalidPreset)?)
			}
			ConfigKey::PotCc(_) => Self::U8(parse_value(s, 127)?),
			ConfigKey::PotChan(_) => Self::U8(parse_value(s, 15)?),
			ConfigKey::PotTriggers(_) => {
				let mut val = 0u8;

				for n in s.split(',') {
					let n = parse_value(n, 3)?;
					val |= 1 << n;
				}

				Self::U8(val)
			}
			ConfigKey::Button { .. } => Self::ButtonAction(parse_button_action(s)?),
		};

		Ok(val)
	}

	fn preset_parse(s: &str, key: PresetConfigKey) -> Result<Self, ParseError> {
		let val = match key {
			PresetConfigKey::Name => Self::PresetName(
				ArrayString::try_from(s).map_err(|_| ParseError::PresetNameTooLong)?,
			),
			PresetConfigKey::PotCc(_) => Self::U8(parse_value(s, 127)?),
			PresetConfigKey::PotChan(_) => Self::U8(parse_value(s, 15)?),
			PresetConfigKey::PotTriggers(_) => {
				let mut val = 0u8;

				for n in s.split(',') {
					let n = parse_value(n, 3)?;
					val |= 1 << n;
				}

				Self::U8(val)
			}
			PresetConfigKey::Button { .. } => Self::ButtonAction(parse_button_action(s)?),
		};

		Ok(val)
	}

	pub fn unwrap_u8(self) -> u8 {
		match self {
			Self::U8(val) => val,
			_ => defmt::panic!("ConfigValue::unwrap_u8 called on a non-U8 variant"),
		}
	}

	pub fn unwrap_preset_name(self) -> ArrayString<32> {
		match self {
			Self::PresetName(val) => val,
			_ => {
				defmt::panic!("ConfigValue::unwrap_preset_name called on a non-PresetName variant")
			}
		}
	}

	pub fn unwrap_button_action(self) -> ButtonAction {
		match self {
			Self::ButtonAction(val) => val,
			_ => defmt::panic!(
				"ConfigValue::unwrap_button_action called on a non-ButtonAction variant"
			),
		}
	}
}

pub enum GetConfigKey {
	Preset,
	// Pot(u8),
}

impl GetConfigKey {
	fn parse(s: &str) -> Result<Self, ParseError> {
		if s == "preset" {
			return Ok(Self::Preset);
		}

		Err(ParseError::InvalidConfigKey)
	}
}

fn parse_value<T: FromStr + PartialOrd>(value: &str, max: T) -> Result<T, ParseError> {
	let n = value.parse().map_err(|_| ParseError::InvalidValue)?;

	if n <= max {
		Ok(n)
	} else {
		Err(ParseError::ValueOutOfRange)
	}
}

fn parse_button_action(s: &str) -> Result<ButtonAction, ParseError> {
	fn last<'a>(mut iter: core::str::Split<'a, char>) -> Result<&'a str, ParseError> {
		let s = iter.next().ok_or(ParseError::InvalidValue)?;
		if iter.next().is_some() {
			Err(ParseError::InvalidValue)
		} else {
			Ok(s)
		}
	}

	let x = match s {
		"none" => ButtonAction::None,
		"next-preset" => ButtonAction::NextPreset,
		"prev-preset" => ButtonAction::PreviousPreset,
		_ => {
			let mut args = s.split('|');

			match args.next().ok_or(ParseError::InvalidValue)? {
				"preset" => {
					let preset = last(args)?;
					ButtonAction::Preset {
						preset: parse_value(preset, 3).map_err(|_| ParseError::InvalidValue)?,
					}
				}

				"cc" => {
					let channel = args.next().ok_or(ParseError::InvalidValue)?;
					let cc = last(args)?;

					ButtonAction::Cc {
						cc: parse_value(cc, 127)?,
						channel: parse_value(channel, 15)?,
					}
				}

				"note" => {
					let channel = args.next().ok_or(ParseError::InvalidValue)?;
					let note = last(args)?;

					ButtonAction::Note {
						channel: parse_value(channel, 15)?,
						note: parse_value(note, 127)?,
					}
				}

				_ => return Err(ParseError::InvalidValue),
			}
		}
	};

	Ok(x)
}

fn parse_button_key(s: &str) -> Result<(u8, ButtonGesture), ParseError> {
	let s = s.strip_prefix("btn").ok_or(ParseError::InvalidAssignment)?;
	let (button, subkey) = s.split_once('.').ok_or(ParseError::InvalidButton)?;

	let button = parse_value(button, 3).map_err(|_| ParseError::InvalidButton)?;

	let gesture = match subkey {
		"1" => ButtonGesture::Click(1),
		"2" => ButtonGesture::Click(2),
		"3" => ButtonGesture::Click(3),
		"hold" => ButtonGesture::Hold,
		_ => return Err(ParseError::InvalidButtonGesture),
	};

	Ok((button, gesture))
}
