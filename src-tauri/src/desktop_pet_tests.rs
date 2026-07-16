use crate::desktop_pet::{
    DesktopPetManager, DesktopPetRegistry, DesktopPetSettings, PetEvent, PetPackageSource,
    PetResourceRoot, PetStateKind, PetStateMachine,
};
use image::{Rgba, RgbaImage};
use serde_json::json;
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
}

#[test]
fn manager_selects_the_builtin_default_pet_when_settings_have_no_selection() {
    let temp = tempdir().expect("tempdir");
    let builtin_root = temp.path().join("builtin");
    let user_root = temp.path().join("user");
    fs::create_dir_all(&builtin_root).expect("builtin root");
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
fn package_selects_frames_from_actual_clip_length_and_fps() {
    let temp = tempdir().expect("tempdir");
    write_package(temp.path(), "frog", 3);
    let registry = DesktopPetRegistry::scan_roots(vec![PetResourceRoot::new(
        temp.path().to_path_buf(),
        PetPackageSource::User,
    )]);
    let package = registry.package("frog").expect("package");

    let first = package
        .frame_at(PetStateKind::Idle, None, 0.01)
        .expect("first");
    let second = package
        .frame_at(PetStateKind::Idle, None, 0.15)
        .expect("second");
    let wrapped = package
        .frame_at(PetStateKind::Idle, None, 0.39)
        .expect("wrapped");

    assert_eq!(first.path.file_name().unwrap(), "idle_001.png");
    assert_eq!(second.path.file_name().unwrap(), "idle_002.png");
    assert_eq!(wrapped.path.file_name().unwrap(), "idle_001.png");
}
