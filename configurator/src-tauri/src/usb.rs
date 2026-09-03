use std::{
	io::{
		self,
		Read,
		Write,
	},
	time::Duration,
};

use nusb::{
	descriptors::TransferType,
	io::{
		EndpointRead,
		EndpointWrite,
	},
	list_devices,
	MaybeFuture,
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

		println!("Opening device...");
		let device = device_info.open().wait()?;
		println!("Claiming interface {}...", CONFIG_INTERFACE);
		let interface = device.claim_interface(CONFIG_INTERFACE).wait()?;
		println!("Interface claimed");

		let descriptor = interface
			.descriptor()
			.ok_or_else(|| io::Error::other("Config interface has no descriptor"))?;
		println!("Interface descriptor found");

		let mut bulk_in = None;
		let mut bulk_out = None;

		for endpoint in descriptor.endpoints() {
			println!(
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

		let mut reader = interface
			.endpoint::<nusb::transfer::Bulk, nusb::transfer::In>(bulk_in)?
			.reader(256);

		let mut writer = interface
			.endpoint::<nusb::transfer::Bulk, nusb::transfer::Out>(bulk_out)?
			.writer(256);

		reader.set_read_timeout(Duration::from_millis(250));
		writer.set_write_timeout(Duration::from_millis(250));

		Ok(Self { writer, reader })
	}

	pub fn write_message(&mut self, message: &str) -> io::Result<()> {
		self.writer.write_all(message.as_bytes())?;
		self.writer.write_all(b"\n")?;
		self.writer.flush()?;
		Ok(())
	}

	pub fn read_message(&mut self) -> io::Result<Vec<u8>> {
		let mut message = Vec::with_capacity(512);
		let mut byte = [0u8; 1];

		loop {
			self.reader.read_exact(&mut byte)?;

			if byte[0] == b'\n' {
				return Ok(message);
			}

			message.push(byte[0]);

			if message.len() > 1024 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"Config message exceeds 128 bytes",
				));
			}
		}
	}

	pub fn command(&mut self, command: &str) -> io::Result<String> {
		self.write_message(command)?;
		let reply = self.read_message()?;

		let reply =
			String::from_utf8(reply).map_err(|_| io::Error::other("Response is non UTF-8"))?;

		Ok(reply)
	}

	pub fn command_ok(&mut self, command: &str) -> Result<(), String> {
		let resp = self.command(command).map_err(|e| e.to_string())?;
		if &resp == "ok" {
			Ok(())
		} else {
			Err(resp)
		}
	}
}
