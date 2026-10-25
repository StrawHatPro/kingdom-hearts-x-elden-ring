//! The runtime's log lines (`party: ...`, `party-play: ...`, `party-trace: ...`, `party_draw: ...`). The host
//! chooses where they go ([`set_logger`]); by default they are printed to stdout, as the test level always did.
//! Lines are said at once (never buffered), so their order among the host's own lines is the order of the calls.

use std::sync::RwLock;

type Logger = Box<dyn Fn(&str) + Send + Sync>;

static LOGGER: RwLock<Option<Logger>> = RwLock::new(None);

/// Where the runtime's lines go from now on (kh-er: its log file). `None` = stdout.
pub fn set_logger(logger: Option<Logger>) {
    if let Ok(mut l) = LOGGER.write() {
        *l = logger;
    }
}

/// One line.
pub fn line(text: &str) {
    match LOGGER.read().ok().as_deref().and_then(|l| l.as_ref()) {
        Some(f) => f(text),
        None => println!("{text}"),
    }
}

/// `say!("party: {x}")`: one formatted line through [`line`].
#[macro_export]
macro_rules! say {
    ($($t:tt)*) => {
        $crate::log::line(&format!($($t)*))
    };
}
