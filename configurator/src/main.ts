import {
	type DeviceConfig,
	type ButtonAction,
	type ButtonConfig,
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
// For the button action pop-up.
let activeEditingTarget: {
	buttonIndex: number;
	gestureType: "click" | "hold";
	gestureIndex?: number;
} | null = null;

function updateGui(): void {
	if (!guiState) {
		return;
	}

	const preset = guiState.presets[selectedPreset];

	const presetName = document.getElementById("preset-name") as HTMLInputElement;

	// const heading = document.getElementById("preset-heading");
	// if (heading) {
	// 	heading.textContent = preset.name || `Preset ${selectedPreset + 1} (Unnamed)`;
	// }

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

		// Trigger checkboxes.
		for (let targetPot = 0; targetPot < 4; targetPot++) {
			const triggerCheckbox = document.querySelector<HTMLInputElement>(
				`input[data-source-pot="${potIndex}"][data-pot="${targetPot}"][data-field="trigger"]`,
			);
			if (triggerCheckbox) {
				triggerCheckbox.checked = pot.triggers[targetPot];
			}
		}
	});

	const radios = document.querySelectorAll<HTMLInputElement>('input[name="preset"]');

	radios.forEach((radio, index) => {
		radio.checked = index === selectedPreset;
	});

	updateButtonActions();
	updateUseButton();
	updatePresetLabels();
	updateDirtyMarkers();
}

