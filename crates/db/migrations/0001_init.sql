-- Esquema inicial. Nombres y campos siguen los conceptos de Granola.
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  is_personal INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE spaces (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('my_notes','team','custom')),
  name TEXT NOT NULL,
  icon TEXT,
  description TEXT
);

CREATE TABLE folders (
  id TEXT PRIMARY KEY,
  space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  icon TEXT,
  description TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL DEFAULT '',
  raw_doc TEXT NOT NULL DEFAULT '{"type":"doc","content":[]}',
  enhanced_doc TEXT,
  template_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  calendar_event_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  trashed_at TEXT
);
CREATE INDEX idx_notes_created ON notes(created_at DESC);
CREATE INDEX idx_notes_event ON notes(calendar_event_id);

CREATE TABLE folder_notes (
  folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  PRIMARY KEY (folder_id, note_id)
);

-- Auto-add de reuniones recurrentes a una carpeta.
CREATE TABLE folder_auto_add (
  folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  recurring_event_id TEXT NOT NULL,
  PRIMARY KEY (folder_id, recurring_event_id)
);

CREATE TABLE speakers (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  display_name TEXT,
  person_id TEXT,
  embedding BLOB
);

CREATE TABLE segments (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  channel TEXT NOT NULL CHECK (channel IN ('mic','system')),
  speaker_id TEXT REFERENCES speakers(id) ON DELETE SET NULL,
  start_s REAL NOT NULL,
  end_s REAL NOT NULL,
  text TEXT NOT NULL,
  confidence REAL,
  is_final INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_segments_note ON segments(note_id, start_s);

CREATE TABLE companies (
  id TEXT PRIMARY KEY,
  domain TEXT NOT NULL UNIQUE,
  name TEXT,
  logo_url TEXT
);

CREATE TABLE people (
  id TEXT PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  name TEXT,
  title TEXT,
  avatar_url TEXT,
  company_id TEXT REFERENCES companies(id) ON DELETE SET NULL
);

CREATE TABLE note_people (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  person_id TEXT NOT NULL REFERENCES people(id) ON DELETE CASCADE,
  PRIMARY KEY (note_id, person_id)
);

CREATE TABLE calendar_events (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  calendar_id TEXT NOT NULL,
  title TEXT NOT NULL,
  start_at TEXT NOT NULL,
  end_at TEXT NOT NULL,
  attendees TEXT NOT NULL DEFAULT '[]',
  conference_url TEXT,
  recurring_event_id TEXT,
  description TEXT,
  raw TEXT
);
CREATE INDEX idx_events_start ON calendar_events(start_at);

CREATE TABLE templates (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  purpose TEXT NOT NULL DEFAULT '',
  style TEXT NOT NULL DEFAULT '',
  structure TEXT NOT NULL DEFAULT '',
  is_builtin INTEGER NOT NULL DEFAULT 0,
  shared_with_workspace INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE recipes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  prompt TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('single_meeting','multiple_meetings')),
  model_slot TEXT,
  is_builtin INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE chats (
  id TEXT PRIMARY KEY,
  scope_kind TEXT NOT NULL,   -- home | note | folder | selection | person | company
  scope_ref TEXT,
  title TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE chat_messages (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('user','assistant')),
  content TEXT NOT NULL,
  citations TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE snapshots (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  taken_at_s REAL NOT NULL
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE models (
  slot TEXT NOT NULL,
  name TEXT NOT NULL,
  path TEXT,
  sha256 TEXT,
  installed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (slot, name)
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}',
  run_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  error TEXT
);
