ALTER TABLE knowledge_change_sets ADD COLUMN classification_summary_json TEXT NOT NULL DEFAULT '{"new":0,"update":0,"disputed":0,"no_material":0}';
ALTER TABLE knowledge_change_sets ADD COLUMN citation_audit_json TEXT NOT NULL DEFAULT '{"passed":true,"entry_citations":0,"claim_citations":0,"issues":[]}';
ALTER TABLE knowledge_change_sets ADD COLUMN impact_summary_json TEXT NOT NULL DEFAULT '{"entries":0,"claims":0,"relations":0,"citations":0}';

ALTER TABLE knowledge_changes ADD COLUMN classification TEXT NOT NULL DEFAULT 'update'
    CHECK (classification IN ('new', 'update', 'disputed'));
ALTER TABLE knowledge_changes ADD COLUMN citation_audit_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE knowledge_changes ADD COLUMN impact_json TEXT NOT NULL DEFAULT '{}';

UPDATE knowledge_changes
SET classification = CASE WHEN expected_revision IS NULL THEN 'new' ELSE 'update' END;

