#!/usr/bin/env python3
"""Login MyKlass sekali, simpan cookie ke ~/.bilik/cookies.json.

Dipakai karena myklass-*.umy.ac.id di belakang Cloudflare interactive challenge.
Botasaurus bisa lewat; Chromium Playwright mentah tidak. Jadi login di sini,
lalu cookie (termasuk cf_clearance) ditanam ke browser Bilik.

PENTING: cf_clearance terikat ke User-Agent. Skrip ini menyimpan UA yang dipakai,
dan Bilik memakai UA yang sama persis. Jangan ganti UA di Bilik.

  python3 scripts/login.py
  python3 scripts/login.py myklass-mkwu.umy.ac.id
"""
import os
import sys
import json
import time

BILIK_DIR = os.path.expanduser("~/.bilik")
OUT = os.path.join(BILIK_DIR, "cookies.json")
CRED = "/home/admin_/krs-umy/.env"


def load_env(path):
    d = {}
    with open(path) as f:
        for line in f:
            line = line.strip()
            if "=" in line and not line.startswith("#"):
                k, v = line.split("=", 1)
                d[k.strip()] = v.strip().strip('"').strip("'")
    return d


def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "myklass-eng.umy.ac.id"
    env = load_env(CRED)

    from pyvirtualdisplay import Display
    from botasaurus.browser import browser, Driver

    os.makedirs(BILIK_DIR, exist_ok=True)
    display = Display(visible=False, size=(1440, 900))
    display.start()
    result = {}
    try:
        @browser(headless=False, block_images=True, output=None)
        def run(driver: Driver, data):
            base = "https://" + host
            driver.get(base + "/login/index.php", bypass_cloudflare=True)
            driver.sleep(5)
            ok = False
            for _ in range(8):
                if driver.run_js("return !!document.querySelector('#username')"):
                    ok = True
                    break
                driver.sleep(5)
            if not ok:
                return {"error": "login_form_not_found", "url": driver.current_url}
            driver.run_js("""
                var u=document.querySelector('#username'), p=document.querySelector('#password');
                u.value=%r; u.dispatchEvent(new Event('input',{bubbles:true}));
                p.value=%r; p.dispatchEvent(new Event('input',{bubbles:true}));
                document.querySelector('#loginbtn').click();
            """ % (env.get("KRS_USERNAME", ""), env.get("KRS_PASSWORD", "")))
            driver.sleep(10)
            for _ in range(8):
                if "/login/index.php" not in (driver.current_url or ""):
                    break
                driver.sleep(5)
            if "/login/index.php" in (driver.current_url or ""):
                return {"error": "login_failed", "url": driver.current_url}
            driver.get(base + "/my/", bypass_cloudflare=True)
            driver.sleep(4)
            return {"url": driver.current_url,
                    "ua": driver.run_js("return navigator.userAgent"),
                    "cookies": driver.get_cookies()}

        result = run(None, None) or {}
    except Exception as e:
        result = {"error": "%s: %s" % (type(e).__name__, e)}
    finally:
        try:
            display.stop()
        except Exception:
            pass

    if result.get("error") or not result.get("cookies"):
        print(json.dumps(result, indent=1)[:600])
        return 1

    with open(OUT, "w") as f:
        json.dump({"ua": result["ua"], "cookies": result["cookies"]}, f)
    names = [c["name"] for c in result["cookies"]]
    print(json.dumps({"saved": OUT, "logged_in_url": result["url"],
                      "has_cf_clearance": "cf_clearance" in names,
                      "cookies": len(result["cookies"])}, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())