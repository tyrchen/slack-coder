//! Metadata management for enriched logging
//!
//! This module provides lazy-loading metadata cache for Slack channels and users.
//! It enables human-readable logging while maintaining structured fields.
//!
//! Key features:
//! - Lazy-loading: Only fetches metadata when needed
//! - No bulk fetching: Never fetches all workspace users
//! - TTL-based caching: 1-hour default (configurable)
//! - Graceful degradation: Falls back to IDs if API fails

mod cache;
mod types;

pub use cache::{CacheStats, MetadataCache};
pub use types::{ChannelInfo, ChannelType, LogContext, UserInfo};
