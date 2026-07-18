use tracing_subscriber::{filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Service {
    Cli,
    Desktop,
    Mcp,
}

impl Service {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "deskgraph_cli",
            Self::Desktop => "deskgraph_desktop",
            Self::Mcp => "deskgraph_mcp",
        }
    }
}

/// Installs a structured logger whose call sites accept only fixed event fields.
///
/// Returning `false` means another subscriber was already installed. This is safe
/// in tests and in hosts that configure tracing before DeskGraph starts.
#[must_use]
pub fn init_privacy_safe_logging(service: Service) -> bool {
    let service_name = service.as_str();
    let deskgraph_only = filter::filter_fn(|metadata| metadata.target().starts_with("deskgraph"));
    let installed = tracing_subscriber::registry()
        .with(deskgraph_only)
        .with(
            fmt::layer()
                .json()
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .try_init()
        .is_ok();

    if installed {
        tracing::info!(event = "logging_initialized", service = service_name);
    }

    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_names_are_fixed_and_contain_no_user_data() {
        assert_eq!(Service::Cli.as_str(), "deskgraph_cli");
        assert_eq!(Service::Desktop.as_str(), "deskgraph_desktop");
        assert_eq!(Service::Mcp.as_str(), "deskgraph_mcp");
    }

    #[test]
    fn repeated_logger_initialization_fails_safely() {
        let _first = init_privacy_safe_logging(Service::Cli);
        let second = init_privacy_safe_logging(Service::Desktop);

        assert!(!second);
    }
}
