use embassy_futures::select::{
	Either,
	select,
};
use embassy_stm32::{
	peripherals::TIM3,
	time::Hertz,
	timer::{
		Channel,
		simple_pwm::SimplePwm,
	},
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Receiver,
};
use embassy_time::{
	Duration,
	Timer,
};
use embedded_hal_02::Pwm;

pub struct Beep {
	pub fq: u16,
	pub duration_ms: u16,
}

#[embassy_executor::task]
pub async fn beep_task(
	mut pwm: SimplePwm<'static, TIM3>,
	receiver: Receiver<'static, ThreadModeRawMutex, Beep, 2>,
) {
	let channel = Channel::Ch1;

	pwm.set_duty(channel, 0);
	pwm.disable(channel);

	loop {
		let mut beep = receiver.receive().await;

		pwm.set_frequency(Hertz(beep.fq as u32));
		pwm.set_duty(channel, pwm.max_duty_cycle() / 2);
		pwm.enable(channel);

		loop {
			match select(
				receiver.receive(),
				Timer::after(Duration::from_millis(beep.duration_ms as u64)),
			)
			.await
			{
				Either::First(new_beep) => {
					// Interrupted
					beep = new_beep;

					pwm.set_frequency(Hertz(beep.fq as u32));
					pwm.set_duty(channel, pwm.max_duty_cycle() / 2);
				}

				Either::Second(()) => {
					pwm.disable(channel);
					break;
				}
			}
		}
	}
}
