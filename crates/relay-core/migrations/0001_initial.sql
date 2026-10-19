CREATE TABLE sessions (
    session_id UUID PRIMARY KEY,
    creator_participant_id UUID NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('created', 'active', 'ended')),
    invite_hash BYTEA NOT NULL CHECK (octet_length(invite_hash) = 32),
    invite_expires_at_ms BIGINT NOT NULL CHECK (invite_expires_at_ms > 0),
    invite_join_count SMALLINT NOT NULL DEFAULT 0 CHECK (invite_join_count BETWEEN 0 AND 7),
    next_evidence_order BIGINT NOT NULL DEFAULT 1 CHECK (next_evidence_order BETWEEN 1 AND 110001),
    event_count INTEGER NOT NULL DEFAULT 1 CHECK (event_count BETWEEN 1 AND 10000),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms > 0),
    started_at_ms BIGINT,
    ended_at_ms BIGINT,
    final_manifest JSONB
);

CREATE TABLE participants (
    session_id UUID NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    participant_id UUID NOT NULL,
    nickname TEXT NOT NULL CHECK (length(nickname) BETWEEN 1 AND 32),
    public_key BYTEA NOT NULL CHECK (octet_length(public_key) = 32),
    head_sequence BIGINT NOT NULL DEFAULT 0 CHECK (head_sequence BETWEEN 0 AND 10000),
    head_hash BYTEA NOT NULL DEFAULT decode(repeat('00', 32), 'hex') CHECK (octet_length(head_hash) = 32),
    connection_generation BIGINT NOT NULL DEFAULT 0 CHECK (connection_generation >= 0),
    heartbeat_counter BIGINT NOT NULL DEFAULT 0 CHECK (heartbeat_counter >= 0),
    last_heartbeat_ms BIGINT,
    liveness_state TEXT NOT NULL DEFAULT 'disconnected' CHECK (liveness_state IN ('online', 'stale', 'disconnected', 'ended')),
    joined_at_ms BIGINT NOT NULL CHECK (joined_at_ms > 0),
    PRIMARY KEY (session_id, participant_id)
);

CREATE UNIQUE INDEX participants_nickname_ci ON participants (session_id, lower(nickname));

CREATE TABLE events (
    session_id UUID NOT NULL,
    participant_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 10000),
    event_id UUID NOT NULL,
    event_hash BYTEA NOT NULL CHECK (octet_length(event_hash) = 32),
    event_json JSONB NOT NULL,
    evidence_order BIGINT NOT NULL CHECK (evidence_order > 0),
    accepted_at_ms BIGINT NOT NULL CHECK (accepted_at_ms > 0),
    receipt_json JSONB NOT NULL,
    PRIMARY KEY (session_id, participant_id, sequence),
    UNIQUE (session_id, event_id),
    UNIQUE (session_id, evidence_order),
    FOREIGN KEY (session_id, participant_id) REFERENCES participants(session_id, participant_id) ON DELETE CASCADE
);

CREATE INDEX events_session_order ON events (session_id, evidence_order);

CREATE TABLE relay_notices (
    session_id UUID NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    notice_id UUID NOT NULL,
    evidence_order BIGINT NOT NULL CHECK (evidence_order > 0),
    notice_json JSONB NOT NULL,
    PRIMARY KEY (session_id, notice_id),
    UNIQUE (session_id, evidence_order)
);

CREATE TABLE relay_key_metadata (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    public_key BYTEA NOT NULL CHECK (octet_length(public_key) = 32),
    fingerprint BYTEA NOT NULL CHECK (octet_length(fingerprint) = 32),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms > 0)
);
