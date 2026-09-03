use defmt::*;
use embassy_stm32::{
	peripherals::USB,
	usb::Driver as UsbDriver,
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Receiver,
};
use embassy_usb::driver::EndpointError;

use crate::{
	Response,
	config::class::ConfigSender,
};

#[embassy_executor::task]
pub async fn config_sender_task(
	mut sender: ConfigSender<'static, UsbDriver<'static, USB>>,
	receiver: Receiver<'static, ThreadModeRawMutex, Response, 4>,
) {
	info!("Task config_sender started");
	let mut active = false;

	loop {
		if !active {
			info!("config_sender: waiting for a client");
			sender.wait_connection().await;
			info!("config_sender: client activated");
			active = true;
		}

		let message = receiver.receive().await;

		match sender.write_message(&message).await {
			Ok(()) => {}

			Err(EndpointError::Disabled) => {
				active = false;
				info!("config_sender: client disconnected");
			}

			Err(e) => {
				defmt::error!("config send error: {:?}", e);
			}
		}
	}
}