function updateButtonActions(): void {
	if (!guiState) {
		return;
	}

	const currentPreset = guiState.presets[selectedPreset];
	const actionButtons = document.querySelectorAll<HTMLButtonElement>(
		"#button-config .button-action-btn",
	);

	for (const btn of actionButtons) {
		const btnIdx = Number(btn.dataset.btn);
		const gestureType = btn.dataset.gestureType as "click" | "hold";
		const gestureIndex =
			btn.dataset.gestureIndex !== undefined ? Number(btn.dataset.gestureIndex) : undefined;

		if (isNaN(btnIdx)) {
			continue;
		}

		const action = getButtonAction(currentPreset.buttons[btnIdx], gestureType, gestureIndex);
		btn.textContent = formatButtonAction(action);

		const dirty = buttonActionIsDirty(selectedPreset, btnIdx, gestureType, gestureIndex);
		btn.classList.toggle("dirty", dirty);
	}
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

	const potsDirty = guiPreset.pots.some((pot, potIndex) => {
		const savedPot = savedPreset.pots[potIndex];

		if (pot.cc !== savedPot.cc || pot.channel !== savedPot.channel) {
			return true;
		}

		// Check triggers.
		return pot.triggers.some((trig, targetIdx) => trig !== savedPot.triggers[targetIdx]);
	});
	if (potsDirty) {
		return true;
	}

	return guiPreset.buttons.some((btn, btnIdx) => {
		const savedBtn = savedPreset.buttons[btnIdx];
		if (JSON.stringify(btn.hold) !== JSON.stringify(savedBtn.hold)) return true;
		return btn.clicks.some(
			(click, clickIdx) => JSON.stringify(click) !== JSON.stringify(savedBtn.clicks[clickIdx]),
		);
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

function triggerFieldIsDirty(
	presetIndex: number,
	sourcePotIndex: number,
	targetPotIndex: number,
): boolean {
	if (!guiState || !savedState) {
		return false;
	}

	return (
		guiState.presets[presetIndex].pots[sourcePotIndex].triggers[targetPotIndex] !==
		savedState.presets[presetIndex].pots[sourcePotIndex].triggers[targetPotIndex]
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

	// Text input markers
	const textInputs = document.querySelectorAll<HTMLInputElement>(
		"#pot-config input[data-field='cc'], #pot-config input[data-field='channel']",
	);
	for (const input of textInputs) {
		const potIndex = Number(input.dataset.pot);
		const field = input.dataset.field as "cc" | "channel" | undefined;

		if (isNaN(potIndex) || (field !== "cc" && field !== "channel")) {
			continue;
		}

		input.classList.toggle("dirty", potFieldIsDirty(selectedPreset, potIndex, field));
	}

	// Checkbox markers
	const checkboxInputs = document.querySelectorAll<HTMLInputElement>(
		"#pot-config input[data-field='trigger']",
	);
	for (const input of checkboxInputs) {
		const sourcePotIndex = Number(input.dataset.sourcePot);
		const targetPotIndex = Number(input.dataset.pot);

		if (isNaN(sourcePotIndex) || isNaN(targetPotIndex)) {
			continue;
		}

		input.classList.toggle(
			"dirty",
			triggerFieldIsDirty(selectedPreset, sourcePotIndex, targetPotIndex),
		);
	}
}

function updateDirtyMarkers(): void {
	updatePresetLabels();
	updatePotDirtyMarkers();

	const save = document.getElementById("save") as HTMLButtonElement;

	save.disabled = !isDirty();
}

function setupButtonConfig(): void {
	const dialog = document.getElementById("button-action-dialog") as HTMLDialogElement;
	const form = document.getElementById("button-action-form") as HTMLFormElement;
	const typeSelect = document.getElementById("action-type-select") as HTMLSelectElement;
	const cancelBtn = document.getElementById("dialog-cancel-btn") as HTMLButtonElement;

	const paramGroups = document.querySelectorAll<HTMLElement>(
		"#action-params-container .param-group",
	);

	function updateVisibleParamFields(): void {
		const selectedType = typeSelect.value;
		for (const group of paramGroups) {
			const matches = group.dataset.action === selectedType;
			group.hidden = !matches;

			for (const input of group.querySelectorAll<HTMLInputElement>("input")) {
				input.required = matches;
			}
		}
	}

	typeSelect.addEventListener("change", updateVisibleParamFields);

	// Event delegation for opening the dialog
	document.getElementById("button-config")?.addEventListener("click", e => {
		const target = (e.target as HTMLElement).closest<HTMLButtonElement>(".button-action-btn");
		if (!target || !guiState || operationInProgress) {
			return;
		}

		const buttonIndex = Number(target.dataset.btn);
		const gestureType = target.dataset.gestureType as "click" | "hold";
		const gestureIndex =
			target.dataset.gestureIndex !== undefined ? Number(target.dataset.gestureIndex) : undefined;

		activeEditingTarget = { buttonIndex, gestureType, gestureIndex };

		const currentAction = getButtonAction(
			guiState.presets[selectedPreset].buttons[buttonIndex],
			gestureType,
			gestureIndex,
		);

		// Clear stale data.
		form.reset();

		// Populate fields based on active state
		typeSelect.value = currentAction.type;
		updateVisibleParamFields();

		if (currentAction.type === "Preset") {
			(document.getElementById("param-preset") as HTMLInputElement).value = String(
				currentAction.data.preset + 1,
			);
		} else if (currentAction.type === "Cc") {
			(document.getElementById("param-cc-number") as HTMLInputElement).value = String(
				currentAction.data.cc + 1,
			);
			(document.getElementById("param-cc-channel") as HTMLInputElement).value = String(
				currentAction.data.channel + 1,
			);
		} else if (currentAction.type === "Note") {
			(document.getElementById("param-note-number") as HTMLInputElement).value = String(
				currentAction.data.note + 1,
			);
			(document.getElementById("param-note-channel") as HTMLInputElement).value = String(
				currentAction.data.channel + 1,
			);
		}

		dialog.showModal();
	});

	cancelBtn.addEventListener("click", () => {
		dialog.close();
	});

	form.addEventListener("submit", async e => {
		e.preventDefault();
		if (!guiState || !activeEditingTarget || operationInProgress) {
			return;
		}

		const submitter = e.submitter as HTMLButtonElement | null;
		const applyToAll = submitter?.value === "all";

		const { buttonIndex, gestureType, gestureIndex } = activeEditingTarget;
		const selectedType = typeSelect.value;

		let newAction: ButtonAction;

		switch (selectedType) {
			case "None":
				newAction = { type: "None" };
				break;
			case "NextPreset":
				newAction = { type: "NextPreset" };
				break;
			case "PreviousPreset":
				newAction = { type: "PreviousPreset" };
				break;
			case "Preset": {
				const presetVal =
					Number((document.getElementById("param-preset") as HTMLInputElement).value) - 1;
				newAction = { type: "Preset", data: { preset: presetVal } };
				break;
			}
			case "Cc": {
				const ccVal =
					Number((document.getElementById("param-cc-number") as HTMLInputElement).value) - 1;
				const channelVal =
					Number((document.getElementById("param-cc-channel") as HTMLInputElement).value) - 1;
				newAction = { type: "Cc", data: { cc: ccVal, channel: channelVal } };
				break;
			}
			case "Note": {
				const noteVal =
					Number((document.getElementById("param-note-number") as HTMLInputElement).value) - 1;
				const channelVal =
					Number((document.getElementById("param-note-channel") as HTMLInputElement).value) - 1;
				newAction = { type: "Note", data: { note: noteVal, channel: channelVal } };
				break;
			}
			default:
				return;
		}

		const targetPresets = applyToAll ? guiState.presets.map((_, index) => index) : [selectedPreset];

		const changePromises = targetPresets.map(async presetIndex => {
			const buttonCfg = guiState!.presets[presetIndex].buttons[buttonIndex];

			setButtonAction(buttonCfg, structuredClone(newAction), gestureType, gestureIndex);

			return changeSetting({
				type: "Preset",
				data: {
					preset: presetIndex,
					change: {
						type: "Button",
						data: {
							button: buttonIndex,
							clicks: buttonCfg.clicks,
							hold: buttonCfg.hold,
						},
					},
				},
			});
		});

		dialog.close();
		updateGui();

		await Promise.all(changePromises);
	});
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
				preset: selectedPreset,
				change: {
					type: "Name",
					data: input.value,
				},
			},
		});
	});
}

