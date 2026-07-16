use mac_address::get_mac_address;
use uuid::Uuid;

pub fn resolve_device_id() -> String {
    match get_mac_address() {
        Ok(Some(mac)) => normalize_device_id(&mac.to_string()),
        _ => format!("uuid_{}", Uuid::new_v4()),
    }
}

#[cfg(test)]
pub fn normalize_mac(input: &str) -> String {
    normalize_device_id(input)
}

pub fn normalize_device_id(input: &str) -> String {
    let trimmed = input.trim().to_lowercase();
    let compact: String = trimmed
        .chars()
        .filter(|ch| *ch != ':' && *ch != '-')
        .collect();
    if compact.len() == 12 && compact.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return compact
            .as_bytes()
            .chunks(2)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(":");
    }
    trimmed
}

pub fn is_mac_device_id(input: &str) -> bool {
    let normalized = normalize_device_id(input);
    let compact: String = normalized.chars().filter(|ch| *ch != ':').collect();
    normalized.matches(':').count() == 5
        && compact.len() == 12
        && compact.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub fn resolve_profile_device_id(stored: &str, resolved: &str) -> String {
    let stored = normalize_device_id(stored);
    let resolved = normalize_device_id(resolved);
    if is_mac_device_id(&resolved) && stored != resolved {
        resolved
    } else {
        stored
    }
}

#[cfg(test)]
pub fn device_id_from_mac(input: &str) -> String {
    normalize_mac(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_stable_device_id_from_raw_mac() {
        let first = device_id_from_mac("AA:BB:CC:DD:EE:FF");
        let second = device_id_from_mac("aa-bb-cc-dd-ee-ff");

        assert_eq!(first, second);
        assert_eq!("aa:bb:cc:dd:ee:ff", first);
    }

    #[test]
    fn normalizes_mac_addresses_before_use() {
        assert_eq!(
            normalize_mac("AA-BB-CC-DD-EE-FF"),
            normalize_mac("aa:bb:cc:dd:ee:ff")
        );
    }

    #[test]
    fn migrates_existing_non_mac_profile_id_to_real_mac() {
        assert_eq!(
            "aa:bb:cc:dd:ee:ff",
            resolve_profile_device_id("uuid_old-install-id", "AA-BB-CC-DD-EE-FF")
        );
        assert_eq!(
            "aa:bb:cc:dd:ee:ff",
            resolve_profile_device_id("old-derived-device-id", "aa:bb:cc:dd:ee:ff")
        );
    }

    #[test]
    fn keeps_stored_id_when_runtime_mac_is_unavailable() {
        assert_eq!(
            "old-derived-device-id",
            resolve_profile_device_id("old-derived-device-id", "uuid_runtime-fallback")
        );
    }
}
