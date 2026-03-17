// tests/unit/utils.rs
use gallery_rs::config::Config;
use gallery_rs::utils::{format_file_size, is_valid_image_type, resolve_safe_path};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_format_file_size() {
    assert_eq!(format_file_size(0), "0 B");
    assert_eq!(format_file_size(500), "500 B");
    assert_eq!(format_file_size(1023), "1023 B");
    assert_eq!(format_file_size(1024), "1.00 KB");
    assert_eq!(format_file_size(1536), "1.50 KB");
    assert_eq!(format_file_size(1_048_576), "1.00 MB");
    assert_eq!(format_file_size(1_572_864), "1.50 MB");
    assert_eq!(format_file_size(1_073_741_824), "1.00 GB");
}

#[test]
fn test_is_valid_image_type() {
    let mut config = Config::default();
    config.storage.allowed_extensions = vec![
        "jpg".to_string(),
        "jpeg".to_string(),
        "png".to_string(),
        "gif".to_string(),
        "webp".to_string(),
    ];

    assert!(is_valid_image_type(Path::new("test.jpg"), &config));
    assert!(is_valid_image_type(Path::new("test.JPG"), &config));
    assert!(is_valid_image_type(Path::new("test.jpeg"), &config));
    assert!(is_valid_image_type(Path::new("test.png"), &config));
    assert!(is_valid_image_type(Path::new("test.gif"), &config));
    assert!(is_valid_image_type(Path::new("test.webp"), &config));

    assert!(!is_valid_image_type(Path::new("test.txt"), &config));
    assert!(!is_valid_image_type(Path::new("test.pdf"), &config));
    assert!(!is_valid_image_type(Path::new("test"), &config)); // No extension
    assert!(!is_valid_image_type(Path::new(".hidden"), &config));
}

#[test]
fn test_resolve_safe_path_with_existing_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a test file
    let test_file = root.join("test.jpg");
    std::fs::write(&test_file, b"test").unwrap();

    let resolved = resolve_safe_path(root, "test.jpg");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap(), test_file.canonicalize().unwrap());
}

#[test]
fn test_resolve_safe_path_with_nonexistent_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Non-existent but within root
    let resolved = resolve_safe_path(root, "nonexistent.jpg");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap(), root.join("nonexistent.jpg"));
}

#[test]
fn test_resolve_safe_path_traversal() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Path traversal attempts
    assert!(resolve_safe_path(root, "../etc/passwd").is_none());
    assert!(resolve_safe_path(root, "..").is_none());
    assert!(resolve_safe_path(root, "photos/../../etc/passwd").is_none());
}

#[test]
fn test_resolve_safe_path_with_absolute_input() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Input looks like absolute path
    let resolved = resolve_safe_path(root, "/etc/passwd");
    assert!(resolved.is_none());
}

#[test]
fn test_resolve_safe_path_with_symlink() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a file and a symlink to it
    let real_file = root.join("real.txt");
    std::fs::write(&real_file, b"test").unwrap();

    #[cfg(unix)]
    {
        let symlink = root.join("link.txt");
        std::os::unix::fs::symlink(&real_file, &symlink).unwrap();

        let resolved = resolve_safe_path(root, "link.txt");
        assert!(resolved.is_some());

        // Should resolve to the real file's canonical path
        let resolved_path = resolved.unwrap();
        assert!(resolved_path.starts_with(root));
        assert_eq!(resolved_path, real_file.canonicalize().unwrap());
    }
}
