use crate::desktop_pet::{
    DesktopPetManager, DesktopPetPackage, DesktopPetRegistry, DesktopPetSettings, PetEvent,
    PetPackageSource, PetResourceRoot, PetStateKind, PetStateMachine, PetStatePlaybackConfig,
};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_png(path: &Path) {
    let image = RgbaImage::from_pixel(8, 8, Rgba([40, 180, 90, 0]));
    image.save(path).expect("write png");
}

fn write_package(root: &Path, id: &str, flat_idle_frames: usize) {
    let package = root.join(id);
    for state in ["Idle", "Alert", "Move", "Interact", "Life"] {
        fs::create_dir_all(package.join(state)).expect("create state");
        write_png(
            &package
                .join(state)
                .join(format!("{}_001.png", state.to_lowercase())),
        );
    }
    for index in 2..=flat_idle_frames {
        write_png(&package.join("Idle").join(format!("idle_{index:03}.png")));
    }
    write_png(&package.join("preview.png"));
    write_png(&package.join("icon.png"));
    fs::write(
        package.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 2,
            "id": id,
            "name": "测试桌宠",
            "version": "1.0.0",
            "resolution": 8,
            "fps": 8,
            "transparent": true,
            "defaultState": "Idle",
            "states": {
                "Idle": {"loop": "repeat"},
                "Alert": {"loop": "repeat"},
                "Move": {"loop": "repeat"},
                "Interact": {"loop": "once"},
                "Life": {"loop": "once"}
            }
        }))
        .expect("manifest json"),
    )
    .expect("write manifest");
}

#[test]
fn registry_uses_actual_frame_count_instead_of_recommended_count() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 3);

    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");
    let idle = package.state(PetStateKind::Idle).expect("idle");

    assert_eq!(idle.iter().map(|clip| clip.frames.len()).sum::<usize>(), 3);
}

#[test]
fn registry_treats_each_child_folder_as_an_independent_clip() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 1);
    let idle = temp.path().join("frog").join("Idle");
    fs::create_dir_all(idle.join("blink")).expect("clip");
    write_png(&idle.join("blink").join("idle_blink_001.png"));
    fs::create_dir_all(idle.join("breathe")).expect("clip");
    write_png(&idle.join("breathe").join("idle_breathe_001.png"));
    write_png(&idle.join("breathe").join("idle_breathe_002.png"));

    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let clips = registry
        .package("frog")
        .expect("package")
        .state(PetStateKind::Idle)
        .expect("idle");

    assert_eq!(clips.len(), 3);
    assert_eq!(
        clips
            .iter()
            .find(|clip| clip.id == "blink")
            .unwrap()
            .frames
            .len(),
        1
    );
    assert_eq!(
        clips
            .iter()
            .find(|clip| clip.id == "breathe")
            .unwrap()
            .frames
            .len(),
        2
    );
}

#[test]
fn registry_rejects_an_invalid_icon_and_keeps_preview_fallback() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 1);
    fs::write(temp.path().join("frog").join("icon.png"), b"not a png").expect("break icon");

    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");

    assert!(package.icon_path.is_none());
    assert_eq!(
        package.preview_path.as_deref(),
        Some(temp.path().join("frog").join("preview.png").as_path())
    );
    assert!(package
        .warnings
        .iter()
        .any(|warning| warning.contains("icon.png")));
}

