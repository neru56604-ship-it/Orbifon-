-- User Profiles & Follow System

ALTER TABLE users ADD COLUMN IF NOT EXISTS bio VARCHAR(500);
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_url VARCHAR(500);
ALTER TABLE users ADD COLUMN IF NOT EXISTS cover_image_url VARCHAR(500);
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_verified TINYINT(1) DEFAULT 0;

CREATE TABLE IF NOT EXISTS follows (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    follower_id     BIGINT UNSIGNED     NOT NULL,
    following_id    BIGINT UNSIGNED     NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_follow (follower_id, following_id),
    KEY idx_follower (follower_id),
    KEY idx_following (following_id),
    CONSTRAINT fk_follow_follower FOREIGN KEY (follower_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_follow_following FOREIGN KEY (following_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Hashtags & Mentions

CREATE TABLE IF NOT EXISTS hashtags (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    post_id         BIGINT UNSIGNED     NOT NULL,
    tag             VARCHAR(100)        NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_post_tag (post_id, tag),
    KEY idx_tag (tag),
    KEY idx_tag_created (tag, created_at DESC),
    CONSTRAINT fk_hash_post FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS mentions (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    post_id         BIGINT UNSIGNED     NOT NULL,
    user_id         BIGINT UNSIGNED     NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_post_mention (post_id, user_id),
    KEY idx_user (user_id),
    CONSTRAINT fk_mention_post FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
    CONSTRAINT fk_mention_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Bookmarks

CREATE TABLE IF NOT EXISTS bookmarks (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id         BIGINT UNSIGNED     NOT NULL,
    post_id         BIGINT UNSIGNED     NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_bookmark (user_id, post_id),
    KEY idx_user (user_id),
    CONSTRAINT fk_bookmark_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_bookmark_post FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
