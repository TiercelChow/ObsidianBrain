-- Immutable extraction metadata. Legacy spans deliberately have no inferred
-- outline or neighbors; their text and old citation snapshots remain intact.
CREATE TABLE source_span_structures (
    source_span_id TEXT PRIMARY KEY REFERENCES source_spans(id) ON DELETE CASCADE,
    locator_json TEXT NOT NULL CHECK (json_valid(locator_json) AND json_type(locator_json) = 'object')
);