#[test]
fn package_normalizes_manifest_playback_ranges_and_uses_state_defaults() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 1);
    let manifest_path = temp.path().join("frog").join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest")).expect("json");
    manifest["states"]["Move"] = json!({
        "loop": "repeat",
        "minDurationMs": 2400,
        "maxDurationMs": 1200,
        "minActionCount": 5,
        "maxActionCount": 2,
        "minIntervalMs": 900,
        "maxIntervalMs": 300
    });
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");

    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");

    assert_eq!(
        package.playback_config(PetStateKind::Move),
        PetStatePlaybackConfig {
            min_duration_ms: 1200,
            max_duration_ms: 2400,
            min_action_count: 2,
            max_action_count: 5,
            min_interval_ms: 300,
            max_interval_ms: 900,
        }
    );
    assert_eq!(
        package.playback_config(PetStateKind::Interact),
        PetStatePlaybackConfig {
            min_duration_ms: 0,
            max_duration_ms: 0,
            min_action_count: 1,
            max_action_count: 1,
            min_interval_ms: 0,
            max_interval_ms: 0,
        }
    );
    assert_eq!(
        package.playback_config(PetStateKind::Life),
        PetStatePlaybackConfig {
            min_duration_ms: 0,
            max_duration_ms: 0,
            min_action_count: 2,
            max_action_count: 4,
            min_interval_ms: 800,
            max_interval_ms: 2000,
        }
    );
}

#[test]
fn package_exposes_equal_clip_candidates_and_direction_filtering() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 1);
    let move_root = temp.path().join("frog").join("Move");
    fs::create_dir_all(move_root.join("jump_left")).expect("left clip");
    fs::create_dir_all(move_root.join("jump_right")).expect("right clip");
    write_png(&move_root.join("jump_left").join("left_001.png"));
    write_png(&move_root.join("jump_right").join("right_001.png"));
    let manifest_path = temp.path().join("frog").join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest")).expect("json");
    manifest["clips"] = json!({
        "Move/jump_left": {"direction": "left", "weight": 100},
        "Move/jump_right": {"direction": "right", "weight": 1}
    });
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");

    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");

    let all = package.clip_candidates(PetStateKind::Move, None);
    assert_eq!(all.len(), 3);
    assert_eq!(
        package
            .clip_by_uniform_index(PetStateKind::Move, None, 0)
            .unwrap()
            .id,
        "default"
    );
    assert_eq!(
        package
            .clip_by_uniform_index(PetStateKind::Move, None, 1)
            .unwrap()
            .id,
        "jump_left"
    );
    assert_eq!(
        package
            .clip_by_uniform_index(PetStateKind::Move, None, 2)
            .unwrap()
            .id,
        "jump_right"
    );
    let right = package.clip_candidates(PetStateKind::Move, Some("right"));
    assert_eq!(right.len(), 1);
    assert_eq!(right[0].id, "jump_right");
}

#[test]
fn user_package_overrides_portable_and_builtin_packages_with_same_id() {
    let builtin = tempdir().expect("builtin");
    let portable = tempdir().expect("portable");
    let user = tempdir().expect("user");
    write_package(builtin.path(), "frog", 1);
    write_package(portable.path(), "frog", 2);
    write_package(user.path(), "frog", 3);

    let registry = DesktopPetRegistry::scan_roots(vec![
        PetResourceRoot::new(builtin.path().to_path_buf(), PetPackageSource::BuiltIn),
        PetResourceRoot::new(portable.path().to_path_buf(), PetPackageSource::Portable),
        PetResourceRoot::new(user.path().to_path_buf(), PetPackageSource::User),
    ]);
    let package = registry.package("frog").expect("package");

    assert_eq!(package.source, PetPackageSource::User);
    assert_eq!(
        package.state(PetStateKind::Idle).unwrap()[0].frames.len(),
        3
    );
}

#[test]
fn alert_preempts_every_state_and_returns_to_idle_when_cleared() {
    let mut machine = PetStateMachine::new();
    machine.handle(PetEvent::LifeTimer);
    assert_eq!(machine.current(), PetStateKind::Life);
    machine.handle(PetEvent::AlertRaised);
    assert_eq!(machine.current(), PetStateKind::Alert);
    machine.handle(PetEvent::PointerInteract);
    assert_eq!(machine.current(), PetStateKind::Alert);
    machine.handle(PetEvent::AlertCleared);
    assert_eq!(machine.current(), PetStateKind::Idle);
}

