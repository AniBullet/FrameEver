import * as dialog from "@tauri-apps/plugin-dialog";
import type { createOptionsQuery } from "./queries";
import type { RecordingAction } from "./tauri";

export function handleRecordingResult(
	result: Promise<RecordingAction>,
	_setOptions: ReturnType<typeof createOptionsQuery>["setOptions"] | undefined,
) {
	return result
		.then(async (result) => {
			if (result === "Started") return;
			await dialog.message(`Error: ${result}`, {
				title: "Error starting recording",
			});
		})
		.catch((err) =>
			dialog.message(err, {
				title: "Error starting recording",
				kind: "error",
			}),
		);
}
