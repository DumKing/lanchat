use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

pub fn permission_diff(previous: &[String], requested: &[String]) -> PermissionDiff {
    let previous = previous.iter().cloned().collect::<BTreeSet<_>>();
    let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
    PermissionDiff {
        added: requested.difference(&previous).cloned().collect(),
        removed: previous.difference(&requested).cloned().collect(),
    }
}

pub fn effective_permissions(requested: &[String], granted: &[String]) -> Vec<String> {
    let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
    let granted = granted.iter().cloned().collect::<BTreeSet<_>>();
    requested.intersection(&granted).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn reports_permission_changes_in_stable_order() {
        let diff = permission_diff(
            &values(&["rooms.read", "theme.read"]),
            &values(&["network.lan", "rooms.read"]),
        );
        assert_eq!(diff.added, values(&["network.lan"]));
        assert_eq!(diff.removed, values(&["theme.read"]));
    }

    #[test]
    fn revoked_permission_is_not_effective() {
        assert_eq!(
            effective_permissions(
                &values(&["rooms.read", "rooms.write"]),
                &values(&["rooms.read"])
            ),
            values(&["rooms.read"])
        );
    }
}
