import { createQuery } from "@tanstack/solid-query";
import type { Component, ComponentProps, JSX } from "solid-js";
import { Dynamic } from "solid-js/web";

import { useI18n } from "~/i18n";
import {
	createCurrentRecordingQuery,
	isSystemAudioSupported,
} from "~/utils/queries";
import { useRecordingOptions } from "../OptionsContext";
import InfoPill from "./InfoPill";

export default function SystemAudio() {
	return (
		<SystemAudioToggleRoot
			class="flex flex-row gap-2 items-center px-2 w-full h-[42px] rounded-lg border border-gray-5 transition-colors cursor-default disabled:opacity-70 bg-gray-3 disabled:text-gray-11 KSelect"
			PillComponent={InfoPill}
			icon={<IconPhMonitorBold class="text-gray-10 size-4" />}
		/>
	);
}

export function SystemAudioToggleRoot(
	props: Omit<
		ComponentProps<"button">,
		"onClick" | "disabled" | "title" | "type" | "children"
	> & {
		PillComponent: Component<{
			variant: "blue" | "red";
			children: JSX.Element;
		}>;
		icon: JSX.Element;
	},
) {
	const { t } = useI18n();
	const { rawOptions, setOptions } = useRecordingOptions();
	const currentRecording = createCurrentRecordingQuery();
	const systemAudioSupported = createQuery(() => isSystemAudioSupported);

	const isDisabled = () =>
		!!currentRecording.data || systemAudioSupported.data === false;
	const tooltipMessage = () => {
		if (systemAudioSupported.data === false) {
			return t("main.device.systemAudio.unsupported");
		}
		return undefined;
	};

	return (
		<button
			{...props}
			type="button"
			title={tooltipMessage()}
			onClick={() => {
				if (!rawOptions || isDisabled()) return;
				setOptions({ captureSystemAudio: !rawOptions.captureSystemAudio });
			}}
			disabled={isDisabled()}
		>
			{props.icon}
			<p class="flex-1 text-sm text-left truncate">
				{rawOptions.captureSystemAudio
					? t("main.device.systemAudio.on")
					: t("main.device.systemAudio.off")}
			</p>
			<Dynamic
				component={props.PillComponent}
				variant={rawOptions.captureSystemAudio ? "blue" : "red"}
			>
				{rawOptions.captureSystemAudio
					? t("main.device.pill.on")
					: t("main.device.pill.off")}
			</Dynamic>
		</button>
	);
}
