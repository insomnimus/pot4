import { invoke } from "@tauri-apps/api/core";

export interface DeviceConfig {
	active_preset: number;
	presets: [Preset, Preset, Preset, Preset];
}

export interface Preset {
	name: string;
	pots: [PotConfig, PotConfig, PotConfig, PotConfig];
}

export interface PotConfig {
	cc: number;
	channel: number;
}

export type ConfigChange =
	| { type: "ActivePreset"; data: number }
	| { type: "Preset"; data: PresetChange };

export type PresetChange =
	| { type: "Name"; data: { preset: number; name: string } }
	| { type: "Pot"; data: { preset: number; pot: number; cc: number; channel: number } };

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
