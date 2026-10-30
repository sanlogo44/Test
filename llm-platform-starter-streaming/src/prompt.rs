//! Deterministic, inspectable prompt structuring. This is not an LLM rewrite and does not invent context.
use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OptimizedPrompt {
    pub original: String,
    pub optimized: String,
    pub mode: String,
}

pub fn optimize(original: &str, mode: &str) -> Result<OptimizedPrompt> {
    let request = original.trim();
    if request.is_empty() {
        bail!("prompt must not be empty");
    }
    if request.len() > 100_000 {
        bail!("prompt exceeds the 100000-byte safety limit");
    }
    let mode = mode.trim().to_ascii_lowercase();
    if mode != "conservative" && mode != "advanced" {
        bail!("unsupported prompt optimization mode `{mode}`; use conservative or advanced");
    }
    let guidance = if mode == "advanced" {
        "\n\nConstraints and rules:\n- Preserve the user's intent and requested language.\n- Do not invent facts, sources, or missing context; state assumptions explicitly.\n- If essential information is missing, ask a concise clarification question.\n- Keep the response relevant and proportionate to the task.\n\nOutput format:\n- Use a clear structure suited to the task.\n- Distinguish verified facts from assumptions and uncertainty."
    } else {
        "\n\nConstraints and rules:\n- Preserve the original scope and intent.\n- Do not invent facts or context.\n- Ask for clarification only if essential information is missing.\n\nOutput format:\n- Use a format appropriate to the request."
    };
    let optimized = format!(
        "Role and persona:\nAct as a capable assistant for the task below.\n\nContext and background:\nUse only context explicitly supplied by the user or otherwise available to you.\n\nTask:\n{request}{guidance}"
    );
    Ok(OptimizedPrompt {
        original: request.to_owned(),
        optimized,
        mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_original_request_and_rejects_empty_input() {
        let result = optimize("  Summarize this report.  ", "conservative").unwrap();
        assert_eq!(result.original, "Summarize this report.");
        assert!(result.optimized.contains("Summarize this report."));
        assert!(optimize("  ", "conservative").is_err());
    }
    #[test]
    fn advanced_mode_adds_explicit_uncertainty_rules() {
        let result = optimize("Compare these options", "advanced").unwrap();
        assert!(result.optimized.contains("Do not invent facts"));
        assert!(optimize("hello", "disabled").is_err());
    }
}
