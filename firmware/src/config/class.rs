use arrayvec::ArrayVec;
use embassy_usb::{
	Builder,
	driver::{
		Driver,
		Endpoint,
		EndpointError,
		EndpointIn,
		EndpointOut,
	},
	msos::{
		CompatibleIdFeatureDescriptor,
		PropertyData,
		RegistryPropertyFeatureDescriptor,
	},
};

const CONFIG_GUID: &str = "{6b526a6d-af08-4470-b02e-6371a41de793}";
// Maximum USB packet size.
const MAX_PACKET_SIZE: usize = 64;
// Maximum protocol message size excluding \n.
pub const MAX_MESSAGE_SIZE: usize = 256;

#[derive(Debug)]
pub enum ConfigReceiverError {
	Endpoint(EndpointError),
	MessageTooLong,
}

impl From<EndpointError> for ConfigReceiverError {
	fn from(error: EndpointError) -> Self {
		Self::Endpoint(error)
	}
}

pub struct ConfigClass<'d, D: Driver<'d>> {
	receiver: D::EndpointOut,
	sender: D::EndpointIn,
}

pub struct ConfigSender<'d, D: Driver<'d>> {
	sender: D::EndpointIn,
}

pub struct ConfigReceiver<'d, D: Driver<'d>> {
	receiver: D::EndpointOut,

	packet: [u8; MAX_PACKET_SIZE],
	packet_len: usize,
	packet_pos: usize,

	message: ArrayVec<u8, MAX_MESSAGE_SIZE>,

	discarding: bool,
}

impl<'d, D: Driver<'d>> ConfigClass<'d, D> {
	pub fn new(builder: &mut Builder<'d, D>, max_packet_size: u16) -> Self {
		let mut function = builder.function(0xff, 0x00, 0x00);

		function.msos_feature(CompatibleIdFeatureDescriptor::new("WINUSB", ""));

		function.msos_feature(RegistryPropertyFeatureDescriptor::new(
			"DeviceInterfaceGUIDs",
			PropertyData::RegMultiSz(&[CONFIG_GUID]),
		));

		let mut interface = function.interface();
		let mut alt = interface.alt_setting(0xff, 0x00, 0x00, None);

		let sender = alt.endpoint_bulk_in(None, max_packet_size);
		let receiver = alt.endpoint_bulk_out(None, max_packet_size);

		Self { receiver, sender }
	}

	pub fn split(self) -> (ConfigSender<'d, D>, ConfigReceiver<'d, D>) {
		(
			ConfigSender {
				sender: self.sender,
			},
			ConfigReceiver {
				receiver: self.receiver,
				packet: [0; MAX_PACKET_SIZE],
				packet_len: 0,
				packet_pos: 0,
				message: ArrayVec::new(),
				discarding: false,
			},
		)
	}
}

impl<'d, D: Driver<'d>> ConfigSender<'d, D> {
	/// Sends a single protocol message.
	///
	/// # Panics
	/// Panics if `message.len() > MAX_MESSAGE_SIZE` or if the message contains `\n`.
	pub async fn write_message(&mut self, message: &[u8]) -> Result<(), EndpointError> {
		assert!(
			message.len() <= MAX_MESSAGE_SIZE,
			"config message exceeds MAX_MESSAGE_SIZE"
		);

		assert!(!message.contains(&b'\n'), "config message contains newline");

		let mut packet = [0u8; MAX_MESSAGE_SIZE + 1];
		packet[..message.len()].copy_from_slice(message);
		packet[message.len()] = b'\n';

		self.sender
			.write_transfer(&packet[..message.len() + 1], false)
			.await
	}

	pub async fn wait_connection(&mut self) {
		self.sender.wait_enabled().await;
	}
}

impl<'d, D: Driver<'d>> ConfigReceiver<'d, D> {
	/// Reads a single protocol message.
	pub async fn read_message(
		&mut self,
	) -> Result<ArrayVec<u8, MAX_MESSAGE_SIZE>, ConfigReceiverError> {
		loop {
			// Get another USB packet when we've consumed the current one.
			if self.packet_pos == self.packet_len {
				self.packet_len = self.receiver.read(&mut self.packet).await?;
				self.packet_pos = 0;
			}

			while self.packet_pos < self.packet_len {
				let byte = self.packet[self.packet_pos];
				self.packet_pos += 1;

				if self.discarding {
					if byte == b'\n' {
						self.discarding = false;
					}

					continue;
				}

				if byte == b'\n' {
					return Ok(core::mem::take(&mut self.message));
				}

				if self.message.len() == MAX_MESSAGE_SIZE {
					self.message.clear();
					self.discarding = true;

					return Err(ConfigReceiverError::MessageTooLong);
				}

				self.message.push(byte);
			}
		}
	}

	pub fn reset(&mut self) {
		self.packet_len = 0;
		self.packet_pos = 0;
		self.message.clear();
		self.discarding = false;
	}

	pub async fn wait_connection(&mut self) {
		self.receiver.wait_enabled().await;
	}
}
