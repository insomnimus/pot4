use defmt::*;
use embassy_stm32::{
	peripherals::USB,
	usb::Driver as UsbDriver,
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Sender,
};
use embassy_usb::driver::EndpointError;

use crate::{
	Response,
	config::{
		class::{
			ConfigReceiver,
			ConfigReceiverError,
		},
		command::Command,
	},
	send_request,
};

/// Reads from the USB config endpoint and generates either a Request to be handled elsewhere, or a Response.
#[embassy_executor::task]
pub async fn config_receiver_task(
	mut receiver: ConfigReceiver<'static, UsbDriver<'static, USB>>,
	resp_sender: Sender<'static, ThreadModeRawMutex, Response, 4>,
) {
	info!("Task config_receiver started");
	let mut active = false;

	loop {
		if !active {
			info!("config_receiver: waiting for a client");
			receiver.wait_connection().await;
			info!("config_receiver: client activated");
			active = true;
		}

		match receiver.read_message().await {
			Ok(msg) => match Command::parse(&msg) {
				Ok(command) => send_request(true, command).await,
				Err(e) => {
					defmt::error!("command parse error: {}", e);
					resp_sender.send(e.to_error_message()).await;
				}
			},
			Err(e) => match e {
				ConfigReceiverError::Endpoint(EndpointError::Disabled) => {
					active = false;
					info!("config_receiver: client disconnected");
					receiver.reset();
				}
				ConfigReceiverError::MessageTooLong => {
					resp_sender
						.send(b"error message too long".iter().copied().collect())
						.await;
				}
				ConfigReceiverError::Endpoint(e) => {
					defmt::error!("ConfigReceiverError::Endpoint: {}", e);
				}
			},
		}
	}
}
