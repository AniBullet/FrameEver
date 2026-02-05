import { createQuery, queryOptions } from "@tanstack/solid-query";
import { createMemo, createSignal, Show } from "solid-js";
import { useI18n } from "~/i18n";
import { createTauriEventListener } from "~/utils/createEventListener";
import { commands, events, type LibraryItem } from "~/utils/tauri";
import DetailPanel from "./DetailPanel";
import Grid from "./Grid";
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
});

export default function Library() {
	const { t } = useI18n();

	const [typeFilter, setTypeFilter] = createSignal<FilterType>("all");
	const [statusFilter, setStatusFilter] = createSignal<StatusFilter>("all");
	const [viewMode, setViewMode] = createSignal<ViewMode>("grid");
	const [searchQuery, setSearchQuery] = createSignal("");
	const [selectedItem, setSelectedItem] = createSignal<LibraryItem | null>(
		null,
	);

	const library = createQuery(() => libraryQuery);

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
				/>

				<div class="flex-1 flex min-h-0">
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
								selectedItem={selectedItem()}
								onSelectItem={setSelectedItem}
								viewMode={viewMode()}
							/>
						</Show>
					</div>

					<Show when={selectedItem()}>
						{(item) => (
							<DetailPanel
								item={item()}
								onClose={() => setSelectedItem(null)}
								onRefetch={handleRefresh}
							/>
						)}
					</Show>
				</div>
			</div>
		</div>
	);
}
