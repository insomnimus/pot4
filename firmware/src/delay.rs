use core::mem;

pub struct DelayLine<T, const N: usize> {
	buffer: [T; N],
	head: usize,
}

impl<T, const N: usize> DelayLine<T, N> {
	pub const fn new(initial: [T; N]) -> Self {
		Self {
			buffer: initial,
			head: 0,
		}
	}

	pub fn push(&mut self, item: T) -> T {
		let old = mem::replace(&mut self.buffer[self.head], item);

		self.head += 1;
		if self.head >= N {
			self.head = 0;
		}

		old
	}

	pub const fn newest_mut(&mut self) -> &mut T {
		let index = if self.head == 0 { N - 1 } else { self.head - 1 };
		&mut self.buffer[index]
	}
}
