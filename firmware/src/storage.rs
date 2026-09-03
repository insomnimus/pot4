use core::marker::PhantomData;

use crc::{
	CRC_32_ISCSI,
	Crc,
};
use embassy_stm32::{
	Peri,
	flash::{
		Blocking,
		FLASH_SIZE,
		Flash,
		WRITE_SIZE,
	},
	peripherals::FLASH,
};
use postcard::from_bytes_crc32;
use serde::{
	Deserialize,
	Serialize,
};

const MAGIC: u32 = 0x504F_5434; // "POT4" in big endian
const FORMAT_VERSION: u16 = 1;

const PAGE_SIZE: usize = 2 << 10;
const SCRATCH_BUFFER_SIZE: usize = 1 << 10;

const HEADER_SIZE: usize = 8;
const CRC_SIZE: usize = 4;

const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

const PAGE_OFFSET: u32 = (FLASH_SIZE - PAGE_SIZE) as u32;

#[derive(Debug)]
pub enum Error {
	Flash(embassy_stm32::flash::Error),
	Postcard,
	InvalidRecord,
	RecordTooLarge,
	FlashFull,
}

impl defmt::Format for Error {
	fn format(&self, f: defmt::Formatter) {
		use Error::*;

		match self {
			Flash(e) => e.format(f),
			Postcard => defmt::write!(f, "PostCard de/serialization error"),
			InvalidRecord => defmt::write!(f, "Invalid record"),
			RecordTooLarge => defmt::write!(f, "Record too large"),
			FlashFull => defmt::write!(f, "Storage is full"),
		}
	}
}

impl From<embassy_stm32::flash::Error> for Error {
	fn from(err: embassy_stm32::flash::Error) -> Self {
		Self::Flash(err)
	}
}

impl From<postcard::Error> for Error {
	fn from(_err: postcard::Error) -> Self {
		Self::Postcard
	}
}

pub struct Storage<T> {
	flash: Flash<'static, Blocking>,
	buf: [u8; SCRATCH_BUFFER_SIZE],
	write_offset: usize,
	_marker: PhantomData<T>,
}