function setupPotInputs(): void {
	const textInputs = document.querySelectorAll<HTMLInputElement>(
		"#pot-config input[data-field='cc'], #pot-config input[data-field='channel']",
	);

	for (const input of textInputs) {
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
					preset: selectedPreset,
					change: {
						type: "Pot",
						data: {
							pot: potIndex,
							cc: pot.cc,
							channel: pot.channel,
							triggers: pot.triggers,
						},
					},
				},
			});
		});
	}

	// Handlers for Trigger Checkboxes
	const checkboxInputs = document.querySelectorAll<HTMLInputElement>(
		"#pot-config input[data-field='trigger']",
	);

	for (const input of checkboxInputs) {
		input.addEventListener("change", async () => {
			if (!guiState || operationInProgress) {
				return;
			}

			const sourcePotIndex = Number(input.dataset.sourcePot);
			const targetPotIndex = Number(input.dataset.pot);

			if (isNaN(sourcePotIndex) || isNaN(targetPotIndex)) {
				return;
			}

			const pot = guiState.presets[selectedPreset].pots[sourcePotIndex];
			pot.triggers[targetPotIndex] = input.checked;

			updateDirtyMarkers();

			await changeSetting({
				type: "Preset",
				data: {
					preset: selectedPreset,
					change: {
						type: "Pot",
						data: {
							pot: sourcePotIndex,
							cc: pot.cc,
							channel: pot.channel,
							triggers: pot.triggers,
						},
					},
				},
			});
		});
	}
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
				announceToScreenReader(
					`Active preset changed to ${guiState.presets[activePreset].name || `Unnamed (Slot ${activePreset + 1})`}`,
				);
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

function formatButtonAction(action: ButtonAction): string {
	switch (action.type) {
		case "None":
			return "None";
		case "NextPreset":
			return "Next preset";
		case "PreviousPreset":
			return "Previous preset";
		case "Preset":
			return `Select preset ${action.data.preset + 1}`;
		case "Cc":
			return `CC ${action.data.cc + 1} on channel ${action.data.channel + 1}`;
		case "Note":
			return `Note ${action.data.note + 1} on channel ${action.data.channel + 1}`;
	}
}

function getButtonAction(
	buttonCfg: ButtonConfig,
	gestureType: "click" | "hold",
	gestureIndex?: number,
): ButtonAction {
	if (gestureType === "hold") {
		return buttonCfg.hold;
	}
	return buttonCfg.clicks[gestureIndex ?? 0];
}

function setButtonAction(
	buttonCfg: ButtonConfig,
	action: ButtonAction,
	gestureType: "click" | "hold",
	gestureIndex?: number,
): void {
	if (gestureType === "hold") {
		buttonCfg.hold = action;
	} else if (gestureIndex !== undefined) {
		buttonCfg.clicks[gestureIndex] = action;
	}
}

function buttonActionIsDirty(
	presetIndex: number,
	buttonIndex: number,
	gestureType: "click" | "hold",
	gestureIndex?: number,
): boolean {
	if (!guiState || !savedState) return false;

	const guiAction = getButtonAction(
		guiState.presets[presetIndex].buttons[buttonIndex],
		gestureType,
		gestureIndex,
	);
	const savedAction = getButtonAction(
		savedState.presets[presetIndex].buttons[buttonIndex],
		gestureType,
		gestureIndex,
	);

	return JSON.stringify(guiAction) !== JSON.stringify(savedAction);
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
	setupButtonConfig();

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
