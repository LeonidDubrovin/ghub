// Metadata module - Strategy pattern for external metadata sources
mod strategy;
mod steam;
mod itch;
mod aggregator;
#[cfg(test)]
mod tests;

pub use strategy::MetadataStrategy;
pub use steam::SteamStrategy;
pub use itch::ItchStrategy;
pub use aggregator::MetadataAggregator;
