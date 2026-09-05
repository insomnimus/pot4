# Pot4: MIDI CC Controller
This repository hosts the firmware and configuration clients for the Pot4 MIDI CC Controller.

## The Hardware
The device is a MIDI controller with 4 knobs and 4 buttons. For the microcontroller, an STM32-F3 Discovery board is used.

### Wiring
- Pots 1-4 (10k each):
	* Pot pin 1 -> GND
	* Pot pin 2 -> PA0-PA3
	* Pot pin 2 -> 100nF capacitor -> GND
	* Pot pin 3 -> 3.3V power
- Buttons 1-4:
	* One end to GND
	* Other end to PB0-PB3

Powered over the user USB port on the board.

### Knobs and Buttons
The 4 knobs are assigned to a MIDI channel and CC number each. Turning a knob sends the relevant MIDI CC event over the USB MIDI connection.

The buttons select presets, described later in this document.

## The Firmware
The firmware is written in Rust using the Embassy ecosystem.

It enumerates as a USB composite device:
- An interface for USB MIDI
- An interface for configuration (MSOS descriptor set to use winusb)

Configuration is handled in the firmware, and the firmware makes a distinction between saved / unsaved (ephemeral) configuration.
Saved configuration is written to the last page of the flash / bank1 with wear-leveling, so as to extend the lifespan of the device.

## Configuration

There are 2 config clients; one is a GUI app, and the other a CLI app.
The CLI configurator (located in `cli/`) exposes a basic shell to send commands and read output.
The GUI configurator (located in `configurator/`) is currently work in progress, but it will eventually support every feature the device offers.

### Config sources
There are 2 sources of configuration:
- Live config is device's currently operating configuration. Unless saved, the values will be forgotten after a reboot.
- Persistent config is configuration that's saved to persistent storage on the device. Each time the device boots, the last saved configuration is loaded from flash.

### Config: Active Preset
A number ranging from 0 to 3, this stores the active preset.

### Config: Presets
There are 4 preset slots. Each slot has a name (maximum 32 bytes), and configuration for each knob (CC, midi channel, and other pots it triggers).

## The Configuration Wire Format
The configuration endpoint uses a UTF-8 text based wire format akin to a shell.

Messages are delimited by a newline (`\n`), and there are no escape sequences.

Each request is in the form `<command> [arguments]`.

Responses can vary.
- `ok`: Returned in response to most commands.
- `error <error-message>`: Returned in response to any command, if there was an error related to the command.
- In other cases, the requested data is returned.


### Commands
#### `ping`
If the wire communication's functioning, a `pong` response is returned.

#### `factory-reset`
Resets the device to its factory settings.

There's no confirmation or a way to undo this command.

examples
```
> factory-reset
ok
```

#### `config.get`
Without any arguments, the active preset, and its pot configuration is returned.

Optional arguments:
- `saved`: Returns the last saved configuration.
- `preset`: Returns the active preset only.
- `saved.preset`: Returns the active preset of the saved configuration.

Examples
```
> config.get
preset=0;pot0.cc=9;pot0.chan=0;pot0.triggers=0;pot1.cc=10;pot1.chan=0;pot1.triggers=1;pot2.cc=11;pot2.chan=0;pot2.triggers=2;pot3.cc=12;pot3.chan=0;pot3.triggers=3
> config.get preset
preset=0
> config.get saved
preset=3;pot0.cc=9;pot0.chan=0;pot0.triggers=0;pot1.cc=10;pot1.chan=0;pot1.triggers=1;pot2.cc=11;pot2.chan=0;pot2.triggers=2;pot3.cc=12;pot3.chan=0;pot3.triggers=3
> config.get saved.preset
preset=3
```

#### `config.set`
Sets up to 13 config values.
The settings will be applied to whichever preset's active.

The syntax is the same as the output of `config.get`.

Examples
```
> config.set pot0.cc=3
ok
> config.set pot0.triggers=0,2
ok
> config.set preset=2;pot0.chan=12
ok
```

#### `config.reset`
Discards changes and loads the last saved configuration.

Examples
```
> config.reset
ok
```

#### `config.save`
Saves the live configuration to flash memory.

Examples
```
> config.save
ok
```

#### `preset.get`
Returns configuration of a specified preset.

Arguments:
- `<preset-index>`: Index of the preset. Values are read from the live configuration.
- `saved.<preset-index>`: Values are read from the last saved configuration.

Examples
```
> preset.get 0
name=Preset 1;preset=0;pot0.cc=9;pot0.chan=0;pot0.triggers=0;pot1.cc=10;pot1.chan=0;pot1.triggers=1;pot2.cc=11;pot2.chan=0;pot2.triggers=2;pot3.cc=12;pot3.chan=0;pot3.triggers=3
> preset.get saved.0
name=Preset 1;preset=0;pot0.cc=9;pot0.chan=0;pot0.triggers=0;pot1.cc=10;pot1.chan=0;pot1.triggers=1;pot2.cc=11;pot2.chan=0;pot2.triggers=2;pot3.cc=12;pot3.chan=0;pot3.triggers=3
```

#### `preset.set`
Sets up to 13 config values.
The settings will be applied to the specified preset.

Arguments:
- `<preset-index>`: The preset to change.
- `<KEY1=VALUE1;KEY2=VALUE2...>`: Up to 9 `key=value` pairs separated by semicolons. The syntax and keys are the same as the output of `preset.get`.

Examples
```
> preset.set 0 name=DAW Controls
ok
> preset.set 2 pot0.cc=9;pot0.chan=0;pot3.cc=3;pot2.triggers=1,2
ok
```

## Building The Project
For all 3 sub-projects, you need a Rust toolchain, and for the firmware specifically, you need to install the `thumbv7em-none-eabihf` target:
`rustup target add thumbv7em-none-eabihf`.

`cli` can be built simply with:
```shell
cd cli/
cargo build --release
# the binary will be in target/release/pot4 (with a .exe extension on windows)
```

The firmware can be built, but requires probe-rs to flash.
Connect the ST-Link end of the STM32-F3 Discovery board via a USB cable, and run:
```shell
# For other installation options visit https://probe.rs/docs/getting-started/installation/
cargo install probe-rs-tools --locked
cd firmware/
cargo embed --release # This will put you in an RTT session
# To exit the RTT session, press ctrl-c. The firmware is flashed.
```

After flashing the firmware, you can connect it with the user USB port.
To test it, connect it via the user USB port, and run the cli:
```shell
pot4 --interactive
# Or for more verbose output:
# pot4 -vv --interactive
```

It should put you in a config shell.
