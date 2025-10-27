use crate::slack::ChannelId;
use uuid::Uuid;

pub type SessionId = String;

/// Generate a unique session ID for a channel
///
/// Format: session-{channel_id}-{timestamp}-{random}
/// Example: session-C09NNKZ8SPP-1761520471-a3f9b2
pub fn generate_session_id(channel_id: &ChannelId) -> SessionId {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let random = &Uuid::new_v4().to_string()[..6];

    format!("session-{}-{}-{}", channel_id.as_str(), timestamp, random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_format() {
        let channel = ChannelId::new("C09NNKZ8SPP");
        let session_id = generate_session_id(&channel);

        // Should start with "session-C09NNKZ8SPP-"
        assert!(session_id.starts_with("session-C09NNKZ8SPP-"));

        // Should have the right number of parts (4: prefix, channel, timestamp, random)
        let parts: Vec<&str> = session_id.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "session");
        assert_eq!(parts[1], "C09NNKZ8SPP");

        // Timestamp should be numeric
        assert!(parts[2].parse::<u64>().is_ok());

        // Random should be 6 chars
        assert_eq!(parts[3].len(), 6);
    }

    #[test]
    fn test_session_id_uniqueness() {
        let channel = ChannelId::new("C09NNKZ8SPP");

        let id1 = generate_session_id(&channel);
        let id2 = generate_session_id(&channel);

        // Should be different (random suffix)
        assert_ne!(id1, id2);
    }
}
