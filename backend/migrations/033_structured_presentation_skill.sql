-- The Rust migration hook overwrites the active built-in presentation Skill
-- in place. Development databases intentionally retain only the current Skill
-- body, so no parallel legacy version is created.
SELECT 1;
