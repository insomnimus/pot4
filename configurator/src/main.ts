import {
	type DeviceConfig,
	type Preset,
	type PotConfig,
	type ConfigChange,
	type PresetChange,
	connectDevice,
	getConfig,
	changeSetting,
	resetConfig,
	saveChanges,
} from "./backend";

let guiState: DeviceConfig | null = null;
let savedState: DeviceConfig | null = null;
// Which preset the user is currently viewing/editing.
let selectedPreset = 0;
let operationInProgress = false;

function updateGui(): void {
	if (!guiState) {
		return;
	}

	const preset = guiState.presets[selectedPreset];

	const presetName = document.getElementById("preset-name") as HTMLInputElement;

	presetName.value = preset.name;

	const rows = document.querySelectorAll<HTMLTableRowElement>("#pot-config tr");

	rows.forEach((row, index) => {
		const pot = preset.pots[index];

		const cc = row.querySelector<HTMLInputElement>('input[data-field="cc"]')!;

		const channel = row.querySelector<HTMLInputElement>('input[data-field="channel"]')!;

		cc.value = String(pot.cc + 1);
		channel.value = String(pot.channel + 1);
	});

	const radios = document.querySelectorAll<HTMLInputElement>('input[name="preset"]');

	radios.forEach((radio, index) => {
		radio.checked = index === selectedPreset;
	});

	updateUseButton();
	updatePresetLabels();
	updateDirtyMarkers();
}

async function initialize(): Promise<void> {
	guiState = await getConfig(false);
	savedState = await getConfig(true);

	selectedPreset = 0;

	updateGui();
}

function presetIsDirty(index: number): boolean {
	if (!guiState || !savedState) {
		return false;
	}

	const guiPreset = guiState.presets[index];
	const savedPreset = savedState.presets[index];

	if (guiPreset.name !== savedPreset.name) {
		return true;
	}

	return guiPreset.pots.some((pot, potIndex) => {
		const savedPot = savedPreset.pots[potIndex];

		return pot.cc !== savedPot.cc || pot.channel !== savedPot.channel;
	});
}

function potFieldIsDirty(presetIndex: number, potIndex: number, field: "cc" | "channel"): boolean {
	if (!guiState || !savedState) {
		return false;
	}

	return (
		guiState.presets[presetIndex].pots[potIndex][field] !==
		savedState.presets[presetIndex].pots[potIndex][field]
	);
}

function isDirty(): boolean {
	if (!guiState || !savedState) {
		return false;
	}

	if (guiState.active_preset !== savedState.active_preset) {
		return true;
	}

	return [0, 1, 2, 3].some(presetIsDirty);
}

function updatePresetLabels(): void {
	if (!guiState) {
		return;
	}

	const radios = document.querySelectorAll<HTMLInputElement>('input[name="preset"]');

	radios.forEach((radio, index) => {
		const span = radio.parentElement?.querySelector("span");

		if (span === null || span === undefined) {
			return;
		}

		const name = guiState!.presets[index].name || "Unnamed";
		const marker = presetIsDirty(index) ? " *" : "";

		span.textContent = `${index + 1}. ${name}${marker}`;
	});
}

function updatePotDirtyMarkers(): void {
	if (!guiState || !savedState) {
		return;
	}

	const rows = document.querySelectorAll<HTMLTableRowElement>("#pot-config tr");

	rows.forEach((row, potIndex) => {
		for (const field of ["cc", "channel"] as const) {
			const input = row.querySelector<HTMLInputElement>(`input[data-field="${field}"]`);

			if (input === null) {
				continue;
			}

			input.classList.toggle("dirty", potFieldIsDirty(selectedPreset, potIndex, field));
		}
	});
}

function updateDirtyMarkers(): void {
	updatePresetLabels();
	updatePotDirtyMarkers();

	const save = document.getElementById("save") as HTMLButtonElement;

	save.disabled = !isDirty();
}

function setupPresetName(): void {
	const input = document.getElementById("preset-name") as HTMLInputElement;

	input.addEventListener("input", async () => {
		if (guiState === null || operationInProgress) {
			return;
		}

		guiState.presets[selectedPreset].name = input.value;

		updateDirtyMarkers();
		await changeSetting({
			type: "Preset",
			data: [
				{
					type: "Name",
					data: { preset: selectedPreset, name: input.value },
				},
			],
		});
	});
}

