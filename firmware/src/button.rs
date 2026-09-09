#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
	Pressed,
	Released { count: u8 },
	Hold,
}

pub struct Button {
	debouncer: Debouncer,
	press_count: u8,
	last_release: Option<u32>,
	press_start: Option<u32>,
	last_pressed: bool,
	hold_triggered: bool,
	sequence_active: bool,
}

impl Default for Button {
	fn default() -> Self {
		Self::new()
	}
}

impl Button {
	pub fn new() -> Self {
		Self {
			debouncer: Debouncer::new(),
			press_count: 0,
			last_release: None,
			press_start: None,
			last_pressed: false,
			hold_triggered: false,
			sequence_active: false,
		}
	}

	/// Updates button state, returning an [Event] if there's one.
	pub fn update(
		&mut self,
		pressed: bool,
		ticks: u32,
		timeout: u32,
		hold_timeout: u32,
	) -> Option<Event> {
		let pressed = self.debouncer.update(pressed);

		let just_pressed = !self.last_pressed && pressed;
		let just_released = self.last_pressed && !pressed;

		self.last_pressed = pressed;

		// Physical press
		if just_pressed {
			self.press_start = Some(ticks);
			self.hold_triggered = false;

			if !self.sequence_active {
				// First press of a new gesture sequence
				self.sequence_active = true;
				return Some(Event::Pressed);
			}
			// Subsequent presses in the same multi-click sequence emit no Pressed event
			return None;
		}

		// Physical release
		if just_released {
			self.press_start = None;

			if self.hold_triggered {
				self.reset();
				return Some(Event::Released { count: 0 });
			}

			return self.handle_release(ticks, timeout);
		}

		// Active hold
		if pressed
			&& !self.hold_triggered
			&& let Some(start) = self.press_start
			&& ticks.wrapping_sub(start) >= hold_timeout
		{
			self.hold_triggered = true;
			return Some(Event::Hold);
		}

		// Multi-click decision timeout (when the key's physically up)
		if !pressed {
			return self.handle_timeout(ticks, timeout);
		}

		None
	}

	fn handle_release(&mut self, ticks: u32, timeout: u32) -> Option<Event> {
		let within_window = self
			.last_release
			.is_some_and(|last| ticks.wrapping_sub(last) <= timeout);

		if within_window {
			self.press_count += 1;
		} else {
			self.press_count = 1;
		}

		self.last_release = Some(ticks);

		// We track at most 3 presses.
		if self.press_count == 3 {
			let count = self.press_count;
			self.reset();
			return Some(Event::Released { count });
		}

		None
	}

	fn handle_timeout(&mut self, ticks: u32, timeout: u32) -> Option<Event> {
		let last_release = self.last_release?;

		if ticks.wrapping_sub(last_release) <= timeout {
			return None;
		}

		let count = self.press_count;
		self.reset();

		if count > 0 {
			Some(Event::Released { count })
		} else {
			None
		}
	}

	fn reset(&mut self) {
		self.press_count = 0;
		self.last_release = None;
		self.press_start = None;
		self.hold_triggered = false;
		self.sequence_active = false;
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

	pub fn update(&mut self, pressed: bool) -> bool {
		self.history <<= 1;
		self.history |= pressed as u8;

		self.is_transitioning_down() || self.is_down()
	}

	fn is_down(&self) -> bool {
		self.history == 0b11111111
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
