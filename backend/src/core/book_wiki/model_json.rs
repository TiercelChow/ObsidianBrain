use serde_json::Value;

use crate::error::BrainError;

fn invalid(message: impl Into<String>) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

/// Recover only a single, complete object. Never combine objects or repair JSON
/// escapes heuristically: either could silently change the model's claims.
pub(super) fn single_json_object(answer: &str) -> Result<(Value, bool), BrainError> {
    let trimmed = answer.trim().trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(invalid("模型返回空正文；必须返回完整 JSON 对象"));
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| invalid("模型结果不是有效 JSON 对象"))?;
    let prefix = &trimmed[..start];
    if prefix.contains(['[', ']', '}']) || serde_json::from_str::<Value>(prefix.trim()).is_ok() {
        return Err(invalid("模型顶层必须为单个对象，不能包装成数组"));
    }
    let mut stream = serde_json::Deserializer::from_str(&trimmed[start..]).into_iter::<Value>();
    let value = stream
        .next()
        .ok_or_else(|| invalid("模型缺少 JSON 对象"))?
        .map_err(|error| invalid(format!("模型 JSON 解析失败: {error}")))?;
    let suffix = trimmed[start + stream.byte_offset()..].trim();
    if suffix.contains(['{', '}', '[', ']']) {
        return Err(invalid(
            "模型返回多个对象或尾随结构；只允许一个完整 JSON 对象",
        ));
    }
    // A second scalar JSON value is also a second response, not commentary.
    if !suffix.is_empty() && serde_json::from_str::<Value>(suffix).is_ok() {
        return Err(invalid("模型 JSON 后存在第二个值"));
    }
    Ok((value, start > 0 || !suffix.is_empty()))
}
