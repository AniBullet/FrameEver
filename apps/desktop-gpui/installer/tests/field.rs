use frameever_installer_app::field::{self, Colors, FieldEvent, PathField};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext, rgb};
use std::{cell::RefCell, rc::Rc};

const PATH: &str = r"C:\Users\me\AppData\Local\Programs\Very Long Folder Name\FrameEver";

fn colors() -> Colors {
    Colors {
        text: rgb(0xffffff),
        caret: rgb(0xffffff),
        selection: rgb(0x444444),
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<PathField>, &mut VisualTestContext) {
    cx.update(|cx| cx.bind_keys(field::bindings()));
    let (field, cx) = cx.add_window_view(|_, cx| PathField::new(PATH, colors(), cx));
    cx.update(|window, cx| {
        let focus = field.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (field, cx)
}

#[gpui::test]
fn arrow_keys_home_and_end_move_the_caret(cx: &mut TestAppContext) {
    let (field, cx) = open(cx);
    let end = PATH.len();
    assert_eq!(field.read_with(cx, |it, _| it.selection()), end..end);
    cx.simulate_keystrokes("left left");
    assert_eq!(
        field.read_with(cx, |it, _| it.selection()),
        end - 2..end - 2
    );
    cx.simulate_keystrokes("home");
    assert_eq!(field.read_with(cx, |it, _| it.selection()), 0..0);
    cx.simulate_keystrokes("right shift-right");
    assert_eq!(field.read_with(cx, |it, _| it.selection()), 1..2);
    cx.simulate_keystrokes("shift-end");
    assert_eq!(field.read_with(cx, |it, _| it.selection()), 1..end);
    cx.simulate_keystrokes("end ctrl-a");
    assert_eq!(field.read_with(cx, |it, _| it.selection()), 0..end);
}

#[gpui::test]
fn changing_only_the_drive_letter_reports_the_new_path(cx: &mut TestAppContext) {
    let (field, cx) = open(cx);
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    cx.update(|_, cx| {
        cx.subscribe(&field, move |_, event, _| {
            if let FieldEvent::Changed(text) = event {
                sink.borrow_mut().push(text.clone());
            }
        })
        .detach();
    });
    cx.simulate_keystrokes("home shift-right");
    cx.simulate_input("D");
    cx.run_until_parked();
    let expected = PATH.replacen('C', "D", 1);
    assert_eq!(field.read_with(cx, |it, _| it.text().to_string()), expected);
    assert_eq!(reported.borrow().last(), Some(&expected));
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        field.read_with(cx, |it, _| it.text().to_string()),
        &PATH[1..]
    );
}
