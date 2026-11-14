use std::{sync::OnceLock, time::Duration};

use gpui::App;

use crate::instance_control::{Control, Reply};

const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

type Request = (Control, flume::Sender<Reply>);

fn channel() -> &'static (flume::Sender<Request>, flume::Receiver<Request>) {
    static CHANNEL: OnceLock<(flume::Sender<Request>, flume::Receiver<Request>)> = OnceLock::new();
    CHANNEL.get_or_init(flume::unbounded)
}

pub(crate) fn answer(control: Control) -> Reply {
    let (reply, replied) = flume::bounded(1);
    if channel().0.send((control, reply)).is_err() {
        return unanswered(control);
    }
    replied
        .recv_timeout(ANSWER_TIMEOUT)
        .unwrap_or_else(|_| unanswered(control))
}

fn unanswered(control: Control) -> Reply {
    match control {
        Control::Busy => Reply::Busy,
        Control::Quit => Reply::Refused,
    }
}

fn handle(control: Control, cx: &mut App) -> Reply {
    let busy = crate::updates::work_in_flight(cx);
    match control {
        Control::Busy if busy => Reply::Busy,
        Control::Busy => Reply::Idle,
        Control::Quit => {
            let saved = !busy && crate::app_windows::flush_pending_editor_saves(cx).is_ok();
            Reply::to_quit(busy, saved)
        }
    }
}

pub(crate) fn init(cx: &mut App) {
    let requests = channel().1.clone();
    cx.spawn(async move |cx| {
        while let Ok((control, reply)) = requests.recv_async().await {
            cx.update(|cx| {
                let answer = handle(control, cx);
                tracing::info!(?control, ?answer, "the installer asked FrameEver");
                if reply.send(answer).is_err() {
                    tracing::warn!("the installer stopped waiting for an answer");
                }
                if answer == Reply::Quitting {
                    crate::menus::quit(cx);
                }
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_installer_that_gets_no_answer_never_closes_frameever() {
        assert_eq!(unanswered(Control::Busy), Reply::Busy);
        assert_eq!(unanswered(Control::Quit), Reply::Refused);
    }

    #[test]
    fn frameever_refuses_to_quit_for_the_installer_while_work_is_in_flight() {
        assert_eq!(Reply::to_quit(true, true), Reply::Refused);
        assert_eq!(Reply::to_quit(false, false), Reply::Refused);
        assert_eq!(Reply::to_quit(false, true), Reply::Quitting);
        for control in [Control::Busy, Control::Quit] {
            assert_eq!(Control::from_size(control.size()), Some(control));
        }
        for reply in [Reply::Idle, Reply::Busy, Reply::Quitting, Reply::Refused] {
            assert_eq!(Reply::from_byte(reply.byte()), Some(reply));
        }
    }
}
