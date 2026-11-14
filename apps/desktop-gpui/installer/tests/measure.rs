use std::path::{Path, PathBuf};

use frameever_installer_app::{
    measure::{Bar, Need, drive_letter, format_size, install_fraction, needs, removal_fraction},
    version::{Existing, compare},
};

#[test]
fn the_installed_version_picks_the_page() {
    assert_eq!(compare("1.0.0", None), Existing::Unknown);
    assert_eq!(
        compare("1.0.0", Some("0.9.2")),
        Existing::Older("0.9.2".into())
    );
    assert_eq!(compare("1.0.0", Some("1.0.0")), Existing::Same);
    assert_eq!(
        compare("1.0.0", Some("1.1.0")),
        Existing::Newer("1.1.0".into())
    );
    assert_eq!(
        compare("1.0.0", Some("1.0.0-dev")),
        Existing::Older("1.0.0-dev".into())
    );
    assert_eq!(compare("1.0.0", Some(" 1.0.0 ")), Existing::Same);
    assert_eq!(compare("1.0.0", Some("garbage")), Existing::Unknown);
    assert_eq!(compare("1.0.0", Some("")), Existing::Unknown);
}

#[test]
fn installing_is_one_bar_that_never_moves_back() {
    let mut bar = Bar::default();
    assert_eq!(bar.advance(0.10), 10);
    assert_eq!(bar.advance(0.05), 10);
    assert_eq!(bar.advance(0.555), 55);
    assert_eq!(bar.advance(1.4), 99);
    assert_eq!(bar.advance(f64::NAN), 99);
    assert_eq!(bar.complete(), 100);
    assert_eq!(bar.advance(0.2), 100);
}

#[test]
fn install_progress_counts_the_extraction_and_nsis_writing_twice_the_installed_bytes() {
    let payload = 100;
    let installed = 400;
    assert_eq!(install_fraction(0, 0, payload, installed), 0.0);
    assert!((install_fraction(100, 0, payload, installed) - 100.0 / 900.0).abs() < 1e-9);
    assert!((install_fraction(100, 400, payload, installed) - 500.0 / 900.0).abs() < 1e-9);
    assert!((install_fraction(100, 800, payload, installed) - 1.0).abs() < 1e-9);
    assert_eq!(install_fraction(0, 0, 0, 0), 0.0);
}

#[test]
fn uninstall_progress_counts_the_files_already_removed() {
    assert_eq!(removal_fraction(10, 10), 0.0);
    assert_eq!(removal_fraction(10, 4), 0.6);
    assert_eq!(removal_fraction(10, 0), 1.0);
    assert_eq!(removal_fraction(0, 0), 1.0);
    assert_eq!(removal_fraction(10, 12), 0.0);
}

fn volume(path: &Path) -> Option<String> {
    drive_letter(path).map(|letter| format!("{letter}:"))
}

#[test]
fn the_space_check_covers_the_install_drive_and_the_temp_copy() {
    let apart = needs(
        Path::new("D:\\Apps\\FrameEver"),
        Path::new("C:\\Users\\me\\AppData\\Local\\Temp"),
        400,
        150,
        volume,
    );
    assert_eq!(
        apart,
        vec![
            Need {
                volume: "D:".into(),
                bytes: 400
            },
            Need {
                volume: "C:".into(),
                bytes: 550
            },
        ]
    );
    let together = needs(
        Path::new("C:\\Users\\me\\AppData\\Local\\FrameEver"),
        Path::new("C:\\Users\\me\\AppData\\Local\\Temp"),
        400,
        150,
        volume,
    );
    assert_eq!(
        together,
        vec![Need {
            volume: "C:".into(),
            bytes: 950
        }]
    );
}

#[test]
fn drive_letters_come_from_the_path() {
    assert_eq!(drive_letter(Path::new("c:\\x")), Some('C'));
    assert_eq!(drive_letter(&PathBuf::from("relative")), None);
}

#[test]
fn sizes_read_like_the_design() {
    assert_eq!(format_size(380 * 1024 * 1024), "380 MB");
    assert_eq!(format_size(120 * 1024 * 1024), "120 MB");
    assert_eq!(format_size(58 * 1024 * 1024 * 1024), "58 GB");
    assert_eq!(format_size(1536 * 1024 * 1024), "1.5 GB");
    assert_eq!(format_size(300 * 1024), "1 MB");
}
