//! AI INTELLIGENCE — the brain of DIAD AGENT.
//!
//! Four capabilities, one module:
//!
//!   1. Usage & cost tracking   — every LLM request is recorded to an
//!      append-only JSONL log (`~/.diad/logs/usage.jsonl`) with estimated
//!      tokens, latency and outcome. Aggregates are exposed over REST
//!      (`/api/v1/ai/usage`) so the OS can show live spend per provider.
//!   2. Provider failover       — `failover_chain()` returns the ordered list
//!      of ready providers (current first, then every other provider that has
//!      a key), so the chat loop can transparently retry on the next provider
//!      when one errors. Zero-config resilience.
//!   3. Session memory (recall)— keyword scoring over saved sessions so the
//!      agent can ground answers in what the user already did. This is the
//!      long-term memory of the OS.
//!   4. Session export          — any session serializes to Markdown for
//!      sharing, backups, or feeding a RAG pipeline.
//!
//! Everything here is pure, deterministic and unit-testable without a network.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::orchestrator::session::{Session, SessionManager};

// ===========================================================================
// 1. Usage & cost tracking
// ===========================================================================

/// A single LLM request as recorded by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// When the request finished.
    pub ts: DateTime<Utc>,
    /// Provider name (e.g. "anthropic", "kimi").
    pub provider: String,
    /// Model name.
    pub model: String,
    /// Estimated input tokens (chars / 4).
    pub prompt_tokens: u64,
    /// Estimated output tokens (chars / 4).
    pub completion_tokens: u64,
    /// End-to-end latency in milliseconds.
    pub latency_ms: u64,
    /// Whether the turn completed successfully.
    pub success: bool,
}

/// Approximate list prices per 1M tokens (USD), mid-2026 ballpark figures.
/// Providers/models not listed fall back to `FALLBACK_PRICE`.
const PRICING: &[(&str, &str, f64, f64)] = &[
    // (provider, model-substring, in$/1M, out$/1M)
    ("anthropic", "opus", 15.0, 75.0),
    ("anthropic", "sonnet", 3.0, 15.0),
    ("anthropic", "haiku", 1.0, 5.0),
    ("openai", "gpt-5", 1.25, 10.0),
    ("openai", "o4", 2.0, 12.0),
    ("gemini", "pro", 1.25, 10.0),
    ("gemini", "flash", 0.30, 2.50),
    ("kimi", "", 0.60, 2.50),
    ("deepseek", "", 0.27, 1.10),
    ("groq", "", 0.30, 0.60),
    ("mistral", "", 2.00, 6.00),
    ("openrouter", "", 1.00, 3.00),
];

/// Fallback for unknown providers (a reasonable mid-range estimate).
const FALLBACK_PRICE: (f64, f64) = (1.00, 3.00);

/// Estimate the dollar cost of a request, using the pricing table above.
pub fn estimate_cost(
    provider: &str,
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> f64 {
    let p = provider.to_lowercase();
    let m = model.to_lowercase();
    let (in_p, out_p) = PRICING
        .iter()
        .find(|(prov, sub, _, _)| prov == &p.as_str() && (sub.is_empty() || m.contains(sub)))
        .map(|(_, _, i, o)| (*i, *o))
        .unwrap_or(FALLBACK_PRICE);
    (prompt_tokens as f64 / 1_000_000.0) * in_p + (completion_tokens as f64 / 1_000_000.0) * out_p
}

/// Estimate tokens from character count (4 chars ≈ 1 token).
pub fn estimate_tokens(chars: usize) -> u64 {
    (chars / 4) as u64
}

/// Per-provider usage aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub provider: String,
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub est_cost_usd: f64,
}

/// Rolled-up usage summary (what `/api/v1/ai/usage` returns).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub est_cost_usd: f64,
    pub avg_latency_ms: u64,
    pub failed_requests: u64,
    pub by_provider: Vec<ProviderUsage>,
}

