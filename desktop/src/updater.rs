//! Self-update from GitHub Releases.
//!
//! CI publishes a release tagged `v0.1.<build>` for every change on `main`,
//! with the raw Windows exe and Linux binary attached. The app compares that
//! build number with its own, downloads the new file next to itself, and on
//! request swaps it in and restarts.

use anyhow::{anyhow, bail, Context};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Where releases are looked for (newest build wins). The repo must be public:
/// GitHub answers 404 to anonymous requests for a private one. Copies from
/// before the repo was renamed from Audio-Streaming still reach it through
/// GitHub's rename redirect.
pub const REPOS: [&str; 1] = ["Campus2454/Stream-Sound"];

#[cfg(windows)]
const ASSET: &str = "StreamSound-windows-x64.exe";
#[cfg(not(windows))]
const ASSET: &str = "StreamSound-linux-x86_64";

/// Build number baked in by CI (`SSND_BUILD`); 0 for local builds.
pub fn current_build() -> u32 {
    option_env!("SSND_BUILD").and_then(|s| s.parse().ok()).unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct Release {
    pub build: u32,
    pub tag: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    /// Nothing has been published on GitHub Releases yet.
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

fn latest_urls() -> Vec<String> {
    // Overridable so the updater can be tested against a local server.
    match std::env::var("SSND_UPDATE_URL") {
        Ok(u) => vec![u],
        Err(_) => REPOS.iter().map(|r| format!("https://api.github.com/repos/{r}/releases/latest")).collect(),
    }
}

fn build_from_tag(tag: &str) -> Option<u32> {
    tag.rsplit('.').next()?.parse().ok()
}

pub enum Check {
    UpToDate,
    NoRelease,
    Newer(Release),
}

/// The latest release at one address; `None` if there is none (or the repo
/// is private or missing, which GitHub also answers with 404).
fn latest(url: &str) -> anyhow::Result<Option<(u32, String, serde_json::Value)>> {
    let resp = match agent().get(url).set("Accept", "application/vnd.github+json").call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let v: serde_json::Value = serde_json::from_reader(resp.into_reader())?;
    let tag = v["tag_name"].as_str().ok_or_else(|| anyhow!("release has no tag"))?.to_string();
    let build = build_from_tag(&tag).ok_or_else(|| anyhow!("unexpected tag {tag}"))?;
    Ok(Some((build, tag, v)))
}

/// Compare the newest release with this build.
pub fn check() -> anyhow::Result<Check> {
    let mut best: Option<(u32, String, serde_json::Value)> = None;
    let mut err = None;
    for url in latest_urls() {
        match latest(&url) {
            Ok(Some(r)) if best.as_ref().map_or(true, |b| r.0 > b.0) => best = Some(r),
            Ok(_) => {}
            Err(e) => err = Some(e),
        }
    }
    let Some((build, tag, v)) = best else {
        return match err {
            Some(e) => Err(e),
            None => Ok(Check::NoRelease),
        };
    };
    if build <= current_build() {
        return Ok(Check::UpToDate);
    }
    let url = v["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str() == Some(ASSET))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| anyhow!("release {tag} has no {ASSET}"))?
        .to_string();
    Ok(Check::Newer(Release { build, tag, url }))
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

/// Check and, if there is a newer build, download it in the background.
pub fn check_and_download_in_background(status: Arc<parking_lot::Mutex<Status>>) {
    {
        let mut s = status.lock();
        if matches!(*s, Status::Checking | Status::Downloading { .. } | Status::Ready { .. }) {
            return;
        }
        *s = Status::Checking;
    }
    let _ = std::thread::Builder::new().name("ssnd-update".into()).spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<Status> {
            let release = match check()? {
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

    #[test]
    fn parses_build_from_tag() {
        assert_eq!(build_from_tag("v0.1.42"), Some(42));
        assert_eq!(build_from_tag("v0.1.x"), None);
    }
}
