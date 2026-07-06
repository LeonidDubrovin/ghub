//! Shared HTTP client constants used across the app.

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";

pub const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

/// Cookie header used to bypass the Steam age gate on mature pages.
pub const STEAM_AGE_GATE_COOKIE: &str = "wants_mature_content=1; birthtime=0; lastagecheckage=1-January-1980";

