import {
	type DeviceConfig,
	connectDevice,
	getConfig,
	changeSetting,
	resetConfig,
	saveChanges,
	fetchActivePresetIndex,
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

	const heading = document.getElementById("preset-heading");
	if (heading) {
		heading.textContent = preset.name || `Preset ${selectedPreset + 1} (Unnamed)`;
	}

	presetName.value = preset.name;

	preset.pots.forEach((pot, potIndex) => {
		const ccInput = document.querySelector<HTMLInputElement>(
			`input[data-pot="${potIndex}"][data-field="cc"]`,
		);
		const channelInput = document.querySelector<HTMLInputElement>(
			`input[data-pot="${potIndex}"][data-field="channel"]`,
		);

		if (ccInput) ccInput.value = String(pot.cc + 1);
		if (channelInput) channelInput.value = String(pot.channel + 1);
	});

	const radios = document.querySelectorAll<HTMLInputElement>('input[name="preset"]');

	radios.forEach((radio, index) => {
		radio.checked = index === selectedPreset;
	});

	updateUseButton();
	updatePresetLabels();
	updateDirtyMarkers();
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

		span.textContent = `${name}${marker}`;
	});
}

function updatePotDirtyMarkers(): void {
	if (!guiState || !savedState) {
		return;
	}

	const inputs = document.querySelectorAll<HTMLInputElement>("#pot-config input[data-field]");
	inputs.forEach(input => {
		const potIndex = Number(input.dataset.pot);
		const field = input.dataset.field as "cc" | "channel" | undefined;

		if (isNaN(potIndex) || (field !== "cc" && field !== "channel")) return;

		input.classList.toggle("dirty", potFieldIsDirty(selectedPreset, potIndex, field));
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

	// This one just updates the GUI.
	input.addEventListener("input", async () => {
		if (guiState === null || operationInProgress) {
			return;
		}

		guiState.presets[selectedPreset].name = input.value;

		// document.getElementById(`preset-radio-${selectedPreset}`)!.textContent = `${selectedPreset + 1}. input.value`;
		updatePresetLabels();
		updateDirtyMarkers();
	});

	// This one actually updates the device.
	input.addEventListener("change", async () => {
		await changeSetting({
			type: "Preset",
			data: {
				type: "Name",
				data: { preset: selectedPreset, name: input.value },
			},
		});
	});
}

function setupPotInputs(): void {
	const inputs = document.querySelectorAll<HTMLInputElement>("#pot-config input[data-field]");

	inputs.forEach(input => {
		// Up / down
		input.addEventListener("keydown", (e: KeyboardEvent) => {
			if (e.key !== "ArrowUp" && e.key !== "ArrowDown") {
				return;
			}

			e.preventDefault();

			const isCC = input.dataset.field === "cc";
			const min = 1;
			const max = isCC ? 128 : 16;

			let val = Number(input.value) || min;
			val = e.key === "ArrowUp" ? val + 1 : val - 1;
			val = Math.max(min, Math.min(max, val));

			input.value = String(val);
			input.dispatchEvent(new Event("input", { bubbles: true }));
		});

		input.addEventListener("input", async () => {
			if (guiState === null || operationInProgress) {
				return;
			}

			const potIndex = Number(input.dataset.pot);
			const field = input.dataset.field;

			if (isNaN(potIndex) || (field !== "cc" && field !== "channel")) return;

			const pot = guiState.presets[selectedPreset].pots[potIndex];
			pot[field] = Number(input.value) - 1;

			updateDirtyMarkers();

			await changeSetting({
				type: "Preset",
				data: {
					type: "Pot",
					data: {
						preset: selectedPreset,
						pot: potIndex,
						cc: pot.cc,
						channel: pot.channel,
					},
				},
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

			selectPreset(Number(radio.value));
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
		announceToScreenReader("No changes to save.");
		return;
	}

	operationInProgress = true;
	updateOperationState();

	try {
		await saveChanges();

		guiState = await getConfig(false);
		savedState = await getConfig(true);

		updateDirtyMarkers();
		announceToScreenReader("Configuration saved.");
	} finally {
		operationInProgress = false;
		updateOperationState();
	}
}

async function reset(): Promise<void> {
	if (operationInProgress) {
		announceToScreenReader("Can't reset while another operation's in progress.");
		return;
	}

	operationInProgress = true;
	updateOperationState();

	try {
		await resetConfig();
		announceToScreenReader("Changes discarded.");

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

async function pollActivePreset(): Promise<void> {
	while (true) {
		if (!guiState) {
			continue;
		}

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
	const reconnectButton = document.getElementById("reconnect")! as HTMLButtonElement;
	reconnectButton.disabled = true;

	const status = document.getElementById("connection-status")!;

	operationInProgress = true;
	updateOperationState();

	status.textContent = "Connecting to Pot4 MIDI CC Controller...";

	try {
		await connectDevice();

		status.textContent = "Retreiving configuration...";
		guiState = await getConfig(false);
		savedState = await getConfig(true);

		status.textContent = "Connected.";
		updateGui();

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

function selectPreset(index: number, announce: boolean = false): void {
	selectedPreset = index;

	// Update all GUI values (populates #preset-name, pot inputs, dirty markers)
	updateGui();

	if (announce && guiState) {
		const name = guiState.presets[index].name || "Unnamed";
		const marker = presetIsDirty(index) ? " *" : "";

		// Announce current preset context to screen readers without moving focus.
		announceToScreenReader(`Preset ${index + 1} - ${name}${marker}`);
	}
}

function announceToScreenReader(message: string): void {
	const announcer = document.getElementById("sr-announcer");
	if (!announcer) {
		return;
	}

	// Clear and reset to ensure screen readers re-announce identical strings if triggered rapidly
	announcer.textContent = "";

	// Slight delay allows NVDA to detect the DOM mutation reliably
	setTimeout(() => {
		announcer.textContent = message;
	}, 50);
}

function setupKeyboardShortcuts(): void {
	window.addEventListener("keydown", (event: KeyboardEvent) => {
		// Ctrl+letter combos
		if (event.ctrlKey && !event.altKey && !event.metaKey) {
			const key = event.key.toLowerCase();

			switch (key) {
				case "s": {
					event.preventDefault();

					void save();
					return;
				}
				case "d": {
					event.preventDefault();

					void reset();
					return;
				}
			}
		}

		// Ctrl+Tab and Ctrl+Shift+Tab
		if (event.ctrlKey && event.key === "Tab") {
			if (operationInProgress || !guiState) {
				return;
			}

			event.preventDefault();

			const totalPresets = guiState.presets.length;
			let targetPreset = selectedPreset;

			if (event.shiftKey) {
				// Ctrl+Shift+Tab: Previous preset (wrap around)
				targetPreset = (selectedPreset - 1 + totalPresets) % totalPresets;
			} else {
				// Ctrl+Tab: Next preset (wrap around)
				targetPreset = (selectedPreset + 1) % totalPresets;
			}

			if (targetPreset !== selectedPreset) {
				// Change preset and announce
				selectPreset(targetPreset, true);
			}
		}
	});
}

try {
	await connectDeviceWithStatus();

	selectedPreset = 0;
	pollActivePreset();

	setupKeyboardShortcuts();
	setupPresetName();
	setupPotInputs();
	setupPresetSelection();
	setupUseButton();

	for (const [id, func] of [
		["reset", reset],
		["save", save],
	] as const) {
		const button = document.getElementById(id);
		button?.addEventListener("click", async () => await func());
	}

	updateGui();
} catch (e) {
	console.error(`error initializing: ${e}`);
	alert(`Error initializing: ${e}`);
}
