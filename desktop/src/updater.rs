//! Self-update from GitHub Releases.
//!
//! Releases are tagged `vX.Y` (official) or `vX.Y.Z` (beta, the Z-th change
//! on main after vX.Y); see .github/version.sh. CI bakes the version into the
//! app (`SSND_VERSION`). The app picks the newest release it may take (betas
//! only if the user wants them) and downloads its installer to a temporary
//! folder. On request it starts that installer in update mode, which shows
//! its progress, replaces this copy once it has closed and starts the new
//! one (see setup).

use anyhow::{bail, Context};
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

/// What an update downloads: the Windows installer, or the Linux file,
/// which is its own installer. (Copies from before the installers took
/// StreamSound-windows-x64.exe, which releases still carry.)
#[cfg(windows)]
const ASSET: &str = "StreamSound-windows-x64-setup.exe";
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

/// The repo's web address. Updates are found through github.com pages, not
/// the REST API, because the API allows only 60 anonymous requests an hour
/// per home connection, which a few devices checking every 5 minutes would
/// use up. Overridable so the updater can be tested against a local server.
fn base_url() -> String {
    std::env::var("SSND_UPDATE_URL").unwrap_or_else(|_| format!("https://github.com/{REPO}"))
}

pub enum Check {
    UpToDate,
    NoRelease,
    Newer(Release),
}

/// Release tags linked from `text` ("…/releases/tag/v0.2.1…").
fn tags_in(text: &str) -> Vec<&str> {
    const MARK: &str = "/releases/tag/";
    let mut tags = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(MARK) {
        rest = &rest[i + MARK.len()..];
        let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.')).unwrap_or(rest.len());
        tags.push(&rest[..end]);
    }
    tags
}

/// The newest version among `tags`; betas only if `beta`.
fn newest(tags: &[&str], beta: bool) -> Option<Version> {
    tags.iter().filter_map(|t| Version::parse(t)).filter(|v| beta || !v.is_beta()).max()
}

/// Tags of the newest releases. With betas on, from the releases feed (the
/// latest 10, pre-releases included). Otherwise from where /releases/latest
/// redirects to, which is the newest official release.
fn release_tags(beta: bool) -> anyhow::Result<Option<String>> {
    let base = base_url();
    if beta {
        return match agent().get(&format!("{base}/releases.atom")).call() {
            Ok(r) => Ok(Some(r.into_string()?)),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(e.into()),
        };
    }
    let no_redirects = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent("StreamSound-updater")
        .redirects(0)
        .build();
    match no_redirects.get(&format!("{base}/releases/latest")).call() {
        Ok(r) => Ok(r.header("Location").map(str::to_string)),
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Compare the newest release (official only unless `beta`) with this copy.
pub fn check(beta: bool) -> anyhow::Result<Check> {
    let text = release_tags(beta)?.unwrap_or_default();
    let Some(version) = newest(&tags_in(&text), beta) else {
        return Ok(Check::NoRelease);
    };
    if current().is_some_and(|c| version <= c) {
        return Ok(Check::UpToDate);
    }
    let tag = format!("v{version}");
    let url = format!("{}/releases/download/{tag}/{ASSET}", base_url());
    Ok(Check::Newer(Release { version, tag, url }))
}

/// A release whose files are still being uploaded; it is picked up on the
/// next check.
#[derive(Debug)]
pub struct NotUploadedYet;

impl fmt::Display for NotUploadedYet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("the new version's files are still being uploaded")
    }
}

impl std::error::Error for NotUploadedYet {}

/// Where downloads wait to be installed: the system's temporary folder on
/// Windows, ~/.cache on Linux.
fn download_dir() -> PathBuf {
    #[cfg(not(windows))]
    if let Some(c) = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    {
        return c.join("stream-sound");
    }
    std::env::temp_dir().join("StreamSound")
}

