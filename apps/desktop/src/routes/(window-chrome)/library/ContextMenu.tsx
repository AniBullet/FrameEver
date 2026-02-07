import { createEventListener } from "@solid-primitives/event-listener";
import { onCleanup, onMount } from "solid-js";
import { useI18n } from "~/i18n";
import type { LibraryItem } from "~/utils/tauri";
import IconLucideEdit from "~icons/lucide/edit";
import IconLucideExternalLink from "~icons/lucide/external-link";
import IconLucideFolder from "~icons/lucide/folder";
import IconLucidePlay from "~icons/lucide/play";
import IconLucideTrash from "~icons/lucide/trash-2";
import {
	canEdit,
	canOpen,
	canPlay,
	editItem,
	openFolder as openFolderForItem,
	openWithDefaultApp,
} from "./library-actions";

type Props = {
	item: LibraryItem;
	x: number;
	y: number;
	onClose: () => void;
	onDelete: (item: LibraryItem) => void;
};

export default function ContextMenu(props: Props) {
	const { t } = useI18n();

	onMount(() => {
		const handler = (e: MouseEvent) => {
			const target = e.target as Node;
			if (!document.getElementById("library-context-menu")?.contains(target)) {
				props.onClose();
			}
		};
		createEventListener(window, "click", handler, { capture: true });
		createEventListener(window, "contextmenu", () => props.onClose(), {
			capture: true,
		});
		onCleanup(() => {
			window.removeEventListener("click", handler, true);
		});
	});

	const play = () => {
		if (props.item.exportedFilePath)
			openWithDefaultApp(props.item.exportedFilePath);
		props.onClose();
	};
	const open = () => {
		if (props.item.exportedFilePath)
			openWithDefaultApp(props.item.exportedFilePath);
		props.onClose();
	};
	const edit = () => {
		editItem(props.item);
		props.onClose();
	};
	const openFolder = () => {
		openFolderForItem(props.item);
		props.onClose();
	};
	const del = () => {
		props.onDelete(props.item);
		props.onClose();
	};

	return (
		<div
			id="library-context-menu"
			role="menu"
			class="fixed z-[100] min-w-[10rem] py-1 rounded-lg bg-gray-2 border border-gray-4 shadow-lg"
			style={{ left: `${props.x}px`, top: `${props.y}px` }}
		>
			{canPlay(props.item) && (
				<button
					type="button"
					role="menuitem"
					class="w-full flex items-center gap-2 px-3 py-2 text-left text-sm text-gray-12 hover:bg-gray-4"
					onClick={play}
				>
					<IconLucidePlay class="size-4" />
					{t("library.detail.play")}
				</button>
			)}
			{canOpen(props.item) && (
				<button
					type="button"
					role="menuitem"
					class="w-full flex items-center gap-2 px-3 py-2 text-left text-sm text-gray-12 hover:bg-gray-4"
					onClick={open}
				>
					<IconLucideExternalLink class="size-4" />
					{t("library.detail.open")}
				</button>
			)}
			{canEdit(props.item) && (
				<button
					type="button"
					role="menuitem"
					class="w-full flex items-center gap-2 px-3 py-2 text-left text-sm text-gray-12 hover:bg-gray-4"
					onClick={edit}
				>
					<IconLucideEdit class="size-4" />
					{t("library.detail.edit")}
				</button>
			)}
			<button
				type="button"
				role="menuitem"
				class="w-full flex items-center gap-2 px-3 py-2 text-left text-sm text-gray-12 hover:bg-gray-4"
				onClick={openFolder}
			>
				<IconLucideFolder class="size-4" />
				{t("library.detail.openFolder")}
			</button>
			<button
				type="button"
				role="menuitem"
				class="w-full flex items-center gap-2 px-3 py-2 text-left text-sm text-red-11 hover:bg-gray-4"
				onClick={del}
			>
				<IconLucideTrash class="size-4" />
				{t("library.detail.delete")}
			</button>
		</div>
	);
}
