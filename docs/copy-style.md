# Copy style

How FrameEver's UI text is written in English and Simplified Chinese. `docs/i18n.md` covers how text is translated and the term table; this page covers what the text says and how it reads. Decided in "Copy tone for English and Simplified Chinese" (#52).

## Voice

- Speak to the user as "you" / 你 (never 您). Calm, direct, plain.
- No exclamation marks, no "Please", "Oops", "Sorry", "successfully", no 请… politeness padding.
- Welcome and onboarding keep their warmer, lightly playful voice, matching their hand-drawn style. Everywhere else stays neutral.

## Length

- Labels and buttons: at most 3 English words, 2–6 Chinese characters.
- Descriptions: one sentence.
- Tooltips: the control's name plus its shortcut, not a sentence (`Split  S`).

## Capitalisation (English)

- Sentence case everywhere: buttons, headings, menus, settings, tabs. Only the first word is capitalised.
- Proper nouns keep their own capitals wherever they sit: FrameEver, Windows, File Explorer, Recycle Bin, GIF, MP4, HEVC, Whisper.
- FrameEver's own proper nouns are the mode names Studio, Instant and Screenshot, and Studio Sound. Any other feature (auto zoom, captions, library, teleprompter) is an ordinary word. A new feature name becomes a proper noun only once it is listed in `GLOSSARY.md`.

| Before | After |
|---|---|
| Fade Duration | Fade duration |
| Camera Only | Camera only |
| Camera Only padding | Camera only padding |
| Top Left | Top left |

## Buttons

- A verb, naming the action: Save, Export, Delete recording, 开始录制. Never OK, Yes, No, 确定 on a choice.

## Errors

Every error says what happened and what to do next.

- English: `Couldn't <verb> <object>.`, then the cause if known, then the next step. Not "Failed to…" or "Could not…".
- Chinese: `无法<动词><对象>。` with the same follow-ups.
- The raw error text never goes in the message body. It sits behind a Details expander, with an Export logs action.
- Never point to support. The way out is Export logs or Report on GitHub.

| Before | After |
|---|---|
| Failed to load recording data: {error} | Couldn't open this recording. Its project files may be damaged. [Details] [Export logs] |
| Failed to save automations. Your changes have not been saved. | Couldn't save your automations. Your changes are still here; try again. |
| Could not save onboarding progress. Please try again. | Couldn't save your progress. Try again. |
| The recording failed so this file may have issues in the editor! If your having issues recovering the file please reach out to support! | This recording stopped early, so part of it may be missing. [Export logs] [Report on GitHub] |
| 无法记住你的动态渐变设置。 | 无法保存动态渐变设置。再试一次。 |

## Success and progress

- Success is a past-tense state with no exclamation mark: Copied / 已复制.
- Ongoing work uses "…" and a percentage when known: Exporting… 42% / 正在导出… 42%.
- No success message when the result is already visible.

| Before | After |
|---|---|
| Screenshot copied to clipboard! | Screenshot copied / 截图已复制 |
| Export complete! | Exported / 已导出 |
| Video saved! | Video saved / 视频已保存 |

## Confirmations

- Confirm only what can't be undone. Deletes in the library go to the Recycle Bin and show an Undo message instead of a dialog.
- Title: the question, naming the object. Body: the consequence. Buttons: the specific verb and Cancel.
- Never "Are you sure…".

| Before | After |
|---|---|
| Are you sure you want to delete this recording? | Deleted "Demo 3" [Undo] (no dialog) |
| Delete the original video? The compressed version stays, but the original can't be restored. | Title: Delete the original video? Body: The compressed copy stays. This can't be undone. Buttons: Delete original · Cancel |

## Empty states

- One line saying what appears here, then one action button. No "to get started".

| Before | After |
|---|---|
| Your screen recordings will appear here. Start recording to get started! | Recordings you make appear here. [Start recording] |
| No presets yet — create one in the editor first. | Presets you save in the editor appear here. [Open editor] |

## Simplified Chinese

- Natural, concise Chinese, not word-for-word. The product is 帧刻 in Chinese text.
- Full-width punctuation in sentences (，。：；？！“”), ASCII for keys, numbers and units.
- Always a half-width space between Chinese and Latin words or numbers; none next to full-width punctuation; none between a number and its unit or %: 打开 macOS 设置, 导出 3 个文件, 42%.
- Instant is 即时 (即时录制, 即时模式); Studio is 工作室; Screenshot is 截图.
- Proper nouns stay English (Windows, Whisper, HEVC, CLI, Webhook, PATH), with the full term table in `docs/i18n.md`.
