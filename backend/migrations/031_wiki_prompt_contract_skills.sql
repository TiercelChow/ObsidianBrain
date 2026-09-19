-- The readable Markdown files remain the source of truth for these built-in
-- skills. Migration 31 deliberately keeps the existing v3 identities: the
-- Rust migration hook invalidates quality results tied to the previous bodies,
-- overwrites metadata and SKILL.md rows inside the same transaction, then makes
-- each v3 the active version.
SELECT 1;
