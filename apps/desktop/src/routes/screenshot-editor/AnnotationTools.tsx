import { cx } from "cva";
import type { Component } from "solid-js";
import Tooltip from "~/components/Tooltip";
import type { TranslationKey } from "~/i18n";
import { useI18n } from "~/i18n";
import IconLucideArrowUpRight from "~icons/lucide/arrow-up-right";
import IconLucideCircle from "~icons/lucide/circle";
import IconLucideEyeOff from "~icons/lucide/eye-off";
import IconLucideLayers from "~icons/lucide/layers";
import IconLucideMousePointer2 from "~icons/lucide/mouse-pointer-2";
import IconLucideSquare from "~icons/lucide/square";
import IconLucideType from "~icons/lucide/type";
import { type AnnotationType, useScreenshotEditorContext } from "./context";

export function AnnotationTools() {
	const { t } = useI18n();
	const { layersPanelOpen, setLayersPanelOpen } = useScreenshotEditorContext();

	return (
		<div class="flex items-center gap-1">
			<Tooltip content={t("editor.screenshot.layers")} kbd={["L"]}>
				<button
					type="button"
					onClick={() => setLayersPanelOpen(!layersPanelOpen())}
					class={cx(
						"flex items-center justify-center rounded-[0.5rem] transition-all size-8",
						layersPanelOpen()
							? "bg-blue-3 text-blue-11"
							: "bg-transparent hover:bg-gray-3 text-gray-11",
					)}
				>
					<IconLucideLayers class="size-4" />
				</button>
			</Tooltip>
			<div class="w-px h-4 bg-gray-4 mx-1" />
			<ToolButton
				tool="select"
				icon={IconLucideMousePointer2}
				labelKey="editor.screenshot.tool.select"
				shortcut="V"
			/>
			<ToolButton
				tool="arrow"
				icon={IconLucideArrowUpRight}
				labelKey="editor.screenshot.tool.arrow"
				shortcut="A"
			/>
			<ToolButton
				tool="rectangle"
				icon={IconLucideSquare}
				labelKey="editor.screenshot.tool.rectangle"
				shortcut="R"
			/>
			<ToolButton
				tool="mask"
				icon={IconLucideEyeOff}
				labelKey="editor.screenshot.tool.mask"
				shortcut="M"
			/>
			<ToolButton
				tool="circle"
				icon={IconLucideCircle}
				labelKey="editor.screenshot.tool.circle"
				shortcut="C"
			/>
			<ToolButton
				tool="text"
				icon={IconLucideType}
				labelKey="editor.screenshot.tool.text"
				shortcut="T"
			/>
		</div>
	);
}

function ToolButton(props: {
	tool: AnnotationType | "select";
	icon: Component<{ class?: string }>;
	labelKey: TranslationKey;
	shortcut?: string;
}) {
	const { t } = useI18n();
	const { activeTool, setActiveTool, setSelectedAnnotationId } =
		useScreenshotEditorContext();
	return (
		<Tooltip
			content={t(props.labelKey)}
			kbd={props.shortcut ? [props.shortcut] : undefined}
		>
			<button
				type="button"
				onClick={() => {
					setActiveTool(props.tool);
					if (props.tool !== "select") {
						setSelectedAnnotationId(null);
					}
				}}
				class={cx(
					"flex items-center justify-center rounded-[0.5rem] transition-all size-8",
					activeTool() === props.tool
						? "bg-blue-3 text-blue-11"
						: "bg-transparent hover:bg-gray-3 text-gray-11",
				)}
			>
				<props.icon class="size-4" />
			</button>
		</Tooltip>
	);
}
