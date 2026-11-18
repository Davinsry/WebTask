"""Bilik — launcher browser + UI. Jalankan: python3 -m bilik"""
import os
import time
import json
import queue

from . import store, keys, server

PORT = int(os.environ.get("BILIK_PORT", "8765"))
COOKIE_FILES = [os.path.expanduser("~/.bilik/cookies.json"),
                os.path.expanduser("~/.seb_lite_cookies.json")]
PROFILE = os.path.expanduser("~/.bilik/profile")
SEB_HOSTS = ("myklass-", ".umy.ac.id")

JS_SHA = r"""
function __sha256(a){function r(v,n){return (v>>>n)|(v<<(32-n))}var M=Math.pow,mx=M(2,32),res='',w=[],L=a.length*8;
var h=__sha256.h=__sha256.h||[],k=__sha256.k=__sha256.k||[],p=k.length,c={};for(var q=2;p<64;q++){if(!c[q]){for(var i=0;i<313;i+=q)c[i]=q;h[p]=(M(q,.5)*mx)|0;k[p++]=(M(q,1/3)*mx)|0}}
a+='\x80';while(a.length%64-56)a+='\x00';for(i=0;i<a.length;i++){var j=a.charCodeAt(i);if(j>>8)return '';w[i>>2]|=j<<((3-i)%4)*8}
w[w.length]=((L/mx)|0);w[w.length]=L;for(j=0;j<w.length;){var W=w.slice(j,j+=16),o=h;h=h.slice(0,8);
for(i=0;i<64;i++){var w15=W[i-15],w2=W[i-2],A=h[0],E=h[4];
var t1=h[7]+(r(E,6)^r(E,11)^r(E,25))+((E&h[5])^((~E)&h[6]))+k[i]+(W[i]=(i<16)?W[i]:(W[i-16]+(r(w15,7)^r(w15,18)^(w15>>>3))+W[i-7]+(r(w2,17)^r(w2,19)^(w2>>>10)))|0);
var t2=(r(A,2)^r(A,13)^r(A,22))+((A&h[1])^(A&h[2])^(h[1]&h[2]));h=[(t1+t2)|0].concat(h);h[4]=(h[4]+t1)|0}
for(i=0;i<8;i++)h[i]=(h[i]+o[i])|0}
for(i=0;i<8;i++)for(j=3;j+1;j--){var b=(h[i]>>(j*8))&255;res+=((b<16)?0:'')+b.toString(16)}return res}
"""

VERIFY_JS = r"""
() => new Promise(resolve => {
  function go(){
    try{
      require(['core/ajax'], function(Ajax){
        var m = location.search.match(/[?&](?:cmid|id)=(\d+)/);
        var s = window.SafeExamBrowser && window.SafeExamBrowser.security;
        if(!s){ resolve({ok:false, err:'no_stub'}); return; }
        Ajax.call([{methodname:'quizaccess_seb_validate_quiz_keys',
          args:{cmid: m?parseInt(m[1]):0, url: location.href,
                configkey: s.configKey, browserexamkey: s.browserExamKey}}])[0]
          .then(r => resolve({ok:true, res:r}))
          .catch(e => resolve({ok:false, err:String(e && e.message || e)}));
      });
    }catch(e){ setTimeout(go, 400); }
  }
  go();
  setTimeout(()=>resolve({ok:false, err:'timeout'}), 15000);
})
"""

TOAST_JS = r"""
([txt, color]) => {
  var d = document.getElementById('__bilik_toast');
  if(!d){
    d = document.createElement('div'); d.id='__bilik_toast';
    d.style.cssText='position:fixed;z-index:2147483647;left:16px;bottom:16px;padding:10px 14px;'+
      'border-radius:10px;font:600 13px/1.3 system-ui,sans-serif;color:#fff;box-shadow:0 6px 24px rgba(0,0,0,.18)';
    document.body.appendChild(d);
    setTimeout(()=>{d.style.opacity='0';d.style.transition='opacity .4s';}, 6000);
    setTimeout(()=>d.remove(), 6800);
  }
  d.textContent = txt; d.style.background = color || '#111';
}
"""


def load_cookies():
    for p in COOKIE_FILES:
        if os.path.exists(p):
            d = json.load(open(p))
            return d.get("ua"), d.get("cookies") or []
    return None, []


def to_pw_cookies(raw):
    out = []
    for c in raw:
        pw = {"name": c.get("name"), "value": c.get("value"), "domain": c.get("domain"),
              "path": c.get("path", "/"), "secure": bool(c.get("secure")),
              "httpOnly": bool(c.get("httpOnly"))}
        exp = c.get("expiry") or c.get("expires")
        if exp:
            pw["expires"] = float(exp)
        ss = (c.get("sameSite") or "").lower()
        pw["sameSite"] = {"lax": "Lax", "strict": "Strict", "none": "None"}.get(ss, "Lax")
        out.append(pw)
    return out


def resolve_key(url):
    """Config yang dipakai untuk URL ini (match by cmid, fallback aktif)."""
    cmid = keys.parse_cmid(url)
    c = store.for_cmid(cmid)
    return (c or {}).get("configKey"), c


