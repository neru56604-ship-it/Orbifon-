// Migration for DM and scaling tables

CREATE TABLE IF NOT EXISTS direct_messages (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    sender_id       BIGINT UNSIGNED     NOT NULL,
    receiver_id     BIGINT UNSIGNED     NOT NULL,
    body            VARCHAR(2000)       NOT NULL,
    client_nonce    CHAR(36)            NULL,
    read            TINYINT(1)          NOT NULL DEFAULT 0,
    is_deleted      TINYINT(1)          NOT NULL DEFAULT 0,
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_messages_nonce (sender_id, receiver_id, client_nonce),
    KEY idx_dm_conversation (sender_id, receiver_id, created_at DESC),
    KEY idx_dm_receiver (receiver_id, read, created_at DESC),
    CONSTRAINT fk_dm_sender FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_dm_receiver FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Index for fast lookups
CREATE TABLE IF NOT EXISTS connection_stats (
    id              BIGINT UNSIGNED     NOT NULL AUTO_INCREMENT,
    server_id       VARCHAR(50)         NOT NULL,
    active_connections INT              NOT NULL DEFAULT 0,
    message_count   BIGINT              NOT NULL DEFAULT 0,
    updated_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uq_server_id (server_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
