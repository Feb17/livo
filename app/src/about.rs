//! 检查更新：读仓库里的 app_version.json（发布流程以它为准；不用 Releases API，它每 IP 每小时只给 60 次），
//! 和当前版本比较，预发行版只有开了测试版通道才收。
//! 自动更新：下载 Release 里的 dmg，用内置公钥验过 .sig 签名后解出来放在安装位置旁边，
//! 退出时由一个小脚本等本进程结束后替换（“立即重启”则替换完再打开）。签名私钥在仓库 secret LIVO_UPDATE_KEY。
use crate::{rt, Livo};
use anyhow::{bail, ensure, Context as _};
use gpui::Context;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "https://github.com/Feb17/livo";
const VERSION_URL: &str = "https://raw.githubusercontent.com/Feb17/livo/master/app_version.json";
/// 发布签名的 Ed25519 公钥（release.yml 用对应私钥给每个安装包生成 .sig）
const PUBLIC_KEY: [u8; 32] = [
    0x2b, 0x36, 0x75, 0x62, 0x41, 0xd6, 0x0b, 0xd2, 0xe2, 0xb3, 0x64, 0x18, 0x65, 0x53, 0xef, 0x02, 0x89, 0x6f, 0xbb, 0xe5, 0x7e, 0xb0, 0x78, 0xff, 0x47, 0x48, 0x54, 0x83, 0x97, 0x25, 0xa4, 0x45,
];

#[derive(Default)]
pub enum Update {
    #[default]
    Idle,
    Checking,
    Latest,
    Available { version: String, desc: String },
    Downloading { version: String, desc: String, got: u64, total: u64 },
    /// 新版本已解好放在安装位置旁边，退出时安装
    Ready { version: String, desc: String },
    Failed(String),
}

#[derive(Deserialize)]
struct Remote {
    version: String,
    #[serde(default)]
    version_desc: String,
    #[serde(default)]
    prerelease: bool,
}

/// "0.1.10" > "0.1.9"；解析不了的段按 0 算
fn parse(v: &str) -> Vec<u32> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn newer(remote: &str, local: &str) -> bool {
    parse(remote) > parse(local)
}

fn client(timeout: u64) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder().user_agent("Livo").connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(timeout)).build()
}

/// 该通道能收的最新版本 (版本号, 更新说明)
async fn fetch(beta: bool) -> anyhow::Result<Option<(String, String)>> {
    let r: Remote = serde_json::from_slice(&client(15)?.get(VERSION_URL).send().await?.error_for_status()?.bytes().await?)?;
    Ok((beta || !r.prerelease).then_some((r.version, r.version_desc)))
}

fn verify(key: &[u8], pkg: &[u8], sig: &[u8]) -> anyhow::Result<()> {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(pkg, sig).map_err(|_| anyhow::anyhow!("签名校验失败"))
}

/// 发布包的下载地址，文件名同 bundle-macos.sh；只发了 Apple silicon
fn asset_url(v: &str) -> Option<String> {
    cfg!(all(target_os = "macos", target_arch = "aarch64")).then(|| format!("{REPO}/releases/download/v{v}/Livo-macOS-arm64-v{v}.dmg"))
}

/// 自己的安装位置：.app 包；cargo 构建（target/ 下）返回 None，只能手动下载
fn install_target() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let dir = exe.parent()?;
    if dir.parent().is_some_and(|p| p.ends_with("target")) {
        return None;
    }
    let app = dir.parent()?.parent()?;
    (app.extension()? == "app").then(|| app.to_path_buf())
}

/// 新版本先放在安装位置旁边：同一个卷上改名替换是原子的，没有写权限也能在下载完就发现
fn staged_path(target: &Path) -> PathBuf {
    target.with_file_name(".Livo-update.app")
}

pub fn can_install(version: &str) -> bool {
    asset_url(version).is_some() && install_target().is_some()
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    let out = cmd.output().with_context(|| format!("{cmd:?}"))?;
    ensure!(out.status.success(), "{:?}: {}", cmd.get_program(), String::from_utf8_lossy(&out.stderr).trim());
    Ok(())
}

