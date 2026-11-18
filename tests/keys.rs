//! Test vector config key.

#[path = "../src/keys.rs"]
mod keys;

const SEB: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>showTaskBar</key><true/><key>allowWlan</key><false/><key>showReloadButton</key><true/><key>showTime</key><true/><key>showInputLanguage</key><true/><key>allowQuit</key><true/><key>quitURLConfirm</key><true/><key>audioControlEnabled</key><false/><key>audioMute</key><false/><key>browserMediaCaptureCamera</key><false/><key>browserMediaCaptureMicrophone</key><false/><key>allowSpellCheck</key><false/><key>browserWindowAllowReload</key><true/><key>URLFilterEnable</key><false/><key>URLFilterEnableContentFilter</key><false/><key>hashedQuitPassword</key><string>9277abc49216725cf96e07e74980648ea71f50c2af1e0a22da6fe0e39e988e84</string><key>URLFilterRules</key><array/><key>startURL</key><string>https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097</string><key>sendBrowserExamKey</key><true/><key>browserWindowWebView</key><integer>3</integer><key>examSessionClearCookiesOnStart</key><false/><key>allowPreferencesWindow</key><false/></dict></plist>
"#;

const EXPECTED_KEY: &str = "ce349737793de563f17dd658c8fd29e6a856193342566cd289b01c2a0505cb45";
const EXPECTED_HEADER: &str = "1af7f2a8bbb173cd0913f0e064e57ae1e4234c5c7d5eee83856254afb5dc2c44";
const VIEW_URL: &str = "https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097";

#[test]
fn config_key_matches_reference() {
    let meta = keys::load_seb(SEB.as_bytes(), "config-big-data.seb").expect("parse");
    assert_eq!(meta.config_key, EXPECTED_KEY);
    assert_eq!(meta.cmid, Some(137097));
    assert_eq!(meta.host, "myklass-eng.umy.ac.id");
}

#[test]
fn header_value_matches_reference() {
    assert_eq!(keys::header_value(VIEW_URL, EXPECTED_KEY), EXPECTED_HEADER);
}

#[test]
fn rejects_binary_plist() {
    let raw = [b"bplist00".as_slice(), &[0u8; 40]].concat();
    let err = keys::parse_seb(&raw).unwrap_err().to_string().to_lowercase();
    assert!(err.contains("binary") || err.contains("terenkripsi"), "{err}");
}

#[test]
fn rejects_non_plist() {
    assert!(keys::parse_seb(b"hello world").is_err());
}

#[test]
fn parses_cmid_and_host() {
    assert_eq!(keys::parse_cmid(VIEW_URL), Some(137097));
    assert_eq!(
        keys::parse_cmid("https://x/mod/quiz/attempt.php?attempt=3&cmid=42"),
        Some(42)
    );
    assert_eq!(keys::host_of(VIEW_URL), "myklass-eng.umy.ac.id");
}