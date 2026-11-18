DELETE FROM knowledge_compile_reports;
DELETE FROM knowledge_compile_checkpoints;
DELETE FROM knowledge_change_sets;
UPDATE knowledge_tasks SET knowledge_change_state = 'none' WHERE knowledge_change_state <> 'none';
DELETE FROM knowledge_entries WHERE entry_type <> 'source_section';
DELETE FROM knowledge_entries_fts
WHERE NOT EXISTS (SELECT 1 FROM knowledge_entries WHERE id = entry_id);

UPDATE knowledge_bases
SET compile_mode = 'chapter', compile_state = 'not_started', compile_error = NULL,
    last_compiled_at = NULL, compile_processed_sources = 0, compile_total_sources = 0,
    pending_review_count = 0, compile_phase = 'idle', compile_message = '',
    compile_current_batch = 0, compile_total_batches = 0, compile_active_run_id = NULL,
    compile_change_set_id = NULL, compile_started_at = NULL, compile_heartbeat_at = NULL,
    compile_cancel_requested = 0;

-- Remove only unpublished prototype pages superseded by a successful whole-book build.
-- Retain pages from the current plan, human edits, saved questions, and in-flight work.
DELETE FROM wiki_topic_pages AS page
WHERE page.published_id IS NULL AND page.running_attempt IS NULL
    AND NOT EXISTS (SELECT 1 FROM wiki_topic_questions WHERE page_id = page.id)
    AND NOT EXISTS (
        SELECT 1 FROM wiki_topic_revisions AS revision
        JOIN agent_runs AS run ON run.id = revision.run_id
        WHERE revision.page_id = page.id AND (
            revision.parent_id IS NOT NULL OR revision.published_at IS NOT NULL
            OR run.task_type <> 'wiki_topic_compile'
            OR json_type(run.input_json, '$.required_topic_citations') = 'array'
        )
    )
    AND EXISTS (
        SELECT 1 FROM wiki_topic_book_builds AS build
        WHERE build.knowledge_base_id = page.knowledge_base_id AND build.status = 'ready'
            AND NOT EXISTS (
                SELECT 1 FROM json_each(build.state_json, '$.pages') AS planned
                WHERE json_extract(planned.value, '$.page_id') = page.id
            )
            AND julianday(page.created_at) < (
                SELECT MIN(julianday(revision.created_at))
                FROM json_each(build.state_json, '$.pages') AS planned
                JOIN wiki_topic_revisions AS revision
                    ON revision.id = json_extract(planned.value, '$.revision_id')
            )
    );
