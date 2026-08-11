-- Orbifon — Gwalior Pilot — Initial Schema + Seed Data
-- Run with: sqlx migrate run

CREATE TABLE colleges (
    id              TINYINT UNSIGNED   NOT NULL AUTO_INCREMENT,
    name            VARCHAR(120)       NOT NULL,
    short_tag       VARCHAR(20)        NOT NULL,
    email_domain    VARCHAR(100)       NOT NULL,
    slug            VARCHAR(30)        NOT NULL,
    is_active       TINYINT(1)         NOT NULL DEFAULT 1,
    created_at      TIMESTAMP          NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_colleges_domain (email_domain),
    UNIQUE KEY uq_colleges_slug (slug)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE users (
    id                  BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    college_id          TINYINT UNSIGNED    NOT NULL,
    username            VARCHAR(30)         NOT NULL,
    email               VARCHAR(150)        NOT NULL,
    password_hash       VARCHAR(255)        NOT NULL,
    display_name        VARCHAR(60)         NOT NULL,
    avatar_url          VARCHAR(255)        NULL,
    email_verified_at   TIMESTAMP           NULL,
    verification_token  VARCHAR(100)        NULL,
    is_active           TINYINT(1)          NOT NULL DEFAULT 1,
    created_at          TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_username (username),
    UNIQUE KEY uq_users_email (email),
    KEY idx_users_college (college_id),
    CONSTRAINT fk_users_college FOREIGN KEY (college_id) REFERENCES colleges(id)
        ON DELETE RESTRICT ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE posts (
    id               BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id          BIGINT UNSIGNED     NOT NULL,
    college_id       TINYINT UNSIGNED    NOT NULL,
    body             TEXT                NULL,
    image_path       VARCHAR(255)        NULL,
    upvotes_count    INT UNSIGNED        NOT NULL DEFAULT 0,
    downvotes_count  INT UNSIGNED        NOT NULL DEFAULT 0,
    comments_count   INT UNSIGNED        NOT NULL DEFAULT 0,
    reposts_count    INT UNSIGNED        NOT NULL DEFAULT 0,
    hot_score        DECIMAL(12,6)       NOT NULL DEFAULT 0,
    is_deleted       TINYINT(1)          NOT NULL DEFAULT 0,
    created_at       TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at       TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_posts_hotscore (is_deleted, hot_score DESC),
    KEY idx_posts_created (is_deleted, created_at DESC),
    KEY idx_posts_user (user_id),
    KEY idx_posts_college (college_id),
    CONSTRAINT fk_posts_user FOREIGN KEY (user_id) REFERENCES users(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_posts_college FOREIGN KEY (college_id) REFERENCES colleges(id)
        ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT chk_posts_has_content CHECK (body IS NOT NULL OR image_path IS NOT NULL)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE votes (
    id          BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    post_id     BIGINT UNSIGNED     NOT NULL,
    user_id     BIGINT UNSIGNED     NOT NULL,
    vote_type   ENUM('up','down')   NOT NULL,
    created_at  TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_votes_post_user (post_id, user_id),
    KEY idx_votes_user (user_id),
    CONSTRAINT fk_votes_post FOREIGN KEY (post_id) REFERENCES posts(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_votes_user FOREIGN KEY (user_id) REFERENCES users(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE comments (
    id                 BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    post_id            BIGINT UNSIGNED     NOT NULL,
    user_id            BIGINT UNSIGNED     NOT NULL,
    parent_comment_id  BIGINT UNSIGNED     NULL,
    body               VARCHAR(1000)       NOT NULL,
    upvotes_count      INT UNSIGNED        NOT NULL DEFAULT 0,
    is_deleted         TINYINT(1)          NOT NULL DEFAULT 0,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_comments_post (post_id, created_at),
    KEY idx_comments_parent (parent_comment_id),
    CONSTRAINT fk_comments_post FOREIGN KEY (post_id) REFERENCES posts(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_comments_user FOREIGN KEY (user_id) REFERENCES users(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_comments_parent FOREIGN KEY (parent_comment_id) REFERENCES comments(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE reposts (
    id                 BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id            BIGINT UNSIGNED     NOT NULL,
    original_post_id   BIGINT UNSIGNED     NOT NULL,
    caption            VARCHAR(280)        NULL,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_reposts_user_post (user_id, original_post_id),
    KEY idx_reposts_original (original_post_id),
    CONSTRAINT fk_reposts_user FOREIGN KEY (user_id) REFERENCES users(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_reposts_original FOREIGN KEY (original_post_id) REFERENCES posts(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE hot_town_servers (
    id          SMALLINT UNSIGNED  NOT NULL AUTO_INCREMENT,
    college_id  TINYINT UNSIGNED   NOT NULL,
    name        VARCHAR(100)       NOT NULL,
    slug        VARCHAR(50)        NOT NULL,
    created_at  TIMESTAMP          NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_httown_college (college_id),
    UNIQUE KEY uq_httown_slug (slug),
    CONSTRAINT fk_httown_college FOREIGN KEY (college_id) REFERENCES colleges(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE hot_town_channels (
    id              SMALLINT UNSIGNED  NOT NULL AUTO_INCREMENT,
    server_id       SMALLINT UNSIGNED  NOT NULL,
    name            VARCHAR(50)        NOT NULL,
    display_label   VARCHAR(60)        NOT NULL,
    position        TINYINT UNSIGNED   NOT NULL DEFAULT 0,
    is_anonymous    TINYINT(1)         NOT NULL DEFAULT 0,
    created_at      TIMESTAMP          NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_channel_server_name (server_id, name),
    CONSTRAINT fk_channels_server FOREIGN KEY (server_id) REFERENCES hot_town_servers(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE messages (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    channel_id      SMALLINT UNSIGNED   NOT NULL,
    user_id         BIGINT UNSIGNED     NOT NULL,
    body            VARCHAR(2000)       NOT NULL,
    client_nonce    CHAR(36)            NULL,
    is_deleted      TINYINT(1)          NOT NULL DEFAULT 0,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_messages_nonce (channel_id, client_nonce),
    KEY idx_messages_channel_feed (channel_id, id),
    KEY idx_messages_user (user_id),
    CONSTRAINT fk_messages_channel FOREIGN KEY (channel_id) REFERENCES hot_town_channels(id)
        ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_messages_user FOREIGN KEY (user_id) REFERENCES users(id)
        ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Seed: Gwalior pilot colleges
INSERT INTO colleges (name, short_tag, email_domain, slug) VALUES
('ABV-IIITM Gwalior', 'IIITM Gwalior', 'iiitm.ac.in', 'iiitm-gwalior'),
('MITS Gwalior',      'MITS Gwalior', 'mitsgwalior.in', 'mits-gwalior');

INSERT INTO hot_town_servers (college_id, name, slug)
SELECT id, CONCAT('Hot Town: ', short_tag), CONCAT('hot-town-', slug) FROM colleges;

INSERT INTO hot_town_channels (server_id, name, display_label, position, is_anonymous)
SELECT s.id, c.name, c.display_label, c.position, c.is_anonymous
FROM hot_town_servers s
CROSS JOIN (
    SELECT 'general-gossip'  AS name, '#general-gossip'  AS display_label, 1 AS position, 0 AS is_anonymous
    UNION ALL SELECT 'placement-grind', '#placement-grind', 2, 0
    UNION ALL SELECT 'confessions',     '#confessions',     3, 1
) c;
