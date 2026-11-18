"""Penyimpanan config Bilik (multi-config, ~/.bilik/configs.json)."""
import os
import json
import time
import uuid

DATA_DIR = os.path.expanduser("~/.bilik")
STORE = os.path.join(DATA_DIR, "configs.json")


def _empty():
    return {"configs": [], "active": None}


def load():
    try:
        with open(STORE) as f:
            d = json.load(f)
        if not isinstance(d, dict) or "configs" not in d:
            return _empty()
        return d
    except FileNotFoundError:
        return _empty()
    except Exception:
        return _empty()


def save(d):
    os.makedirs(DATA_DIR, exist_ok=True)
    tmp = STORE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(d, f, indent=1)
    os.replace(tmp, STORE)


def add(meta):
    """Tambah config (replace kalau cmid+host sama). Kembalikan entry."""
    d = load()
    entry = dict(meta)
    entry["id"] = uuid.uuid4().hex[:12]
    entry["importedAt"] = time.strftime("%Y-%m-%d %H:%M:%S")

    # config baru untuk quiz yang sama menggantikan yang lama
    d["configs"] = [c for c in d["configs"]
                    if not (c.get("cmid") == entry.get("cmid") and c.get("host") == entry.get("host"))]
    d["configs"].insert(0, entry)
    d["active"] = entry["id"]
    save(d)
    return entry


def activate(cid):
    d = load()
    for c in d["configs"]:
        if c["id"] == cid:
            d["active"] = cid
            save(d)
            return c
    return None


def delete(cid):
    d = load()
    d["configs"] = [c for c in d["configs"] if c["id"] != cid]
    if d["active"] == cid:
        d["active"] = d["configs"][0]["id"] if d["configs"] else None
    save(d)
    return d


def active():
    d = load()
    for c in d["configs"]:
        if c["id"] == d["active"]:
            return c
    return None


def for_cmid(cmid):
    """Config yang paling cocok untuk cmid tertentu (fallback: config aktif)."""
    if cmid is not None:
        for c in load()["configs"]:
            if c.get("cmid") == cmid:
                return c
    return active()


def all_configs():
    return load()["configs"]