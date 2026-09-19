use arrayvec::ArrayVec;
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
use embassy_time::{
	Duration,
	Instant,
	Timer,
};
use static_cell::StaticCell;

use crate::{
	MutexedConfig,
	button::{
		Button,
		Event,
	},
	config::{
		ButtonAction,
		command::{
			Command,
			ConfigChange,
			ConfigKey,
			ConfigValue,
		},
	},
	delay::DelayLine,
	midi,
	send_midi_packet,
	send_request,
};

// Length of a tick.
const SAMPLE_PERIOD_MS: u32 = 1;
const SAMPLE_PERIOD: Duration = Duration::from_millis(SAMPLE_PERIOD_MS as u64);

const DELAY_LENGTH: usize = 50;
static DELAYED_PACKETS: StaticCell<DelayLine<ArrayVec<[u8; 4], 4>, DELAY_LENGTH>> =
	StaticCell::new();

pub struct ButtonPins {
	pub button0: Peri<'static, PB0>,
	pub button1: Peri<'static, PB1>,
	pub button2: Peri<'static, PB2>,
	pub button3: Peri<'static, PB3>,
}

#[embassy_executor::task]
pub async fn buttons_task(device_config: &'static MutexedConfig, pins: ButtonPins) {
	let delayed_packets = DELAYED_PACKETS.init(DelayLine::new(
		[(); DELAY_LENGTH].map(|_| ArrayVec::new_const()),
	));

	let mut button_pins = [
		Input::new(pins.button0, Pull::Up),
		Input::new(pins.button1, Pull::Up),
		Input::new(pins.button2, Pull::Up),
		Input::new(pins.button3, Pull::Up),
	];

	let mut buttons = [(); 4].map(|_| Button::new());
	let mut ignore_hold_releases = [false; 4];

	let mut ticks = 0;
	loop {
		let start = Instant::now();

		let button_configs = device_config.lock().await.active_preset().buttons;

		// Send queued packets from the last iteration.
		for &packet in &delayed_packets.push(ArrayVec::new_const()) {
			send_midi_packet(packet).await;
		}
		let delayed_packets = delayed_packets.newest_mut();

		let mut readings = [false; 4];
		for (pin, val) in button_pins.iter_mut().zip(&mut readings) {
			*val = pin.is_high();
		}

		for (reading, (ignore_hold_release, (button, button_config))) in readings.into_iter().zip(
			ignore_hold_releases
				.iter_mut()
				.zip(buttons.iter_mut().zip(&button_configs)),
		) {
			let Some(click) = button.update(
				!reading,
				ticks,
				button_config.click_time as _,
				button_config.hold_time as _,
			) else {
				continue;
			};

			match click {
				Event::Hold => {
					match button_config.hold {
						ButtonAction::None => (),

						ButtonAction::NextPreset => {
							send_request(false, Command::NextPreset).await;
							*ignore_hold_release = true;
						}
						ButtonAction::PreviousPreset => {
							send_request(false, Command::PreviousPreset).await;
							*ignore_hold_release = true;
						}
						ButtonAction::Preset { preset } => {
							send_request(
								false,
								Command::SetConfig {
									changes: ArrayVec::from_iter([ConfigChange {
										key: ConfigKey::Preset,
										value: ConfigValue::U8(preset),
									}]),
								},
							)
							.await;
							*ignore_hold_release = true;
						}

						ButtonAction::Cc { cc, channel } => {
							// Start holding, we'll put the CC off at a release event.
							send_midi_packet(midi::cc(cc, channel, 127)).await;
						}

						ButtonAction::Note { note, channel } => {
							// Start holding, we'll put the note off at a release event.
							send_midi_packet(midi::note_on(note, 100, channel)).await;
						}
					}
				}

				// Hold released
				Event::Released { count: 0 } if *ignore_hold_release => {
					*ignore_hold_release = false;
				}
				Event::Released { count: 0 } => match button_config.hold {
					ButtonAction::Cc { cc, channel } => {
						send_midi_packet(midi::cc(cc, channel, 0)).await;
					}

					ButtonAction::Note { note, channel } => {
						send_midi_packet(midi::note_off(note, channel)).await;
					}

					ButtonAction::None
					| ButtonAction::NextPreset
					| ButtonAction::PreviousPreset
					| ButtonAction::Preset { .. } => (),
				},

				// Single, double or triple click
				Event::Released { count } => {
					match button_config.clicks[count as usize - 1] {
						ButtonAction::None => (),

						ButtonAction::NextPreset => {
							send_request(false, Command::NextPreset).await;
						}
						ButtonAction::PreviousPreset => {
							send_request(false, Command::PreviousPreset).await;
						}
						ButtonAction::Preset { preset } => {
							send_request(
								false,
								Command::SetConfig {
									changes: ArrayVec::from_iter([ConfigChange {
										key: ConfigKey::Preset,
										value: ConfigValue::U8(preset),
									}]),
								},
							)
							.await;
						}

						ButtonAction::Cc { channel, cc } => {
							// Turn the CC off after a while.
							delayed_packets.push(midi::cc(cc, channel, 0));

							send_midi_packet(midi::cc(cc, channel, 127)).await;
						}

						ButtonAction::Note { note, channel } => {
							// Turn the note off later.
							delayed_packets.push(midi::note_off(note, channel));

							send_midi_packet(midi::note_on(note, 100, channel)).await;
						}
					}
				}

				Event::Pressed => (),
			}
		}

		ticks += 1;
		let elapsed = start.elapsed();
		if elapsed < SAMPLE_PERIOD {
			Timer::after(SAMPLE_PERIOD - elapsed).await;
		}
	}
}
