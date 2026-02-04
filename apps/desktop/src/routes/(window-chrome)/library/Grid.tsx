import { convertFileSrc } from "@tauri-apps/api/core";
import { cx } from "cva";
import { createSignal, For, Match, Show, Switch } from "solid-js";
import { useI18n } from "~/i18n";
import type { LibraryItem } from "~/utils/tauri";
import IconCapFilmCut from "~icons/cap/film-cut";
import IconCapInstant from "~icons/cap/instant";
import IconLucideCheckCircle from "~icons/lucide/check-circle";
import IconLucideEdit from "~icons/lucide/edit";
import IconLucideFileVideo from "~icons/lucide/file-video";
import IconLucideImage from "~icons/lucide/image";
import IconLucideVideo from "~icons/lucide/video";

type ViewMode = "grid" | "list" | "compact";

type Props = {
	items: LibraryItem[];
	selectedItem: LibraryItem | null;
	onSelectItem: (item: LibraryItem) => void;
	viewMode: ViewMode;
};

export default function Grid(props: Props) {
	return (
		<Switch>
			<Match when={props.viewMode === "grid"}>
				<div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4 p-6">
					<For each={props.items}>
						{(item) => (
							<GridCard
								item={item}
								isSelected={props.selectedItem?.id === item.id}
								onSelect={() => props.onSelectItem(item)}
							/>
						)}
					</For>
				</div>
			</Match>
			<Match when={props.viewMode === "list"}>
				<div class="flex flex-col p-6 gap-2">
					<For each={props.items}>
						{(item) => (
							<ListCard
								item={item}
								isSelected={props.selectedItem?.id === item.id}
								onSelect={() => props.onSelectItem(item)}
							/>
						)}
					</For>
				</div>
			</Match>
			<Match when={props.viewMode === "compact"}>
				<div class="flex flex-col p-6">
					<For each={props.items}>
						{(item) => (
							<CompactCard
								item={item}
								isSelected={props.selectedItem?.id === item.id}
								onSelect={() => props.onSelectItem(item)}
							/>
						)}
					</For>
				</div>
			</Match>
		</Switch>
	);
}

function GridCard(props: {
	item: LibraryItem;
	isSelected: boolean;
	onSelect: () => void;
}) {
	const { t } = useI18n();
	const [imageExists, setImageExists] = createSignal(true);

	const thumbnailSrc = () => {
		if (props.item.thumbnailPath) {
			return `${convertFileSrc(props.item.thumbnailPath)}?t=${Date.now()}`;
		}
		return "";
	};

	const statusIcon = () => {
		switch (props.item.status) {
			case "editing":
				return <IconLucideEdit class="size-3 text-orange-500" />;
			case "exported":
			case "exportedNoSource":
				return <IconLucideCheckCircle class="size-3 text-green-500" />;
		}
	};

	const statusLabel = () => {
		switch (props.item.status) {
			case "editing":
				return t("library.status.editing");
			case "exported":
			case "exportedNoSource":
				return t("library.status.exported");
		}
	};

	return (
		<button
			type="button"
			onClick={props.onSelect}
			class={cx(
				"group relative flex flex-col bg-gray-2 rounded-xl border-2 transition-all duration-200 overflow-hidden hover:shadow-lg hover:scale-[1.02]",
				props.isSelected
					? "border-blue-9 shadow-lg"
					: "border-gray-4 hover:border-gray-6",
			)}
		>
			<div class="aspect-video w-full bg-gray-3 relative overflow-hidden">
				<Show
					when={imageExists() && thumbnailSrc()}
					fallback={
						<div class="absolute inset-0 flex items-center justify-center">
							{props.item.itemType === "video" ? (
								<IconLucideVideo class="size-12 text-gray-8" />
							) : (
								<IconLucideImage class="size-12 text-gray-8" />
							)}
						</div>
					}
				>
					<img
						src={thumbnailSrc()}
						alt={props.item.name}
						class="w-full h-full object-cover"
						onError={() => setImageExists(false)}
					/>
				</Show>

				<div class="absolute top-2 left-2 flex items-center gap-1.5 px-2 py-1 rounded-full bg-gray-900/80 backdrop-blur-sm">
					{statusIcon()}
					<span class="text-[10px] font-medium text-white">
						{statusLabel()}
					</span>
				</div>
			</div>

			<div class="p-3 flex flex-col gap-1 text-left">
				<p
					class="text-sm font-medium text-gray-12 truncate"
					title={props.item.name}
				>
					{props.item.name}
				</p>
				<p class="text-xs text-gray-10">
					{props.item.itemType === "video"
						? t("library.type.video")
						: t("library.type.screenshot")}
				</p>
			</div>
		</button>
	);
}

