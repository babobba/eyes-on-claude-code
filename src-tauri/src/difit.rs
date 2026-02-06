use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

/// Default base branch for branch diff comparison
const DEFAULT_BASE_BRANCH: &str = "main";

/// Default port for difit server
const DEFAULT_DIFIT_PORT: u16 = 4966;

/// Diff types supported by the application
#[derive(Debug, Clone, Copy)]
pub enum DiffType {
    /// Unstaged changes (working directory vs index)
    Unstaged,
    /// Staged changes (index vs HEAD)
    Staged,
    /// Latest commit diff (HEAD vs HEAD~1)
    LatestCommit,
    /// Branch diff (current branch vs main/master)
    Branch,
}

impl DiffType {
    /// Get the git diff arguments for this diff type
    fn git_diff_args(self, branch: Option<&str>) -> Result<Vec<String>, String> {
        match self {
            DiffType::Unstaged => Ok(vec!["diff".to_string()]),
            DiffType::Staged => Ok(vec!["diff".to_string(), "--cached".to_string()]),
            DiffType::LatestCommit => Ok(vec![
                "diff".to_string(),
                "HEAD~1".to_string(),
                "HEAD".to_string(),
            ]),
            DiffType::Branch => {
                let base = branch.unwrap_or(DEFAULT_BASE_BRANCH);
                // Validate branch name to prevent git option injection
                if base.starts_with('-') {
                    return Err(format!("Invalid branch name: {}", base));
                }
                Ok(vec!["diff".to_string(), base.to_string()])
            }
        }
    }
}

/// Result of starting a difit server
pub struct DifitServerInfo {
    pub url: String,
    pub process: Child,
}

struct RegistryInner {
    processes: HashMap<String, Child>,
    diff_hashes: HashMap<String, u64>,
    next_port: u16,
}

/// Result of comparing and updating a diff hash
#[derive(Debug, PartialEq)]
pub enum HashCompareResult {
    /// Hash is the same as before, no update needed
    Unchanged,
    /// Hash has changed, process was killed and hash updated
    Changed,
    /// No previous hash existed, new hash was set
    NewEntry,
}

/// Registry to track running difit processes by window label
pub struct DifitProcessRegistry {
    inner: Mutex<RegistryInner>,
}

impl DifitProcessRegistry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RegistryInner {
                processes: HashMap::new(),
                diff_hashes: HashMap::new(),
                next_port: DEFAULT_DIFIT_PORT,
            }),
        }
    }

    /// Get the next available port
    pub fn get_next_port(&self) -> u16 {
        match self.inner.lock() {
            Ok(mut inner) => {
                let current = inner.next_port;
                inner.next_port = inner.next_port.wrapping_add(1);
                if inner.next_port < DEFAULT_DIFIT_PORT {
                    inner.next_port = DEFAULT_DIFIT_PORT;
                }
                current
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for port: {}", e);
                DEFAULT_DIFIT_PORT
            }
        }
    }

    /// Register a difit process with a window label
    pub fn register(&self, window_label: String, process: Child) {
        match self.inner.lock() {
            Ok(mut inner) => {
                inner.processes.insert(window_label, process);
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for register: {}", e);
            }
        }
    }

    /// Store the diff hash for a window
    pub fn set_diff_hash(&self, window_label: &str, hash: u64) {
        match self.inner.lock() {
            Ok(mut inner) => {
                inner.diff_hashes.insert(window_label.to_string(), hash);
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for set_diff_hash: {}", e);
            }
        }
    }

    /// Atomically compare hash and update if changed, killing the process if needed.
    /// Returns the comparison result.
    pub fn compare_and_update_hash(&self, window_label: &str, new_hash: u64) -> HashCompareResult {
        match self.inner.lock() {
            Ok(mut inner) => {
                let previous_hash = inner.diff_hashes.get(window_label).copied();
                match previous_hash {
                    Some(old_hash) if old_hash == new_hash => HashCompareResult::Unchanged,
                    Some(_) => {
                        // Hash changed, kill process and update hash
                        if let Some(mut process) = inner.processes.remove(window_label) {
                            let _ = process.kill();
                            let _ = process.wait();
                        }
                        inner.diff_hashes.insert(window_label.to_string(), new_hash);
                        HashCompareResult::Changed
                    }
                    None => {
                        inner.diff_hashes.insert(window_label.to_string(), new_hash);
                        HashCompareResult::NewEntry
                    }
                }
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for compare_and_update_hash: {}", e);
                // Treat as changed to trigger reload (safer default)
                HashCompareResult::Changed
            }
        }
    }

    /// Kill and remove a difit process and its hash by window label
    pub fn kill(&self, window_label: &str) {
        match self.inner.lock() {
            Ok(mut inner) => {
                if let Some(mut process) = inner.processes.remove(window_label) {
                    let _ = process.kill();
                    let _ = process.wait();
                }
                inner.diff_hashes.remove(window_label);
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for kill: {}", e);
            }
        }
    }

    /// Kill all registered difit processes
    pub fn kill_all(&self) {
        match self.inner.lock() {
            Ok(mut inner) => {
                for (_, mut process) in inner.processes.drain() {
                    let _ = process.kill();
                    let _ = process.wait();
                }
                inner.diff_hashes.clear();
            }
            Err(e) => {
                log::warn!(target: "eocc.difit", "Failed to lock registry for kill_all: {}", e);
            }
        }
    }
}

