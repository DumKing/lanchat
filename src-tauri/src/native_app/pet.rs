use crate::desktop_pet::{DesktopPetPackage, PetStateKind};
use std::path::PathBuf;

pub fn initial_idle_frame(package: &DesktopPetPackage) -> Option<PathBuf> {
    package
        .clip_candidates(PetStateKind::Idle, None)
        .into_iter()
        .find_map(|clip| clip.frames.first().map(|frame| frame.path.clone()))
}

#[cfg(test)]
mod tests {
    use super::initial_idle_frame;
    use crate::desktop_pet::{
        DesktopPetPackage, PetClip, PetFrame, PetManifest, PetPackageSource, PetStateKind,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn selects_the_first_idle_frame_from_the_active_package() {
        let frame = PetFrame {
            path: PathBuf::from("C:/pets/frog/Idle/idle-01.png"),
            width: 256,
            height: 256,
        };
        let package = DesktopPetPackage {
            manifest: PetManifest {
                schema_version: 1,
                id: "frog".to_string(),
                name: "Frog".to_string(),
                version: "1.0.0".to_string(),
                author: String::new(),
                description: String::new(),
                resolution: 256,
                fps: 8.0,
                transparent: true,
                default_state: "Idle".to_string(),
                states: serde_json::json!(["Idle"]),
                clips: HashMap::new(),
            },
            source: PetPackageSource::BuiltIn,
            root: PathBuf::from("C:/pets/frog"),
            preview_path: None,
            icon_path: None,
            states: HashMap::from([(
                PetStateKind::Idle,
                vec![PetClip {
                    id: "idle".to_string(),
                    state: PetStateKind::Idle,
                    frames: vec![frame.clone()],
                    fps: 8.0,
                    loop_mode: "loop".to_string(),
                    direction: None,
                    weight: 1,
                }],
            )]),
            warnings: Vec::new(),
        };

        assert_eq!(Some(frame.path), initial_idle_frame(&package));
    }
}
