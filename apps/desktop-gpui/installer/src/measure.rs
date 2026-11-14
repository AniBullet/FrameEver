use std::path::Path;

pub const NSIS_WRITES_PER_INSTALLED_BYTE: u64 = 2;
const LAST_BEFORE_DONE: u8 = 99;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    percent: u8,
}

impl Bar {
    pub fn advance(&mut self, fraction: f64) -> u8 {
        if self.percent < LAST_BEFORE_DONE && fraction.is_finite() {
            let next = (fraction.clamp(0., 1.) * 100.).floor() as u8;
            self.percent = self.percent.max(next.min(LAST_BEFORE_DONE));
        }
        self.percent
    }

    pub fn complete(&mut self) -> u8 {
        self.percent = 100;
        self.percent
    }

    pub fn percent(&self) -> u8 {
        self.percent
    }
}

pub fn install_fraction(extracted: u64, nsis_written: u64, payload: u64, installed: u64) -> f64 {
    let total = payload + installed * NSIS_WRITES_PER_INSTALLED_BYTE;
    if total == 0 {
        return 0.;
    }
    (extracted.min(payload) + nsis_written) as f64 / total as f64
}

pub fn removal_fraction(at_start: usize, remaining: usize) -> f64 {
    if at_start == 0 {
        return 1.;
    }
    at_start.saturating_sub(remaining) as f64 / at_start as f64
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    pub volume: String,
    pub bytes: u64,
}

pub fn needs(
    install_dir: &Path,
    temp_dir: &Path,
    installed: u64,
    payload: u64,
    volume: impl Fn(&Path) -> Option<String>,
) -> Vec<Need> {
    let mut needs: Vec<Need> = Vec::new();
    let mut add = |path: &Path, bytes: u64| {
        let Some(volume) = volume(path) else {
            return;
        };
        match needs.iter_mut().find(|need| need.volume == volume) {
            Some(need) => need.bytes += bytes,
            None => needs.push(Need { volume, bytes }),
        }
    };
    add(install_dir, installed);
    add(temp_dir, installed + payload);
    needs
}

pub fn drive_letter(path: &Path) -> Option<char> {
    let text = path.as_os_str().to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let mut chars = text.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    (chars.next() == Some(':')).then(|| letter.to_ascii_uppercase())
}

pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024. * 1024.;
    const GB: f64 = MB * 1024.;
    let value = bytes as f64;
    if value >= GB {
        let gigabytes = value / GB;
        if gigabytes >= 10. || (gigabytes.fract() * 10.).round() == 0. {
            format!("{} GB", gigabytes.round() as u64)
        } else {
            format!("{gigabytes:.1} GB")
        }
    } else {
        format!("{} MB", (value / MB).ceil().max(1.) as u64)
    }
}
