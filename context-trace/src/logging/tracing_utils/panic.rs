//! Panic hook installation

use super::config::PanicConfig;
use std::backtrace::Backtrace;
use std::sync::Once;

static PANIC_HOOK_INIT: Once = Once::new();

/// Install a panic hook that logs panic information before unwinding
pub(super) fn install_panic_hook(config: PanicConfig) {
    PANIC_HOOK_INIT.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // Capture backtrace immediately
            let backtrace = Backtrace::force_capture();
            
            // Log panic before unwinding closes spans (if enabled)
            if config.show {
                if config.show_message {
                    // Extract panic message from payload
                    let panic_msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                        s.clone()
                    } else {
                        format!("{}", panic_info)
                    };
                    
                    let bt_str = format!("{}", backtrace);
                    
                    // Extract location info for structured logging
                    if let Some(location) = panic_info.location() {
                        tracing::error!(
                            panic_file = %location.file(),
                            panic_line = location.line(),
                            panic_column = location.column(),
                            backtrace = %bt_str,
                            "PANIC: {}", panic_msg
                        );
                    } else {
                        tracing::error!(
                            backtrace = %bt_str,
                            "PANIC: {}", panic_msg
                        );
                    }
                } else {
                    tracing::error!("PANIC occurred!");
                }
            }

            // Also write to stderr for visibility (if enabled)
            if config.show_stderr {
                eprintln!("\n🔥 PANIC: {}", panic_info);
                eprintln!("Backtrace:\n{}", backtrace);
            }

            // Call the default hook (which prints to stderr) if enabled
            if config.show_default_hook {
                default_hook(panic_info);
            }
        }));
    });
}