/// Aggregate a list of records into a summary (deterministic, sorted).
pub fn aggregate(records: &[UsageRecord]) -> UsageSummary {
    let mut by_provider: Vec<ProviderUsage> = Vec::new();
    for rec in records {
        let provider = rec.provider.clone();
        match by_provider.iter_mut().find(|p| p.provider == provider) {
            Some(p) => {
                p.requests += 1;
                p.prompt_tokens += rec.prompt_tokens;
                p.completion_tokens += rec.completion_tokens;
                p.est_cost_usd += estimate_cost(
                    &rec.provider,
                    &rec.model,
                    rec.prompt_tokens,
                    rec.completion_tokens,
                );
            }
            None => by_provider.push(ProviderUsage {
                provider,
                requests: 1,
                prompt_tokens: rec.prompt_tokens,
                completion_tokens: rec.completion_tokens,
                est_cost_usd: estimate_cost(
                    &rec.provider,
                    &rec.model,
                    rec.prompt_tokens,
                    rec.completion_tokens,
                ),
            }),
        }
    }
    by_provider.sort_by(|a, b| {
        b.est_cost_usd
            .partial_cmp(&a.est_cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_requests = records.len() as u64;
    let total_prompt_tokens = records.iter().map(|r| r.prompt_tokens).sum();
    let total_completion_tokens = records.iter().map(|r| r.completion_tokens).sum();
    let est_cost_usd: f64 = by_provider.iter().map(|p| p.est_cost_usd).sum();
    let failed_requests = records.iter().filter(|r| !r.success).count() as u64;
    let avg_latency_ms = records
        .iter()
        .map(|r| r.latency_ms)
        .sum::<u64>()
        .checked_div(total_requests)
        .unwrap_or(0);

    UsageSummary {
        total_requests,
        total_prompt_tokens,
        total_completion_tokens,
        est_cost_usd,
        avg_latency_ms,
        failed_requests,
        by_provider,
    }
}

/// Append-only usage journal. Records live at `~/.diad/logs/usage.jsonl`.
pub struct UsageTracker {
    dir: PathBuf,
}

impl UsageTracker {
    /// Create the tracker rooted at `~/.diad/logs` (created on demand).
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Self::new_with_dir(PathBuf::from(home).join(".diad").join("logs"))
    }

    /// Create the tracker rooted at an explicit directory (tests use tempdirs).
    pub fn new_with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    /// Path of the journal file.
    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("usage.jsonl")
    }

    /// Append one record. Never fails hard — a journaling failure must not
    /// break the chat loop, so errors are returned for the caller to log.
    pub fn record(&self, rec: &UsageRecord) -> anyhow::Result<()> {
        if let Some(parent) = self.journal_path().parent() {
            fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(rec)?;
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.journal_path())?;
        writeln!(f, "{line}")?;
        Ok(())
    }

    /// Load every recorded usage record (bad lines are skipped).
    pub fn load_all(&self) -> Vec<UsageRecord> {
        let Ok(content) = fs::read_to_string(self.journal_path()) else {
            return Vec::new();
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str::<UsageRecord>(l).ok())
            .collect()
    }

    /// Aggregate the full journal.
    pub fn summary(&self) -> UsageSummary {
        aggregate(&self.load_all())
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 2. Provider failover
// ===========================================================================

/// The ordered failover chain for a provider: the current provider first,
/// then every other ready provider (has a key or keyless), by priority.
///
/// The chat loop uses this to retry automatically when a provider errors —
/// zero-configuration resilience, exactly what an operating system shell
/// should do when its brain hiccups.
pub fn failover_chain(factory: &crate::providers::ProviderFactory, current: &str) -> Vec<String> {
    // Stable priority: anthropic, openai, gemini, qwen, then the OpenAI-compat
    // presets in registry order, then user-defined providers, then current last.
    let mut ordered: Vec<String> = Vec::new();
    let mut push = |name: String| {
        if !ordered.contains(&name) {
            ordered.push(name);
        }
    };

    for name in factory.available_providers() {
        push(name);
    }

    // Ensure the current provider is first in the chain.
    let mut chain: Vec<String> = Vec::new();
    let current_l = current.to_lowercase();
    if let Some(pos) = ordered
        .iter()
        .position(|n| n.eq_ignore_ascii_case(&current_l))
    {
        chain.push(ordered.remove(pos));
    }
    chain.extend(ordered);
    if chain.is_empty() {
        chain.push(current_l);
    }
    chain
}

// ===========================================================================
// 3. Session memory (recall)
// ===========================================================================

/// A single recall hit — a past message that matched the query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallHit {
    pub session_id: String,
    pub title: String,
    pub role: String,
    pub snippet: String,
    pub score: u32,
    pub ts: DateTime<Utc>,
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "was", "with", "this", "that", "from", "have", "has", "had", "you",
    "your", "what", "when", "where", "will", "would", "could", "should", "about", "into", "over",
    "than", "then", "them", "they", "there", "these", "those", "because", "been", "but",
];

/// Tokenize a string into lowercase keyword tokens (≥3 chars, non-stopword).
///
/// Tokens are split on whitespace, stripped of surrounding punctuation, and
/// internal non-alphanumerics are removed — so "Wi-Fi", "wi_fi" and "wifi"
/// all normalize to `wifi`. This makes cross-session matching forgiving.
fn keywords(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Score a message against a query's keywords (token overlap, user messages
/// weigh double — the user's own words matter most).
fn score_message(
    msg: &crate::orchestrator::session::SessionMessage,
    query_terms: &[String],
) -> u32 {
    let text_terms = keywords(&msg.content);
    if text_terms.is_empty() {
        return 0;
    }
    let weight = if msg.role == "user" { 2 } else { 1 };
    query_terms
        .iter()
        .map(|q| if text_terms.contains(q) { weight } else { 0 })
        .sum()
}

/// Retrieve the most relevant past messages for a query across all sessions.
///
/// This is the long-term memory of DIAD: the chat loop injects the top hits
/// as grounding context before asking the model. Pure and deterministic.
pub fn recall(manager: &SessionManager, query: &str, limit: usize) -> Vec<RecallHit> {
    let terms = keywords(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let limit = limit.clamp(1, 50);
    let Ok(sessions) = manager.list_sessions() else {
        return Vec::new();
    };

    let mut hits: Vec<RecallHit> = Vec::new();
    for session in sessions {
        for msg in session
            .messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
        {
            let score = score_message(msg, &terms);
            if score > 0 {
                let snippet: String = msg.content.chars().take(400).collect();
                hits.push(RecallHit {
                    session_id: session.id.clone(),
                    title: session.title.clone(),
                    role: msg.role.clone(),
                    snippet,
                    score,
                    ts: msg.timestamp,
                });
            }
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(b.ts.cmp(&a.ts)));
    hits.truncate(limit);
    hits
}

// ===========================================================================
// 4. Session export (Markdown)
// ===========================================================================

/// Serialize a session to Markdown for sharing, backup or RAG ingestion.
pub fn session_to_markdown(session: &Session) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", session.title));
    out.push_str(&format!(
        "- **Session**: `{}`\n- **Provider**: {} {}\n- **Created**: {}\n- **Updated**: {}\n\n---\n\n",
        session.id,
        session.active_provider,
        session.active_model,
        session.created_at.to_rfc3339(),
        session.updated_at.to_rfc3339(),
    ));
    for msg in &session.messages {
        let role = match msg.role.as_str() {
            "user" => "🧑‍💻 **User**",
            "assistant" => "🤖 **DIAD**",
            "system" => "⚙️ **System**",
            other => other,
        };
        out.push_str(&format!("## {role} — {}\n\n", msg.timestamp.to_rfc3339()));
        out.push_str(&msg.content);
        out.push_str("\n\n---\n\n");
    }
    if session.messages.is_empty() {
        out.push_str("_No messages yet._\n");
    }
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::session::Session;

    fn rec(provider: &str, model: &str, in_t: u64, out_t: u64, ok: bool, lat: u64) -> UsageRecord {
        UsageRecord {
            ts: Utc::now(),
            provider: provider.into(),
            model: model.into(),
            prompt_tokens: in_t,
            completion_tokens: out_t,
            latency_ms: lat,
            success: ok,
        }
    }

    #[test]
    fn test_cost_estimation_known_provider() {
        // 1M in + 1M out on claude sonnet = 3 + 15 = $18
        let c = estimate_cost("anthropic", "claude-sonnet-4", 1_000_000, 1_000_000);
        assert!((c - 18.0).abs() < 1e-6, "got {c}");

        // 1M in + 1M out on claude opus = 15 + 75 = $90
        let c = estimate_cost("anthropic", "claude-opus-4", 1_000_000, 1_000_000);
        assert!((c - 90.0).abs() < 1e-6, "got {c}");

        // gemini flash: 0.30 + 2.50 = $2.80
        let c = estimate_cost("gemini", "gemini-2.5-flash", 1_000_000, 1_000_000);
        assert!((c - 2.80).abs() < 1e-6, "got {c}");
    }

    #[test]
    fn test_cost_estimation_fallback() {
        let c = estimate_cost("mystery-provider", "mystery-model", 1_000_000, 1_000_000);
        assert!((c - 4.0).abs() < 1e-6, "got {c}");
    }

    #[test]
    fn test_token_estimation() {
        assert_eq!(estimate_tokens(400), 100);
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(3), 0);
    }

    #[test]
    fn test_aggregate_groups_and_sorts() {
        let records = vec![
            rec("anthropic", "sonnet", 100, 50, true, 500),
            rec("kimi", "kimi-k2", 200, 80, true, 300),
            rec("anthropic", "sonnet", 100, 50, false, 900),
        ];
        let s = aggregate(&records);
        assert_eq!(s.total_requests, 3);
        assert_eq!(s.total_prompt_tokens, 400);
        assert_eq!(s.total_completion_tokens, 180);
        assert_eq!(s.failed_requests, 1);
        assert_eq!(s.avg_latency_ms, 566);
        assert_eq!(s.by_provider.len(), 2);
        assert_eq!(s.by_provider[0].provider, "anthropic"); // higher cost → first
        assert_eq!(s.by_provider[0].requests, 2);
        assert!(s.est_cost_usd > 0.0);
    }

    #[test]
    fn test_tracker_persists_and_loads() {
        let dir = std::env::temp_dir().join(format!("diad-usage-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let tracker = UsageTracker::new_with_dir(dir.clone());
        tracker
            .record(&rec("deepseek", "deepseek-chat", 10, 20, true, 100))
            .unwrap();
        tracker
            .record(&rec("deepseek", "deepseek-chat", 10, 20, false, 100))
            .unwrap();
        let all = tracker.load_all();
        assert_eq!(all.len(), 2);
        assert!(all[0].success);
        assert!(!all[1].success);
        assert_eq!(tracker.summary().total_requests, 2);
        assert_eq!(tracker.summary().failed_requests, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_tracker_missing_journal_is_empty() {
        let dir = std::env::temp_dir().join(format!("diad-usage-missing-{}", std::process::id()));
        let tracker = UsageTracker::new_with_dir(dir.clone());
        assert!(tracker.load_all().is_empty());
        assert_eq!(tracker.summary().total_requests, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_keywords_filter() {
        let k = keywords("Fix the Wi-Fi and GPU drivers, please!");
        assert!(k.contains(&"wifi".to_string()));
        assert!(k.contains(&"gpu".to_string()));
        assert!(k.contains(&"drivers".to_string()));
        assert!(!k.contains(&"the".to_string())); // stopword
        assert!(!k.contains(&"and".to_string())); // stopword
    }

    #[test]
    fn test_recall_finds_relevant_messages() {
        let dir = std::env::temp_dir().join(format!("diad-recall-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let manager = SessionManager::new_with_dir(dir.clone());
        let mut s = Session::new("Network work", "anthropic");
        s.add_message(
            "user",
            "The Wi-Fi keeps disconnecting, please check the NetworkManager logs",
        );
        s.add_message(
            "assistant",
            "I restarted NetworkManager and set the regulatory domain.",
        );
        manager.save_session(&s).unwrap();

        let mut s2 = Session::new("Cooking", "openai");
        s2.add_message("user", "How do I make pasta al dente?");
        manager.save_session(&s2).unwrap();

        let hits = recall(&manager, "wifi networkmanager disconnects", 5);
        assert!(!hits.is_empty(), "expected at least one hit");
        assert_eq!(hits[0].session_id, s.id);
        assert!(hits[0].snippet.contains("Wi-Fi"));

        let hits2 = recall(&manager, "pasta recipes", 5);
        assert!(!hits2.is_empty());
        assert_eq!(hits2[0].session_id, s2.id);

        // Query with no overlap → empty
        let hits3 = recall(&manager, "quantum physics equations", 5);
        assert!(hits3.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recall_limit_clamped() {
        let dir = std::env::temp_dir().join(format!("diad-recall-limit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let manager = SessionManager::new_with_dir(dir.clone());
        let mut s = Session::new("T", "openai");
        for i in 0..6 {
            s.add_message("user", format!("repeat keyword test {i}"));
        }
        manager.save_session(&s).unwrap();
        let hits = recall(&manager, "repeat keyword", 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(recall(&manager, "repeat keyword", 999).len(), 6);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_markdown_export() {
        let mut s = Session::new("Test session", "anthropic");
        s.active_model = "claude-sonnet-4".into();
        s.add_message("user", "Hello DIAD");
        s.add_message("assistant", "Hello human");
        let md = session_to_markdown(&s);
        assert!(md.contains("# Test session"));
        assert!(md.contains("claude-sonnet-4"));
        assert!(md.contains("Hello DIAD"));
        assert!(md.contains("Hello human"));
        assert!(md.contains("**User**"));
        assert!(md.contains("**DIAD**"));
    }

    #[test]
    fn test_empty_session_markdown() {
        let s = Session::new("Empty", "openai");
        let md = session_to_markdown(&s);
        assert!(md.contains("No messages yet"));
    }
}
