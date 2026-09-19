pub mod class;
pub mod command;

use core::fmt::{
	self,
	Write,
};

use arrayvec::{
	ArrayString,
	ArrayVec,
};
use defmt::Format;
use serde::{
	Deserialize,
	Serialize,
};

use self::command::{
	ConfigChange,
	ConfigKey,
	GetConfigKey,
	PresetConfigChange,
	PresetConfigKey,
};
use crate::{
	midi,
	storage::Versioned,
};

const DEFAULT_HOLD_TIME: u16 = 300;
const DEFAULT_CLICK_TIME: u16 = 250;

const MAX_SERIALIZED_CONFIG_SIZE: usize = 1024;
const MAX_SERIALIZED_PRESET_CONFIG_SIZE: usize = 1024;

#[derive(Copy, Clone, Serialize, Deserialize, Format)]
pub struct PotConfig {
	#[serde(rename = "ch")]
	pub channel: u8,
	pub cc: u8,
	pub triggers: u8,
}

impl PotConfig {
	pub const DEFAULT_POTS: [Self; 4] = [
		Self {
			cc: 9,
			channel: 0,
			triggers: 1 << 0,
		},
		Self {
			cc: 10,
			channel: 0,
			triggers: 1 << 1,
		},
		Self {
			cc: 11,
			channel: 0,
			triggers: 1 << 2,
		},
		Self {
			cc: 12,
			channel: 0,
			triggers: 1 << 3,
		},
	];

	pub fn create_cc_packet(self, value: u8) -> [u8; 4] {
		midi::cc(self.cc, self.channel, value)
	}

	pub fn create_cc_packets(&self, pots: &[PotConfig; 4], value: u8) -> ArrayVec<[u8; 4], 4> {
		let mut packets = ArrayVec::new();

		for pot in 0..4 {
			let mask = 1 << pot;
			if self.triggers & mask == mask {
				packets.push(pots[pot as usize].create_cc_packet(value));
			}
		}

		packets
	}
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
	pub presets: [Preset; 4],
	pub active_preset: u8,
}

impl DeviceConfig {
	pub const FACTORY: Self = Self {
		active_preset: 0,
		presets: [Preset::FACTORY; 4],
	};

	pub fn serialize(&self) -> ArrayVec<u8, MAX_SERIALIZED_CONFIG_SIZE> {
		let mut buf = ArrayVec::new();
		write!(
			ArrayVecWriter { buf: &mut buf },
			"preset={}",
			self.active_preset,
		)
		.unwrap();

		let preset = self.active_preset();

		for (i, pot) in preset.pots.iter().enumerate() {
			let mut writer = ArrayVecWriter { buf: &mut buf };

			write!(
				writer,
				";pot{}.cc={};pot{}.chan={}",
				i, pot.cc, i, pot.channel
			)
			.unwrap();

			write!(writer, ";pot{}.triggers=", i).unwrap();
			let mut n_triggers = 0;
			for pot_index in 0..4 {
				let mask = 1 << pot_index;

				if pot.triggers & mask == mask {
					if n_triggers > 0 {
						writer.write_str(",").unwrap();
					}

					n_triggers += 1;
					write!(writer, "{}", pot_index).unwrap();
				}
			}
		}

		for (i, button) in preset.buttons.iter().enumerate() {
			buf.push(b';');
			button.serialize_into(&mut buf, i as u8).unwrap();
		}

		buf
	}

	pub fn serialize_key(&self, key: GetConfigKey) -> ArrayVec<u8, 16> {
		let mut buf = ArrayVec::new();
		let mut f = ArrayVecWriter { buf: &mut buf };

		match key {
			GetConfigKey::Preset => write!(f, "preset={}", self.active_preset).unwrap(),
		}

		buf
	}