impl<T> Storage<T>
where
	T: Serialize + for<'de> Deserialize<'de>,
{
	pub fn init(flash: Peri<'static, FLASH>) -> Self {
		let mut storage = Self {
			flash: Flash::new_blocking(flash),
			buf: [0; SCRATCH_BUFFER_SIZE],
			write_offset: 0,
			_marker: PhantomData,
		};

		storage.write_offset = match storage.find_write_offset() {
			Ok(n) => n,
			Err(e) => {
				defmt::panic!("Storage: failed to locate the write offset: {}", e);
			}
		};

		storage
	}

	pub fn load(&mut self) -> Result<Option<T>, Error> {
		let mut offset = 0;
		let mut found = None;

		while offset + HEADER_SIZE <= PAGE_SIZE {
			let header = self.read_header(offset)?;

			if is_erased(&header) {
				break;
			}

			let (magic, version, length) = parse_header(&header);

			if magic != MAGIC {
				break;
			}

			let length = length as usize;
			if length < CRC_SIZE {
				break;
			}

			let record_size = aligned_record_size(length);
			if offset + record_size > PAGE_SIZE {
				break;
			}

			if version == FORMAT_VERSION {
				if length > SCRATCH_BUFFER_SIZE {
					return Err(Error::RecordTooLarge);
				}

				self.read_to_scratch(offset + HEADER_SIZE, length)?;
				if let Ok(value) = from_bytes_crc32::<T>(&self.buf[..length], CRC.digest()) {
					found = Some(value);
				}
			}

			offset += record_size;
		}

		Ok(found)
	}

	pub fn save(&mut self, val: &T) -> Result<(), Error> {
		// Serialize into the scratch buffer. The buffer contains both the postcard data and its CRC.
		let encoded = postcard::to_slice_crc32(val, &mut self.buf, CRC.digest())?;

		let length = encoded.len();
		if length < CRC_SIZE {
			return Err(Error::InvalidRecord);
		}

		let record_size = aligned_record_size(length);
		if record_size > PAGE_SIZE {
			return Err(Error::FlashFull);
		}

		// If there isn't enough room, erase the page and start over.
		if self.write_offset + record_size > PAGE_SIZE {
			self.flash
				.blocking_erase(PAGE_OFFSET, PAGE_OFFSET + PAGE_SIZE as u32)?;

			self.write_offset = 0;
		}

		let payload_offset = self.write_offset + HEADER_SIZE;

		// The flash programming operation requires WRITE_SIZE writes.
		// Padding bytes are irrelevant and remain erased (0xFF).
		//
		// Add padding.
		let padded_length = align_up(length, WRITE_SIZE);
		for byte in &mut self.buf[length..padded_length] {
			*byte = 0xFF;
		}

		self.write_from_scratch(payload_offset, padded_length)?;

		// Write the header only after the payload has been completely written. This acts as the record's commit marker.
		let mut header = [0xFF; HEADER_SIZE];

		header[0..4].copy_from_slice(&MAGIC.to_le_bytes());
		header[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
		header[6..8].copy_from_slice(&(length as u16).to_le_bytes());

		self.write(self.write_offset, &header)?;
		self.write_offset += record_size;

		Ok(())
	}

	fn find_write_offset(&mut self) -> Result<usize, Error> {
		let mut offset = 0;

		while offset + HEADER_SIZE <= PAGE_SIZE {
			let header = self.read_header(offset)?;
			if is_erased(&header) {
				return Ok(offset);
			}

			let (magic, _version, length) = parse_header(&header);

			if magic != MAGIC {
				return Ok(offset);
			}

			let length = length as usize;
			if length < CRC_SIZE {
				return Ok(offset);
			}

			let record_size = aligned_record_size(length);

			if offset + record_size > PAGE_SIZE {
				return Ok(offset);
			}

			offset += HEADER_SIZE + record_size;
		}

		Ok(PAGE_SIZE)
	}

	fn read_header(&mut self, offset: usize) -> Result<[u8; HEADER_SIZE], Error> {
		let mut header = [0u8; HEADER_SIZE];

		self.flash
			.blocking_read(PAGE_OFFSET + offset as u32, &mut header)?;

		Ok(header)
	}

	fn read_to_scratch(&mut self, offset: usize, length: usize) -> Result<(), Error> {
		self.flash
			.blocking_read(PAGE_OFFSET + offset as u32, &mut self.buf[..length])?;

		Ok(())
	}

	fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), Error> {
		assert_eq!(offset % WRITE_SIZE, 0);
		assert_eq!(data.len() % WRITE_SIZE, 0);

		self.flash
			.blocking_write(PAGE_OFFSET + offset as u32, data)?;

		Ok(())
	}

	fn write_from_scratch(&mut self, offset: usize, length: usize) -> Result<(), Error> {
		assert_eq!(offset % WRITE_SIZE, 0);
		assert_eq!(length % WRITE_SIZE, 0);

		self.flash
			.blocking_write(PAGE_OFFSET + offset as u32, &self.buf[..length])?;

		Ok(())
	}
}

fn parse_header(header: &[u8; HEADER_SIZE]) -> (u32, u16, u16) {
	let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
	let version = u16::from_le_bytes([header[4], header[5]]);
	let length = u16::from_le_bytes([header[6], header[7]]);

	(magic, version, length)
}

const fn align_up(value: usize, alignment: usize) -> usize {
	(value + alignment - 1) & !(alignment - 1)
}

const fn aligned_record_size(payload_length: usize) -> usize {
	HEADER_SIZE + align_up(payload_length, WRITE_SIZE)
}

fn is_erased(data: &[u8]) -> bool {
	data.iter().all(|&byte| byte == 0xFF)
}
