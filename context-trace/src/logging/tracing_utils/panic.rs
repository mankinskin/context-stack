//! Panic hook installation

use super::config::PanicConfig;
use std::sync::Once;

static PANIC_HOOK_INIT: Once = Once::new();

/// Install a panic hook that logs panic information before unwinding
pub(super) fn install_panic_hook(config: PanicConfig) {
    PANIC_HOOK_INIT.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // Log panic before unwinding closes spans (if enabled)
            if config.show {
                if config.show_message {
                    tracing::error!(
                        panic_message = %panic_info,
                        "PANIC occurred!"
                    );
                } else {
                    tracing::error!("PANIC occurred!");
                }
            }

            // Also write to stderr for visibility (if enabled)
            if config.show_stderr {
                eprintln!("\n🔥 PANIC: {}", panic_info);
            }

            // Call the default hook (which prints to stderr)
            default_hook(panic_info);
        }));
    });
}
