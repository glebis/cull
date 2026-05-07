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

-- Embeddings
CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    vector BLOB NOT NULL,
    dims INTEGER NOT NULL,
    dtype TEXT NOT NULL DEFAULT 'float32',
    normalized INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS embeddings_image_model_uq ON embeddings(image_id, model_name);

-- Indexes
CREATE UNIQUE INDEX IF NOT EXISTS images_sha256_uq ON images(sha256_hash);
CREATE INDEX IF NOT EXISTS image_files_path_idx ON image_files(path);
CREATE INDEX IF NOT EXISTS image_files_image_idx ON image_files(image_id);
CREATE INDEX IF NOT EXISTS images_imported_idx ON images(imported_at);
CREATE INDEX IF NOT EXISTS image_projects_project_idx ON image_projects(project_id, image_id);

CREATE INDEX IF NOT EXISTS iterations_parent_idx ON iterations(parent_id);
CREATE INDEX IF NOT EXISTS iterations_child_idx ON iterations(child_id);

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
