use defmt::*;
use embassy_sync::{
	blocking_mutex::raw::ThreadModeRawMutex,
	channel::{
		Receiver,
		Sender,
	},
};

use crate::{
	MutexedConfig,
	Request,
	Response,
	config::{
		DeviceConfig,
		command::Command,
	},
	storage::Storage,
};

const OK: &[u8] = b"OK";

/// Reads Requests, sends Responses.
#[embassy_executor::task]
pub async fn request_handler_task(
	device_config: &'static MutexedConfig,
	mut storage: Storage<DeviceConfig>,
	request_receiver: Receiver<'static, ThreadModeRawMutex, Request, 4>,
	resp_sender: Sender<'static, ThreadModeRawMutex, Response, 4>,
) {
	info!("Task request_handler started");

	loop {
		let req = request_receiver.receive().await;
		let is_ext = req.is_external;

		match req.cmd {
			Command::Ping => send(&resp_sender, is_ext, b"pong").await,

			Command::GetConfig { saved, key } => {
				if is_ext {
					let config = if saved {
						match storage.load() {
							Ok(x) => x.unwrap_or(DeviceConfig::FACTORY),
							Err(e) => {
								defmt::error!("failed to load config: {}", e);
								send(&resp_sender, true, b"error Failed to load config").await;
								continue;
							}
						}
					} else {
						*device_config.lock().await
					};

					match key {
						None => send(&resp_sender, true, &config.serialize()).await,
						Some(key) => send(&resp_sender, true, &config.serialize_key(key)).await,
					}
				}
			}
			Command::SetConfig { changes } => {
				device_config.lock().await.apply(&changes);
				send(&resp_sender, is_ext, OK).await;
			}
			Command::SaveConfig => {
				let config = *device_config.lock().await;
				match storage.save(&config) {
					Ok(_) => {
						send(&resp_sender, is_ext, OK).await;
					}
					Err(e) => {
						defmt::error!("error saving config: {}", e);
						send(&resp_sender, is_ext, b"error Failed to save configuration").await;
					}
				}
			}
			Command::ResetConfig => match storage.load() {
				Ok(None) => {
					*device_config.lock().await = DeviceConfig::FACTORY;
					send(&resp_sender, is_ext, OK).await;
				}
				Ok(Some(config)) => {
					*device_config.lock().await = config;
					send(&resp_sender, is_ext, OK).await;
				}
				Err(e) => {
					defmt::error!("error loading config: {}", e);
					send(
						&resp_sender,
						is_ext,
						b"error Failed to load saved configuration",
					)
					.await;
				}
			},

			Command::GetPreset { preset, saved } => {
				if is_ext {
					let resp = if saved {
						match storage.load() {
							Ok(x) => x
								.unwrap_or(DeviceConfig::FACTORY)
								.preset(preset)
								.serialize(),
							Err(e) => {
								defmt::error!("error loading config: {}", e);
								send(&resp_sender, true, b"error Failed to load config").await;
								continue;
							}
						}
					} else {
						device_config.lock().await.preset(preset).serialize()
					};

					send(&resp_sender, true, &resp).await;
				}
			}
			Command::SetPreset { preset, changes } => {
				device_config
					.lock()
					.await
					.preset_mut(preset)
					.apply(&changes);
				send(&resp_sender, is_ext, OK).await;
			}
		}
	}
}

async fn send(
	sender: &Sender<'static, ThreadModeRawMutex, Response, 4>,
	do_send: bool,
	message: &[u8],
) {
	if do_send {
		sender.send(message.iter().copied().collect()).await;
	}
}
