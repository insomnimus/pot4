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
use crate::storage::Versioned;

const MAX_SERIALIZED_CONFIG_SIZE: usize =
	"pot3.chan=15;pot3.cc=127;pot3.triggers=0,1,2,3".len() * 4 + ";preset=3".len();
const MAX_SERIALIZED_PRESET_CONFIG_SIZE: usize =
	"pot3.chan=15;pot3.cc=127;pot3.triggers=0,1,2,3".len() * 4 + ";name=".len() + 32;

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
		[
			0x0b,                         // Header: Cable 0 + CIN 0x0B (Control Change)
			0xb0 | (self.channel & 0x0f), // Status Byte: 0xB0 (Control Change) + Channel (0-15)
			self.cc & 0x7f,               // CC Number (0-127)
			value & 0x7f,                 // CC Value (0-127)
		]
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
					self.active_preset_mut().pots[pot as usize].cc = change.value
				}
				ConfigKey::PotChan(pot) => {
					self.active_preset_mut().pots[pot as usize].channel = change.value
				}
				ConfigKey::PotTriggers(pot) => {
					self.active_preset_mut().pots[pot as usize].triggers = change.value
				}
				ConfigKey::Preset => self.active_preset = change.value,
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
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Preset {
	pub name: ArrayString<32>,
	pub pots: [PotConfig; 4],
}

impl Preset {
	pub const FACTORY: Self = Self {
		name: ArrayString::new_const(),
		pots: PotConfig::DEFAULT_POTS,
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
	const VERSION: u16 = 2;
}
