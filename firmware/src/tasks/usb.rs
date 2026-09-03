use embassy_stm32::usb::Driver as UsbDriver;

#[embassy_executor::task]
pub async fn usb_task(
	mut usb: embassy_usb::UsbDevice<'static, UsbDriver<'static, embassy_stm32::peripherals::USB>>,
) {
	defmt::info!("Task usb started");
	usb.run().await;
}
