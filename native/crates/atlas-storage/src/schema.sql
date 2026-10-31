CREATE TABLE profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    dataset_id TEXT NOT NULL,
    progress_version INTEGER NOT NULL CHECK (progress_version = 2),
    revision INTEGER NOT NULL CHECK (revision > 0)
) STRICT;
CREATE TABLE cards (
    word_id TEXT PRIMARY KEY NOT NULL,
    due INTEGER NOT NULL CHECK (due >= 0),
    stability REAL NOT NULL CHECK (stability >= 0),
    difficulty REAL NOT NULL CHECK (difficulty BETWEEN 0 AND 11),
    elapsed_days INTEGER NOT NULL CHECK (elapsed_days >= 0),
    scheduled_days INTEGER NOT NULL CHECK (scheduled_days >= 0),
    learning_steps INTEGER NOT NULL CHECK (learning_steps >= 0),
    reps INTEGER NOT NULL CHECK (reps >= 0),
    lapses INTEGER NOT NULL CHECK (lapses >= 0),
    state INTEGER NOT NULL CHECK (state BETWEEN 0 AND 3),
    last_review INTEGER,
    seen INTEGER NOT NULL CHECK (seen >= 0)
) STRICT;
CREATE INDEX cards_due ON cards(due);
CREATE TABLE day_stats (
    day TEXT PRIMARY KEY NOT NULL,
    new_words INTEGER NOT NULL CHECK (new_words >= 0),
    reviews INTEGER NOT NULL CHECK (reviews >= new_words),
    correct INTEGER NOT NULL CHECK (correct BETWEEN 0 AND reviews),
    active_seconds INTEGER NOT NULL CHECK (active_seconds >= 0)
) STRICT;
CREATE TABLE ordered_words (
    kind TEXT NOT NULL CHECK (kind IN ('bookmarks', 'mistakes')),
    position INTEGER NOT NULL CHECK (position >= 0),
    word_id TEXT NOT NULL,
    PRIMARY KEY (kind, position)
) STRICT;
CREATE TABLE typing_rounds (
    position INTEGER PRIMARY KEY CHECK (position >= 0),
    at INTEGER NOT NULL CHECK (at >= 0),
    mode TEXT NOT NULL CHECK (mode IN ('word', 'sentence', 'dictation')),
    items INTEGER NOT NULL CHECK (items >= 0),
    wpm INTEGER NOT NULL CHECK (wpm BETWEEN 0 AND 400),
    accuracy INTEGER NOT NULL CHECK (accuracy BETWEEN 0 AND 100)
) STRICT;
CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL CHECK (json_valid(value))
) STRICT;
CREATE TABLE active_session (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL CHECK (json_valid(payload))
) STRICT;
CREATE TABLE review_log (
    id INTEGER PRIMARY KEY,
    word_id TEXT NOT NULL,
    reviewed_at INTEGER NOT NULL,
    grade INTEGER NOT NULL CHECK (grade BETWEEN 1 AND 4),
    previous_stability REAL,
    stability REAL NOT NULL,
    difficulty REAL NOT NULL,
    due INTEGER NOT NULL,
    reps INTEGER NOT NULL
) STRICT;
CREATE INDEX review_log_word_time ON review_log(word_id, reviewed_at);
CREATE TABLE recovery_snapshot (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    raw_json TEXT NOT NULL
) STRICT;
CREATE TABLE recovery_reviews (
    id INTEGER PRIMARY KEY,
    word_id TEXT NOT NULL,
    reviewed_at INTEGER NOT NULL,
    grade INTEGER NOT NULL CHECK (grade BETWEEN 1 AND 4),
    previous_stability REAL,
    stability REAL NOT NULL,
    difficulty REAL NOT NULL,
    due INTEGER NOT NULL,
    reps INTEGER NOT NULL
) STRICT;
PRAGMA application_id = 0x5741544c;
PRAGMA user_version = 1;
