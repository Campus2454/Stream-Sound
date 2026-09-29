//! Self-update from GitHub Releases.
//!
//! Releases are tagged `vX.Y` (official) or `vX.Y.Z` (beta, the Z-th change
//! on main after vX.Y); see .github/version.sh. CI bakes the version into the
//! app (`SSND_VERSION`). The app picks the newest release it may take (betas
//! only if the user wants them), downloads its file next to itself, and on
//! request swaps it in and restarts.

use anyhow::{anyhow, bail, Context};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Where releases are published. It must be public: GitHub answers 404 to
/// anonymous requests for a private repo. Copies from before the rename from
/// Audio-Streaming still reach it through GitHub's rename redirect.
pub const REPO: &str = "Campus2454/Stream-Sound";

#[cfg(windows)]
const ASSET: &str = "StreamSound-windows-x64.exe";
#[cfg(not(windows))]
const ASSET: &str = "StreamSound-linux-x86_64";

/// `X.Y` (official) or `X.Y.Z` (beta). An official release sorts before the
/// betas built after it: 0.2 < 0.2.1 < 0.2.2 < 0.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub beta: Option<u32>,
}

impl Version {
    /// Parses "v0.2", "0.2" or "v0.2.3".
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let mut parts = s.strip_prefix('v').unwrap_or(s).split('.');
        let mut num = || -> Option<u32> {
            let p = parts.next()?;
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let major = num()?;
        let minor = num()?;
        let beta = num();
        if beta.is_none() && s.matches('.').count() != 1 {
            return None;
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Version { major, minor, beta })
    }

    pub fn is_beta(&self) -> bool {
        self.beta.is_some()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.beta {
            Some(z) => write!(f, "{}.{}.{}", self.major, self.minor, z),
            None => write!(f, "{}.{}", self.major, self.minor),
        }
    }
}

/// This copy's version, baked in by CI; `None` for local builds, which take
/// any release.
pub fn current() -> Option<Version> {
    option_env!("SSND_VERSION").and_then(Version::parse)
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    /// No release this copy may take has been published yet.
    NoRelease,
    Downloading { release: Release, percent: u32 },
    Ready { release: Release, file: PathBuf },
    Failed(String),
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent("StreamSound-updater")
        .build()
}

/// Betas are GitHub pre-releases, which /releases/latest leaves out, so with
/// betas on the newest few releases are listed instead.
fn releases_url(beta: bool) -> String {
    // Overridable so the updater can be tested against a local server.
    if let Ok(u) = std::env::var("SSND_UPDATE_URL") {
        return u;
    }
    if beta {
        format!("https://api.github.com/repos/{REPO}/releases?per_page=20")
    } else {
        format!("https://api.github.com/repos/{REPO}/releases/latest")
    }
}

pub enum Check {
    UpToDate,
    NoRelease,
    Newer(Release),
}

/// The newest release in `releases` (one release object or a list) that has
/// this platform's file; betas only if `beta`.
fn pick(releases: &serde_json::Value, beta: bool) -> Option<Release> {
    let list = match releases {
        serde_json::Value::Array(a) => a.iter().collect::<Vec<_>>(),
        one => vec![one],
    };
    list.into_iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let version = Version::parse(tag)?;
            if version.is_beta() && !beta {
                return None;
            }
            let url = r["assets"]
                .as_array()?
                .iter()
                .find(|a| a["name"].as_str() == Some(ASSET))?["browser_download_url"]
                .as_str()?;
            Some(Release { version, tag: tag.to_string(), url: url.to_string() })
        })
        .max_by_key(|r| r.version)
}

/// Compare the newest release (official only unless `beta`) with this copy.
pub fn check(beta: bool) -> anyhow::Result<Check> {
    let resp = match agent().get(&releases_url(beta)).set("Accept", "application/vnd.github+json").call() {
        Ok(r) => r,
        // No release yet (or the repo is private or missing).
        Err(ureq::Error::Status(404, _)) => return Ok(Check::NoRelease),
        Err(e) => return Err(e.into()),
    };
    let v: serde_json::Value = serde_json::from_reader(resp.into_reader())?;
    let Some(release) = pick(&v, beta) else {
        return Ok(Check::NoRelease);
    };
    if current().is_some_and(|c| release.version <= c) {
        return Ok(Check::UpToDate);
    }
    Ok(Check::Newer(release))
}

fn exe_path() -> anyhow::Result<PathBuf> {
    let p = std::env::current_exe()?;
    Ok(fs::canonicalize(&p).unwrap_or(p))
}

