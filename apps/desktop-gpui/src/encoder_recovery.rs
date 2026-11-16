use serde_json::{Value, json};

use crate::crash_capture::{self, UncleanEnd};

const DISABLED_BY_CRASH: &str = "hardwareEncodersDisabledByCrash";

pub fn configure(previous: Option<&UncleanEnd>) {
    if !cfg!(windows) {
        return;
    }
    frameever_enc_ffmpeg::hardware_crash::set_hardware_encoder_observer(|name, active| {
        if active {
            crash_capture::enter_hardware_encoder(name);
        } else {
            crash_capture::exit_hardware_encoder(name);
        }
    });

    let current = env!("CARGO_PKG_VERSION");
    let raw = crate::store::store_section(crate::store::GENERAL_SETTINGS)
        .get(DISABLED_BY_CRASH)
        .cloned();
    let stored = raw.as_ref().and_then(Disabled::from_value);
    let crashed = previous.map(blamed_encoders).unwrap_or_default();
    let next = next_disabled(stored.as_ref(), &crashed, current);

    let next_value = next.as_ref().map(Disabled::to_value);
    if next_value != raw.filter(|value| !value.is_null())
        && !crate::store::set_store_setting(
            crate::store::GENERAL_SETTINGS,
            DISABLED_BY_CRASH,
            next_value.unwrap_or(Value::Null),
        )
    {
        tracing::warn!("could not save the hardware encoder crash recovery state");
    }

    let module = previous
        .and_then(|previous| previous.native_crash.as_ref())
        .and_then(|crash| crash.module.as_deref());
    if !crashed.is_empty() {
        tracing::error!(
            encoders = ?crashed,
            module = ?module,
            "the previous FrameEver session crashed inside a hardware video encoder; recording skips it until the next app update"
        );
    } else if let Some(previous) = previous
        && previous.native_crash.is_some()
        && !previous.hardware_encoders.is_empty()
    {
        tracing::warn!(
            encoders = ?previous.hardware_encoders,
            module = ?module,
            "the previous FrameEver session crashed outside the live hardware video encoders and their GPU drivers; keeping them"
        );
    }
    match (&next, &stored) {
        (Some(next), _) if crashed.is_empty() => tracing::warn!(
            encoders = ?next.encoders,
            "hardware video encoders stay off after a crash in this app version"
        ),
        (None, Some(stored)) => tracing::info!(
            previous_version = %stored.version,
            encoders = ?stored.encoders,
            "app version changed; hardware video encoders that crashed get another try"
        ),
        _ => {}
    }

    frameever_enc_ffmpeg::hardware_crash::set_disabled_hardware_encoders(
        next.map(|next| next.encoders).unwrap_or_default(),
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Disabled {
    version: String,
    encoders: Vec<String>,
}

impl Disabled {
    fn from_value(value: &Value) -> Option<Self> {
        let version = value.get("version")?.as_str()?.to_owned();
        let encoders = value
            .get("encoders")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        Some(Self { version, encoders })
    }

    fn to_value(&self) -> Value {
        json!({ "version": self.version, "encoders": self.encoders })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    Encoder(&'static str),
    Driver(&'static [&'static str]),
    Elsewhere,
}

fn classify_module(module: &str) -> Fault {
    const INTEL: &[&str] = &["h264_qsv", "h264_mf"];
    const NVIDIA: &[&str] = &["h264_nvenc", "h264_mf"];
    const AMD: &[&str] = &["h264_amf", "h264_mf"];
    const MODULES: &[(&str, Fault)] = &[
        ("mfx_mft", Fault::Encoder("h264_mf")),
        ("nvencmft", Fault::Encoder("h264_mf")),
        ("amdh264enc", Fault::Encoder("h264_mf")),
        ("nvencodeapi", Fault::Encoder("h264_nvenc")),
        ("amfrt", Fault::Encoder("h264_amf")),
        ("igd", Fault::Driver(INTEL)),
        ("igxe", Fault::Driver(INTEL)),
        ("igc", Fault::Driver(INTEL)),
        ("intel_gfx_api", Fault::Driver(INTEL)),
        ("libmfx", Fault::Driver(INTEL)),
        ("libvpl", Fault::Driver(INTEL)),
        ("mfx", Fault::Driver(INTEL)),
        ("nvcuda", Fault::Driver(NVIDIA)),
        ("nvwgf2um", Fault::Driver(NVIDIA)),
        ("nvd3dum", Fault::Driver(NVIDIA)),
        ("nvldumd", Fault::Driver(NVIDIA)),
        ("nvlddmkm", Fault::Driver(NVIDIA)),
        ("atiumd", Fault::Driver(AMD)),
        ("atidxx", Fault::Driver(AMD)),
        ("amdxx", Fault::Driver(AMD)),
    ];
    let module = module.to_ascii_lowercase();
    MODULES
        .iter()
        .find(|(prefix, _)| module.starts_with(prefix))
        .map_or(Fault::Elsewhere, |(_, fault)| *fault)
}

fn blamed_encoders(previous: &UncleanEnd) -> Vec<String> {
    let Some(module) = previous
        .native_crash
        .as_ref()
        .and_then(|crash| crash.module.as_deref())
    else {
        return Vec::new();
    };
    match classify_module(module) {
        Fault::Encoder(encoder) => vec![encoder.to_owned()],
        Fault::Driver(encoders) => previous
            .hardware_encoders
            .iter()
            .filter(|live| encoders.contains(&live.as_str()))
            .cloned()
            .collect(),
        Fault::Elsewhere => Vec::new(),
    }
}

fn next_disabled(stored: Option<&Disabled>, crashed: &[String], current: &str) -> Option<Disabled> {
    let mut encoders: Vec<String> = stored
        .filter(|stored| stored.version == current)
        .map(|stored| stored.encoders.clone())
        .unwrap_or_default();
    for encoder in crashed {
        if !encoders.contains(encoder) {
            encoders.push(encoder.clone());
        }
    }
    (!encoders.is_empty()).then(|| Disabled {
        version: current.to_owned(),
        encoders,
    })
}

#[cfg(test)]
mod tests {
    use super::{Disabled, Fault, blamed_encoders, classify_module, next_disabled};
    use crate::crash_capture::{NativeCrash, UncleanEnd};

    fn crash(module: Option<&str>, live: &[&str]) -> UncleanEnd {
        UncleanEnd {
            hardware_encoders: live.iter().map(|name| name.to_string()).collect(),
            native_crash: Some(NativeCrash {
                module: module.map(str::to_owned),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn modules_are_classified_by_encoder_and_vendor() {
        let encoder = Fault::Encoder;
        assert_eq!(classify_module("mfx_mft_h264ve_64.dll"), encoder("h264_mf"));
        assert_eq!(classify_module("nvEncMFTH264x.dll"), encoder("h264_mf"));
        assert_eq!(classify_module("nvEncodeAPI64.dll"), encoder("h264_nvenc"));
        assert_eq!(classify_module("amfrt64.dll"), encoder("h264_amf"));
        assert_eq!(
            classify_module("igd10iumd64.dll"),
            Fault::Driver(&["h264_qsv", "h264_mf"])
        );
        assert_eq!(
            classify_module("libmfxhw64.dll"),
            Fault::Driver(&["h264_qsv", "h264_mf"])
        );
        assert_eq!(
            classify_module("nvwgf2umx.dll"),
            Fault::Driver(&["h264_nvenc", "h264_mf"])
        );
        assert_eq!(
            classify_module("atiumd64.dll"),
            Fault::Driver(&["h264_amf", "h264_mf"])
        );
        for elsewhere in [
            "ntdll.dll",
            "KERNELBASE.dll",
            "ucrtbase.dll",
            "mfplat.dll",
            "d3d11.dll",
            "FrameEver.exe",
        ] {
            assert_eq!(classify_module(elsewhere), Fault::Elsewhere, "{elsewhere}");
        }
    }

    #[test]
    fn only_a_crash_in_an_encoder_or_its_driver_blames_it() {
        assert_eq!(
            blamed_encoders(&crash(Some("mfx_mft_h264ve_64.dll"), &["h264_mf"])),
            ["h264_mf"]
        );
        assert_eq!(
            blamed_encoders(&crash(Some("mfx_mft_h264ve_64.dll"), &[])),
            ["h264_mf"]
        );
        assert_eq!(
            blamed_encoders(&crash(Some("nvwgf2umx.dll"), &["h264_nvenc"])),
            ["h264_nvenc"]
        );
        assert!(blamed_encoders(&crash(Some("nvwgf2umx.dll"), &["h264_qsv"])).is_empty());
        assert_eq!(
            blamed_encoders(&crash(Some("igd10iumd64.dll"), &["h264_nvenc", "h264_mf"])),
            ["h264_mf"]
        );
        assert!(blamed_encoders(&crash(Some("igd10iumd64.dll"), &[])).is_empty());
        for module in [
            Some("ntdll.dll"),
            Some("ucrtbase.dll"),
            Some("FrameEver.exe"),
            None,
        ] {
            assert!(
                blamed_encoders(&crash(module, &["h264_nvenc"])).is_empty(),
                "{module:?}"
            );
        }
        let killed = UncleanEnd {
            hardware_encoders: vec!["h264_mf".into()],
            ..Default::default()
        };
        assert!(blamed_encoders(&killed).is_empty());
    }

    #[test]
    fn a_crash_disables_its_encoder_until_the_next_version() {
        let disabled = |version: &str, encoders: &[&str]| Disabled {
            version: version.into(),
            encoders: encoders.iter().map(|name| name.to_string()).collect(),
        };
        let mf = ["h264_mf".to_string()];
        let qsv = ["h264_qsv".to_string()];

        assert_eq!(next_disabled(None, &[], "0.6.2"), None);
        assert_eq!(
            next_disabled(None, &mf, "0.6.2"),
            Some(disabled("0.6.2", &["h264_mf"]))
        );
        let after_mf = disabled("0.6.2", &["h264_mf"]);
        assert_eq!(
            next_disabled(Some(&after_mf), &[], "0.6.2"),
            Some(after_mf.clone())
        );
        assert_eq!(
            next_disabled(Some(&after_mf), &qsv, "0.6.2"),
            Some(disabled("0.6.2", &["h264_mf", "h264_qsv"]))
        );
        assert_eq!(
            next_disabled(Some(&after_mf), &mf, "0.6.2"),
            Some(after_mf.clone())
        );
        assert_eq!(next_disabled(Some(&after_mf), &[], "0.6.3"), None);
        assert_eq!(
            next_disabled(Some(&after_mf), &qsv, "0.6.3"),
            Some(disabled("0.6.3", &["h264_qsv"]))
        );
        assert_eq!(Disabled::from_value(&after_mf.to_value()), Some(after_mf));
        assert_eq!(Disabled::from_value(&serde_json::json!("0.6.2")), None);
    }
}
