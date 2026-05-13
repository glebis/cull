-- Core
CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    sha256_hash TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    format TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

-- Source detection evidence (added to images table via migration in db.rs)
-- source_label TEXT, source_confidence REAL, source_evidence_json TEXT,
-- source_detected_at TEXT, source_detector_version TEXT,
-- is_ai_generated INTEGER, ai_prompt TEXT, aspect_ratio REAL,
-- orientation TEXT, original_date TEXT, megapixels REAL

CREATE TABLE IF NOT EXISTS image_files (
    id TEXT PRIMARY KEY,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    missing_at TEXT
);

CREATE TABLE IF NOT EXISTS image_metadata (
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY (image_id, key, source)
);

-- Organization
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS image_projects (
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY (image_id, project_id)
);

CREATE TABLE IF NOT EXISTS selections (
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL DEFAULT '__global__',
    star_rating INTEGER CHECK (star_rating BETWEEN 0 AND 5),
    color_label TEXT CHECK (color_label IN ('red', 'green', 'blue', 'yellow', NULL)),
    decision TEXT CHECK (decision IN ('accept', 'reject', 'undecided')) DEFAULT 'undecided',
    PRIMARY KEY (image_id, project_id)
);

-- Iterations
CREATE TABLE IF NOT EXISTS iterations (
    id TEXT PRIMARY KEY,
    parent_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    child_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    prompt TEXT,
    negative_prompt TEXT,
    region_id TEXT,
    seed INTEGER,
    model_used TEXT,
    params_json TEXT,
    created_at TEXT NOT NULL
);

-- Collections (ordered image sets using existing projects table)
CREATE TABLE IF NOT EXISTS collection_items (
    collection_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (collection_id, image_id)
);

CREATE INDEX IF NOT EXISTS collection_items_pos_idx ON collection_items(collection_id, position);

-- Session activity ledger
CREATE TABLE IF NOT EXISTS session_events (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'agent', 'system')),
    actor_id TEXT,
    subject_type TEXT,
    subject_id TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS session_events_session_created_idx ON session_events(session_id, created_at);
CREATE INDEX IF NOT EXISTS session_events_type_created_idx ON session_events(event_type, created_at);
CREATE INDEX IF NOT EXISTS session_events_subject_idx ON session_events(subject_type, subject_id);

-- Local UI diagnostics for asset rendering failures.
-- Stores only privacy-safe identifiers, never full filesystem paths.
CREATE TABLE IF NOT EXISTS asset_load_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    view TEXT NOT NULL,
    image_id TEXT,
    asset_kind TEXT NOT NULL,
    image_format TEXT,
    fallback_used INTEGER NOT NULL DEFAULT 0,
    fallback_succeeded INTEGER,
    path_basename TEXT,
    path_hash TEXT,
    error_kind TEXT NOT NULL,
    details_json TEXT
);
CREATE INDEX IF NOT EXISTS asset_load_events_created_idx ON asset_load_events(created_at);
CREATE INDEX IF NOT EXISTS asset_load_events_image_created_idx ON asset_load_events(image_id, created_at);
CREATE INDEX IF NOT EXISTS asset_load_events_error_idx ON asset_load_events(error_kind, created_at);

-- Embeddings
CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    model_run_id TEXT,
    vector BLOB NOT NULL,
    dims INTEGER NOT NULL,
    dtype TEXT NOT NULL DEFAULT 'float32',
    normalized INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS embeddings_image_model_uq ON embeddings(image_id, model_name);
CREATE INDEX IF NOT EXISTS embeddings_model_run_idx ON embeddings(model_run_id);

-- Model processing provenance
CREATE TABLE IF NOT EXISTS model_profiles (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    provider TEXT NOT NULL,
    task TEXT NOT NULL,
    model_id TEXT NOT NULL,
    runtime TEXT NOT NULL,
    source TEXT NOT NULL,
    privacy_class TEXT NOT NULL DEFAULT 'local',
    config_json TEXT NOT NULL DEFAULT '{}',
    license_class TEXT NOT NULL DEFAULT 'unknown',
    license_acknowledged_at TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT,
    parent_run_id TEXT REFERENCES model_runs(id),
    profile_id TEXT REFERENCES model_profiles(id),
    task TEXT NOT NULL,
    provider TEXT NOT NULL,
    model_id TEXT NOT NULL,
    model_revision TEXT,
    status TEXT NOT NULL,
    input_scope_json TEXT NOT NULL,
    params_json TEXT NOT NULL DEFAULT '{}',
    output_summary_json TEXT NOT NULL DEFAULT '{}',
    cost_estimate_usd REAL,
    cost_actual_usd REAL,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS model_run_items (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES model_runs(id) ON DELETE CASCADE,
    image_id TEXT REFERENCES images(id),
    input_asset_uri TEXT NOT NULL,
    input_hash TEXT,
    status TEXT NOT NULL,
    output_ref_kind TEXT,
    output_ref_id TEXT,
    audit_payload_json TEXT,
    cost_usd REAL,
    attempt_count INTEGER NOT NULL DEFAULT 1,
    error TEXT,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS model_runs_job_idx ON model_runs(job_id);
CREATE INDEX IF NOT EXISTS model_runs_status_idx ON model_runs(status);
CREATE INDEX IF NOT EXISTS model_runs_parent_idx ON model_runs(parent_run_id);
CREATE INDEX IF NOT EXISTS model_run_items_run_status_idx ON model_run_items(run_id, status);
CREATE INDEX IF NOT EXISTS model_run_items_image_run_idx ON model_run_items(image_id, run_id);
CREATE INDEX IF NOT EXISTS model_run_items_input_hash_idx ON model_run_items(input_hash);

-- App Settings
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Indexes
CREATE UNIQUE INDEX IF NOT EXISTS images_sha256_uq ON images(sha256_hash);
CREATE INDEX IF NOT EXISTS image_files_path_idx ON image_files(path);
CREATE INDEX IF NOT EXISTS image_files_image_idx ON image_files(image_id);
CREATE INDEX IF NOT EXISTS images_imported_idx ON images(imported_at);
CREATE INDEX IF NOT EXISTS image_projects_project_idx ON image_projects(project_id, image_id);

CREATE INDEX IF NOT EXISTS iterations_parent_idx ON iterations(parent_id);
CREATE INDEX IF NOT EXISTS iterations_child_idx ON iterations(child_id);

-- Detections (YOLO object detection + NudeNet content safety)
CREATE TABLE IF NOT EXISTS detections (
    id TEXT PRIMARY KEY,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    class_name TEXT NOT NULL,
    confidence REAL NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    width REAL NOT NULL,
    height REAL NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_detections_image ON detections(image_id);
CREATE INDEX IF NOT EXISTS idx_detections_class ON detections(class_name);
CREATE INDEX IF NOT EXISTS idx_detections_model ON detections(model_name);

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
