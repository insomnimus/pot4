#![no_std]
#![no_main]

mod button;
mod config;
mod pot;
mod storage;
mod tasks;

use arrayvec::ArrayVec;
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
	Config,
	adc::Adc,
	bind_interrupts,
	rcc::{
		AHBPrescaler,
		APBPrescaler,
		AdcClockSource,
		AdcPllPrescaler,
		Hse,
		HseMode,
		Pll,
		PllMul,
		PllPreDiv,
		PllSource,
		Sysclk,
	},
	time::Hertz,
	usb,
};
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::Channel,
	mutex::Mutex,
};
use embassy_usb::{
	Builder,
	Config as UsbConfig,
	class::midi::MidiClass,
};
use panic_probe as _;
use static_cell::StaticCell;

use self::{
	config::{
		DeviceConfig,
		class::{
			ConfigClass,
			MAX_MESSAGE_SIZE,
		},
	},
	storage::Storage,
};
use crate::config::command::Command;

bind_interrupts!(struct Irqs {
	ADC1_2 => embassy_stm32::adc::InterruptHandler<embassy_stm32::peripherals::ADC1>;
	USB_LP_CAN_RX0 => usb::InterruptHandler<embassy_stm32::peripherals::USB>;
});

pub type Response = ArrayVec<u8, MAX_MESSAGE_SIZE>;

pub struct Request {
	/// The command: most of the time comes from the configurator client.
	pub cmd: Command,
	/// `true` if the request originates from a client.
	pub is_external: bool,
}

impl Request {
	pub fn new(cmd: Command, is_external: bool) -> Self {
		Self { cmd, is_external }
	}
}

const VID: u16 = 0x16C0;
const PID: u16 = 0x05E4;
const MANUFACTURER: &str = "Insomnia";
const PRODUCT: &str = "Pot4 MIDI CC Controller";
const SERIAL: &str = "00001";

type MutexedConfig = Mutex<ThreadModeRawMutex, DeviceConfig>;
static DEVICE_CONFIG: StaticCell<MutexedConfig> = StaticCell::new();

// USB buffers
static CONFIG_DESCRIPTOR: StaticCell<[u8; 512]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static MSOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();

static REQUEST_CHANNEL: Channel<ThreadModeRawMutex, Request, 4> = Channel::new();
static RESPONSE_CHANNEL: Channel<ThreadModeRawMutex, Response, 4> = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
	let device_config = DEVICE_CONFIG.init(Mutex::new(DeviceConfig::FACTORY));

	let mut config = Config::default();

	config.rcc.hse = Some(Hse {
		freq: Hertz::mhz(8),
		mode: HseMode::Bypass,
	});

	config.rcc.pll = Some(Pll {
		src: PllSource::HSE,
		prediv: PllPreDiv::DIV1,
		mul: PllMul::MUL6,
	});

	config.rcc.sys = Sysclk::PLL1_P;

	config.rcc.ahb_pre = AHBPrescaler::DIV1;
	config.rcc.apb1_pre = APBPrescaler::DIV2;
	config.rcc.apb2_pre = APBPrescaler::DIV1;

	config.rcc.adc = AdcClockSource::Pll(AdcPllPrescaler::DIV1);

	let p = embassy_stm32::init(config);
	info!("Started");

	let adc = Adc::new(p.ADC1, Irqs);
	info!("ADC initialized");

	// Initialize USB buffers
	let config_descriptor = CONFIG_DESCRIPTOR.init([0; 512]);
	let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
	let control_buf = CONTROL_BUF.init([0; 64]);
	let msos_descriptor = MSOS_DESCRIPTOR.init([0; 256]);

	let mut usb_config = UsbConfig::new(VID, PID);
	usb_config.manufacturer = Some(MANUFACTURER);
	usb_config.product = Some(PRODUCT);
	usb_config.serial_number = Some(SERIAL);
	usb_config.max_power = 100;
	usb_config.max_packet_size_0 = 64;

	usb_config.device_class = 0xEF; // Miscellaneous Device Class
	usb_config.device_sub_class = 0x02; // Common Class
	usb_config.device_protocol = 0x01; // Interface Association Descriptor

	// Disable automatic IAD generation for builder.function() calls
	usb_config.composite_with_iads = false;

	let driver = usb::Driver::new(p.USB, Irqs, p.PA12, p.PA11);

	let mut builder = Builder::new(
		driver,
		usb_config,
		config_descriptor,
		bos_descriptor,
		msos_descriptor,
		control_buf,
	);

	builder.msos_descriptor(0x06030000, 0x20);

	let midi = MidiClass::new(
		&mut builder,
		1,  // Input count
		0,  // Output count
		64, // Max packet size
	);
	let (midi_sender, _) = midi.split();

	let config_class = ConfigClass::new(&mut builder, 64);
	let (config_sender, config_receiver) = config_class.split();

	let usb = builder.build();
	info!("USB initialized");

	let storage = Storage::init(p.FLASH);

	spawner.spawn(tasks::usb::usb_task(usb).unwrap());
	spawner.spawn(
		tasks::config_sender::config_sender_task(config_sender, RESPONSE_CHANNEL.receiver())
			.unwrap(),
	);
	spawner.spawn(
		tasks::config_receiver::config_receiver_task(
			config_receiver,
			REQUEST_CHANNEL.sender(),
			RESPONSE_CHANNEL.sender(),
		)
		.unwrap(),
	);
	spawner.spawn(
		tasks::adc::adc_task(
			device_config,
			adc,
			midi_sender,
			tasks::adc::AdcPins {
				PA0: p.PA0,
				PA1: p.PA1,
				PA2: p.PA2,
				PA3: p.PA3,
			},
		)
		.unwrap(),
	);
	spawner.spawn(
		tasks::buttons::buttons_task(
			tasks::buttons::ButtonPins {
				button0: p.PB0,
				button1: p.PB1,
				button2: p.PB2,
				button3: p.PB3,
			},
			REQUEST_CHANNEL.sender(),
		)
		.unwrap(),
	);
	spawner.spawn(
		tasks::request_handler::request_handler_task(
			device_config,
			storage,
			REQUEST_CHANNEL.receiver(),
			RESPONSE_CHANNEL.sender(),
		)
		.unwrap(),
	);

	// Finally, trigger a config load.
	REQUEST_CHANNEL
		.sender()
		.send(Request {
			cmd: Command::ResetConfig,
			is_external: false,
		})
		.await;
}
