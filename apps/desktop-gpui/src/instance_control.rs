pub const PIDFILE_NAME: &str = "cap-gpui.pid";
pub const ENDPOINT_EXTENSION: &str = "ipc";

const BUSY_QUERY: u32 = u32::MAX;
const QUIT_REQUEST: u32 = u32::MAX - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Busy,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Idle,
    Busy,
    Quitting,
    Refused,
}

impl Control {
    pub fn from_size(size: u32) -> Option<Self> {
        match size {
            BUSY_QUERY => Some(Self::Busy),
            QUIT_REQUEST => Some(Self::Quit),
            _ => None,
        }
    }

    pub fn size(self) -> u32 {
        match self {
            Self::Busy => BUSY_QUERY,
            Self::Quit => QUIT_REQUEST,
        }
    }
}

impl Reply {
    pub fn byte(self) -> u8 {
        match self {
            Self::Idle => 10,
            Self::Busy => 11,
            Self::Quitting => 12,
            Self::Refused => 13,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        [Self::Idle, Self::Busy, Self::Quitting, Self::Refused]
            .into_iter()
            .find(|reply| reply.byte() == byte)
    }

    pub fn to_quit(busy: bool, saved: bool) -> Self {
        if busy || !saved {
            Self::Refused
        } else {
            Self::Quitting
        }
    }
}
