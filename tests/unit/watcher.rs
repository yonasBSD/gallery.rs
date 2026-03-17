// tests/unit/watcher.rs
use gallery_rs::{
    config::Config,
    models::{ChangeType, GalleryEvent},
    watcher::handle_fs_event,
};
use notify::{Event, EventKind};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;

#[tokio::test]
async fn test_handle_fs_create_event() {
    let config = Arc::new(Config::default());
    let (tx, mut rx) = broadcast::channel(10);

    let event = Event {
        kind: EventKind::Create(notify::event::CreateKind::File),
        paths: vec![config.storage.base_dir.join("new.jpg")],
        ..Default::default()
    };

    handle_fs_event(event, &config, &tx);

    if let Ok(recv) = rx.try_recv() {
        assert_eq!(recv.change_type, ChangeType::Added);
        assert_eq!(recv.rel_path, "new.jpg");
    } else {
        panic!("Expected event to be sent");
    }
}

#[tokio::test]
async fn test_handle_fs_modify_event() {
    let config = Arc::new(Config::default());
    let (tx, mut rx) = broadcast::channel(10);

    let event = Event {
        kind: EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )),
        paths: vec![config.storage.base_dir.join("existing.jpg")],
        ..Default::default()
    };

    handle_fs_event(event, &config, &tx);

    if let Ok(recv) = rx.try_recv() {
        assert_eq!(recv.change_type, ChangeType::Updated);
        assert_eq!(recv.rel_path, "existing.jpg");
    } else {
        panic!("Expected event to be sent");
    }
}

#[tokio::test]
async fn test_handle_fs_remove_event() {
    let config = Arc::new(Config::default());
    let (tx, mut rx) = broadcast::channel(10);

    let event = Event {
        kind: EventKind::Remove(notify::event::RemoveKind::File),
        paths: vec![config.storage.base_dir.join("deleted.jpg")],
        ..Default::default()
    };

    handle_fs_event(event, &config, &tx);

    if let Ok(recv) = rx.try_recv() {
        assert_eq!(recv.change_type, ChangeType::Removed);
        assert_eq!(recv.rel_path, "deleted.jpg");
    } else {
        panic!("Expected event to be sent");
    }
}

#[tokio::test]
async fn test_handle_fs_non_image_event() {
    let config = Arc::new(Config::default());
    let (tx, mut rx) = broadcast::channel(10);

    let event = Event {
        kind: EventKind::Create(notify::event::CreateKind::File),
        paths: vec![config.storage.base_dir.join("not_image.txt")],
        ..Default::default()
    };

    handle_fs_event(event, &config, &tx);

    // Should not broadcast for non-image files
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn test_handle_fs_remove_non_image() {
    let config = Arc::new(Config::default());
    let (tx, mut rx) = broadcast::channel(10);

    // Removals should be broadcast even for non-images
    let event = Event {
        kind: EventKind::Remove(notify::event::RemoveKind::File),
        paths: vec![config.storage.base_dir.join("not_image.txt")],
        ..Default::default()
    };

    handle_fs_event(event, &config, &tx);

    if let Ok(recv) = rx.try_recv() {
        assert_eq!(recv.change_type, ChangeType::Removed);
        assert_eq!(recv.rel_path, "not_image.txt");
    } else {
        panic!("Expected removal event to be sent even for non-images");
    }
}
