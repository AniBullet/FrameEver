use cap_cursor_info::CursorShape;

#[allow(unreachable_code)]
fn main() {
    return run();
    panic!("Unsupported platform!");
}

fn run() {
    use windows::Win32::{
        Foundation::POINT,
        UI::WindowsAndMessaging::{CURSORINFO, CURSORINFO_FLAGS, GetCursorInfo, HCURSOR},
    };

    println!("Starting cursor monitoring...\n");

    loop {
        unsafe {
            let mut cursor_info = CURSORINFO {
                cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                flags: CURSORINFO_FLAGS(0),
                hCursor: HCURSOR(std::ptr::null_mut()),
                ptScreenPos: POINT { x: 0, y: 0 },
            };

            if GetCursorInfo(&mut cursor_info).is_ok() {
                // Try to convert HCURSOR to CursorShape using the TryFrom implementation
                match CursorShape::try_from(&cursor_info.hCursor) {
                    Ok(cursor_shape) => {
                        println!("CursorShape: {cursor_shape}");
                    }
                    Err(_) => {
                        println!("Unknown cursor: {:?}", cursor_info.hCursor);
                    }
                }
            } else {
                println!("Failed to get cursor info");
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
