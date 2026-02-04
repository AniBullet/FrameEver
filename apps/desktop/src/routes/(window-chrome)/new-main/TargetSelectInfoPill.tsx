import type { Component, ComponentProps } from "solid-js";
import { Dynamic } from "solid-js/web";
import { useI18n } from "~/i18n";

export default function TargetSelectInfoPill<T>(props: {
	value: T | null;
	permissionGranted: boolean;
	requestPermission: () => void;
	onClick: (e: MouseEvent) => void;
	PillComponent: Component<
		ComponentProps<"button"> & { variant: "blue" | "red" }
	>;
}) {
	const { t } = useI18n();

	return (
		<Dynamic
			component={props.PillComponent}
			variant={props.value !== null && props.permissionGranted ? "blue" : "red"}
			onPointerDown={(e) => {
				if (!props.permissionGranted || props.value === null) return;

				e.stopPropagation();
			}}
			onClick={(e) => {
				if (!props.permissionGranted) {
					props.requestPermission();
					return;
				}

				props.onClick(e);
			}}
		>
			{!props.permissionGranted
				? t("main.device.pill.requestPermission")
				: props.value !== null
					? t("main.device.pill.on")
					: t("main.device.pill.off")}
		</Dynamic>
	);
}
