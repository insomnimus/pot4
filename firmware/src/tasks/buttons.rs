use embassy_stm32::{
	Peri,
	gpio::{
		Input,
		Pull,
	},
	peripherals::{
		PB0,
		PB1,
		PB2,
		PB3,
	},
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Sender,
};
use embassy_time::{
	Duration,
	Instant,
	Timer,
};

use crate::{
	Request,
	button::{
		Button,
		Click,
	},
	config::command::{
		Command,
		ConfigChange,
		ConfigKey,
	},
};

// Length of a tick.
const SAMPLE_PERIOD_MS: u32 = 1;
const SAMPLE_PERIOD: Duration = Duration::from_millis(SAMPLE_PERIOD_MS as u64);
const MULTIPRESS_TIMEOUT_TICKS: u32 = 300; // 300 ticks

pub struct ButtonPins {
	pub button0: Peri<'static, PB0>,
	pub button1: Peri<'static, PB1>,
	pub button2: Peri<'static, PB2>,
	pub button3: Peri<'static, PB3>,
}

#[embassy_executor::task]
pub async fn buttons_task(
	pins: ButtonPins,
	sender: Sender<'static, ThreadModeRawMutex, Request, 4>,
) {
	let mut button_pins = [
		Input::new(pins.button0, Pull::Up),
		Input::new(pins.button1, Pull::Up),
		Input::new(pins.button2, Pull::Up),
		Input::new(pins.button3, Pull::Up),
	];

	let mut buttons = [(); 4].map(|_| Button::new());

	let mut ticks = 0;
	loop {
		let start = Instant::now();
		let mut readings = [false; 4];
		for (pin, val) in button_pins.iter_mut().zip(&mut readings) {
			*val = pin.is_high();
		}

		for (i, (reading, button)) in readings.into_iter().zip(&mut buttons).enumerate() {
			if let Some(click) = button.update(reading, ticks, MULTIPRESS_TIMEOUT_TICKS) {
				match click {
					Click::Single | Click::Double | Click::Triple => {
						sender
							.send(Request::new(
								Command::SetConfig {
									changes: [ConfigChange {
										key: ConfigKey::Preset,
										value: i as u8,
									}]
									.into_iter()
									.collect(),
								},
								false,
							))
							.await;
					}
				}
			}
		}

		ticks += 1;
		let elapsed = start.elapsed();
		if elapsed < SAMPLE_PERIOD {
			Timer::after(SAMPLE_PERIOD - elapsed).await;
		}
	}
}