function ListCard(props: {
	item: LibraryItem;
	isSelected: boolean;
	onSelect: () => void;
}) {
	const { t } = useI18n();
	const [imageExists, setImageExists] = createSignal(true);

	const thumbnailSrc = () => {
		if (props.item.thumbnailPath) {
			return `${convertFileSrc(props.item.thumbnailPath)}?t=${Date.now()}`;
		}
		return "";
	};

	const statusLabel = () => {
		switch (props.item.status) {
			case "Editing":
				return t("library.status.editing");
			case "Exported":
			case "ExportedNoSource":
				return t("library.status.exported");
		}
	};

	const createdDate = () => {
		const date = new Date(props.item.createdAt * 1000);
		return date.toLocaleString();
	};

	return (
		<button
			type="button"
			onClick={props.onSelect}
			class={cx(
				"flex items-center gap-4 p-4 rounded-lg border-2 transition-all duration-200 hover:shadow-md",
				props.isSelected
					? "border-blue-9 bg-blue-1 shadow-md"
					: "border-gray-4 bg-gray-2 hover:border-gray-6 hover:bg-gray-3",
			)}
		>
			<div class="w-32 h-20 bg-gray-3 rounded-lg overflow-hidden flex-shrink-0">
				<Show
					when={imageExists() && thumbnailSrc()}
					fallback={
						<div class="w-full h-full flex items-center justify-center">
							{props.item.itemType === "video" ? (
								<IconLucideVideo class="size-8 text-gray-8" />
							) : (
								<IconLucideImage class="size-8 text-gray-8" />
							)}
						</div>
					}
				>
					<img
						src={thumbnailSrc()}
						alt={props.item.name}
						class="w-full h-full object-cover"
						onError={() => setImageExists(false)}
					/>
				</Show>
			</div>

			<div class="flex-1 flex items-center gap-4 text-left min-w-0">
				<div class="flex-1 min-w-0">
					<p
						class="text-sm font-medium text-gray-12 truncate"
						title={props.item.name}
					>
						{props.item.name}
					</p>
					<p class="text-xs text-gray-10 mt-0.5">
						{props.item.itemType === "video"
							? t("library.type.video")
							: t("library.type.screenshot")}
					</p>
				</div>

				<div class="flex items-center gap-6 text-xs text-gray-11">
					<span class="whitespace-nowrap">{statusLabel()}</span>
					<span class="whitespace-nowrap">{createdDate()}</span>
				</div>
			</div>
		</button>
	);
}

function CompactCard(props: {
	item: LibraryItem;
	isSelected: boolean;
	onSelect: () => void;
}) {
	const [imageExists, setImageExists] = createSignal(true);

	const thumbnailSrc = () => {
		if (props.item.thumbnailPath) {
			return `${convertFileSrc(props.item.thumbnailPath)}?t=${Date.now()}`;
		}
		return "";
	};

	const createdDate = () => {
		const date = new Date(props.item.createdAt * 1000);
		return date.toLocaleDateString();
	};

	return (
		<button
			type="button"
			onClick={props.onSelect}
			class={cx(
				"flex items-center gap-3 px-4 py-2 border-b border-gray-4 transition-colors hover:bg-gray-2",
				props.isSelected && "bg-blue-1",
			)}
		>
			<div class="w-10 h-10 bg-gray-3 rounded overflow-hidden flex-shrink-0">
				<Show
					when={imageExists() && thumbnailSrc()}
					fallback={
						<div class="w-full h-full flex items-center justify-center">
							{props.item.itemType === "video" ? (
								<IconLucideFileVideo class="size-5 text-gray-8" />
							) : (
								<IconLucideImage class="size-5 text-gray-8" />
							)}
						</div>
					}
				>
					<img
						src={thumbnailSrc()}
						alt={props.item.name}
						class="w-full h-full object-cover"
						onError={() => setImageExists(false)}
					/>
				</Show>
			</div>

			<span
				class="flex-1 text-sm text-gray-12 text-left truncate"
				title={props.item.name}
			>
				{props.item.name}
			</span>

			<span class="text-xs text-gray-10 whitespace-nowrap">
				{createdDate()}
			</span>
		</button>
	);
}
