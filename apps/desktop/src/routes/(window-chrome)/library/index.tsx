import { createQuery, queryOptions } from "@tanstack/solid-query";
import { ask } from "@tauri-apps/plugin-dialog";
import { createMemo, createSignal, onMount, Show } from "solid-js";
import { useI18n } from "~/i18n";
import { createTauriEventListener } from "~/utils/createEventListener";
import { commands, events, type LibraryItem } from "~/utils/tauri";
import ContextMenu from "./ContextMenu";
import DetailPanel from "./DetailPanel";
import Grid from "./Grid";
import { executePrimaryAction } from "./library-actions";
import Sidebar from "./Sidebar";
import Toolbar from "./Toolbar";

type FilterType = "all" | "videos" | "screenshots";
type StatusFilter = "all" | "editing" | "exported";
type ViewMode = "grid" | "list" | "compact";

const libraryQuery = queryOptions<LibraryItem[]>({
	queryKey: ["library-items"],
	queryFn: async () => {
		const result = await commands
			.listLibraryItems()
			.catch(() => [] as LibraryItem[]);
		return result;
	},
	reconcile: "id",
	refetchOnMount: "always",
});

export default function Library() {
	const { t } = useI18n();

	const [typeFilter, setTypeFilter] = createSignal<FilterType>("all");
	const [statusFilter, setStatusFilter] = createSignal<StatusFilter>("all");
	const [viewMode, setViewMode] = createSignal<ViewMode>("grid");
	const [searchQuery, setSearchQuery] = createSignal("");
	const [selectedIds, setSelectedIds] = createSignal<Set<string>>(new Set());
	const [lastClickedIndex, setLastClickedIndex] = createSignal<number>(-1);
	const [contextMenu, setContextMenu] = createSignal<{
		item: LibraryItem;
		x: number;
		y: number;
	} | null>(null);

	const library = createQuery(() => libraryQuery);

	onMount(() => {
		library.refetch();
	});

	createTauriEventListener(events.recordingDeleted, () => library.refetch());
	createTauriEventListener(events.newScreenshotAdded, () => library.refetch());
	createTauriEventListener(events.newStudioRecordingAdded, () =>
		library.refetch(),
	);

	const handleTypeFilterChange = (newType: FilterType) => {
		setTypeFilter(newType);
		if (newType === "screenshots" && statusFilter() === "editing") {
			setStatusFilter("all");
		}
	};

	const filteredItems = createMemo(() => {
		let items = library.data ?? [];

		const type = typeFilter();
		if (type !== "all") {
			items = items.filter((item: LibraryItem) => {
				if (type === "videos") return item.itemType === "video";
				if (type === "screenshots") return item.itemType === "screenshot";
				return true;
			});
		}

		const status = statusFilter();
		if (status !== "all") {
			items = items.filter((item: LibraryItem) => {
				if (status === "editing") return item.status === "editing";
				if (status === "exported")
					return (
						item.status === "exported" || item.status === "exportedNoSource"
					);
				return true;
			});
		}

		const query = searchQuery().trim().toLowerCase();
		if (query) {
			items = items.filter((item) => {
				return item.name.toLowerCase().includes(query);
			});
		}

		return items;
	});

	const stats = createMemo(() => {
		const all = library.data ?? [];
		const videos = all.filter((item) => item.itemType === "video");
		const screenshots = all.filter((item) => item.itemType === "screenshot");

		return {
			total: all.length,
			videos: videos.length,
			screenshots: screenshots.length,
			editing: all.filter((item) => item.status === "editing").length,
			exported: all.filter(
				(item) =>
					item.status === "exported" || item.status === "exportedNoSource",
			).length,
		};
	});

	const handleRefresh = () => {
		library.refetch();
	};

	const items = () => filteredItems();
	const handleSelect = (e: MouseEvent, item: LibraryItem) => {
		const list = items();
		const index = list.findIndex((i) => i.id === item.id);
		if (e.ctrlKey || e.metaKey) {
			setSelectedIds((prev) => {
				const next = new Set(prev);
				if (next.has(item.id)) next.delete(item.id);
				else next.add(item.id);
				return next;
			});
			setLastClickedIndex(index);
			return;
		}
		if (e.shiftKey) {
			const last = lastClickedIndex();
			const from = last >= 0 ? Math.min(last, index) : index;
			const to = last >= 0 ? Math.max(last, index) : index;
			const ids = new Set<string>();
			for (let i = from; i <= to; i++) ids.add(list[i].id);
			setSelectedIds(ids);
			return;
		}
		setSelectedIds(new Set([item.id]));
		setLastClickedIndex(index);
	};

	const selectedItems = createMemo(() => {
		const ids = selectedIds();
		return items().filter((i) => ids.has(i.id));
	});
	const leadItem = () => {
		const s = selectedItems();
		return s.length === 1 ? s[0] : null;
	};

	const handleDoubleClickItem = (item: LibraryItem) => {
		executePrimaryAction(item);
	};

	const handleContextMenuItem = (e: MouseEvent, item: LibraryItem) => {
		setContextMenu({ item, x: e.clientX, y: e.clientY });
	};

	const handleContextMenuDelete = async (item: LibraryItem) => {
		const message =
			item.itemType === "video"
				? t("library.detail.confirmDeleteRecording")
				: t("library.detail.confirmDeleteScreenshot");
		if (!(await ask(message))) return;
		const pathToDelete = item.capProjectPath || item.exportedFilePath;
		if (pathToDelete) await commands.deleteLibraryItem(pathToDelete);
		library.refetch();
		setContextMenu(null);
	};

	const handleBatchDelete = async () => {
		const toDelete = selectedItems();
		if (toDelete.length === 0) return;
		const msg =
			toDelete.length === 1
				? t("library.detail.confirmDeleteRecording")
				: t("library.batch.deleteConfirm", { count: String(toDelete.length) });
		if (!(await ask(msg))) return;
		for (const item of toDelete) {
			const path = item.capProjectPath || item.exportedFilePath;
			if (path) await commands.deleteLibraryItem(path);
		}
		setSelectedIds(new Set<string>());
		library.refetch();
	};

	const handleOpenLibraryFolder = async () => {
		const path = await commands.getDefaultRecordingsPath();
		const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
		revealItemInDir(path);
	};

	return (
		<div class="flex h-full w-full bg-gray-1 text-gray-12">
			<Sidebar
				typeFilter={typeFilter()}
				onTypeFilterChange={handleTypeFilterChange}
				statusFilter={statusFilter()}
				onStatusFilterChange={setStatusFilter}
				stats={stats()}
			/>

			<div class="flex-1 flex flex-col min-w-0">
				<Toolbar
					searchQuery={searchQuery()}
					onSearchChange={setSearchQuery}
					viewMode={viewMode()}
					onViewModeChange={setViewMode}
					onRefresh={handleRefresh}
					onOpenLibraryFolder={handleOpenLibraryFolder}
				/>

				<div class="flex-1 flex min-h-0 relative">
					<div class="flex-1 overflow-auto custom-scroll">
						<Show
							when={filteredItems().length > 0}
							fallback={
								<div class="flex items-center justify-center h-full text-gray-10">
									<div class="text-center">
										<p class="text-lg font-medium">
											{t("library.empty.title")}
										</p>
										<p class="text-sm mt-2">{t("library.empty.description")}</p>
									</div>
								</div>
							}
						>
							<Grid
								items={filteredItems()}
								selectedIds={selectedIds()}
								onSelect={handleSelect}
								onDoubleClickItem={handleDoubleClickItem}
								onContextMenuItem={handleContextMenuItem}
								viewMode={viewMode()}
							/>
						</Show>
					</div>

					<Show when={leadItem()}>
						{(item) => (
							<DetailPanel
								item={item()}
								onClose={() => setSelectedIds(new Set())}
								onRefetch={handleRefresh}
							/>
						)}
					</Show>
					<Show when={selectedItems().length > 1}>
						<div class="absolute bottom-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 px-4 py-2 rounded-lg bg-gray-2 border border-gray-4 shadow-lg">
							<span class="text-sm text-gray-12">
								{t("library.batch.selected", {
									count: String(selectedItems().length),
								})}
							</span>
							<button
								type="button"
								class="px-3 py-1 text-sm rounded bg-red-9 text-white hover:bg-red-10"
								onClick={handleBatchDelete}
							>
								{t("library.batch.delete")}
							</button>
							<button
								type="button"
								class="px-3 py-1 text-sm rounded bg-gray-4 text-gray-12 hover:bg-gray-5"
								onClick={() => setSelectedIds(new Set())}
							>
								{t("library.batch.cancel")}
							</button>
						</div>
					</Show>
					<Show when={contextMenu()}>
						{(menu) => (
							<ContextMenu
								item={menu().item}
								x={menu().x}
								y={menu().y}
								onClose={() => setContextMenu(null)}
								onDelete={() => handleContextMenuDelete(menu().item)}
							/>
						)}
					</Show>
				</div>
			</div>
		</div>
	);
}
