// Commands module - organized by functionality
// This module re-exports all commands for backward compatibility

mod spaces;
mod games;
mod scanning;
mod metadata;
mod settings;
mod downloads;
mod playtime;
mod backup;

// Re-export all commands
pub use spaces::*;
pub use games::*;
pub use scanning::*;
pub use metadata::*;
pub use settings::*;
pub use downloads::*;
pub use playtime::*;
pub use backup::*;