	// Note: this function applies the changes without checking values.
	pub fn apply(&mut self, changes: &[ConfigChange]) {
		for change in changes {
			match change.key {
				ConfigKey::PotCc(pot) => {
					self.active_preset_mut().pots[pot as usize].cc = change.value.unwrap_u8();
				}
				ConfigKey::PotChan(pot) => {
					self.active_preset_mut().pots[pot as usize].channel = change.value.unwrap_u8();
				}
				ConfigKey::PotTriggers(pot) => {
					self.active_preset_mut().pots[pot as usize].triggers = change.value.unwrap_u8();
				}
				ConfigKey::Preset => self.active_preset = change.value.unwrap_u8(),
				ConfigKey::ButtonGesture { button, gesture } => self.active_preset_mut().buttons
					[button as usize]
					.set_gesture(gesture, change.value.unwrap_button_action()),
				ConfigKey::ButtonTime {
					button,
					is_hold: true,
				} => {
					self.active_preset_mut().buttons[button as usize].hold_time =
						change.value.unwrap_u16()
				}
				ConfigKey::ButtonTime {
					button,
					is_hold: false,
				} => {
					self.active_preset_mut().buttons[button as usize].click_time =
						change.value.unwrap_u16()
				}
			}
		}
	}

	pub fn preset(&self, preset: u8) -> &Preset {
		&self.presets[preset as usize]
	}

	pub fn preset_mut(&mut self, preset: u8) -> &mut Preset {
		&mut self.presets[preset as usize]
	}

	pub fn active_preset(&self) -> &Preset {
		&self.presets[self.active_preset as usize]
	}

	pub fn active_preset_mut(&mut self) -> &mut Preset {
		&mut self.presets[self.active_preset as usize]
	}

	pub fn next_preset(&mut self) {
		if self.active_preset as usize + 1 > self.presets.len() {
			self.active_preset = 0;
		} else {
			self.active_preset += 1;
		}
	}

	pub fn previous_preset(&mut self) {
		if self.active_preset == 0 {
			self.active_preset = (self.presets.len() - 1) as u8;
		} else {
			self.active_preset -= 1;
		}
	}
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Preset {
	pub name: ArrayString<32>,
	pub pots: [PotConfig; 4],
	pub buttons: [ButtonConfig; 4],
}

impl Preset {
	pub const FACTORY: Self = Self {
		name: ArrayString::new_const(),
		pots: PotConfig::DEFAULT_POTS,
		buttons: ButtonConfig::DEFAULT_BUTTONS,
	};

	pub fn serialize(&self) -> ArrayVec<u8, MAX_SERIALIZED_PRESET_CONFIG_SIZE> {
		let mut buf = ArrayVec::new();
		write!(ArrayVecWriter { buf: &mut buf }, "name={}", self.name).unwrap();

		for (i, pot) in self.pots.iter().enumerate() {
			let mut writer = ArrayVecWriter { buf: &mut buf };

			write!(
				writer,
				";pot{}.cc={};pot{}.chan={}",
				i, pot.cc, i, pot.channel
			)
			.unwrap();

			write!(writer, ";pot{}.triggers=", i).unwrap();
			let mut n_triggers = 0;
			for pot_index in 0..4 {
				let mask = 1 << pot_index;

				if pot.triggers & mask == mask {
					if n_triggers > 0 {
						writer.write_str(",").unwrap();
					}

					n_triggers += 1;

					write!(writer, "{}", pot_index).unwrap();
				}
			}
		}

		for (i, button) in self.buttons.iter().enumerate() {
			buf.push(b';');
			button.serialize_into(&mut buf, i as u8).unwrap();
		}

		buf
	}

	// Note: this function applies the changes without checking values.
	pub fn apply(&mut self, changes: &[PresetConfigChange]) {
		for change in changes {
			match change.key {
				PresetConfigKey::PotCc(pot) => {
					self.pots[pot as usize].cc = change.value.unwrap_u8()
				}
				PresetConfigKey::PotChan(pot) => {
					self.pots[pot as usize].channel = change.value.unwrap_u8()
				}
				PresetConfigKey::PotTriggers(pot) => {
					self.pots[pot as usize].triggers = change.value.unwrap_u8()
				}
				PresetConfigKey::Name => self.name = change.value.unwrap_preset_name(),
				PresetConfigKey::ButtonGesture { button, gesture } => self.buttons[button as usize]
					.set_gesture(gesture, change.value.unwrap_button_action()),
				PresetConfigKey::ButtonTime {
					button,
					is_hold: true,
				} => self.buttons[button as usize].hold_time = change.value.unwrap_u16(),
				PresetConfigKey::ButtonTime {
					button,
					is_hold: false,
				} => self.buttons[button as usize].click_time = change.value.unwrap_u16(),
			}
		}
	}
}

struct ArrayVecWriter<'a, const N: usize> {
	buf: &'a mut ArrayVec<u8, N>,
}

