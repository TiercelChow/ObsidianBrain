-- During the current development phase, only the active body of each Skill is
-- retained. The Rust migration hook validates every active pointer, removes
-- stale quality gates, and deletes every non-current version transactionally.
SELECT 1;
