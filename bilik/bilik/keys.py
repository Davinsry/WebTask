"""Parsing file .seb dan perhitungan Config Key.

Replika persis quizaccess_seb\\config_key::generate() + property_list::to_json():
  parse plist -> buang originatorVersion -> sort key case-insensitive ->
  json compact (tanpa escape slash/unicode) -> sha256 hex
"""
import json
import hashlib
import plistlib


class ConfigError(Exception):
    pass


def parse_seb(raw: bytes):
    """Kembalikan (data_dict, start_url). Raise ConfigError kalau tidak valid."""
    if not raw:
        raise ConfigError("File kosong.")
    if len(raw) > 1024 * 1024:
        raise ConfigError("File lebih dari 1 MB, tidak wajar untuk config SEB.")
    head = raw[:8]
    if head.startswith(b"bplist"):
        raise ConfigError("Config dalam format binary/terenkripsi. Minta versi unencrypted (.seb XML) ke dosen.")
    if b"<plist" not in raw[:2000]:
        raise ConfigError("Bukan file .seb (plist XML) yang valid.")
    try:
        data = plistlib.loads(raw)
    except Exception as e:
        raise ConfigError("Gagal parse plist: %s. Kalau config dienkripsi password, minta versi unencrypted." % e)
    if not isinstance(data, dict):
        raise ConfigError("Root plist bukan dictionary.")
    start_url = data.get("startURL")
    if not isinstance(start_url, str) or not start_url.startswith("https://"):
        raise ConfigError("Config tidak punya startURL https yang valid.")
    return data, start_url


def config_key_from_data(data: dict) -> str:
    d = dict(data)
    d.pop("originatorVersion", None)
    ordered = {k: v for k, v in sorted(d.items(), key=lambda kv: kv[0].lower())}
    seb_json = json.dumps(ordered, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(seb_json.encode()).hexdigest()


def parse_cmid(url: str):
    import urllib.parse
    q = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)
    for k in ("id", "cmid"):
        if k in q and q[k] and q[k][0].isdigit():
            return int(q[k][0])
    return None


def host_of(url: str) -> str:
    import urllib.parse
    return urllib.parse.urlparse(url).netloc


def header_value(url: str, config_key: str) -> str:
    """Nilai X-SafeExamBrowser-ConfigKeyHash untuk satu URL."""
    return hashlib.sha256((url + config_key).encode()).hexdigest()


def load_seb(raw: bytes, filename: str = ""):
    data, start_url = parse_seb(raw)
    return {
        "filename": filename,
        "startURL": start_url,
        "host": host_of(start_url),
        "cmid": parse_cmid(start_url),
        "configKey": config_key_from_data(data),
    }