import { cx } from "cva";
import { For } from "solid-js";
import { useI18n } from "~/i18n";
import IconLucideCheckCircle from "~icons/lucide/check-circle";
import IconLucideEdit from "~icons/lucide/edit";
import IconLucideImage from "~icons/lucide/image";
import IconLucideLayoutGrid from "~icons/lucide/layout-grid";
import IconLucideVideo from "~icons/lucide/video";

type FilterType = "all" | "videos" | "screenshots";
type StatusFilter = "all" | "editing" | "exported";

type Props = {
	typeFilter: FilterType;
	onTypeFilterChange: (filter: FilterType) => void;
	statusFilter: StatusFilter;
	onStatusFilterChange: (filter: StatusFilter) => void;
	stats: {
		total: number;
		videos: number;
		screenshots: number;
		editing: number;
		exported: number;
	};
};

export default function Sidebar(props: Props) {
	const { t } = useI18n();

	const typeFilters = [
		{
			id: "all" as const,
			label: t("library.sidebar.all"),
			icon: IconLucideLayoutGrid,
			count: () => props.stats.total,
		},
		{
			id: "videos" as const,
			label: t("library.sidebar.videos"),
			icon: IconLucideVideo,
			count: () => props.stats.videos,
		},
		{
			id: "screenshots" as const,
			label: t("library.sidebar.screenshots"),
			icon: IconLucideImage,
			count: () => props.stats.screenshots,
		},
	];

	const statusFilters = [
		{
			id: "all" as const,
			label: t("library.sidebar.allStatus"),
			icon: IconLucideLayoutGrid,
			count: () => props.stats.total,
		},
		{
			id: "editing" as const,
			label: t("library.sidebar.editing"),
			icon: IconLucideEdit,
			count: () => props.stats.editing,
		},
		{
			id: "exported" as const,
			label: t("library.sidebar.exported"),
			icon: IconLucideCheckCircle,
			count: () => props.stats.exported,
		},
	];

	const isStatusDisabled = (filterId: StatusFilter) => {
		if (props.typeFilter === "screenshots") {
			return filterId === "editing";
		}
		return false;
	};

	return (
		<aside class="w-56 bg-gray-2 border-r border-gray-4 flex flex-col p-4 gap-6">
			<div class="flex flex-col gap-2">
				<h3 class="text-xs font-semibold text-gray-11 uppercase tracking-wider px-2">
					{t("library.sidebar.type")}
				</h3>
				<For each={typeFilters}>
					{(filter) => (
						<button
							type="button"
							onClick={() => props.onTypeFilterChange(filter.id)}
							class={cx(
								"flex items-center gap-3 px-3 py-2 rounded-lg transition-all duration-200 text-left",
								props.typeFilter === filter.id
									? "bg-blue-9 text-white"
									: "hover:bg-gray-3 text-gray-12",
							)}
						>
							<filter.icon
								class={cx(
									"size-4",
									props.typeFilter === filter.id
										? "text-white"
										: "text-gray-11",
								)}
							/>
							<span class="flex-1 text-sm font-medium">{filter.label}</span>
							<span
								class={cx(
									"text-xs px-1.5 py-0.5 rounded",
									props.typeFilter === filter.id
										? "bg-white/20 text-white"
										: "bg-gray-4 text-gray-11",
								)}
							>
								{filter.count()}
							</span>
						</button>
					)}
				</For>
			</div>

			<div class="flex flex-col gap-2">
				<h3 class="text-xs font-semibold text-gray-11 uppercase tracking-wider px-2">
					{t("library.sidebar.status")}
				</h3>
				<For each={statusFilters}>
					{(filter) => {
						const disabled = isStatusDisabled(filter.id);
						return (
							<button
								type="button"
								onClick={() => {
									if (!disabled) {
										props.onStatusFilterChange(filter.id);
									}
								}}
								class={cx(
									"flex items-center gap-3 px-3 py-2 rounded-lg transition-all duration-200 text-left",
									disabled
										? "opacity-50 cursor-not-allowed"
										: props.statusFilter === filter.id
											? "bg-gray-5 text-gray-12"
											: "hover:bg-gray-3 text-gray-12",
								)}
								disabled={disabled}
							>
								<filter.icon
									class={cx(
										"size-4",
										disabled
											? "opacity-30 text-gray-11"
											: props.statusFilter === filter.id
												? "opacity-100 text-gray-12"
												: "opacity-60 text-gray-11",
									)}
								/>
								<span class="flex-1 text-sm font-medium">{filter.label}</span>
								<span
									class={cx(
										"text-xs px-1.5 py-0.5 rounded",
										props.statusFilter === filter.id
											? "bg-gray-7 text-gray-12"
											: "bg-gray-4 text-gray-11",
									)}
								>
									{filter.count()}
								</span>
							</button>
						);
					}}
				</For>
			</div>
		</aside>
	);
}
