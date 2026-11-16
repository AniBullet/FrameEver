use frameever_installer_app::location::writable;

#[test]
fn a_new_folder_under_a_writable_one_counts_and_leaves_nothing_behind() {
    let dir = tempfile::tempdir().unwrap();
    assert!(writable(&dir.path().join("Apps").join("FrameEver")));
    assert!(writable(dir.path()));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn a_file_in_the_way_or_a_missing_drive_does_not_count() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("FrameEver");
    std::fs::write(&file, b"").unwrap();
    assert!(!writable(&file));
    assert!(!writable(&file.join("inside")));
    let missing = (b'D'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .find(|root| !std::path::Path::new(root).exists())
        .unwrap();
    assert!(!writable(&std::path::Path::new(&missing).join("FrameEver")));
}
