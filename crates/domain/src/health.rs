use serde::Serialize;

pub const HEALTH_API_VERSION: &str = "deskgraph.health.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Ready,
    NotInitialized,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComponentHealth {
    pub state: LifecycleState,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlatformHealth {
    pub os: &'static str,
    pub architecture: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderHealth {
    pub ocr: ComponentHealth,
    pub embeddings: ComponentHealth,
    pub local_llm: ComponentHealth,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrivacyHealth {
    pub local_only_default: bool,
    pub network_required: bool,
    pub filesystem_locations_included: bool,
    pub authorized_scope_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HealthReport {
    pub api_version: &'static str,
    pub product: &'static str,
    pub app_version: &'static str,
    pub status: &'static str,
    pub platform: PlatformHealth,
    pub database: ComponentHealth,
    pub providers: ProviderHealth,
    pub privacy: PrivacyHealth,
}

#[must_use]
pub fn collect_health() -> HealthReport {
    let provider_disabled = || ComponentHealth {
        state: LifecycleState::Disabled,
        reason: "optional_provider_not_configured",
    };

    HealthReport {
        api_version: HEALTH_API_VERSION,
        product: "DeskGraph",
        app_version: env!("CARGO_PKG_VERSION"),
        status: "ok",
        platform: PlatformHealth {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        database: ComponentHealth {
            state: LifecycleState::NotInitialized,
            reason: "manifest_database_pending_m1",
        },
        providers: ProviderHealth {
            ocr: provider_disabled(),
            embeddings: provider_disabled(),
            local_llm: provider_disabled(),
        },
        privacy: PrivacyHealth {
            local_only_default: true,
            network_required: false,
            filesystem_locations_included: false,
            authorized_scope_count: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_is_honest_before_manifest_initialization() {
        let report = collect_health();

        assert_eq!(report.status, "ok");
        assert_eq!(report.database.state, LifecycleState::NotInitialized);
        assert_eq!(report.providers.ocr.state, LifecycleState::Disabled);
        assert_eq!(report.privacy.authorized_scope_count, 0);
        assert!(!report.privacy.network_required);
    }

    #[test]
    fn serialized_health_uses_the_stable_privacy_safe_schema() {
        let value = serde_json::to_value(collect_health()).expect("health must serialize");

        assert_eq!(value["api_version"], HEALTH_API_VERSION);
        assert_eq!(value["database"]["state"], "not_initialized");
        assert_eq!(value["providers"]["local_llm"]["state"], "disabled");
        assert_eq!(value["privacy"]["filesystem_locations_included"], false);
    }

    #[test]
    fn serialized_health_does_not_leak_filesystem_locations() {
        let serialized = serde_json::to_string(&collect_health()).expect("health must serialize");

        assert!(!serialized.contains("/Users/"));
        assert!(!serialized.contains("C:\\Users\\"));
        assert!(!serialized.contains("HOME"));
    }
}
