use std::io::{
	self,
	Read,
	Write,
};

use log::*;
use nusb::{
	MaybeFuture,
	descriptors::TransferType,
	io::{
		EndpointRead,
		EndpointWrite,
	},
	list_devices,
};

const VID: u16 = 0x16C0;
const PID: u16 = 0x05E4;
const CONFIG_INTERFACE: u8 = 2;

pub struct ConfigDevice {
	writer: EndpointWrite<nusb::transfer::Bulk>,
	reader: EndpointRead<nusb::transfer::Bulk>,
}

impl ConfigDevice {
	pub fn open() -> io::Result<Self> {
		let device_info = list_devices()
			.wait()?
			.find(|device| {
				device.vendor_id() == VID
					&& device.product_id() == PID
					&& device
						.interfaces()
						.any(|interface| interface.interface_number() == CONFIG_INTERFACE)
			})
			.ok_or_else(|| {
				io::Error::new(
					io::ErrorKind::NotFound,
					"Pot4 MIDI CC Controller config interface not found",
				)
			})?;

		debug!("Opening device...");
		let device = device_info.open().wait()?;
		debug!("Claiming interface {}...", CONFIG_INTERFACE);
		let interface = device.claim_interface(CONFIG_INTERFACE).wait()?;
		debug!("Interface claimed");

		let descriptor = interface
			.descriptor()
			.ok_or_else(|| io::Error::other("Config interface has no descriptor"))?;
		trace!("Interface descriptor found");

		let mut bulk_in = None;
		let mut bulk_out = None;

		for endpoint in descriptor.endpoints() {
			debug!(
				"endpoint {:02x}: {:?}",
				endpoint.address(),
				endpoint.transfer_type()
			);

			if endpoint.transfer_type() != TransferType::Bulk {
				continue;
			}

			if endpoint.address() & 0x80 != 0 {
				bulk_in = Some(endpoint.address());
			} else {
				bulk_out = Some(endpoint.address());
			}
		}

		let bulk_in = bulk_in.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::NotFound,
				"Config interface has no bulk IN endpoint",
			)
		})?;

		let bulk_out = bulk_out.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::NotFound,
				"Config interface has no bulk OUT endpoint",
			)
		})?;

		let reader = interface
			.endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(bulk_in)?
			.reader(256);

		let writer = interface
			.endpoint::<nusb::transfer::Bulk, nusb::transfer::Out>(bulk_out)?
			.writer(256);

		Ok(Self { writer, reader })
	}

	pub fn write_message(&mut self, message: &str) -> io::Result<()> {
		self.writer.write_all(message.as_bytes())?;
		self.writer.write_all(b"\n")?;
		self.writer.flush()?;
		Ok(())
	}

	pub fn read_message(&mut self) -> io::Result<Vec<u8>> {
		let mut message = Vec::new();
		let mut byte = [0u8; 1];

		loop {
			self.reader.read_exact(&mut byte)?;

			if byte[0] == b'\n' {
				return Ok(message);
			}

			message.push(byte[0]);

			if message.len() > 256 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"Config message exceeds 256 bytes",
				));
			}
		}
	}

	pub fn read_ok(&mut self) -> io::Result<()> {
		let resp = self.read_message()?;
		if &resp == b"ok" {
			Ok(())
		} else {
			Err(io::Error::other(format!(
				"Got a non-ok response: {}",
				String::from_utf8_lossy_owned(resp)
			)))
		}
	}
}