function setupPotInputs(): void {
	const inputs = document.querySelectorAll<HTMLInputElement>("#pot-config input[data-field]");

	inputs.forEach(input => {
		input.addEventListener("input", async () => {
			if (guiState === null || operationInProgress) {
				return;
			}

			const row = input.closest<HTMLTableRowElement>("tr");

			if (row === null) {
				return;
			}

			const potIndex = Number(row.dataset.pot);
			const field = input.dataset.field;

			if (field !== "cc" && field !== "channel") {
				return;
			}

			const pot = guiState.presets[selectedPreset].pots[potIndex];
			pot[field] = Number(input.value) - 1;

			updateDirtyMarkers();

			await changeSetting({
				type: "Preset",
				data: [
					{
						type: "Pot",
						data: {
							preset: selectedPreset,
							pot: potIndex,
							cc: pot.cc,
							channel: pot.channel,
						},
					},
				],
			});
		});
	});
}

// This does not change config; just changes the screen.
function setupPresetSelection(): void {
	const radios = document.querySelectorAll<HTMLInputElement>('input[name="preset"]');

	radios.forEach(radio => {
		radio.addEventListener("change", () => {
			if (operationInProgress) {
				return;
			}

			selectedPreset = Number(radio.value);

			updateGui();
		});
	});
}

function setupUseButton(): void {
	const button = document.getElementById("use-preset") as HTMLButtonElement;

	button.addEventListener("click", () => {
		void usePreset(selectedPreset);
	});
}

function updateUseButton(): void {
	if (!guiState) {
		return;
	}

	const button = document.getElementById("use-preset") as HTMLButtonElement;

	const active = guiState.active_preset === selectedPreset;

	button.disabled = active || operationInProgress;
	button.textContent = active ? "Active" : "Use";
}

async function usePreset(index: number): Promise<void> {
	if (!guiState || operationInProgress) {
		return;
	}

	operationInProgress = true;
	updateOperationState();

	try {
		await changeSetting({ type: "ActivePreset", data: index });

		guiState.active_preset = index;

		updateDirtyMarkers();
	} finally {
		operationInProgress = false;
		updateOperationState();
	}
}

async function save(): Promise<void> {
	if (!guiState || !savedState) {
		return;
	}

	if (!isDirty() || operationInProgress) {
		return;
	}

	operationInProgress = true;
	updateOperationState();

	try {
		await saveChanges();

		guiState = await getConfig(false);
		savedState = await getConfig(true);

		updateDirtyMarkers();
	} finally {
		operationInProgress = false;
		updateOperationState();
	}
}

async function reset(): Promise<void> {
	if (operationInProgress) {
		return;
	}

	operationInProgress = true;
	updateOperationState();

	try {
		await resetConfig();

		if (savedState) {
			guiState = await getConfig(false);
			savedState = await getConfig(true);
		}

		updateGui();
	} finally {
		operationInProgress = false;
		updateOperationState();
	}
}

function updateOperationState(): void {
	const section = document.getElementById("controls");
	if (!section) {
		return;
	}

	const controls = section.querySelectorAll<HTMLInputElement | HTMLButtonElement>("input, button");

	controls.forEach(control => {
		control.disabled = operationInProgress;
	});

	updateUseButton();
}

async function fetchActivePresetIndex(): Promise<number> {
	throw new Error("TODO");
}

async function pollActivePreset(): Promise<void> {
	while (true) {
		try {
			const activePreset = await fetchActivePresetIndex();

			if (guiState && activePreset !== guiState.active_preset) {
				guiState.active_preset = activePreset;

				updateUseButton();
				updateDirtyMarkers();
			}
		} catch (error) {
			console.error("Failed to fetch active preset:", error);
		}

		await new Promise(resolve => setTimeout(resolve, 1000));
	}
}

async function connectDeviceWithStatus(): Promise<boolean> {
	const reconnectButton = document.getElementById("reconnect-button")! as HTMLButtonElement;
	reconnectButton.disabled = true;

	const status = document.getElementById("midi-status")!;

	operationInProgress = true;
	updateOperationState();

	status.textContent = "Connecting to Pot4 MIDI CC Controller...";

	try {
		await connectDevice();

		status.textContent = "Retreiving configuration...";
		guiState = await getConfig(false);
		savedState = await getConfig(true);

		status.textContent = "Connected.";

		return true;
	} catch (error) {
		console.error("Failed to connect to device:", error);

		status.textContent = "Failed to connect to Pot4 MIDI CC Controller.";
		return false;
	} finally {
		reconnectButton.disabled = false;
		operationInProgress = false;
		updateOperationState();
	}
}
