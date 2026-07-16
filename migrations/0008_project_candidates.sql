CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.project-candidate.v1'),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    root_folder_node_id INTEGER NOT NULL REFERENCES nodes(id),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(scope_id, root_folder_node_id)
);

CREATE TABLE project_suggestions (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    confidence_basis_points INTEGER NOT NULL CHECK (
        confidence_basis_points > 0 AND confidence_basis_points <= 9500
    ),
    observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms >= 0),
    provider_id TEXT NOT NULL CHECK (provider_id = 'deskgraph.folder-marker-rules'),
    provider_version TEXT NOT NULL CHECK (provider_version = '1'),
    model_version TEXT CHECK (model_version IS NULL),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(project_id, observed_at_unix_ms, provider_id, provider_version)
);

CREATE TABLE project_suggestion_signals (
    suggestion_id INTEGER NOT NULL REFERENCES project_suggestions(id),
    ordinal INTEGER NOT NULL CHECK (ordinal > 0),
    signal_kind TEXT NOT NULL CHECK (signal_kind IN (
        'cargo_manifest',
        'javascript_package',
        'python_project',
        'go_module',
        'swift_package',
        'xcode_project',
        'visual_studio_solution',
        'readme'
    )),
    marker_name TEXT NOT NULL,
    weight_basis_points INTEGER NOT NULL CHECK (weight_basis_points > 0),
    CHECK (
        (signal_kind = 'cargo_manifest' AND marker_name = 'Cargo.toml' AND weight_basis_points = 8500) OR
        (signal_kind = 'javascript_package' AND marker_name = 'package.json' AND weight_basis_points = 7500) OR
        (signal_kind = 'python_project' AND marker_name = 'pyproject.toml' AND weight_basis_points = 8000) OR
        (signal_kind = 'go_module' AND marker_name = 'go.mod' AND weight_basis_points = 8500) OR
        (signal_kind = 'swift_package' AND marker_name = 'Package.swift' AND weight_basis_points = 8500) OR
        (signal_kind = 'xcode_project' AND marker_name = '*.xcodeproj' AND weight_basis_points = 9000) OR
        (signal_kind = 'visual_studio_solution' AND marker_name = '*.sln' AND weight_basis_points = 8500) OR
        (signal_kind = 'readme' AND marker_name = 'README' AND weight_basis_points = 1500)
    ),
    PRIMARY KEY(suggestion_id, ordinal),
    UNIQUE(suggestion_id, signal_kind)
);

CREATE TABLE project_feedback_events (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected')),
    created_by TEXT NOT NULL CHECK (created_by = 'user'),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(project_id, sequence)
);

CREATE INDEX projects_recent_idx ON projects(id DESC);
CREATE INDEX project_suggestions_project_idx ON project_suggestions(project_id, observed_at_unix_ms DESC, id DESC);
CREATE INDEX project_feedback_events_project_idx ON project_feedback_events(project_id, sequence DESC);

CREATE TRIGGER projects_immutable_update
BEFORE UPDATE ON projects
BEGIN
    SELECT RAISE(ABORT, 'projects_immutable');
END;

CREATE TRIGGER projects_immutable_delete
BEFORE DELETE ON projects
BEGIN
    SELECT RAISE(ABORT, 'projects_immutable');
END;

CREATE TRIGGER project_suggestions_immutable_update
BEFORE UPDATE ON project_suggestions
BEGIN
    SELECT RAISE(ABORT, 'project_suggestions_immutable');
END;

CREATE TRIGGER project_suggestions_immutable_delete
BEFORE DELETE ON project_suggestions
BEGIN
    SELECT RAISE(ABORT, 'project_suggestions_immutable');
END;

CREATE TRIGGER project_suggestion_signals_immutable_update
BEFORE UPDATE ON project_suggestion_signals
BEGIN
    SELECT RAISE(ABORT, 'project_suggestion_signals_immutable');
END;

CREATE TRIGGER project_suggestion_signals_immutable_delete
BEFORE DELETE ON project_suggestion_signals
BEGIN
    SELECT RAISE(ABORT, 'project_suggestion_signals_immutable');
END;

CREATE TRIGGER project_feedback_events_immutable_update
BEFORE UPDATE ON project_feedback_events
BEGIN
    SELECT RAISE(ABORT, 'project_feedback_events_immutable');
END;

CREATE TRIGGER project_feedback_events_immutable_delete
BEFORE DELETE ON project_feedback_events
BEGIN
    SELECT RAISE(ABORT, 'project_feedback_events_immutable');
END;
