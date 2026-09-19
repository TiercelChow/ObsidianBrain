-- Skill content is embedded at compile time and overwritten by the Rust
-- migration hook so existing development databases receive the same current
-- version without preserving stale built-in bodies.
SELECT 1;