/// 把下载的安装包解到 staged
fn stage(pkg: &[u8], staged: &Path) -> anyhow::Result<()> {
    let tmp = std::env::temp_dir().join("livo-update");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let _ = std::fs::remove_dir_all(staged);
    let (dmg, mnt) = (tmp.join("Livo.dmg"), tmp.join("mnt"));
    std::fs::write(&dmg, pkg)?;
    run(Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(&mnt).arg(&dmg))?;
    let copied = run(Command::new("ditto").arg(mnt.join("Livo.app")).arg(staged));
    let _ = Command::new("hdiutil").args(["detach", "-force"]).arg(&mnt).output();
    copied?;
    ensure!(staged.join("Contents/MacOS/livo").is_file(), "安装包里没有 Livo.app");
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

async fn download(version: String, progress: Arc<[AtomicU64; 2]>) -> anyhow::Result<()> {
    let (Some(url), Some(target)) = (asset_url(&version), install_target()) else { bail!("当前版本不支持自动安装") };
    let resp = client(600)?.get(&url).send().await?;
    // app_version.json 先于 Release 推上去：构建完之前安装包还不存在
    ensure!(resp.status() != reqwest::StatusCode::NOT_FOUND, "新版本安装包还没发布，稍后再试");
    let mut resp = resp.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    progress[1].store(total, Ordering::Relaxed);
    let mut pkg = Vec::with_capacity(total as usize);
    while let Some(chunk) = resp.chunk().await? {
        pkg.extend_from_slice(&chunk);
        progress[0].store(pkg.len() as u64, Ordering::Relaxed);
    }
    ensure!(total == 0 || pkg.len() as u64 == total, "下载不完整");
    let sig = client(30)?.get(format!("{url}.sig")).send().await?.error_for_status().context("缺少签名文件")?.bytes().await?;
    verify(&PUBLIC_KEY, &pkg, &sig)?;
    tokio::task::spawn_blocking(move || stage(&pkg, &staged_path(&target))).await?
}

/// 参数：pid、新版 .app、安装位置、是否重新打开；换不过去就把旧版挪回来
const MAC_INSTALL_SH: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
rm -rf "$3.old"; mv "$3" "$3.old" && { mv "$2" "$3" || mv "$3.old" "$3"; }
rm -rf "$3.old"; xattr -cr "$3"
[ "$4" = 1 ] && open "$3""#;

/// 起一个脱离本进程的脚本：等本进程退出，用 staged 替换安装位置，relaunch 时再打开
fn spawn_installer(relaunch: bool) -> anyhow::Result<()> {
    let target = install_target().context("找不到安装位置")?;
    let staged = staged_path(&target);
    let pid = std::process::id().to_string();
    Command::new("/bin/sh").args(["-c", MAC_INSTALL_SH, "sh", &pid]).arg(&staged).arg(&target).arg(if relaunch { "1" } else { "0" }).spawn()?;
    Ok(())
}

impl Livo {
    /// auto：启动时的自动检查，发现新版本且开着自动更新就直接下载
    pub(crate) fn check_update(&mut self, auto: bool, cx: &mut Context<Self>) {
        if matches!(self.update, Update::Checking | Update::Downloading { .. } | Update::Ready { .. }) {
            return;
        }
        self.update = Update::Checking;
        cx.notify();
        let job = rt().spawn(fetch(self.store.update_beta));
        cx.spawn(async move |this, cx| {
            let r = job.await;
            this.update(cx, |this, cx| {
                this.update = match r {
                    Ok(Ok(Some((version, desc)))) if newer(&version, VERSION) => Update::Available { version, desc },
                    Ok(Ok(_)) => Update::Latest,
                    _ => Update::Failed("检查失败，请稍后再试".into()),
                };
                if auto && this.store.auto_update {
                    this.download_update(cx);
                }
                cx.notify()
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn download_update(&mut self, cx: &mut Context<Self>) {
        let Update::Available { version, desc } = &self.update else { return };
        if !can_install(version) {
            return;
        }
        let (version, desc) = (version.clone(), desc.clone());
        let progress = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);
        let job = rt().spawn(download(version.clone(), progress.clone()));
        self.update = Update::Downloading { version: version.clone(), desc: desc.clone(), got: 0, total: 0 };
        cx.notify();
        cx.spawn(async move |this, cx| {
            while !job.is_finished() {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                let alive = this.update(cx, |this, cx| {
                    if let Update::Downloading { got, total, .. } = &mut this.update {
                        (*got, *total) = (progress[0].load(Ordering::Relaxed), progress[1].load(Ordering::Relaxed));
                        cx.notify()
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
            let r = job.await;
            this.update(cx, |this, cx| {
                this.update = match r {
                    Ok(Ok(())) => Update::Ready { version, desc },
                    Ok(Err(e)) => Update::Failed(format!("更新失败：{e:#}")),
                    Err(e) => Update::Failed(format!("更新失败：{e}")),
                };
                cx.notify()
            })
            .ok();
        })
        .detach();
    }

    /// 退出时（relaunch=false）或“立即重启”时安装已下载好的新版本；只装一次
    pub(crate) fn install_update(&mut self, relaunch: bool, cx: &mut Context<Self>) {
        if !matches!(self.update, Update::Ready { .. }) {
            return;
        }
        match spawn_installer(relaunch) {
            Ok(()) => {
                self.update = Update::Idle;
                if relaunch {
                    cx.quit();
                }
            }
            Err(e) => self.update = Update::Failed(format!("安装失败：{e:#}")),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numerically() {
        assert!(newer("0.1.10", "0.1.9"));
        assert!(newer("v0.2.0", "0.1.4"));
        assert!(!newer("0.1.4", "0.1.4"));
        assert!(!newer("0.1.3", "0.1.4"));
    }

    /// openssl pkeyutl -sign -rawin 对 "livo" 的签名：确认内置公钥和 CI 的签名格式对得上
    #[test]
    fn verifies_release_signature() {
        const SIG: [u8; 64] = [
            0x89, 0x4b, 0x04, 0x18, 0xfd, 0xaf, 0x26, 0x2a, 0x6b, 0xd2, 0x7d, 0x0f, 0x26, 0xd8, 0xbd, 0x12, 0x0b, 0x2b, 0x6d, 0x9a, 0x1f, 0xd6, 0x12, 0xb3, 0x3a, 0x95, 0xd8, 0x0d, 0xfe, 0x97, 0xc1, 0xd8,
            0x61, 0xff, 0xd7, 0x1e, 0x04, 0xad, 0x16, 0xd5, 0x1b, 0xe8, 0x45, 0x60, 0x4c, 0xe7, 0xac, 0xf6, 0x9e, 0x26, 0xbe, 0xba, 0x37, 0xd6, 0xb5, 0xf5, 0x6f, 0xf1, 0x65, 0x55, 0x29, 0xdb, 0x27, 0x04,
        ];
        verify(&PUBLIC_KEY, b"livo", &SIG).unwrap();
        assert!(verify(&PUBLIC_KEY, b"livo!", &SIG).is_err());
    }

    /// 用一个假的 Livo.app 打成 dmg，走一遍解包
    #[cfg(target_os = "macos")]
    #[test]
    fn stages_app_from_dmg() {
        let dir = std::env::temp_dir().join(format!("livo_stage_test_{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("Livo.app/Contents/MacOS")).unwrap();
        std::fs::write(src.join("Livo.app/Contents/MacOS/livo"), "x").unwrap();
        let dmg = dir.join("t.dmg");
        run(Command::new("hdiutil").args(["create", "-quiet", "-fs", "HFS+", "-format", "UDZO", "-srcfolder"]).arg(&src).arg(&dmg)).unwrap();
        let staged = dir.join(".Livo-update.app");
        stage(&std::fs::read(&dmg).unwrap(), &staged).unwrap();
        assert_eq!(std::fs::read(staged.join("Contents/MacOS/livo")).unwrap(), b"x");
        std::fs::remove_dir_all(dir).ok();
    }

    /// 替换脚本要等进程退出后才动安装位置
    #[cfg(target_os = "macos")]
    #[test]
    fn mac_installer_waits_then_replaces() {
        let dir = std::env::temp_dir().join(format!("livo_install_test_{}", std::process::id()));
        let (app, staged) = (dir.join("Livo.app"), dir.join(".Livo-update.app"));
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(app.join("v"), "old").unwrap();
        std::fs::write(staged.join("v"), "new").unwrap();
        let mut busy = Command::new("sleep").arg("1").spawn().unwrap();
        let mut sh = Command::new("/bin/sh").args(["-c", MAC_INSTALL_SH, "sh", &busy.id().to_string()]).arg(&staged).arg(&app).arg("0").spawn().unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(std::fs::read(app.join("v")).unwrap(), b"old", "进程还在就不能换");
        busy.wait().unwrap();
        sh.wait().unwrap();
        assert_eq!(std::fs::read(app.join("v")).unwrap(), b"new");
        assert!(!staged.exists() && !dir.join("Livo.app.old").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