def build_init(version):
    cfgs = {}
    fallback = None
    for c in store.all_configs():
        if c.get("cmid") is not None:
            cfgs[str(c["cmid"])] = c["configKey"]
        if c["id"] == (store.load().get("active") or ""):
            fallback = c["configKey"]
    if fallback is None and store.all_configs():
        fallback = store.all_configs()[0]["configKey"]
    return "(function(){var V=%d;if((window.__BILIK_VER||0)>=V)return;window.__BILIK_VER=V;%s" \
           "\nvar CFG=%s, FB=%s;\n" \
           "function key(){var m=location.search.match(/[?&](?:cmid|id)=(\\d+)/);" \
           " if(m && CFG[m[1]]) return CFG[m[1]]; return FB;}\n" \
           "var sec={version:'Bilik', get configKey(){var k=key();return k?__sha256(location.href+k):''}," \
           " get browserExamKey(){var k=key();return k?__sha256(location.href+k):''}};\n" \
           "Object.defineProperty(window,'SafeExamBrowser',{configurable:true,value:{security:sec}});\n" \
           "})();" % (version, JS_SHA, json.dumps(cfgs), json.dumps(fallback))


def main():
    from playwright.sync_api import sync_playwright

    srv = server.start(PORT)
    print("[bilik] UI  : http://127.0.0.1:%d" % PORT)

    ua, raw_cookies = load_cookies()
    os.makedirs(PROFILE, exist_ok=True)
    state = {"version": 0}

    with sync_playwright() as p:
        ctx = p.chromium.launch_persistent_context(
            PROFILE, headless=bool(os.environ.get("BILIK_HEADLESS")),
            user_agent=ua or None, viewport={"width": 1440, "height": 900},
            args=["--disable-blink-features=AutomationControlled"],
            ignore_default_args=["--enable-automation"],
        )
        if raw_cookies:
            try:
                ctx.add_cookies(to_pw_cookies(raw_cookies))
                print("[bilik] cookie MyKlass dimuat (%d)" % len(raw_cookies))
            except Exception as e:
                print("[bilik] gagal muat cookie:", e)

        state["version"] += 1
        ctx.add_init_script(build_init(state["version"]))

        def on_route(route):
            req = route.request
            url = req.url
            h = dict(req.headers)
            if req.resource_type == "document" and any(s in url for s in SEB_HOSTS):
                k, c = resolve_key(url)
                if k:
                    hv = keys.header_value(url, k)
                    h["x-safeexambrowser-configkeyhash"] = hv
                    h["x-safeexambrowser-requesthash"] = hv
                    print("[bilik] header %s -> %s…" % (url.split("umy.ac.id")[-1][:70], hv[:12]))
                else:
                    print("[bilik] tidak ada config untuk", url[:90])
            route.continue_(headers=h)

        ctx.route("**/*", on_route)

        OPEN_Q = queue.Queue()
        VERIFY_Q = queue.Queue()
        server.HOOKS["open_url"] = lambda u: OPEN_Q.put(u)

        # verifikasi config setiap halaman quiz selesai load
        def verify(pg):
            try:
                if "/mod/quiz/" not in pg.url:
                    return
                res = pg.evaluate(VERIFY_JS)
                if not isinstance(res, dict):
                    return
                cfg = res.get("res") or {}
                if cfg.get("configkey"):
                    pg.evaluate(TOAST_JS, ["Bilik: config cocok", "#059669"])
                    print("[bilik] VERIFY OK  ", pg.url.split("umy.ac.id")[-1][:70])
                else:
                    pg.evaluate(TOAST_JS,
                                ["Bilik: config tidak cocok untuk quiz ini. Import ulang file .seb.",
                                 "#dc2626"])
                    print("[bilik] VERIFY FAIL", res)
            except Exception as e:
                print("[bilik] verify error:", e)

        panel = ctx.pages[0] if ctx.pages else ctx.new_page()
        panel.goto("http://127.0.0.1:%d/" % PORT, wait_until="domcontentloaded")

        def attach(pg):
            try:
                pg.on("load", lambda: VERIFY_Q.put(pg))
            except Exception:
                pass

        ctx.on("page", attach)
        for pg in ctx.pages:
            attach(pg)

        # kalau dijalankan dengan --url, langsung buka
        target = os.environ.get("BILIK_URL")
        if target:
            OPEN_Q.put(target)

        print("[bilik] siap. Tutup jendela atau Ctrl+C untuk keluar.")
        try:
            while True:
                time.sleep(0.4)
                while not OPEN_Q.empty():
                    u = OPEN_Q.get()
                    try:
                        pg = ctx.new_page()
                        pg.goto(u, wait_until="domcontentloaded")
                        print("[bilik] buka", u)
                        try:
                            pg.wait_for_load_state("domcontentloaded", timeout=15000)
                        except Exception:
                            pass
                        if "/mod/quiz/" in pg.url:
                            verify(pg)
                    except Exception as e:
                        print("[bilik] gagal buka %s: %s" % (u, e))
                while not VERIFY_Q.empty():
                    pg = VERIFY_Q.get()
                    try:
                        if "/mod/quiz/" in pg.url:
                            verify(pg)
                    except Exception:
                        pass
                if not ctx.pages:
                    break
        except KeyboardInterrupt:
            pass
        finally:
            srv.shutdown()
            try:
                ctx.close()
            except Exception:
                pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())