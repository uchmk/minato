//! A workflow file read just enough for the Actions view: which job needs
//! which (the graph), the inputs of `workflow_dispatch` (the form), and the
//! tail of a job's log. No YAML library: the file is read as an indented
//! tree of `key: value` lines and `- item` lines, which is all these need.

use serde_json::Value;

use crate::actions::{Job, Run};

/// A line of the file as a node: `key: value` with the lines under it, or a
/// list item (`key` is `-`).
#[derive(Debug, Default)]
struct Node {
    key: String,
    val: String,
    kids: Vec<Node>,
}

impl Node {
    fn get(&self, key: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.key == key)
    }

    /// The scalar under `key`, or an empty string.
    fn text(&self, key: &str) -> String {
        self.get(key).map(|n| n.val.clone()).unwrap_or_default()
    }

    /// The list under `key`: block items, or an inline `[a, b]`, or one scalar.
    fn list(&self, key: &str) -> Vec<String> {
        let Some(n) = self.get(key) else { return Vec::new() };
        let mut out: Vec<String> = n.kids.iter().filter(|k| k.key == "-").map(|k| k.val.clone()).collect();
        let v = n.val.trim();
        if let Some(inner) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            out.extend(inner.split(',').map(unquote).filter(|s| !s.is_empty()));
        } else if !v.is_empty() {
            out.push(unquote(v));
        }
        out
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let q = s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')));
    if q { s[1..s.len() - 1].to_string() } else { s.to_string() }
}

/// Cut a trailing ` # comment` (outside quotes, which is good enough here).
fn uncomment(line: &str) -> &str {
    let (mut quote, mut prev) = (None, ' ');
    for (i, ch) in line.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(q), c) if c == q => quote = None,
            (None, '#') if prev == ' ' || i == 0 => return &line[..i],
            _ => {}
        }
        prev = ch;
    }
    line
}

