mod device_config;
mod usb;

use std::sync::Mutex;

use tauri::Manager;

use self::device_config::{
	ConfigChange,
	DeviceConfig,
	Preset,
};

struct AppState {
	usb: Mutex<Option<ConfigDevice>>,
}

use self::usb::ConfigDevice;

#[tauri::command]
fn connect_device(state: tauri::State<'_, AppState>) -> Result<(), String> {
	// Drop the old connection if it exists.
	state.usb.lock().unwrap().take();

	let usb = ConfigDevice::open().map_err(|e| e.to_string())?;
	*state.usb.lock().unwrap() = Some(usb);

	Ok(())
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>, persistent: bool) -> Result<DeviceConfig, String> {
	let mut usb = state.usb.lock().unwrap();
	let usb = usb.as_mut().ok_or_else(|| String::from("Not connected"))?;

	let config = usb
		.command(if persistent {
			"config.get saved.preset"
		} else {
			"config.get preset"
		})
		.map_err(|e| e.to_string())?;

	let active_preset = config
		.split(';')
		.find_map(|s| {
			let val = s.strip_prefix("preset=")?;
			match val.parse::<u8>() {
				Ok(n) if n < 4 => Some(n),
				Ok(_) => {
					eprintln!(
						"error: device claims active preset is {val}, which is greater than 3"
					);
					None
				}
				Err(_) => {
					eprintln!(
						"error: device claims active preset is {val}, which is not a valid u8"
					);
					None
				}
			}
		})
		.unwrap_or(0);

	let saved = if persistent { "saved." } else { "" };
	let mut presets = Vec::with_capacity(4);
	for i in 0u8..4 {
		let resp = usb
			.command(&format!("preset.get {saved}{i}"))
			.map_err(|e| e.to_string())?;
		presets.push(Preset::parse(&resp).map_err(|e| format!("Failed to parse preset {i}: {e}"))?);
	}

	Ok(DeviceConfig {
		active_preset,
		presets: presets.try_into().unwrap(),
	})
}

#[tauri::command]
fn change_setting(state: tauri::State<'_, AppState>, change: ConfigChange) -> Result<(), String> {
	state
		.usb
		.lock()
		.unwrap()
		.as_mut()
		.ok_or_else(|| String::from("Not connected"))?
		.command_ok(&change.to_command_string())
}

#[tauri::command]
fn reset_config(state: tauri::State<'_, AppState>) -> Result<(), String> {
	state
		.usb
		.lock()
		.unwrap()
		.as_mut()
		.ok_or_else(|| String::from("Not connected"))?
		.command_ok("config.reset")
}

#[tauri::command]
fn save_changes(state: tauri::State<'_, AppState>) -> Result<(), String> {
	state
		.usb
		.lock()
		.unwrap()
		.as_mut()
		.ok_or_else(|| String::from("Not connected"))?
		.command_ok("config.save")
}

#[tauri::command]
fn fetch_active_preset_index(state: tauri::State<'_, AppState>) -> Result<u8, String> {
	let resp = state
		.usb
		.lock()
		.unwrap()
		.as_mut()
		.ok_or_else(|| String::from("Not connected"))?
		.command("config.get preset")
		.map_err(|e| e.to_string())?;

	resp.strip_prefix("preset=")
		.ok_or_else(|| format!("unexpected response: {resp}"))?
		.parse::<u8>()
		.map_err(|_| format!("device returned a non-numeric preset index: {resp}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.manage(AppState {
			usb: Mutex::new(None),
		})
		.on_window_event(|window, event| {
			if let tauri::WindowEvent::Destroyed = event {
				window.state::<AppState>().usb.lock().unwrap().take();
			}
		})
		.plugin(tauri_plugin_opener::init())
		.invoke_handler(tauri::generate_handler![
			connect_device,
			get_config,
			change_setting,
			reset_config,
			save_changes,
			fetch_active_preset_index,
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
