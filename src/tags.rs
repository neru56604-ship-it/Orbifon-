use regex::Regex;
use once_cell::sync::Lazy;

static HASHTAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"#([a-zA-Z0-9_]+)").unwrap()
});

static MENTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"@([a-zA-Z0-9_]+)").unwrap()
});

// =====================================================================
// HASHTAG EXTRACTION & PARSING
// =====================================================================

pub fn extract_hashtags(text: &str) -> Vec<String> {
    HASHTAG_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_lowercase())
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

pub fn extract_mentions(text: &str) -> Vec<String> {
    MENTION_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_lowercase())
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

pub async fn store_post_hashtags(
    db: &sqlx::MySqlPool,
    post_id: u64,
    hashtags: Vec<String>,
) -> Result<(), sqlx::Error> {
    for tag in hashtags {
        sqlx::query(
            "INSERT INTO hashtags (post_id, tag, created_at) VALUES (?, ?, NOW()) \
             ON DUPLICATE KEY UPDATE created_at = NOW()"
        )
        .bind(post_id)
        .bind(&tag)
        .execute(db)
        .await?;
    }
    Ok(())
}

pub async fn store_post_mentions(
    db: &sqlx::MySqlPool,
    post_id: u64,
    mentions: Vec<String>,
) -> Result<(), sqlx::Error> {
    for mention in mentions {
        let user_id: Option<(u64,)> = sqlx::query_as(
            "SELECT id FROM users WHERE username = ? AND is_active = 1"
        )
        .bind(&mention)
        .fetch_optional(db)
        .await?;

        if let Some((uid,)) = user_id {
            sqlx::query(
                "INSERT INTO mentions (post_id, user_id, created_at) VALUES (?, ?, NOW())"
            )
            .bind(post_id)
            .bind(uid)
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hashtags() {
        let text = "This is #placement and #confessions post";
        let tags = extract_hashtags(text);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"placement".to_string()));
        assert!(tags.contains(&"confessions".to_string()));
    }

    #[test]
    fn test_extract_mentions() {
        let text = "Hey @john and @jane check this out";
        let mentions = extract_mentions(text);
        assert_eq!(mentions.len(), 2);
        assert!(mentions.contains(&"john".to_string()));
        assert!(mentions.contains(&"jane".to_string()));
    }
}
