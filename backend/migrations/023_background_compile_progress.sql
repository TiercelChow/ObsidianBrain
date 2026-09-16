ALTER TABLE knowledge_bases ADD COLUMN compile_phase TEXT NOT NULL DEFAULT 'idle';
ALTER TABLE knowledge_bases ADD COLUMN compile_message TEXT NOT NULL DEFAULT '';
ALTER TABLE knowledge_bases ADD COLUMN compile_current_batch INTEGER NOT NULL DEFAULT 0;
ALTER TABLE knowledge_bases ADD COLUMN compile_total_batches INTEGER NOT NULL DEFAULT 0;
ALTER TABLE knowledge_bases ADD COLUMN compile_active_run_id TEXT;
ALTER TABLE knowledge_bases ADD COLUMN compile_change_set_id TEXT;
ALTER TABLE knowledge_bases ADD COLUMN compile_started_at TEXT;
ALTER TABLE knowledge_bases ADD COLUMN compile_heartbeat_at TEXT;
ALTER TABLE knowledge_bases ADD COLUMN compile_cancel_requested INTEGER NOT NULL DEFAULT 0
    CHECK (compile_cancel_requested IN (0, 1));

-- Before background execution existed, a successfully generated but unreviewed
-- change set also used compile_state='compiling'. Preserve that review state on
-- upgrade instead of treating it as an interrupted model call during recovery.
UPDATE knowledge_bases
SET compile_phase = 'waiting_review',
    compile_message = '智能编译完成，等待审核知识变更',
    compile_change_set_id = (
        SELECT kcs.id
        FROM knowledge_change_sets kcs
        WHERE kcs.knowledge_base_id = knowledge_bases.id
          AND kcs.status = 'proposed'
        ORDER BY kcs.created_at DESC
        LIMIT 1
    ),
    compile_heartbeat_at = updated_at
WHERE compile_state = 'compiling'
  AND EXISTS (
      SELECT 1
      FROM knowledge_change_sets kcs
      WHERE kcs.knowledge_base_id = knowledge_bases.id
        AND kcs.status = 'proposed'
  );
