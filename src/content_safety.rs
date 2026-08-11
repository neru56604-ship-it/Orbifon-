use once_cell::sync::Lazy;
use regex::Regex;

pub struct ContentSafety;

// Bad word list (minimal example - use better service in production)
static PROFANITY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(badword1|badword2|badword3)").unwrap()
});

impl ContentSafety {
    pub fn check_content(text: &str) -> ContentCheckResult {
        let mut issues = Vec::new();

        // Check for excessive caps
        let alpha_count = text.chars().filter(|c| c.is_alphabetic()).count();
        if alpha_count > 0 {
            let caps_ratio = text.chars().filter(|c| c.is_uppercase()).count() as f32 / alpha_count as f32;
            if caps_ratio > 0.7 {
                issues.push("excessive_caps".to_string());
            }
        }

        // Check for profanity
        if PROFANITY_RE.is_match(text) {
            issues.push("profanity_detected".to_string());
        }

        // Check for spam indicators (multiple links)
        let link_count = text.matches("http").count();
        if link_count > 5 {
            issues.push("spam_indicators".to_string());
        }

        // Check for repeated characters (spam)
        let chars: Vec<char> = text.chars().collect();
        for i in 0..chars.len().saturating_sub(5) {
            if chars[i] == chars[i + 1]
                && chars[i + 1] == chars[i + 2]
                && chars[i + 2] == chars[i + 3]
            {
                issues.push("spam_indicators".to_string());
                break;
            }
        }

        ContentCheckResult {
            is_safe: issues.is_empty(),
            issues,
            recommendation: if issues.is_empty() {
                ContentRecommendation::Approve
            } else {
                ContentRecommendation::Review
            },
        }
    }
}

#[derive(Debug)]
pub struct ContentCheckResult {
    pub is_safe: bool,
    pub issues: Vec<String>,
    pub recommendation: ContentRecommendation,
}

#[derive(Debug)]
pub enum ContentRecommendation {
    Approve,
    Review,
    Reject,
}