#[test]
fn desktop_pet_settings_are_persisted_for_native_startup() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("desktop-pet-settings.json");
    let settings = DesktopPetSettings {
        selected_pet_id: Some("frog".to_string()),
        scale: 1.35,
        random_move_enabled: false,
        ..DesktopPetSettings::default()
    };

    settings.save(&path).expect("save");
    let restored = DesktopPetSettings::load(&path);

    assert_eq!(restored.selected_pet_id.as_deref(), Some("frog"));
    assert_eq!(restored.scale, 1.35);
    assert!(!restored.random_move_enabled);
    assert_eq!(restored.disco_movement_mode, "jump");
}

#[test]
fn desktop_pet_settings_load_old_files_with_jump_movement_default() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("desktop-pet-settings.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "enabled": true,
            "selectedPetId": "frog",
            "scale": 1.0,
            "positionX": null,
            "positionY": null,
            "monitorId": null,
            "alertMode": "normal",
            "stopHotkey": "Ctrl+Alt+G",
            "randomMoveEnabled": true,
            "randomLifeEnabled": true
        }))
        .expect("settings json"),
    )
    .expect("write settings");

    let restored = DesktopPetSettings::load(&path);
    assert_eq!(restored.selected_pet_id.as_deref(), Some("frog"));
    assert_eq!(restored.disco_movement_mode, "jump");
}

#[test]
fn manager_selects_the_builtin_default_pet_when_settings_have_no_selection() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    write_package(&builtin_root, "frog-buddy", 2);
    write_package(&builtin_root, "violet-tail-girl", 2);

    let manager = DesktopPetManager::new(
        vec![PetResourceRoot::new(
            builtin_root,
            PetPackageSource::BuiltIn,
        )],
        user_root,
        temp.path().join("settings.json"),
    );

    assert_eq!(
        manager.settings().selected_pet_id.as_deref(),
        Some("violet-tail-girl")
    );
    assert_eq!(
        manager.selected_package().expect("default pet").id(),
        "violet-tail-girl"
    );
}

#[test]
fn manager_falls_back_to_frog_buddy_when_default_pet_is_missing() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    write_package(&builtin_root, "frog-buddy", 2);

    let manager = DesktopPetManager::new(
        vec![PetResourceRoot::new(
            builtin_root,
            PetPackageSource::BuiltIn,
        )],
        user_root,
        temp.path().join("settings.json"),
    );

    assert_eq!(
        manager.settings().selected_pet_id.as_deref(),
        Some("frog-buddy")
    );
}

#[test]
fn manager_preserves_an_existing_valid_pet_selection() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    let settings_path = temp.path().join("settings.json");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    write_package(&builtin_root, "frog-buddy", 2);
    write_package(&builtin_root, "violet-tail-girl", 2);
    DesktopPetSettings {
        selected_pet_id: Some("violet-tail-girl".to_string()),
        ..DesktopPetSettings::default()
    }
    .save(&settings_path)
    .expect("save selection");

    let manager = DesktopPetManager::new(
        vec![PetResourceRoot::new(
            builtin_root,
            PetPackageSource::BuiltIn,
        )],
        user_root,
        settings_path,
    );

    assert_eq!(
        manager.settings().selected_pet_id.as_deref(),
        Some("violet-tail-girl")
    );
}

#[test]
fn manager_repairs_a_missing_selection_to_default_pet() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    let settings_path = temp.path().join("settings.json");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    write_package(&builtin_root, "frog-buddy", 2);
    write_package(&builtin_root, "violet-tail-girl", 2);
    DesktopPetSettings {
        selected_pet_id: Some("legacy-frog".to_string()),
        ..DesktopPetSettings::default()
    }
    .save(&settings_path)
    .expect("save stale selection");

    let manager = DesktopPetManager::new(
        vec![PetResourceRoot::new(
            builtin_root,
            PetPackageSource::BuiltIn,
        )],
        user_root,
        settings_path,
    );

    assert_eq!(
        manager.settings().selected_pet_id.as_deref(),
        Some("violet-tail-girl")
    );
    assert_eq!(
        manager.selected_package().expect("fallback pet").id(),
        "violet-tail-girl"
    );
}

