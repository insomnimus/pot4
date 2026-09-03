use arrayvec::ArrayVec;
use pot_conditioner::PotConditioner;

pub struct Pot<const N: usize> {
	last_value: u8,
	buf: ArrayVec<i32, N>,
	tick: u32,
	conditioner: PotConditioner,
}

impl<const N: usize> Pot<N> {
	pub fn new(sampling_rate: i32, input_range: (i32, i32), movement_threshold: i32) -> Self {
		assert!(
			sampling_rate > N as i32,
			"sampling_rate must be greater than N"
		);
		assert_eq!(
			sampling_rate % N as i32,
			0,
			"sampling_rate must be divisible by N"
		);
		assert!(
			(0..=255).contains(&movement_threshold),
			"movement_threshold must be in the range 0-255"
		);

		let mut conditioner = PotConditioner::new(sampling_rate / N as i32, input_range, (0, 127));
		conditioner.set_movement_threshold(movement_threshold);

		Self {
			tick: 0,
			last_value: 0,
			buf: ArrayVec::new(),
			conditioner,
		}
	}

	pub fn _last_value(&self) -> u8 {
		self.last_value
	}

	pub fn update(&mut self, sample: i32) -> Option<u8> {
		if self.buf.try_push(sample).is_err() {
			let average = self.buf.iter().sum::<i32>() / N as i32;
			self.buf.clear();
			self.buf.push(sample);

			let value = self.conditioner.update(average, self.tick as _);
			self.tick += 1;

			if self.conditioner.moved() {
				self.last_value = value as u8;

				return Some(value as u8);
			}
		}

		None
	}
}
