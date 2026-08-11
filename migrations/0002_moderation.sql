-- Moderation and Safety Tables

CREATE TABLE IF NOT EXISTS reports (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    reporter_id     BIGINT UNSIGNED     NOT NULL,
    reported_user_id BIGINT UNSIGNED    NULL,
    post_id         BIGINT UNSIGNED     NULL,
    comment_id      BIGINT UNSIGNED     NULL,
    message_id      BIGINT UNSIGNED     NULL,
    report_type     VARCHAR(50)         NOT NULL,
    reason          VARCHAR(500)        NOT NULL,
    status          VARCHAR(20)         NOT NULL DEFAULT 'pending',
    action_taken    VARCHAR(50)         NULL,
    moderator_id    BIGINT UNSIGNED     NULL,
    reviewed_at     TIMESTAMP           NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_reports_status (status, created_at),
    KEY idx_reports_type (report_type),
    CONSTRAINT fk_reports_reporter FOREIGN KEY (reporter_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_reports_reported_user FOREIGN KEY (reported_user_id) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT fk_reports_moderator FOREIGN KEY (moderator_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS user_suspensions (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id         BIGINT UNSIGNED     NOT NULL,
    reason          VARCHAR(255)        NOT NULL,
    suspended_until TIMESTAMP           NULL,
    created_by      BIGINT UNSIGNED     NOT NULL,
    is_permanent    TINYINT(1)          NOT NULL DEFAULT 0,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_suspensions_user (user_id, suspended_until),
    CONSTRAINT fk_suspend_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_suspend_moderator FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS rate_limit_events (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id         BIGINT UNSIGNED     NOT NULL,
    action          VARCHAR(50)         NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_ratelimit (user_id, action, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS audit_logs (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    actor_id        BIGINT UNSIGNED     NOT NULL,
    action          VARCHAR(100)        NOT NULL,
    target_type     VARCHAR(50)         NOT NULL,
    target_id       BIGINT UNSIGNED     NOT NULL,
    details         JSON                NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_audit (actor_id, action, created_at),
    KEY idx_audit_target (target_type, target_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS reputation_events (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    user_id         BIGINT UNSIGNED     NOT NULL,
    type            VARCHAR(50)         NOT NULL,
    points          INT                 NOT NULL,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_rep (user_id, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Add columns to users table if they don't exist
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_moderator TINYINT(1) NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS reputation_score INT NOT NULL DEFAULT 0;
