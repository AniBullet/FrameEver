use windows::Win32::{
    Foundation::HWND,
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    },
    UI::Shell::{ITaskbarList3, TBPF_ERROR, TBPF_NOPROGRESS, TBPF_NORMAL, TaskbarList},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Off,
    Running(u8),
    Failed,
}

pub struct Taskbar {
    list: ITaskbarList3,
    window: HWND,
    shown: Option<State>,
}

impl Taskbar {
    pub fn new(window: isize) -> Option<Self> {
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if initialized.is_err() {
                return None;
            }
            let list: ITaskbarList3 =
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
            list.HrInit().ok()?;
            Some(Self {
                list,
                window: HWND(window as *mut _),
                shown: None,
            })
        }
    }

    pub fn show(&mut self, state: State) {
        if self.shown == Some(state) {
            return;
        }
        self.shown = Some(state);
        let applied = unsafe {
            match state {
                State::Off => self.list.SetProgressState(self.window, TBPF_NOPROGRESS),
                State::Running(percent) => self
                    .list
                    .SetProgressState(self.window, TBPF_NORMAL)
                    .and_then(|()| {
                        self.list
                            .SetProgressValue(self.window, u64::from(percent), 100)
                    }),
                State::Failed => self
                    .list
                    .SetProgressState(self.window, TBPF_ERROR)
                    .and_then(|()| self.list.SetProgressValue(self.window, 100, 100)),
            }
        };
        if applied.is_err() {
            self.shown = None;
        }
    }
}
