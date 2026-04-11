import { Button } from "@cap/ui-solid";
import { Select as KSelect } from "@kobalte/core/select";
import { cx } from "cva";
import { batch, createMemo, createSignal, Show } from "solid-js";
import { Toggle } from "~/components/Toggle";
import { useI18n } from "~/i18n";
import {
	defaultKeyboardSettings,
	type KeyboardSettings,
} from "~/store/keyboard";
import { commands } from "~/utils/tauri";
import IconCapChevronDown from "~icons/cap/chevron-down";
import IconCapCircleCheck from "~icons/cap/circle-check";
import { useEditorContext } from "./context";
import type { TranslationKey } from "~/i18n";
import { HexColorInput } from "./text-style";
import {
	Field,
	Input,
	MenuItem,
	MenuItemList,
	PopperContent,
	Slider,
	Subfield,
	topSlideAnimateClasses,
} from "./ui";

const POSITION_VALUES = [
	"top-left",
	"top-center",
	"top-right",
	"bottom-left",
	"bottom-center",
	"bottom-right",
] as const;

const POSITION_LABEL_KEYS: Record<string, TranslationKey> = {
	"top-left": "editor.keyboard.position.topLeft",
	"top-center": "editor.keyboard.position.topCenter",
	"top-right": "editor.keyboard.position.topRight",
	"bottom-left": "editor.keyboard.position.bottomLeft",
	"bottom-center": "editor.keyboard.position.bottomCenter",
	"bottom-right": "editor.keyboard.position.bottomRight",
};

const FONT_VALUES = [
	"System Sans-Serif",
	"System Serif",
	"System Monospace",
] as const;

const FONT_LABEL_KEYS: Record<string, TranslationKey> = {
	"System Sans-Serif": "editor.keyboard.font.systemSans",
	"System Serif": "editor.keyboard.font.systemSerif",
	"System Monospace": "editor.keyboard.font.systemMono",
};

const WEIGHT_VALUES = [
	{ label: "editor.keyboard.fontWeight.normal" as TranslationKey, value: 400 },
	{ label: "editor.keyboard.fontWeight.medium" as TranslationKey, value: 500 },
	{ label: "editor.keyboard.fontWeight.bold" as TranslationKey, value: 700 },
];

