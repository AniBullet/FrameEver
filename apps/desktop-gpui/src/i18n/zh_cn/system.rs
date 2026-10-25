pub static ENTRIES: &[(&str, &str)] = &[
    (
        "This recording already has a compressed version",
        "这个录制文件已经有压缩版了",
    ),
    ("Screen Recording", "屏幕录制"),
    ("Accessibility", "辅助功能"),
    ("Microphone", "麦克风"),
    ("Camera", "摄像头"),
    (
        "{permission} access is unavailable. Allow FrameEver in System Settings > Privacy & Security > {permission}, then select the device again.",
        "无法访问{permission}。请在“系统设置 > 隐私与安全性 > {permission}”中允许帧刻，然后重新选择设备。",
    ),
    ("Screen & System Audio Recording", "录屏与系统录音"),
    ("Drag FrameEver into the list", "把帧刻拖进列表"),
    (
        "Drop it on the {pane} list above, then make sure its switch is on.",
        "把它拖到上方的“{pane}”列表里，然后确认开关已打开。",
    ),
    ("Turn on in Settings", "在设置中开启"),
    ("Allow access", "允许访问"),
    ("Mono", "单声道"),
    ("Stereo", "立体声"),
    (
        "Camera is no longer available. Reconnect it and try again.",
        "摄像头已不可用。请重新连接后再试。",
    ),
    (
        "Could not read camera formats. Check camera access and try again.",
        "无法读取摄像头格式。请检查摄像头权限后再试。",
    ),
    (
        "No camera formats are available. Check camera access and try again.",
        "没有可用的摄像头格式。请检查摄像头权限后再试。",
    ),
    (
        "Microphone '{name}' is no longer available",
        "麦克风“{name}”已不可用",
    ),
    (
        "Could not read microphone formats: {error}",
        "无法读取麦克风格式：{error}",
    ),
    ("Display {id}", "显示器 {id}"),
    (
        "Could not resolve the clipboard file: {error}",
        "无法解析要复制的文件：{error}",
    ),
    (
        "Could not read the clipboard file: {error}",
        "无法读取要复制的文件：{error}",
    ),
    (
        "Only files can be copied to the clipboard",
        "只能把文件复制到剪贴板",
    ),
    (
        "Could not read the clipboard image: {error}",
        "无法读取要复制的图片：{error}",
    ),
    ("Images", "图片"),
    ("Audio", "音频"),
    ("Export", "导出"),
    (
        "The clipboard file path is not valid Unicode",
        "要复制的文件路径不是有效的 Unicode",
    ),
    ("Clipboard unavailable: {error}", "剪贴板不可用：{error}"),
    (
        "Could not copy the file to the clipboard: {error}",
        "无法把文件复制到剪贴板：{error}",
    ),
    (
        "Could not load the clipboard image: {error}",
        "无法载入要复制的图片：{error}",
    ),
    (
        "Could not copy the image to the clipboard: {error}",
        "无法把图片复制到剪贴板：{error}",
    ),
    (
        "Could not confirm capture state: {error}",
        "无法确认录制状态：{error}",
    ),
    ("Timed out confirming capture state", "确认录制状态超时"),
    (
        "Capture did not acknowledge the requested pause state",
        "录制未响应暂停或继续的请求",
    ),
    ("Recording task failed.", "录制任务失败。"),
    (
        "{error}. Capture cleanup is unconfirmed. Use Ctrl+Shift+F9 to retry Stop.",
        "{error}。无法确认录制已清理。请按 Ctrl+Shift+F9 重试停止。",
    ),
    ("Recording ended unexpectedly.", "录制意外结束。"),
    (
        "Recording completion task failed: {error}",
        "录制收尾任务失败：{error}",
    ),
    (
        "Low storage. Stopping and saving your recording\u{2026}",
        "存储空间不足。正在停止并保存录制…",
    ),
    ("Stop task failed: {error}", "停止任务失败：{error}"),
    ("Capture stop was not acknowledged", "录制未响应停止请求"),
    ("{first}; cleanup: {error}", "{first}；清理：{error}"),
    (
        "{error}. Studio shutdown remains unconfirmed; recording and files retained.",
        "{error}。无法确认工作室录制已关闭；录制和文件均已保留。",
    ),
    (
        "{error}. Capture cleanup is unconfirmed. Use your recording Stop control to retry.",
        "{error}。无法确认录制已清理。请用录制的停止按钮重试。",
    ),
    (
        "Recording stopped because storage is low. Your recording was saved.",
        "存储空间不足，录制已停止。录制文件已保存。",
    ),
    (
        "Recording stopped because storage is low. Your recording files were kept. Free up space, then recover the recording below.",
        "存储空间不足，录制已停止。录制文件已保留。请腾出空间，然后在下方恢复这段录制。",
    ),
    ("Stop task failed.", "停止任务失败。"),
    (
        "Capture control task failed: {error}",
        "录制控制任务失败：{error}",
    ),
    (
        "{error}. Use Stop if recording cannot continue safely.",
        "{error}。如果录制无法安全继续，请点停止。",
    ),
    (
        "{error}; discard/restart stopped, local recording retained",
        "{error}；已停止丢弃或重新开始，本地录制已保留",
    ),
    (
        "Studio discard task failed: {error}; cleanup is unconfirmed",
        "丢弃工作室录制的任务失败：{error}；无法确认已清理",
    ),
    (
        "Studio discard stop was not acknowledged; local recording retained",
        "工作室录制未响应丢弃请求；本地录制已保留",
    ),
    (
        "Studio capture cleanup is unconfirmed; local files preserved",
        "无法确认工作室录制已清理；本地文件已保留",
    ),
    (
        "Recording finalization ended without a result. Your files were retained.",
        "录制收尾未返回结果。文件已保留。",
    ),
    (
        "Windows startup cleanup failed: {cleanup}; partial recording preserved",
        "Windows 启动清理失败：{cleanup}；已保留部分录制",
    ),
    (
        "Failed capture shutdown is unconfirmed; local files preserved",
        "无法确认失败的录制已关闭；本地文件已保留",
    ),
    (
        "Capture stopped, but stopped metadata could not be saved; local files preserved",
        "录制已停止，但无法保存停止信息；本地文件已保留",
    ),
    (
        "Recording pipeline failed; local files preserved",
        "录制流程出错；本地文件已保留",
    ),
    (
        "Studio discard stop is unconfirmed; local files preserved",
        "无法确认工作室录制已停止丢弃；本地文件已保留",
    ),
    (
        "Studio discard is unconfirmed; local files preserved",
        "无法确认工作室录制已丢弃；本地文件已保留",
    ),
    (
        "Low storage. Your recording files are preserved at {path}. Free up space, then recover the recording in FrameEver.",
        "存储空间不足。录制文件已保留在 {path}。请腾出空间，然后在帧刻中恢复这段录制。",
    ),
    (
        "This instant recording is not waiting to be saved.",
        "这段快速录制不在等待保存。",
    ),
    (
        "The recovered recording has no video track.",
        "恢复的录制没有视频轨道。",
    ),
    (
        "The recovered recording has an invalid frame rate.",
        "恢复的录制帧率无效。",
    ),
    (
        "Camera-only recording requires a selected camera.",
        "仅摄像头录制需要先选择一个摄像头。",
    ),
    (
        "Selected microphone '{label}' is not the connected microphone '{device}'. Please select it again.",
        "所选麦克风“{label}”与已连接的麦克风“{device}”不一致。请重新选择。",
    ),
    (
        "Selected microphone '{label}' is unavailable",
        "所选麦克风“{label}”不可用",
    ),
    (
        "The selected camera is not the connected camera. Please select it again.",
        "所选摄像头与已连接的摄像头不一致。请重新选择。",
    ),
    (
        "Low storage: only {available} GB is available. Free up at least {required} MB before recording.",
        "存储空间不足：仅剩 {available} GB。请至少腾出 {required} MB 再录制。",
    ),
    (
        "Could not check recording storage: {error}",
        "无法检查录制存储空间：{error}",
    ),
    (
        "Device selection changed before it was ready",
        "设备在准备好之前已更换",
    ),
    ("Camera preview worker is unavailable", "摄像头预览不可用"),
    (
        "Device selection changed before permission was granted",
        "设备在获得权限之前已更换",
    ),
    (
        "Device selection changed before the format was applied",
        "设备在应用格式之前已更换",
    ),
    (
        "Camera preview subscription is unavailable",
        "摄像头预览不可用",
    ),
    ("No logs were found in {path}", "在 {path} 中没有找到日志"),
    ("Studio", "工作室"),
    ("Instant", "快速"),
    ("Both", "两者"),
    ("A diagnostic is already running.", "已有诊断正在运行。"),
    ("studio", "工作室"),
    ("instant", "快速"),
    ("Preparing the {mode} test...", "正在准备{mode}测试…"),
    ("Preparing the test...", "正在准备测试…"),
    ("Recording the {mode} test...", "正在录制{mode}测试…"),
    ("Recording the test...", "正在录制测试…"),
    (
        "Playing the test pattern {mode}...",
        "正在播放{mode}测试图案…",
    ),
    ("Playing the test pattern...", "正在播放测试图案…"),
    ("Finalizing the {mode} recording...", "正在完成{mode}录制…"),
    ("Finalizing the recording...", "正在完成录制…"),
    ("Analyzing the {mode} recording...", "正在分析{mode}录制…"),
    ("Analyzing the recording...", "正在分析录制…"),
    ("Exporting the {mode} recording...", "正在导出{mode}录制…"),
    ("Exporting the recording...", "正在导出录制…"),
    ("Wrapping up the sync test...", "正在结束同步测试…"),
    ("Starting the diagnostic...", "正在开始诊断…"),
    ("Collecting system information...", "正在收集系统信息…"),
    ("Cancelling...", "正在取消…"),
    (
        "Failed to start {binary}: {error}",
        "无法启动 {binary}：{error}",
    ),
    (
        "The self-test produced no output stream",
        "自检没有产生输出",
    ),
    ("The self-test exited with {status}", "自检已退出：{status}"),
    (
        "The self-test could not be waited on: {error}",
        "无法等待自检结束：{error}",
    ),
    (
        "Failed to create the diagnostics folder: {error}",
        "无法创建诊断文件夹：{error}",
    ),
    (
        "Failed to serialize the report: {error}",
        "无法生成报告：{error}",
    ),
    (
        "Failed to write {path}: {error}",
        "无法写入 {path}：{error}",
    ),
    (
        "Path is not inside a recordings directory",
        "路径不在录制文件夹中",
    ),
    (
        "Failed to resolve recording path: {error}",
        "无法解析录制路径：{error}",
    ),
    (
        "Failed to load recording metadata: {error}",
        "无法载入录制信息：{error}",
    ),
    (
        "Could not lock the instant recording for recovery: {error}",
        "无法锁定这段快速录制以进行恢复：{error}",
    ),
    (
        "Could not save the instant recording: {error}",
        "无法保存这段快速录制：{error}",
    ),
    (
        "Only incomplete studio recordings can be recovered",
        "只能恢复未完成的工作室录制",
    ),
    (
        "Recording is not waiting for recovery",
        "这段录制不需要恢复",
    ),
    ("No recoverable segments found", "没有找到可恢复的片段"),
    (
        "Failed to recover recording: {error}",
        "无法恢复录制：{error}",
    ),
    (
        "Recovered recording has no display segments",
        "恢复的录制没有屏幕画面片段",
    ),
    ("Invalid path", "路径无效"),
    (
        "Failed to inspect recording path: {error}",
        "无法检查录制路径：{error}",
    ),
    (
        "Failed to delete recording: {error}",
        "无法删除录制：{error}",
    ),
    ("Failed to copy file: {error}", "无法复制文件：{error}"),
    (
        "Path is not inside the screenshots directory",
        "路径不在截图文件夹中",
    ),
    (
        "Failed to resolve screenshot path: {error}",
        "无法解析截图路径：{error}",
    ),
    (
        "Failed to resolve screenshots directory: {error}",
        "无法解析截图文件夹：{error}",
    ),
    (
        "Failed to delete screenshot: {error}",
        "无法删除截图：{error}",
    ),
    ("Video Files", "视频文件"),
    ("Image Files", "图片文件"),
    ("Media Files", "媒体文件"),
    ("Unsupported media file.", "不支持的媒体文件。"),
    (
        "Failed to import video: {message}",
        "无法导入视频：{message}",
    ),
    (
        "Failed to import image: {message}",
        "无法导入图片：{message}",
    ),
    ("Import Error", "导入出错"),
    ("Analyzing video file...", "正在分析视频文件…"),
    (
        "Video format not supported or file is corrupted",
        "不支持该视频格式，或文件已损坏",
    ),
    ("Cannot decode video: {error}", "无法解码视频：{error}"),
    (
        "Failed to create project directory: {error}",
        "无法创建项目文件夹：{error}",
    ),
    (
        "Failed to save initial metadata: {error}",
        "无法保存初始项目信息：{error}",
    ),
    ("Starting conversion...", "正在开始转换…"),
    ("Converting video... {percent}%", "正在转换视频… {percent}%"),
    ("Creating project metadata...", "正在创建项目信息…"),
    (
        "Failed to save metadata: {error}",
        "无法保存项目信息：{error}",
    ),
    ("Import complete!", "导入完成！"),
    ("Image file does not exist", "图片文件不存在"),
    (
        "Failed to create screenshots directory: {error}",
        "无法创建截图文件夹：{error}",
    ),
    ("Importing image...", "正在导入图片…"),
    (
        "Failed to create screenshot project directory: {error}",
        "无法创建截图项目文件夹：{error}",
    ),
    (
        "Failed to save screenshot metadata: {error}",
        "无法保存截图信息：{error}",
    ),
    (
        "Failed to save screenshot project config: {error}",
        "无法保存截图项目设置：{error}",
    ),
    (
        "Select an MP4 video file to import",
        "请选择要导入的 MP4 视频文件",
    ),
    ("{operation} failed: {error}", "{operation}失败：{error}"),
    (
        "Failed to read imported media: {error}",
        "无法读取导入的媒体：{error}",
    ),
    (
        "Failed to open video file: {error}",
        "无法打开视频文件：{error}",
    ),
    ("No video stream found in file", "文件中没有视频流"),
    (
        "The imported audio track cannot be decoded",
        "无法解码导入的音轨",
    ),
    (
        "Imported media contains a corrupt packet",
        "导入的媒体包含损坏的数据包",
    ),
    ("Video decode", "视频解码"),
    (
        "Imported video contains a corrupt frame",
        "导入的视频包含损坏的帧",
    ),
    ("Audio decode", "音频解码"),
    (
        "Imported audio contains a corrupt frame",
        "导入的音频包含损坏的帧",
    ),
    ("Video decode flush", "视频解码收尾"),
    ("Audio decode flush", "音频解码收尾"),
    (
        "Imported media has an empty video or audio track",
        "导入的媒体中视频或音轨为空",
    ),
    (
        "Failed to save imported media: {error}",
        "无法保存导入的媒体：{error}",
    ),
    ("Video encode", "视频编码"),
    ("Audio encode", "音频编码"),
    (
        "Image dimensions exceed maximum: {width}x{height}",
        "图片尺寸超出上限：{width}x{height}",
    ),
    (
        "Image dimensions overflow: {width}x{height}",
        "图片尺寸溢出：{width}x{height}",
    ),
    ("Failed to decode image", "无法解码图片"),
    (
        "Choose a PNG, JPEG, WebP, GIF or BMP image for this project",
        "请为这个项目选择 PNG、JPEG、WebP、GIF 或 BMP 图片",
    ),
    (
        "Image files must be 64 MiB or smaller",
        "图片文件不能超过 64 MiB",
    ),
    ("Failed to save image: {error}", "无法保存图片：{error}"),
    ("Image", "图片"),
    ("Preparing model download", "正在准备下载模型"),
    ("Download complete", "下载完成"),
    ("Download failed: {error}", "下载失败：{error}"),
    (
        "Downloading model: {progress}%",
        "正在下载模型：{progress}%",
    ),
    (
        "Downloading {filename}: {progress}%",
        "正在下载 {filename}：{progress}%",
    ),
    (
        "Downloaded model files did not match expected sizes",
        "下载的模型文件大小不符",
    ),
    (
        "Only studio recordings can be transcribed",
        "只有工作室录制可以转写",
    ),
    (
        "No audio sources found in the recording metadata",
        "录制信息中没有找到音频来源",
    ),
    (
        "Failed to process any audio sources",
        "所有音频来源都处理失败",
    ),
    ("No speech detected in the audio", "音频中没有检测到语音"),
    ("No audio found in the video file", "视频文件中没有音频"),
    (
        "Caption model not found. Please download it first",
        "没有找到字幕模型。请先下载",
    ),
    (
        "Failed to load the caption model. Try downloading it again",
        "无法载入字幕模型。请尝试重新下载",
    ),
    (
        "Parakeet models are not available on Intel Macs. Use a Whisper model instead",
        "Parakeet 模型不支持 Intel Mac。请改用 Whisper 模型",
    ),
    (
        "The Recommended and High Accuracy models only transcribe English and 24 other European languages. Download the Small or Medium model to caption this language",
        "“推荐”和“高准确度”模型只能转写英语和另外 24 种欧洲语言。要为这种语言生成字幕，请下载“小”或“中等”模型",
    ),
];
