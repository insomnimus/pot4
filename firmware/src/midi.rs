pub fn cc(cc: u8, channel: u8, value: u8) -> [u8; 4] {
	[
		0x0b,                    // Header: Cable 0 + CIN 0x0B (Control Change)
		0xb0 | (channel & 0x0f), // Status Byte: 0xB0 (Control Change) + Channel (0-15)
		cc & 0x7f,               // CC Number (0-127)
		value & 0x7f,            // CC Value (0-127)
	]
}

pub fn note_on(note: u8, velocity: u8, channel: u8) -> [u8; 4] {
	[
		0x09,                    // Header: Cable 0 + CIN 0x09 (Note On)
		0x90 | (channel & 0x0f), // Status Byte: 0x90 (Note On) + Channel (0-15)
		note & 0x7f,             // Note Number (0-127)
		velocity & 0x7f,         // Velocity (0-127)
	]
}

pub fn note_off(note: u8, channel: u8) -> [u8; 4] {
	[
		0x08,                    // Header: Cable 0 + CIN 0x08 (Note Off)
		0x80 | (channel & 0x0f), // Status Byte: 0x80 (Note Off) + Channel (0-15)
		note & 0x7f,             // Note Number (0-127)
		0,                       // Velocity (0-127)
	]
}
