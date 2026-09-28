//! Network conditions the scheduler respects (SPEC §7.2).

/// Whether the current internet connection is metered (Windows only; the spec makes other
/// platforms optional, and they report `false`).
#[cfg(windows)]
pub fn is_metered() -> bool {
    use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return false;
    };
    let Ok(cost) = profile.GetConnectionCost() else {
        return false;
    };
    let limited = matches!(
        cost.NetworkCostType(),
        Ok(NetworkCostType::Fixed) | Ok(NetworkCostType::Variable)
    );
    limited || cost.Roaming().unwrap_or(false) || cost.OverDataLimit().unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_metered() -> bool {
    false
}

/// Whether metered detection works on this platform (the setting is hidden elsewhere).
pub const METERED_SUPPORTED: bool = cfg!(windows);