export function KeyboardTab() {
	const { t } = useI18n();
	const { project, setProject, editorState, setEditorState } =
		useEditorContext();

	const getSetting = <K extends keyof KeyboardSettings>(
		key: K,
	): NonNullable<KeyboardSettings[K]> => {
		const settings = project?.keyboard?.settings;
		if (settings && key in settings) {
			return (settings as Record<string, unknown>)[
				key as string
			] as NonNullable<KeyboardSettings[K]>;
		}
		return defaultKeyboardSettings[key] as NonNullable<KeyboardSettings[K]>;
	};

	const updateSetting = <K extends keyof KeyboardSettings>(
		key: K,
		value: KeyboardSettings[K],
	) => {
		if (!project?.keyboard) {
			setProject("keyboard", {
				settings: { ...defaultKeyboardSettings, [key]: value },
			});
			return;
		}
		setProject("keyboard", "settings", key, value);
	};

	const hasKeyboardSegments = createMemo(
		() => (project.timeline?.keyboardSegments?.length ?? 0) > 0,
	);

	const [isGenerating, setIsGenerating] = createSignal(false);

	const ensureKeyboardSettings = (enabled: boolean) => {
		if (!project?.keyboard) {
			setProject("keyboard", {
				settings: { ...defaultKeyboardSettings, enabled },
			});
			return;
		}
		setProject("keyboard", "settings", "enabled", enabled);
	};

	const setKeyboardVisible = (enabled: boolean) => {
		batch(() => {
			ensureKeyboardSettings(enabled);
			setEditorState("timeline", "tracks", "keyboard", enabled);
			if (!enabled && editorState.timeline.selection?.type === "keyboard") {
				setEditorState("timeline", "selection", null);
			}
		});
	};

	const generateSegments = async () => {
		setIsGenerating(true);
		try {
			const segments = await commands.generateKeyboardSegments(
				getSetting("groupingThresholdMs"),
				getSetting("lingerDuration") * 1000,
				getSetting("showModifiers"),
				getSetting("showSpecialKeys"),
			);

			if (segments.length > 0) {
				batch(() => {
					ensureKeyboardSettings(true);
					setProject("timeline", "keyboardSegments", segments);
					setEditorState("timeline", "tracks", "keyboard", true);
				});
			}
		} catch (e) {
			console.error("Failed to generate keyboard segments:", e);
		} finally {
			setIsGenerating(false);
		}
	};

	const selectedSegment = () => {
		const selection = editorState.timeline.selection;
		if (selection?.type !== "keyboard" || selection.indices.length !== 1)
			return null;
		return project.timeline?.keyboardSegments?.[selection.indices[0]] ?? null;
	};

	const selectedIndex = () => {
		const selection = editorState.timeline.selection;
		if (selection?.type !== "keyboard" || selection.indices.length !== 1)
			return -1;
		return selection.indices[0];
	};

	return (
		<Field
			name={t("editor.keyboard.label")}
			icon={<IconLucideKeyboard />}
			badge={t("editor.keyboard.beta")}
		>
			<div class="flex flex-col gap-4">
				<Subfield name={t("editor.keyboard.showPresses")}>
					<Toggle
						checked={getSetting("enabled")}
						onChange={setKeyboardVisible}
					/>
				</Subfield>

				<div
					class={cx(
						"space-y-4",
						!getSetting("enabled") && "opacity-50 pointer-events-none",
					)}
				>
					<Field
						name={t("editor.keyboard.fontSettings")}
						icon={<IconLucideKeyboard />}
					>
						<div class="space-y-3">
							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.fontFamily")}
								</span>
								<KSelect<string>
									options={[...FONT_VALUES]}
									value={getSetting("font")}
									onChange={(value) => {
										if (value === null) return;
										updateSetting("font", value);
									}}
									itemComponent={(props) => (
										<MenuItem<typeof KSelect.Item>
											as={KSelect.Item}
											item={props.item}
										>
											<KSelect.ItemLabel class="flex-1">
												{t(FONT_LABEL_KEYS[props.item.rawValue])}
											</KSelect.ItemLabel>
										</MenuItem>
									)}
								>
									<KSelect.Trigger class="w-full flex items-center justify-between rounded-lg px-3 py-2 bg-gray-2 border border-gray-3 text-gray-12 hover:border-gray-4 hover:bg-gray-3 focus:border-blue-9 focus:ring-1 focus:ring-blue-9 transition-colors">
										<KSelect.Value<string>>
											{(state) => t(FONT_LABEL_KEYS[state.selectedOption()])}
										</KSelect.Value>
										<KSelect.Icon>
											<IconCapChevronDown />
										</KSelect.Icon>
									</KSelect.Trigger>
									<KSelect.Portal>
										<PopperContent<typeof KSelect.Content>
											as={KSelect.Content}
											class={topSlideAnimateClasses}
										>
											<MenuItemList<typeof KSelect.Listbox>
												class="max-h-48 overflow-y-auto"
												as={KSelect.Listbox}
											/>
										</PopperContent>
									</KSelect.Portal>
								</KSelect>
							</div>

							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.size")}
								</span>
								<Slider
									value={[getSetting("size")]}
									onChange={(v) => updateSetting("size", v[0])}
									minValue={12}
									maxValue={100}
									step={1}
								/>
							</div>

							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.textColor")}
								</span>
								<HexColorInput
									value={getSetting("color")}
									onChange={(value) => updateSetting("color", value)}
								/>
							</div>
						</div>
					</Field>

					<Field
						name={t("editor.keyboard.backgroundSettings")}
						icon={<IconLucideKeyboard />}
					>
						<div class="space-y-3">
							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.backgroundColor")}
								</span>
								<HexColorInput
									value={getSetting("backgroundColor")}
									onChange={(value) => updateSetting("backgroundColor", value)}
								/>
							</div>

							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.backgroundOpacity")}
								</span>
								<Slider
									value={[getSetting("backgroundOpacity")]}
									onChange={(v) => updateSetting("backgroundOpacity", v[0])}
									minValue={0}
									maxValue={100}
									step={1}
								/>
							</div>
						</div>
					</Field>

					<Field
						name={t("editor.keyboard.position")}
						icon={<IconLucideKeyboard />}
					>
						<KSelect<string>
							options={[...POSITION_VALUES]}
							value={getSetting("position")}
							onChange={(value) => {
								if (value === null) return;
								updateSetting("position", value);
							}}
							itemComponent={(props) => (
								<MenuItem<typeof KSelect.Item>
									as={KSelect.Item}
									item={props.item}
								>
									<KSelect.ItemLabel class="flex-1">
										{t(POSITION_LABEL_KEYS[props.item.rawValue])}
									</KSelect.ItemLabel>
								</MenuItem>
							)}
						>
							<KSelect.Trigger class="w-full flex items-center justify-between rounded-lg px-3 py-2 bg-gray-2 border border-gray-3 text-gray-12 hover:border-gray-4 hover:bg-gray-3 focus:border-blue-9 focus:ring-1 focus:ring-blue-9 transition-colors">
								<KSelect.Value<string>>
									{(state) => (
										<span>
											{t(POSITION_LABEL_KEYS[state.selectedOption()])}
										</span>
									)}
								</KSelect.Value>
								<KSelect.Icon>
									<IconCapChevronDown />
								</KSelect.Icon>
							</KSelect.Trigger>
							<KSelect.Portal>
								<PopperContent<typeof KSelect.Content>
									as={KSelect.Content}
									class={topSlideAnimateClasses}
								>
									<MenuItemList<typeof KSelect.Listbox> as={KSelect.Listbox} />
								</PopperContent>
							</KSelect.Portal>
						</KSelect>
					</Field>

					<Field
						name={t("editor.keyboard.fontWeight")}
						icon={<IconLucideKeyboard />}
					>
						<KSelect
							options={WEIGHT_VALUES}
							optionValue="value"
							optionTextValue="label"
							value={
								WEIGHT_VALUES.find(
									(w) => w.value === getSetting("fontWeight"),
								) ?? {
									label: "editor.keyboard.fontWeight.custom" as TranslationKey,
									value: getSetting("fontWeight"),
								}
							}
							onChange={(value) => {
								if (!value) return;
								updateSetting("fontWeight", value.value);
							}}
							itemComponent={(selectItemProps) => (
								<MenuItem<typeof KSelect.Item>
									as={KSelect.Item}
									item={selectItemProps.item}
								>
									<KSelect.ItemLabel class="flex-1">
										{t(selectItemProps.item.rawValue.label)}
									</KSelect.ItemLabel>
									<KSelect.ItemIndicator class="ml-auto text-blue-9">
										<IconCapCircleCheck />
									</KSelect.ItemIndicator>
								</MenuItem>
							)}
						>
							<KSelect.Trigger class="flex w-full items-center justify-between rounded-md border border-gray-3 bg-gray-2 px-3 py-2 text-sm text-gray-12 transition-colors hover:border-gray-4 hover:bg-gray-3 focus:border-blue-9 focus:outline-none focus:ring-1 focus:ring-blue-9">
								<KSelect.Value<{
									label: TranslationKey;
									value: number;
								}> class="truncate">
									{(state) => {
										const opt = state.selectedOption();
										if (opt) return t(opt.label);
										const w = getSetting("fontWeight");
										const known = WEIGHT_VALUES.find((v) => v.value === w);
										if (known) return t(known.label);
										return t("editor.keyboard.fontWeight.custom", {
											weight: String(w),
										});
									}}
								</KSelect.Value>
								<KSelect.Icon>
									<IconCapChevronDown class="size-4 shrink-0 transform transition-transform ui-expanded:rotate-180 text-[--gray-500]" />
								</KSelect.Icon>
							</KSelect.Trigger>
							<KSelect.Portal>
								<PopperContent<typeof KSelect.Content>
									as={KSelect.Content}
									class={cx(topSlideAnimateClasses, "z-50")}
								>
									<MenuItemList<typeof KSelect.Listbox>
										class="overflow-y-auto max-h-40"
										as={KSelect.Listbox}
									/>
								</PopperContent>
							</KSelect.Portal>
						</KSelect>
					</Field>

					<Field
						name={t("editor.keyboard.animation")}
						icon={<IconLucideKeyboard />}
					>
						<div class="space-y-3">
							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.fadeDuration")}
								</span>
								<Slider
									value={[getSetting("fadeDuration") * 100]}
									onChange={(v) => updateSetting("fadeDuration", v[0] / 100)}
									minValue={0}
									maxValue={50}
									step={1}
								/>
								<span class="text-xs text-gray-11 text-right">
									{(getSetting("fadeDuration") * 1000).toFixed(0)}ms
								</span>
							</div>

							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.lingerDuration")}
								</span>
								<Slider
									value={[getSetting("lingerDuration") * 100]}
									onChange={(v) => updateSetting("lingerDuration", v[0] / 100)}
									minValue={0}
									maxValue={300}
									step={5}
								/>
								<span class="text-xs text-gray-11 text-right">
									{(getSetting("lingerDuration") * 1000).toFixed(0)}ms
								</span>
							</div>

							<div class="flex flex-col gap-2">
								<span class="text-gray-11 text-sm">
									{t("editor.keyboard.groupingThreshold")}
								</span>
								<Slider
									value={[getSetting("groupingThresholdMs")]}
									onChange={(v) => updateSetting("groupingThresholdMs", v[0])}
									minValue={50}
									maxValue={1000}
									step={10}
								/>
								<span class="text-xs text-gray-11 text-right">
									{getSetting("groupingThresholdMs").toFixed(0)}ms
								</span>
							</div>
						</div>
					</Field>

					<Field
						name={t("editor.keyboard.behavior")}
						icon={<IconLucideKeyboard />}
					>
						<div class="space-y-3">
							<div class="flex flex-col gap-2">
								<div class="flex items-center justify-between">
									<span class="text-gray-11 text-sm">
										{t("editor.keyboard.showModifiers")}
									</span>
									<Toggle
										checked={getSetting("showModifiers")}
										onChange={(checked) =>
											updateSetting("showModifiers", checked)
										}
									/>
								</div>
							</div>

							<div class="flex flex-col gap-2">
								<div class="flex items-center justify-between">
									<span class="text-gray-11 text-sm">
										{t("editor.keyboard.showSpecialKeys")}
									</span>
									<Toggle
										checked={getSetting("showSpecialKeys")}
										onChange={(checked) =>
											updateSetting("showSpecialKeys", checked)
										}
									/>
								</div>
							</div>
						</div>
					</Field>

					<div class="pt-2">
						<Button
							onClick={generateSegments}
							disabled={isGenerating()}
							class="w-full"
						>
							{isGenerating()
								? t("editor.keyboard.generating")
								: hasKeyboardSegments()
									? t("editor.keyboard.regenerate")
									: t("editor.keyboard.generate")}
						</Button>
					</div>

					<Show when={selectedSegment()}>
						{(seg) => (
							<Field
								name={t("editor.keyboard.segmentOverride")}
								icon={<IconLucideKeyboard />}
							>
								<div class="space-y-3">
									<Subfield name={t("editor.keyboard.startTime")}>
										<Input
											type="number"
											value={seg().start.toFixed(2)}
											step="0.1"
											min={0}
											onChange={(e) =>
												setProject(
													"timeline",
													"keyboardSegments",
													selectedIndex(),
													"start",
													Number.parseFloat(e.target.value),
												)
											}
										/>
									</Subfield>
									<Subfield name={t("editor.keyboard.endTime")}>
										<Input
											type="number"
											value={seg().end.toFixed(2)}
											step="0.1"
											min={seg().start}
											onChange={(e) =>
												setProject(
													"timeline",
													"keyboardSegments",
													selectedIndex(),
													"end",
													Number.parseFloat(e.target.value),
												)
											}
										/>
									</Subfield>
									<Subfield name={t("editor.keyboard.displayText")}>
										<Input
											type="text"
											value={seg().displayText}
											onChange={(e) =>
												setProject(
													"timeline",
													"keyboardSegments",
													selectedIndex(),
													"displayText",
													e.target.value,
												)
											}
										/>
									</Subfield>
									<Subfield name={t("editor.keyboard.fadeDurationOverride")}>
										<Slider
											value={[
												(seg().fadeDurationOverride ??
													getSetting("fadeDuration")) * 100,
											]}
											onChange={(v) =>
												setProject(
													"timeline",
													"keyboardSegments",
													selectedIndex(),
													"fadeDurationOverride",
													v[0] / 100,
												)
											}
											minValue={0}
											maxValue={50}
											step={1}
										/>
									</Subfield>
								</div>
							</Field>
						)}
					</Show>

					<Show when={!hasKeyboardSegments()}>
						<div class="text-center text-sm text-gray-11 py-4">
							<p>{t("editor.keyboard.noSegments")}</p>
							<p class="text-xs mt-1 text-gray-10">
								{t("editor.keyboard.noSegmentsHint")}
							</p>
						</div>
					</Show>
				</div>
			</div>
		</Field>
	);
}
