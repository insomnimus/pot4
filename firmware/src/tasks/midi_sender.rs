use defmt::*;
use embassy_futures::select::{
	Either,
	select,
};
use embassy_stm32::{
	peripherals::USB,
	usb::Driver as UsbDriver,
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Receiver,
};
use embassy_usb::{
	class::midi::Sender as MidiSender,
	driver::EndpointError,
};

#[embassy_executor::task]
pub async fn midi_sender_task(
	mut midi_sender: MidiSender<'static, UsbDriver<'static, USB>>,
	packet_receiver: Receiver<'static, ThreadModeRawMutex, [u8; 4], 4>,
) {
	info!("Task midi_sender started");
	let mut active = false;

	loop {
		if !active {
			info!("midi: waiting for activation");
			loop {
				match select(midi_sender.wait_connection(), packet_receiver.receive()).await {
					// No packet was produced before connection
					Either::First(..) => break,
					// A packet was produced, but connection's not active yet.
					Either::Second(..) => (), // Discard packet and keep waiting
				}
			}

			info!("midi: activated");
			active = true;
		}

		let packet = packet_receiver.receive().await;
		match midi_sender.write_packet(&packet).await {
			Ok(_) => (),
			Err(EndpointError::Disabled) => {
				active = false;
				info!("midi: disabled");
			}
			Err(e) => defmt::error!("midi write error: {}", e),
		}
	}
}
