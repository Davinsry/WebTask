"""Test vector config key — jalankan: python3 -m pytest tests/ -q"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bilik import keys  # noqa: E402

SEB = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>showTaskBar</key><true/><key>allowWlan</key><false/><key>showReloadButton</key><true/><key>showTime</key><true/><key>showInputLanguage</key><true/><key>allowQuit</key><true/><key>quitURLConfirm</key><true/><key>audioControlEnabled</key><false/><key>audioMute</key><false/><key>browserMediaCaptureCamera</key><false/><key>browserMediaCaptureMicrophone</key><false/><key>allowSpellCheck</key><false/><key>browserWindowAllowReload</key><true/><key>URLFilterEnable</key><false/><key>URLFilterEnableContentFilter</key><false/><key>hashedQuitPassword</key><string>9277abc49216725cf96e07e74980648ea71f50c2af1e0a22da6fe0e39e988e84</string><key>URLFilterRules</key><array/><key>startURL</key><string>https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097</string><key>sendBrowserExamKey</key><true/><key>browserWindowWebView</key><integer>3</integer><key>examSessionClearCookiesOnStart</key><false/><key>allowPreferencesWindow</key><false/></dict></plist>
"""

EXPECTED_KEY = "ce349737793de563f17dd658c8fd29e6a856193342566cd289b01c2a0505cb45"
EXPECTED_HEADER = "1af7f2a8bbb173cd0913f0e064e57ae1e4234c5c7d5eee83856254afb5dc2c44"
VIEW_URL = "https://myklass-eng.umy.ac.id/mod/quiz/view.php?id=137097"


def test_config_key():
    meta = keys.load_seb(SEB.encode(), "config-big-data.seb")
    assert meta["configKey"] == EXPECTED_KEY
    assert meta["cmid"] == 137097
    assert meta["host"] == "myklass-eng.umy.ac.id"


def test_header_value():
    assert keys.header_value(VIEW_URL, EXPECTED_KEY) == EXPECTED_HEADER


def test_reject_binary_plist():
    try:
        keys.parse_seb(b"bplist00" + b"\x00" * 40)
    except keys.ConfigError as e:
        assert "binary" in str(e).lower() or "terenkripsi" in str(e).lower()
    else:
        raise AssertionError("harus menolak binary plist")


def test_reject_non_plist():
    try:
        keys.parse_seb(b"hello world")
    except keys.ConfigError:
        pass
    else:
        raise AssertionError("harus menolak file bukan plist")