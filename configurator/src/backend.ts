import { invoke } from "@tauri-apps/api/core";

export interface DeviceConfig {
	active_preset: number;
	presets: [Preset, Preset, Preset, Preset];
}

export interface Preset {
	name: string;
	pots: [PotConfig, PotConfig, PotConfig, PotConfig];
	buttons: [ButtonConfig, ButtonConfig, ButtonConfig, ButtonConfig];
}

export interface PotConfig {
	cc: number;
	channel: number;
	triggers: [boolean, boolean, boolean, boolean];
}

export interface ButtonConfig {
	clicks: [ButtonAction, ButtonAction, ButtonAction];
	hold: ButtonAction;
}

export type ButtonAction =
	| { type: "None" }
	| { type: "Preset"; data: { preset: number } }
	| { type: "NextPreset" }
	| { type: "PreviousPreset" }
	| { type: "Cc"; data: { cc: number; channel: number } }
	| { type: "Note"; data: { note: number; channel: number } };

export type ConfigChange =
	| { type: "ActivePreset"; data: number }
	| { type: "Preset"; data: { preset: number; change: PresetChange } };

export type PresetChange =
	| { type: "Name"; data: string }
	| {
			type: "Pot";
			data: {
				pot: number;
				cc: number;
				channel: number;
				triggers: [boolean, boolean, boolean, boolean];
			};
	  }
	| {
			type: "Button";
			data: {
				button: number;
				clicks: [ButtonAction, ButtonAction, ButtonAction];
				hold: ButtonAction;
			};
	  };

export async function connectDevice(): Promise<void> {
	await invoke("connect_device");
}

export async function getConfig(persistent: boolean): Promise<DeviceConfig> {
	return await invoke("get_config", { persistent });
}

export async function resetConfig(): Promise<void> {
	await invoke("reset_config");
}

export async function changeSetting(change: ConfigChange): Promise<void> {
	// console.log("changeSetting: ", JSON.stringify(change, null, 2));
	await invoke("change_setting", { change });
}

export async function saveChanges(): Promise<void> {
	await invoke("save_changes");
}

export async function fetchActivePresetIndex(): Promise<number> {
	return await invoke("fetch_active_preset_index");
}
