#[derive(Debug, Clone, Copy)]
pub enum Click {
	Single,
	Double,
	Triple,
}

pub struct Button {
	debouncer: Debouncer,

	press_count: u8,
	last_release: Option<u32>,
	last_pressed: bool,
}

impl Button {
	pub fn new() -> Self {
		Self {
			debouncer: Debouncer::new(),
			press_count: 0,
			last_release: None,
			last_pressed: false,
		}
	}

	/// Updates the button state.
	///
	/// `ticks` is the current timestamp.
	/// `pressed` is the current physical button state.
	///
	/// Returns a click once a single/double/triple click has been determined.
	pub fn update(&mut self, pressed: bool, ticks: u32, timeout: u32) -> Option<Click> {
		let pressed = self.debouncer.update(pressed);
		// Detect the pressed -> released transition.
		let released = self.last_pressed && !pressed;

		self.last_pressed = pressed;

		if released {
			// The button was just released.
			self.handle_release(ticks, timeout)
		} else {
			// The button wasn't released at this moment.
			self.handle_timeout(ticks, timeout)
		}
	}

	fn handle_release(&mut self, ticks: u32, timeout: u32) -> Option<Click> {
		let within_window = self
			.last_release
			.is_some_and(|last| ticks.wrapping_sub(last) <= timeout);

		if within_window {
			// This release belongs to a multi-click sequence.
			self.press_count += 1;
		} else {
			// This release is possibly the first among a multi-click sequence, or it could just be a single click, but we can't know it yet.
			self.press_count = 1;
		}

		self.last_release = Some(ticks);

		// Triple click can be handled since we don't accumulate for more than that.
		if self.press_count == 3 {
			self.reset();
			return Some(Click::Triple);
		}

		None
	}

	fn handle_timeout(&mut self, ticks: u32, timeout: u32) -> Option<Click> {
		let last_release = self.last_release?;

		if ticks.wrapping_sub(last_release) <= timeout {
			// There's still time to make a decision.
			return None;
		}
		// It's decision time.

		let click = match self.press_count {
			1 => Click::Single,
			2 => Click::Double,
			// 0 => return None, // Impossible
			_ => unreachable!(),
		};

		self.reset();

		Some(click)
	}

	fn reset(&mut self) {
		self.press_count = 0;
		self.last_release = None;
	}
}

const MASK: u8 = 0b11000111;
/// A history debouncer adapted from https://hackaday.com/2015/12/10/embed-with-elliot-debounce-your-noisy-buttons-part-ii
struct Debouncer {
	history: u8,
}

impl Debouncer {
	pub fn new() -> Self {
		Self { history: 0 }
	}

	/// Feed a reading to the Debouncer.
	///
	/// Returns the current debounced logical state: `true` for "button's pressed down", `false` otherwise.
	/// Should be called every tick.
	pub fn update(&mut self, pressed: bool) -> bool {
		self.history <<= 1;
		self.history |= pressed as u8;

		// is_transitioning_down detects transitions, it'll return `false` while holding down (when history's all 1s for example).
		// is_transitioning_down sets self.history to all 1s if it sees a transition, so the next time we call it and it returns false, is_down may still return true.
		self.is_transitioning_down() || self.is_down()
	}

	fn is_down(&self) -> bool {
		self.history == 0b11111111
	}

	fn _is_up(&self) -> bool {
		self.history == 0b00000000
	}

	fn is_transitioning_down(&mut self) -> bool {
		if (self.history & MASK) == 0b00000111 {
			self.history = 0b11111111;
			true
		} else {
			false
		}
	}
}