/// Download the release next to the running program. `progress` gets 0..=100.
pub fn download(release: &Release, progress: impl Fn(u32)) -> anyhow::Result<PathBuf> {
    let exe = exe_path()?;
    let dir = exe.parent().ok_or_else(|| anyhow!("no folder for {}", exe.display()))?;
    let tmp = dir.join(format!("{}.update", exe.file_name().and_then(|n| n.to_str()).unwrap_or("stream-sound")));
    let resp = agent().get(&release.url).call()?;
    let total: u64 = resp.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut file = fs::File::create(&tmp).with_context(|| format!("cannot write to {}", dir.display()))?;
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if total > 0 {
            progress((done * 100 / total) as u32);
        }
    }
    file.flush()?;
    drop(file);
    if (total > 0 && done != total) || done < 512 * 1024 {
        let _ = fs::remove_file(&tmp);
        bail!("download incomplete ({done} of {total} bytes)");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    progress(100);
    Ok(tmp)
}

/// Swap the downloaded file in and start it. Only returns on failure.
pub fn install_and_restart(new_file: &Path) -> anyhow::Result<()> {
    let exe = install(new_file)?;
    std::process::Command::new(&exe).spawn()?;
    std::process::exit(0);
}

/// Replace the running program's file with `new_file`; returns its path.
pub fn install(new_file: &Path) -> anyhow::Result<PathBuf> {
    let exe = exe_path()?;
    #[cfg(windows)]
    {
        // A running exe can be renamed but not overwritten.
        let old = old_path(&exe);
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old)?;
        if let Err(e) = fs::rename(new_file, &exe) {
            let _ = fs::rename(&old, &exe);
            return Err(e.into());
        }
    }
    #[cfg(not(windows))]
    fs::rename(new_file, &exe)?;
    Ok(exe)
}

#[cfg(windows)]
fn old_path(exe: &Path) -> PathBuf {
    exe.with_extension("old.exe")
}

/// Remove what a previous update left behind.
pub fn cleanup() {
    if let Ok(exe) = exe_path() {
        #[cfg(windows)]
        let _ = fs::remove_file(old_path(&exe));
        if let (Some(dir), Some(name)) = (exe.parent(), exe.file_name().and_then(|n| n.to_str())) {
            let _ = fs::remove_file(dir.join(format!("{name}.update")));
        }
    }
}

/// Check and, if there is a newer version, download it in the background.
pub fn check_and_download_in_background(status: Arc<parking_lot::Mutex<Status>>, beta: bool) {
    {
        let mut s = status.lock();
        if matches!(*s, Status::Checking | Status::Downloading { .. } | Status::Ready { .. }) {
            return;
        }
        *s = Status::Checking;
    }
    let _ = std::thread::Builder::new().name("ssnd-update".into()).spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<Status> {
            let release = match check(beta)? {
                Check::UpToDate => return Ok(Status::UpToDate),
                Check::NoRelease => return Ok(Status::NoRelease),
                Check::Newer(r) => r,
            };
            *status.lock() = Status::Downloading { release: release.clone(), percent: 0 };
            let st = status.clone();
            let rel = release.clone();
            let file = download(&release, move |p| {
                *st.lock() = Status::Downloading { release: rel.clone(), percent: p };
            })?;
            Ok(Status::Ready { release, file })
        }));
        *status.lock() = match result {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => Status::Failed(format!("{e:#}")),
            Err(_) => Status::Failed("update check crashed".into()),
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn parses_versions() {
        assert_eq!(v("v0.2"), Version { major: 0, minor: 2, beta: None });
        assert_eq!(v("0.2.13"), Version { major: 0, minor: 2, beta: Some(13) });
        assert_eq!(v("v1.10").to_string(), "1.10");
        assert_eq!(v("v0.1.14").to_string(), "0.1.14");
        for bad in ["", "v", "v1", "v1.", "v1.2.", "v1..2", "v1.2.3.4", "v1.x", "build 8", "v+1.2", "dev"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn official_comes_before_its_betas() {
        let order = ["0.1.8", "0.1.14", "0.2", "0.2.1", "0.2.10", "0.3", "0.10", "1.0", "1.0.1"];
        for w in order.windows(2) {
            assert!(v(w[0]) < v(w[1]), "{} < {}", w[0], w[1]);
        }
    }

    fn release(tag: &str, prerelease: bool, with_file: bool) -> serde_json::Value {
        let assets = if with_file {
            json!([{"name": "other"}, {"name": ASSET, "browser_download_url": format!("https://x/{tag}")}])
        } else {
            json!([])
        };
        json!({"tag_name": tag, "draft": false, "prerelease": prerelease, "assets": assets})
    }

    #[test]
    fn picks_newest_allowed_release() {
        let list = json!([
            release("v0.2.2", true, false), // still uploading its files
            release("v0.2.1", true, true),
            release("v0.2", false, true),
            release("v0.1.14", false, true),
            release("v0.1.x", false, true),
        ]);
        assert_eq!(pick(&list, true).unwrap().tag, "v0.2.1");
        assert_eq!(pick(&list, false).unwrap().tag, "v0.2");
        assert_eq!(pick(&list, true).unwrap().url, "https://x/v0.2.1");
        // /releases/latest answers with one release, not a list.
        assert_eq!(pick(&release("v0.3", false, true), false).unwrap().tag, "v0.3");
        assert!(pick(&release("v0.1.17", false, true), false).is_none());
        assert!(pick(&json!([]), true).is_none());
    }
}