impl Default for DifitProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get diff content for untracked files
fn get_untracked_diff(repo_path: &str) -> Vec<u8> {
    // Get list of untracked files
    let untracked_output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(repo_path)
        .output();

    let untracked_files = match untracked_output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        _ => return Vec::new(),
    };

    if untracked_files.is_empty() {
        return Vec::new();
    }

    // Generate diff for each untracked file
    let mut combined_diff = Vec::new();
    for file in untracked_files {
        let diff_output = Command::new("git")
            .args(["diff", "--no-index", "--", "/dev/null", &file])
            .current_dir(repo_path)
            .output();

        if let Ok(output) = diff_output {
            // git diff --no-index returns exit code 1 when there are differences
            if !output.stdout.is_empty() {
                combined_diff.extend_from_slice(&output.stdout);
            }
        }
    }

    combined_diff
}

/// Get diff content for the specified repository and diff type
pub fn get_diff_content(
    repo_path: &str,
    diff_type: DiffType,
    base_branch: Option<&str>,
) -> Result<Vec<u8>, String> {
    let git_args = diff_type.git_diff_args(base_branch)?;

    log::info!(target: "eocc.difit", "get_diff_content: repo={}, base_branch={:?}, git_args={:?}", repo_path, base_branch, git_args);

    // Run git diff and capture output
    let git_output = Command::new("git")
        .args(&git_args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    if !git_output.status.success() {
        let stderr = String::from_utf8_lossy(&git_output.stderr);
        return Err(format!("git diff failed: {}", stderr));
    }

    let mut diff_content = git_output.stdout;

    // For unstaged diff, also include untracked files
    if matches!(diff_type, DiffType::Unstaged) {
        let untracked_diff = get_untracked_diff(repo_path);
        diff_content.extend(untracked_diff);
    }

    if diff_content.is_empty() {
        return Err("No diff content to display".to_string());
    }

    Ok(diff_content)
}

/// Calculate hash of diff content
pub fn calculate_diff_hash(content: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Start a difit server with pre-fetched diff content
///
/// `npx_path`: Optional path to npx binary. If None or empty, falls back to "npx".
pub fn start_difit_server_with_content(
    diff_content: Vec<u8>,
    repo_path: &str,
    port: u16,
    npx_path: Option<&str>,
) -> Result<DifitServerInfo, String> {
    // Determine npx command to use
    let npx_cmd = npx_path.filter(|p| !p.is_empty()).unwrap_or("npx");

    log::info!(target: "eocc.difit", "Starting difit with npx_cmd={}, port={}", npx_cmd, port);

    // Build command with PATH set to include node binary directory
    let mut cmd = Command::new(npx_cmd);
    cmd.args(["difit", "--no-open", "--port", &port.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(repo_path);

    // If npx_path is provided, add its directory to PATH so `env node` can find node
    if let Some(path) = npx_path.filter(|p| !p.is_empty()) {
        if let Some(bin_dir) = std::path::Path::new(path).parent() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{}", bin_dir.display(), current_path);
            cmd.env("PATH", new_path);
        }
    }

    // Start difit process
    let mut difit_process = cmd
        .spawn()
        .map_err(|e| format!("Failed to start difit (npx_path={}): {}", npx_cmd, e))?;

    // Write git diff to stdin
    {
        let mut stdin = difit_process
            .stdin
            .take()
            .ok_or("Failed to capture difit stdin")?;
        stdin
            .write_all(&diff_content)
            .map_err(|e| format!("Failed to write to difit stdin: {}", e))?;
    } // stdin is dropped here, closing the pipe

    // Read stderr to find the server URL with timeout
    let stderr = difit_process
        .stderr
        .take()
        .ok_or("Failed to capture difit stderr")?;

    // Use a channel to receive the port from a background thread
    let (tx, rx) = mpsc::channel();
    let expected_port = port;
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().take(10).flatten() {
            // Look for "difit server started on http://localhost:XXXX"
            if line.contains("difit server started on") {
                if let Some(url_start) = line.find("http://") {
                    let url = &line[url_start..];
                    // Extract port from URL
                    if let Some(port_str) = url.strip_prefix("http://localhost:") {
                        if let Ok(p) = port_str.trim().parse::<u16>() {
                            let _ = tx.send(p);
                            return;
                        }
                    }
                }
            }
        }
        // Send expected port if we couldn't find the actual one
        let _ = tx.send(expected_port);
    });

    // Wait for up to 5 seconds for the server to start
    let actual_port = rx.recv_timeout(Duration::from_secs(5)).unwrap_or(port);
    // Add cache buster to prevent WebView from caching old responses
    let cache_buster = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let url = format!("http://localhost:{}?_cb={}", actual_port, cache_buster);

    log::info!(target: "eocc.difit", "Difit server started at {}", url);

    Ok(DifitServerInfo {
        url,
        process: difit_process,
    })
}