#[test]
fn manager_imports_selects_and_removes_a_user_package() {
    let temp = tempdir().expect("tempdir");
    let source_root = temp.path().join("source");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&source_root).expect("source root");
    write_package(&source_root, "frog", 2);
    let manager = DesktopPetManager::new(
        vec![PetResourceRoot::new(
            user_root.clone(),
            PetPackageSource::User,
        )],
        user_root,
        temp.path().join("settings.json"),
    );

    manager
        .import_package(&source_root.join("frog"))
        .expect("import");
    manager.select("frog").expect("select");
    assert_eq!(manager.settings().selected_pet_id.as_deref(), Some("frog"));
    assert_eq!(
        manager
            .selected_package()
            .expect("selected")
            .total_frames(PetStateKind::Idle),
        2
    );

    manager.remove_user_package("frog").expect("remove");
    assert!(manager.selected_package().is_none());
    assert!(manager.settings().selected_pet_id.is_none());
}

#[test]
fn removing_a_selected_user_override_falls_back_to_the_builtin_package() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    fs::create_dir_all(&user_root).expect("user root");
    write_package(&builtin_root, "frog-buddy", 1);
    write_package(&user_root, "frog-buddy", 3);
    let manager = DesktopPetManager::new(
        vec![
            PetResourceRoot::new(builtin_root, PetPackageSource::BuiltIn),
            PetResourceRoot::new(user_root.clone(), PetPackageSource::User),
        ],
        user_root,
        temp.path().join("settings.json"),
    );

    manager
        .remove_user_package("frog-buddy")
        .expect("remove override");

    assert_eq!(
        manager.settings().selected_pet_id.as_deref(),
        Some("frog-buddy")
    );
    assert_eq!(
        manager.selected_package().expect("builtin fallback").source,
        PetPackageSource::BuiltIn
    );
}

#[test]
fn manager_copies_builtin_package_before_updating_playback_config() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&builtin_root).expect("builtin root");
    write_package(&builtin_root, "frog-buddy", 2);
    let manager = DesktopPetManager::new(
        vec![
            PetResourceRoot::new(builtin_root.clone(), PetPackageSource::BuiltIn),
            PetResourceRoot::new(user_root.clone(), PetPackageSource::User),
        ],
        user_root.clone(),
        temp.path().join("settings.json"),
    );
    let mut configs = HashMap::new();
    configs.insert(
        "Life".to_string(),
        PetStatePlaybackConfig {
            min_duration_ms: 0,
            max_duration_ms: 0,
            min_action_count: 3,
            max_action_count: 6,
            min_interval_ms: 1200,
            max_interval_ms: 2600,
        },
    );

    let updated = manager
        .update_playback_configs("frog-buddy", configs)
        .expect("update playback config");

    assert_eq!(updated.source, PetPackageSource::User);
    assert_eq!(
        updated.playback_config(PetStateKind::Life).min_action_count,
        3
    );
    assert_eq!(
        updated.playback_config(PetStateKind::Life).max_interval_ms,
        2600
    );
    assert!(user_root.join("frog-buddy").join("manifest.json").is_file());
    let original: serde_json::Value = serde_json::from_slice(
        &fs::read(builtin_root.join("frog-buddy").join("manifest.json")).expect("builtin manifest"),
    )
    .expect("builtin json");
    assert!(original["states"]["Life"].get("minActionCount").is_none());
}

#[test]
fn package_selects_frames_from_the_active_clip_length_and_fps() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 3);
    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");

    let clip = package
        .clip_by_uniform_index(PetStateKind::Idle, None, 0)
        .expect("clip");
    let first = DesktopPetPackage::frame_in_clip(clip, 0.01).expect("first");
    let second = DesktopPetPackage::frame_in_clip(clip, 0.15).expect("second");
    let wrapped = DesktopPetPackage::frame_in_clip(clip, 0.39).expect("wrapped");

    assert_eq!(first.path.file_name().unwrap(), "idle_001.png");
    assert_eq!(second.path.file_name().unwrap(), "idle_002.png");
    assert_eq!(wrapped.path.file_name().unwrap(), "idle_001.png");
    assert_eq!(DesktopPetPackage::clip_cycle_seconds(clip), 0.375);
}