impl<const N: usize> fmt::Write for ArrayVecWriter<'_, N> {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		self.buf
			.try_extend_from_slice(s.as_bytes())
			.map_err(|_| fmt::Error)
	}
}

impl Versioned for DeviceConfig {
	const VERSION: u16 = 4;
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct ButtonConfig {
	pub clicks: [ButtonAction; 3],
	pub hold: ButtonAction,
	pub click_time: u16,
	pub hold_time: u16,
}

impl ButtonConfig {
	pub const DEFAULT_BUTTONS: [Self; 4] = [
		Self {
			clicks: [ButtonAction::Preset { preset: 0 }; 3],
			hold: ButtonAction::None,
			click_time: DEFAULT_CLICK_TIME,
			hold_time: DEFAULT_HOLD_TIME,
		},
		Self {
			clicks: [ButtonAction::Preset { preset: 1 }; 3],
			hold: ButtonAction::None,
			click_time: DEFAULT_CLICK_TIME,
			hold_time: DEFAULT_HOLD_TIME,
		},
		Self {
			clicks: [ButtonAction::Preset { preset: 2 }; 3],
			hold: ButtonAction::None,
			click_time: DEFAULT_CLICK_TIME,
			hold_time: DEFAULT_HOLD_TIME,
		},
		Self {
			clicks: [ButtonAction::Preset { preset: 3 }; 3],
			hold: ButtonAction::None,
			click_time: DEFAULT_CLICK_TIME,
			hold_time: DEFAULT_HOLD_TIME,
		},
	];

	fn set_gesture(&mut self, gesture: ButtonGesture, action: ButtonAction) {
		match gesture {
			ButtonGesture::Hold => self.hold = action,
			ButtonGesture::Click(n) => self.clicks[n as usize - 1] = action,
		}
	}

	fn serialize_into<const N: usize>(
		&self,
		buf: &mut ArrayVec<u8, N>,
		button_index: u8,
	) -> Result<(), fmt::Error> {
		for (i, action) in self.clicks.iter().enumerate() {
			let click_count = i + 1;

			write!(ArrayVecWriter { buf }, "btn{button_index}.{click_count}=",)?;

			action.serialize_into(buf)?;
			buf.push(b';');
		}

		write!(ArrayVecWriter { buf }, "btn{button_index}.hold=")?;
		self.hold.serialize_into(buf)?;

		write!(
			ArrayVecWriter { buf },
			";btn{button_index}.hold-time={hold_time};btn{button_index}.click-time={click_time}",
			hold_time = self.hold_time,
			click_time = self.click_time,
		)?;

		Ok(())
	}
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ButtonAction {
	None,
	Preset { preset: u8 },
	NextPreset,
	PreviousPreset,
	Cc { cc: u8, channel: u8 },
	Note { note: u8, channel: u8 },
}

impl ButtonAction {
	fn serialize_into<const N: usize>(self, buf: &mut ArrayVec<u8, N>) -> Result<(), fmt::Error> {
		match self {
			Self::None => buf.extend("none".bytes()),
			Self::NextPreset => buf.extend("next-preset".bytes()),
			Self::PreviousPreset => buf.extend("prev-preset".bytes()),
			Self::Preset { preset } => {
				write!(ArrayVecWriter { buf }, "preset|{preset}")?;
			}
			Self::Cc { cc, channel } => {
				write!(ArrayVecWriter { buf }, "cc|{channel}|{cc}")?;
			}
			Self::Note { note, channel } => {
				write!(ArrayVecWriter { buf }, "note|{channel}|{note}")?;
			}
		}

		Ok(())
	}
}

#[derive(Copy, Clone)]
pub enum ButtonGesture {
	Click(u8),
	Hold,
}