fn tree(yaml: &str) -> Node {
    // Each open node with the indent of its line; the root is -1.
    let mut stack: Vec<(i32, Node)> = vec![(-1, Node::default())];
    for raw in yaml.lines() {
        let line = uncomment(raw).trim_end();
        let body = line.trim_start();
        if body.is_empty() || body == "---" {
            continue;
        }
        let indent = (line.len() - body.len()) as i32;
        let (key, val) = if let Some(item) = body.strip_prefix("- ") {
            ("-".to_string(), unquote(item))
        } else if body == "-" {
            ("-".to_string(), String::new())
        } else if let Some((k, v)) = body.split_once(':') {
            (unquote(k), unquote(v))
        } else {
            continue;
        };
        while stack.len() > 1 && stack.last().is_some_and(|(i, _)| *i >= indent) {
            let (_, done) = stack.pop().expect("checked");
            stack.last_mut().expect("root stays").1.kids.push(done);
        }
        stack.push((indent, Node { key, val, kids: Vec::new() }));
    }
    while stack.len() > 1 {
        let (_, done) = stack.pop().expect("checked");
        stack.last_mut().expect("root stays").1.kids.push(done);
    }
    stack.pop().expect("root").1
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WJob {
    /// The key under `jobs:`, which `needs:` names.
    pub key: String,
    /// What the API calls the job (`name:`, else the key).
    pub name: String,
    pub needs: Vec<String>,
}

impl WJob {
    /// Whether an API job (a matrix adds ` (…)`, a reusable workflow ` / …`)
    /// comes from this one.
    pub fn is(&self, api_name: &str) -> bool {
        api_name == self.name || api_name.strip_prefix(&self.name).is_some_and(|r| r.starts_with(" (") || r.starts_with(" / "))
    }
}

/// One input of `workflow_dispatch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub description: String,
    pub required: bool,
    /// `string`, `boolean`, `choice`, `number`, `environment`.
    pub kind: String,
    pub default: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workflow {
    pub jobs: Vec<WJob>,
    pub inputs: Vec<Input>,
}

pub fn parse(yaml: &str) -> Workflow {
    let root = tree(yaml);
    let jobs = root
        .get("jobs")
        .map(|j| {
            j.kids
                .iter()
                .filter(|k| k.key != "-")
                .map(|k| {
                    let name = k.text("name");
                    WJob { key: k.key.clone(), name: if name.is_empty() { k.key.clone() } else { name }, needs: k.list("needs") }
                })
                .collect()
        })
        .unwrap_or_default();
    let inputs = root
        .get("on")
        .or_else(|| root.get("true"))
        .and_then(|on| on.get("workflow_dispatch"))
        .and_then(|w| w.get("inputs"))
        .map(|i| {
            i.kids
                .iter()
                .map(|n| {
                    let kind = n.text("type");
                    Input {
                        name: n.key.clone(),
                        description: n.text("description"),
                        required: n.text("required") == "true",
                        kind: if kind.is_empty() { "string".into() } else { kind },
                        default: n.text("default"),
                        options: n.list("options"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Workflow { jobs, inputs }
}

impl Workflow {
    /// The column of each job: 0 when it needs nothing, else one past the
    /// deepest job it needs. A cycle or an unknown name counts as 0.
    pub fn layers(&self) -> Vec<usize> {
        fn depth(w: &Workflow, i: usize, seen: &mut Vec<usize>) -> usize {
            if seen.contains(&i) {
                return 0;
            }
            seen.push(i);
            let d = w.jobs[i].needs.iter().filter_map(|n| w.jobs.iter().position(|j| &j.key == n)).map(|k| depth(w, k, seen) + 1).max().unwrap_or(0);
            seen.pop();
            d
        }
        (0..self.jobs.len()).map(|i| depth(self, i, &mut Vec::new())).collect()
    }

    /// The workflow job an API job comes from.
    pub fn source_of(&self, job: &Job) -> Option<usize> {
        // The longest name wins, so `test` does not claim `test-long`.
        self.jobs.iter().enumerate().filter(|(_, w)| w.is(&job.name)).max_by_key(|(_, w)| w.name.len()).map(|(i, _)| i)
    }
}

/// Where the file of a run is read from, at the run's own commit.
pub fn contents_path(run: &Run) -> Option<String> {
    // `path` may be `.github/workflows/ci.yml@refs/heads/main` for a called workflow.
    let file = run.path.split('@').next().filter(|p| !p.is_empty())?;
    Some(format!("/repos/{}/contents/{file}?ref={}", run.repo, run.head_sha))
}

fn b64(s: &str) -> Vec<u8> {
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0u32);
    for ch in s.bytes() {
        let v = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// The text of a file from the contents API.
pub fn parse_contents(body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        return Err(m.to_string());
    }
    let c = v.get("content").and_then(Value::as_str).ok_or("no content in the answer")?;
    Ok(String::from_utf8_lossy(&b64(c)).into_owned())
}

/// The path of a job's log.
pub fn log_path(run: &Run, job: &Job) -> String {
    format!("/repos/{}/actions/jobs/{}/logs", run.repo, job.id)
}

/// The part of a log worth reading: from a few lines before the first
/// `##[error]` to its end, else the last lines. Timestamps and terminal
/// colour codes are dropped.
pub fn log_tail(log: &str, max: usize) -> Vec<String> {
    let lines: Vec<String> = log
        .lines()
        .map(|l| {
            let l = l.trim_start_matches('\u{feff}');
            match l.split_once(' ') {
                Some((t, rest)) if t.len() >= 20 && t.ends_with('Z') && t.as_bytes()[4] == b'-' => plain(rest),
                _ => plain(l),
            }
        })
        .collect();
    let from = lines.iter().position(|l| l.starts_with("##[error]")).map_or(lines.len().saturating_sub(max), |e| e.saturating_sub(12));
    lines[from..].iter().take(max).cloned().collect()
}

/// A log line without its terminal codes (`ESC [ 1 m` and the like).
fn plain(l: &str) -> String {
    let mut out = String::with_capacity(l.len());
    let mut it = l.chars();
    while let Some(ch) = it.next() {
        if ch != '\u{1b}' {
            out.push(ch);
        } else if it.next() == Some('[') {
            // The parameters run to a final byte in `@`..`~`.
            for c in it.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r#"
name: CI
on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      level:
        description: "How loud" # a comment
        required: true
        type: choice
        default: info
        options:
          - info
          - debug
      dry:
        type: boolean
        default: false
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - run: |
          echo "a: b"
  test:
    name: Test
    needs: lint
    strategy:
      matrix:
        os: [a, b]
  smoke:
    needs: [lint, test]
  deploy:
    needs:
      - smoke
"#;

    #[test]
    fn needs_and_names_are_read() {
        let w = parse(YAML);
        let keys: Vec<_> = w.jobs.iter().map(|j| (j.key.as_str(), j.name.as_str(), j.needs.clone())).collect();
        assert_eq!(keys[1], ("test", "Test", vec!["lint".to_string()]));
        assert_eq!(keys[2].2, ["lint", "test"]);
        assert_eq!(keys[3].2, ["smoke"]);
        assert_eq!(w.layers(), [0, 1, 2, 3]);
    }

    #[test]
    fn dispatch_inputs_are_read() {
        let w = parse(YAML);
        assert_eq!(w.inputs.len(), 2);
        let l = &w.inputs[0];
        assert_eq!((l.name.as_str(), l.description.as_str(), l.required, l.kind.as_str(), l.default.as_str()), ("level", "How loud", true, "choice", "info"));
        assert_eq!(l.options, ["info", "debug"]);
        assert_eq!((w.inputs[1].kind.as_str(), w.inputs[1].default.as_str()), ("boolean", "false"));
        assert!(parse("on: push\njobs:\n  a:\n    runs-on: x\n").inputs.is_empty());
    }

    #[test]
    fn a_matrix_job_comes_from_its_workflow_job() {
        let w = parse(YAML);
        let job = |n: &str| Job { id: 1, name: n.into(), status: crate::actions::Status::Ok, started_at: String::new(), completed_at: String::new(), url: String::new() };
        assert_eq!(w.source_of(&job("Test (ubuntu)")), Some(1));
        assert_eq!(w.source_of(&job("lint")), Some(0));
        assert_eq!(w.source_of(&job("other")), None);
    }

    #[test]
    fn contents_are_decoded() {
        assert_eq!(parse_contents(r#"{"content":"aGVsbG8s\nIHdvcmxk\n"}"#).unwrap(), "hello, world");
        assert_eq!(parse_contents(r#"{"message":"Not Found"}"#), Err("Not Found".into()));
    }

    #[test]
    fn the_log_tail_starts_before_the_first_error() {
        let mut log = String::new();
        for i in 0..40 {
            log.push_str(&format!("2026-10-12T01:00:{i:02}.1234567Z line {i}\n"));
        }
        log.push_str("2026-10-12T01:01:00.0000000Z ##[error]Process completed with exit code 1.\n");
        let t = log_tail(&log, 100);
        assert_eq!(t.first().map(String::as_str), Some("line 28"));
        assert!(t.last().unwrap().starts_with("##[error]"));
        assert_eq!(log_tail("a\nb\nc\n", 2), ["b", "c"]);
    }

    #[test]
    fn the_log_loses_its_colour_codes() {
        let log = "2026-10-12T01:00:00.1234567Z \u{1b}[1m\u{1b}[91merror\u{1b}[0m: test failed\n";
        assert_eq!(log_tail(log, 10), ["error: test failed"]);
    }
}