/// Download the release's installer. `progress` gets 0..=100.
pub fn download(release: &Release, progress: impl Fn(u32)) -> anyhow::Result<PathBuf> {
    let dir = download_dir();
    fs::create_dir_all(&dir).with_context(|| format!("cannot write to {}", dir.display()))?;
    let name = ASSET.replacen("StreamSound-", &format!("StreamSound-{}-", release.tag), 1);
    let file = dir.join(&name);
    let tmp = dir.join(format!("{name}.part"));
    let resp = match agent().get(&release.url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Err(NotUploadedYet.into()),
        Err(e) => return Err(e.into()),
    };
    let total: u64 = resp.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut out = fs::File::create(&tmp).with_context(|| format!("cannot write to {}", dir.display()))?;
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if total > 0 {
            progress((done * 100 / total) as u32);
        }
    }
    out.flush()?;
    drop(out);
    if (total > 0 && done != total) || done < 512 * 1024 {
        let _ = fs::remove_file(&tmp);
        bail!("download incomplete ({done} of {total} bytes)");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, &file)?;
    progress(100);
    Ok(file)
}

/// Start the downloaded installer in update mode. On success this copy
/// should close right away: the installer waits for it before replacing
/// its files, then starts the new version.
pub fn start_install(installer: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        // An installed copy is updated in place with only a progress
        // window. A copy run from a downloaded file gets the full
        // installer, which removes that file once the app is installed.
        let exe = std::env::current_exe()?;
        let params = match exe.parent() {
            // `/D=` goes last and without quotes.
            Some(dir) if crate::setup::installed() => format!("/UPDATE /D={}", dir.display()),
            _ => format!("/FROM=\"{}\"", exe.display()),
        };
        crate::os::shell_open(installer, &params).map_err(anyhow::Error::from)
    }
    #[cfg(target_os = "linux")]
    {
        let me = crate::setup::linux::this_exe().ok_or_else(|| anyhow::anyhow!("can't find this program's file"))?;
        let me = me.to_string_lossy().into_owned();
        crate::setup::linux::launch(installer, &[crate::setup::UPDATE_FLAG, &me])
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = installer;
        bail!("updates aren't supported here")
    }
}

/// The user said no to the administrator prompt.
pub fn cancelled(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some_and(|e| e.raw_os_error() == Some(1223))
}

/// Remove what earlier updates left behind: downloads, and the files older
/// versions swapped next to the program.
pub fn cleanup() {
    if let Ok(entries) = fs::read_dir(download_dir()) {
        for e in entries.flatten() {
            // An installer that is still finishing can't be removed yet;
            // it goes next time.
            let _ = fs::remove_file(e.path());
        }
    }
    if let Some(exe) = std::env::current_exe().ok().map(|p| fs::canonicalize(&p).unwrap_or(p)) {
        #[cfg(windows)]
        let _ = fs::remove_file(exe.with_extension("old.exe"));
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
            Ok(Err(e)) if e.is::<NotUploadedYet>() => Status::Idle,
            Ok(Err(e)) => Status::Failed(format!("{e:#}")),
            Err(_) => Status::Failed("update check crashed".into()),
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn finds_newest_allowed_version() {
        // Trimmed from github.com/<repo>/releases.atom.
        let feed = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <link type="text/html" rel="alternate" href="https://github.com/o/r/releases"/>
  <entry>
    <id>tag:github.com,2008:Repository/1/v0.2.1</id>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v0.2.1"/>
    <title>Stream Sound v0.2.1 (Beta)</title>
    <content type="html">&lt;a href=&quot;https://github.com/o/r/compare/v0.2...v0.2.1&quot;&gt;</content>
  </entry>
  <entry>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v0.2"/>
  </entry>
  <entry>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v0.1.21"/>
  </entry>
  <entry>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/nightly"/>
  </entry>
</feed>"#;
        let tags = tags_in(feed);
        assert_eq!(tags, ["v0.2.1", "v0.2", "v0.1.21", "nightly"]);
        assert_eq!(newest(&tags, true), Some(v("0.2.1")));
        assert_eq!(newest(&tags, false), Some(v("0.2")));
        // Where /releases/latest redirects to.
        let tags = tags_in("https://github.com/o/r/releases/tag/v0.1.21");
        assert_eq!(newest(&tags, true), Some(v("0.1.21")));
        assert_eq!(newest(&tags, false), None);
        assert_eq!(newest(&tags_in(""), true), None);
    }
}
