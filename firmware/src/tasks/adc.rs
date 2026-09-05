use defmt::*;
use embassy_stm32::{
	Peri,
	adc::{
		Adc,
		SampleTime,
	},
	peripherals::{
		ADC1,
		PA0,
		PA1,
		PA2,
		PA3,
		USB,
	},
	usb::Driver as UsbDriver,
};
use embassy_time::{
	Duration,
	Instant,
	Timer,
};
use embassy_usb::{
	class::midi::Sender as MidiSender,
	driver::EndpointError,
};

use crate::{
	MutexedConfig,
	pot::Pot,
};

const SAMPLING_RATE: i32 = 2000;
const ADC_AVERAGE_WINDOW: usize = 8;
const SAMPLE_PERIOD_US: u64 = 1_000_000 / SAMPLING_RATE as u64;
const ADC_INPUT_RANGE: (i32, i32) = (10, 4085);
const MOVEMENT_THRESHOLD: i32 = 50;

#[allow(nonstandard_style)]
pub struct AdcPins {
	pub PA0: Peri<'static, PA0>,
	pub PA1: Peri<'static, PA1>,
	pub PA2: Peri<'static, PA2>,
	pub PA3: Peri<'static, PA3>,
}

#[embassy_executor::task]
pub async fn adc_task(
	device_config: &'static MutexedConfig,
	mut adc: Adc<'static, ADC1>,
	mut midi_sender: MidiSender<'static, UsbDriver<'static, USB>>,
	mut p: AdcPins,
) {
	let mut pots = [(); 4].map(|_| init_pot());

	info!("Task adc started");
	let mut active = false;

	loop {
		if !active {
			info!("adc: waiting for activation");
			pots.fill_with(init_pot);
			midi_sender.wait_connection().await;
			info!("adc: activated");
			active = true;
		}

		let start = Instant::now();

		let pot_samples = [
			adc.read(&mut p.PA0, SampleTime::CYCLES61_5).await,
			adc.read(&mut p.PA1, SampleTime::CYCLES61_5).await,
			adc.read(&mut p.PA2, SampleTime::CYCLES61_5).await,
			adc.read(&mut p.PA3, SampleTime::CYCLES61_5).await,
		];

		let pot_configs = device_config.lock().await.active_preset().pots;
		for (sample, (pot, pot_config)) in pot_samples
			.into_iter()
			.zip(pots.iter_mut().zip(&pot_configs))
		{
			if let Some(value) = pot.update(sample as i32) {
				for packet in pot_config.create_cc_packets(&pot_configs, value) {
					if let Err(e) = midi_sender.write_packet(&packet).await {
						match e {
							EndpointError::Disabled => {
								active = false;
								info!("adc: disabled");
							}
							_ => defmt::error!("midi write error: {}", e),
						}
					}
				}
			}
		}

		let elapsed = start.elapsed();
		let period = Duration::from_micros(SAMPLE_PERIOD_US);

		if elapsed < period {
			Timer::after(period - elapsed).await;
		}
	}
}

fn init_pot() -> Pot<ADC_AVERAGE_WINDOW> {
	Pot::<ADC_AVERAGE_WINDOW>::new(SAMPLING_RATE, ADC_INPUT_RANGE, MOVEMENT_THRESHOLD)
}
