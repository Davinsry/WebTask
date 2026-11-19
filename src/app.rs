//! WebTask Ã¢â‚¬â€ jendela browser + panel UI.
//!
//! Satu jendela `tao`, satu WebView `wry`. WebView menampilkan shell HTML lokal
//! yang berisi tab bar + kontainer panel. Navigasi ke halaman quiz dilakukan oleh
//! WebView yang sama setelah user memilih tab.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Fullscreen, WindowBuilder};
use wry::WebViewBuilder;
#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;

use crate::{creds, keys, server, store};

const SEB_HOSTS: [&str; 2] = ["myklass-", ".umy.ac.id"];

#[derive(Debug)]
pub enum UserEvent {
    OpenUrl(String),
    ShowPanel,
    VerifyQuiz,
    VerifyResult(String),
    /// User memilih mode dari overlay awal.
    PickMode(creds::Mode),
}

const JS_SHA: &str = r#"
function __sha256(a){function r(v,n){return (v>>>n)|(v<<(32-n))}var M=Math.pow,mx=M(2,32),res='',w=[],L=a.length*8;
var h=__sha256.h=__sha256.h||[],k=__sha256.k=__sha256.k||[];p=k.length;c={};
for(var q=2;p<64;q++){if(!c[q]){for(var i=0;i<313;i+=q)c[i]=q;h[p]=(M(q,.5)*mx)|0;k[p++]=(M(q,1/3)*mx)|0}}
a+='\x80';while(a.length%64-56)a+='\x00';for(i=0;i<a.length;i++){var j=a.charCodeAt(i);if(j>>8)return '';w[i>>2]|=j<<((3-i)%4)*8}
w[w.length]=((L/mx)|0);w[w.length]=L;for(j=0;j<w.length;){var W=w.slice(j,j+=16),o=h;h=h.slice(0,8);
for(i=0;i<64;i++){var w15=W[i-15],w2=W[i-2],A=h[0],E=h[4];
var t1=h[7]+(r(E,6)^r(E,11)^r(E,25))+((E&h[5])^((~E)&h[6]))+k[i]+(W[i]=(i<16)?W[i]:(W[i-16]+(r(w15,7)^r(w15,18)^(w15>>>3))+W[i-7]+(r(w2,17)^r(w2,19)^(w2>>>10)))|0);
var t2=(r(A,2)^r(A,13)^r(A,22))+((A&h[1])^(A&h[2])^(h[1]&h[2]));h=[(t1+t2)|0].concat(h);h[4]=(h[4]+t1)|0}
for(i=0;i<8;i++)h[i]=(h[i]+o[i])|0}
for(i=0;i<8;i++)for(j=3;j+1;j--){var b=(h[i]>>(j*8))&255;res+=((b<16)?0:'')+b.toString(16)}return res}
"#;

/// JS yang dievaluasi di halaman quiz. Berupa IIFE yang MENYIMPAN hasil ke
/// `localStorage`/global, karena `evaluate_script_with_callback` tidak menunggu Promise.
const VERIFY_CALL_JS: &str = r#"
window.__WebTask_verify = null;
(function(){
  function go(){
    try{
      require(['core/ajax'], function(Ajax){
        var m = location.search.match(/[?&](?:cmid|id)=(\d+)/);
        var s = window.SafeExamBrowser && window.SafeExamBrowser.security;
        if(!s){ window.__WebTask_verify = JSON.stringify({ok:false, err:'no_stub'}); return; }
        Ajax.call([{methodname:'quizaccess_seb_validate_quiz_keys',
          args:{cmid: m?parseInt(m[1]):0, url: location.href,
                configkey: s.configKey, browserexamkey: s.browserExamKey}}])[0]
          .then(r => finish({ok:true, res:r}))
          .catch(e => finish({ok:false, err:String(e && e.message || e)}));
      });
    }catch(e){ setTimeout(go, 400); }
  }
  function finish(payload){
    payload.scanUrl = location.href;
    try { window.ipc.postMessage(JSON.stringify({type:'verify', line: JSON.stringify(payload)})); } catch(_) {}
  }
  go();
  setTimeout(function(){ finish({ok:false, err:'timeout'}); }, 15000);
})();
"#;

fn init_script() -> String {
    let all = store::all_configs();
    let mut cfgs = serde_json::Map::new();
    for c in &all {
        if let Some(cmid) = c.cmid {
            cfgs.insert(cmid.to_string(), json!(c.config_key));
        }
    }
    let fallback = store::active()
        .map(|c| c.config_key)
        .or_else(|| all.first().map(|c| c.config_key.clone()));

    format!(
        "(function(){{\nwindow.__WebTaskMode={mode};\nwindow.__WebTaskPick=true;\nvar CFG={cfgs},FB={fb},CRED={cred};\n{sha}\n\
function key(){{var m=location.search.match(/[?&](?:cmid|id)=(\\d+)/);\n\
 if(m && CFG[m[1]]) return CFG[m[1]]; return FB;}}\n\
var sec={{version:'WebTask',\n\
 get configKey(){{var k=key();return k?__sha256(location.href+k):''}},\n\
 get browserExamKey(){{var k=key();return k?__sha256(location.href+k):''}}}};\n\
Object.defineProperty(window,'SafeExamBrowser',{{configurable:true,value:{{security:sec}}}});\n\
try{{\n\
 if(/\\/mod\\/quiz\\//.test(location.pathname)){{\n\
  var add=function(){{\n\
   if(!document.body||document.getElementById('__WebTask_bar'))return;\n\
   var b=document.createElement('div');b.id='__WebTask_bar';\n\
   b.style.cssText='position:fixed;z-index:2147483647;right:16px;top:12px;display:flex;gap:8px';\n\
   var p=document.createElement('button');p.type='button';\n\
   p.innerHTML='<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\" style=\"vertical-align:-2px;margin-right:5px\"><path d=\"M3 10.5 12 3l9 7.5\"/><path d=\"M5 9.5V20h14V9.5\"/></svg>Panel';\n\
   p.style.cssText='display:inline-flex;align-items:center;background:#1F4E79;color:#fff;border:0;border-radius:6px;padding:7px 12px;font:600 12px \"Segoe UI\",system-ui;cursor:pointer';\n\
   p.onclick=function(){{try{{window.ipc.postMessage(JSON.stringify({{type:'panel'}}));}}catch(e){{}}}};\n\
   b.appendChild(p);document.body.appendChild(b);\n\
  }};\n\
if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',add);else add();\n\
 }}\n\
 }}catch(e){{}}\n\
 try{{\n\
  var m=/\\/login\\/index\\.php/.test(location.pathname);\n\
  if(m){{\n\
   var fill=function(){{\n\
    var u=document.querySelector('input[name=\"username\"],#username');\n\
    var p=document.querySelector('input[name=\"password\"],#password');\n\
    if(!u||!p)return;\n\
    if(CRED&&!u.value){{u.value=CRED.u;u.dispatchEvent(new Event('input',{{bubbles:true}}));}}\n\
    if(CRED&&!p.value){{p.value=CRED.p;p.dispatchEvent(new Event('input',{{bubbles:true}}));}}\n\
    var save=function(){{\n\
     if(!u.value||!p.value)return;\n\
     try{{window.ipc.postMessage(JSON.stringify({{type:'save_login',username:u.value,password:p.value}}));}}catch(e){{}}\n\
    }};\n\
    var f=p.form||document.querySelector('form#login,form.loginform');\n\
    if(f)f.addEventListener('submit',save,true);\n\
    var b=document.querySelector('button[type=\"submit\"],#loginbtn');\n\
    if(b)b.addEventListener('click',save,true);\n\
   }};\n\
   if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',fill);else fill();\n\
  }}\n\
 }}catch(e){{}}\n\
}})();",
        cfgs = json!(cfgs),
        fb = json!(fallback),
        sha = JS_SHA,
        mode = json!(creds::mode().as_str()),
        cred = match creds::credentials() {
            Some((u, p)) => json!({ "u": u, "p": p }).to_string(),
            None => "null".to_string(),
        },
    )
}

fn is_seb_host(url: &str) -> bool {
    SEB_HOSTS.iter().any(|h| url.contains(h))
}

fn is_quiz_page(url: &str) -> bool {
    url.contains("/mod/quiz/")
}

/// Skrip yang melumpuhkan anti-cheat client-side MyKlass.
///
/// Anti-cheat itu murni JavaScript di halaman (overlay hitam 60 detik, blokir
/// klik kanan/copy/shortcut via `preventDefault`, dan deteksi pindah tab lewat
/// `blur`/`visibilitychange`). Tidak ada laporan ke server, jadi cukup membuat
/// handler-nya tidak pernah terpasang.
///
/// Trik utamanya: `document.body.id` dan `classList` diganti lebih dulu saat
/// `document-start`, sebelum blok anti-cheat di BODY sempat jalan, sehingga
/// guard `body.id === 'page-mod-quiz-attempt'` selalu false dan seluruh rantai
/// handler jadi no-op. Sebagai jaring kedua, listener yang tetap terpasang
/// dibatalkan lewat `stopImmediatePropagation` pada fase capture dan overlay
/// dibuang.
///
/// Catatan: ini hanya menyentuh halaman, bukan form attempt, jadi jawaban yang
/// dikirim ke Moodle tetap sama.
const ANTICHEAT_NEUTRALIZE_JS: &str = r#"
(function(){
  // Hanya berlaku di host MyKlass; panel lokal dan halaman lain tidak disentuh.
  var h = location.hostname || '';
  if (!(h.indexOf('myklass-') === 0 || h.slice(-11) === '.umy.ac.id')) return;

  var QUIZ_ID = 'page-mod-quiz-attempt';
  try {
    // 1. Lepas id + class guard. CSS anti-cheat memakai selector
    //    body#page-mod-quiz-attempt, jadi seleksi teks ikut kembali normal.
    if (document.body) {
      document.body.id = 'x';
      document.body.classList.remove('path-mod-quiz');
    } else {
      document.addEventListener('DOMContentLoaded', function(){
        if (document.body) {
          document.body.id = 'x';
          document.body.classList.remove('path-mod-quiz');
        }
      }, true);
    }

    // 2. Buang overlay hitam kalau sudah sempat dibuat, dan cegah dibuat lagi.
    function dropOverlay(){
      var o = document.getElementById('anti-cheat-overlay');
      if (o && o.parentNode) o.parentNode.removeChild(o);
    }
    dropOverlay();
    new MutationObserver(dropOverlay).observe(document.documentElement, {
      childList: true, subtree: true
    });

    // 3. Netralkan handler halaman. Listener ini dipasang di window dengan
    //    fase capture, jadi jalan sebelum handler anti-cheat di document/body.
    function swallow(e){
      // Biarkan form kuis tetap normal: jangan sentuh input/textarea/select.
      var t = e.target;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' ||
                t.tagName === 'SELECT' || t.isContentEditable)) return;
      if (e.type === 'contextmenu' || e.type === 'copy' || e.type === 'cut' ||
          e.type === 'paste' || e.type === 'mouseup' || e.type === 'selectstart') {
        e.stopImmediatePropagation();
      }
      if (e.type === 'keydown') {
        var k = e.key;
        if (e.ctrlKey || e.metaKey) {
          var c = (k || '').toLowerCase();
          if (c === 'c' || c === 'v' || c === 'x' || c === 'a' || c === 'p' || c === 's') {
            e.stopImmediatePropagation();
          }
        }
        if (k === 'PrintScreen' || k === 'Print') e.stopImmediatePropagation();
      }
    }
    ['contextmenu','copy','cut','paste','mouseup','selectstart','keydown','keyup']
      .forEach(function(t){ window.addEventListener(t, swallow, true); });

    // 4. Matikan deteksi pindah tab/aplikasi: blur, visibilitychange, dan
    //    document.hidden dilaporkan selalu "aman".
    window.addEventListener('blur', function(e){ e.stopImmediatePropagation(); }, true);
    window.addEventListener('visibilitychange', function(e){ e.stopImmediatePropagation(); }, true);
    document.addEventListener('visibilitychange', function(e){ e.stopImmediatePropagation(); }, true);
    window.addEventListener('beforeprint', function(e){ e.stopImmediatePropagation(); }, true);
    try {
      Object.defineProperty(document, 'hidden', { configurable: true, get: function(){ return false; } });
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: function(){ return 'visible'; } });
    } catch (_) {}
    try { window.print = function(){}; } catch (_) {}

    // 5. Kembalikan hak copy. CSS anti-cheat memakai !important, jadi seleksi
    //    teks dipaksa lewat stylesheet yang lebih akhir.
    var st = document.createElement('style');
    st.textContent = 'body#page-mod-quiz-attempt,#page-mod-quiz-attempt #content,' +
      'body#page-mod-quiz-attempt *{-webkit-user-select:text!important;user-select:text!important}' +
      '@media print{body#page-mod-quiz-attempt{display:block!important}}';
    (document.head || document.documentElement).appendChild(st);
  } catch (e) {}
})();
"#;


/// Ikon jendela dari ICO yang tertanam di binary (tanpa file eksternal saat distribusi).
#[cfg(target_os = "windows")]
fn window_icon() -> Option<tao::window::Icon> {

    const ICO: &[u8] = include_bytes!("../assets/icon.ico");

    // ICO menyimpan banyak ukuran; ambil entri 32x32 bila ada agar tajam di taskbar.
    let count = u16::from_le_bytes([ICO[4], ICO[5]]) as usize;
    let mut best: Option<(u32, u32, usize, usize)> = None;

    for i in 0..count {
        let off = 6 + i * 16;
        if off + 16 > ICO.len() {
            break;
        }
        let w = if ICO[off] == 0 { 256 } else { ICO[off] as u32 };
        let h = if ICO[off + 1] == 0 { 256 } else { ICO[off + 1] as u32 };
        let size = u32::from_le_bytes([
            ICO[off + 8], ICO[off + 9], ICO[off + 10], ICO[off + 11],
        ]) as usize;
        let data_off = u32::from_le_bytes([
            ICO[off + 12], ICO[off + 13], ICO[off + 14], ICO[off + 15],
        ]) as usize;
        if data_off + size > ICO.len() || w == 0 || h == 0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((bw, _, _, _)) => {
                (w == 32 && bw != 32) || (w != 32 && bw != 32 && w > bw)
            }
        };
        if better {
            best = Some((w, h, data_off, size));
        }
    }

    let (w, h, data_off, size) = best?;
    let data = &ICO[data_off..data_off + size];

    // Untuk ukuran < 256, data ICO adalah DIB (BITMAPINFOHEADER) yang perlu
    // dikonversi ke RGBA. Ukuran 256 memakai PNG; tao::Icon butuh RGBA mentah.
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        return decode_png_to_icon(data, w, h);
    }
    decode_dib_to_icon(data, w, h)
}

#[cfg(target_os = "windows")]
fn decode_dib_to_icon(data: &[u8], w: u32, h: u32) -> Option<tao::window::Icon> {
    use tao::window::Icon;

    // BITMAPINFOHEADER (40 byte): biSize, biWidth, biHeight, biPlanes, biBitCount ...
    if data.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let bit_count = u16::from_le_bytes([data[14], data[15]]);
    if bit_count != 32 {
        return None;
    }
    let pixels = data.get(header_size..)?;
    let row_bytes = (w as usize) * 4;
    let height = h as usize;

    // DIB disimpan bottom-up dan BGRA.
    let mut rgba = vec![0u8; row_bytes * height];
    for y in 0..height {
        let src_row = (height - 1 - y) * row_bytes;
        let dst_row = y * row_bytes;
        if src_row + row_bytes > pixels.len() {
            return None;
        }
        for x in 0..w as usize {
            let s = src_row + x * 4;
            let d = dst_row + x * 4;
            let b = pixels[s];
            let g = pixels[s + 1];
            let r = pixels[s + 2];
            let a = pixels[s + 3];
            rgba[d] = r;
            rgba[d + 1] = g;
            rgba[d + 2] = b;
            rgba[d + 3] = a;
        }
    }
    Icon::from_rgba(rgba, w, h).ok()
}

#[cfg(target_os = "windows")]
fn decode_png_to_icon(data: &[u8], w: u32, h: u32) -> Option<tao::window::Icon> {
    // Ikon 256px jarang dipakai Windows untuk taskbar; biarkan None dan pakai DIB
    // 32px. Fungsi ini hanya jaring pengaman bila ICO hanya memuat PNG.
    let _ = (data, w, h);
    None
}

/// Header `ConfigKeyHash` untuk URL dokumen, kalau ada config yang cocok.
pub fn header_for(url: &str) -> Option<(String, String)> {
    let cmid = keys::parse_cmid(url);
    let cfg = store::for_cmid(cmid)?;
    Some((keys::header_value(url, &cfg.config_key), url.to_string()))
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Jendela selalu dibuka kecil tanpa kiosk. Shell memuat overlay pilihan
    // mode; setelah user memilih, jendela diubah sesuai mode (kiosk untuk SEB,
    // jendela normal untuk web).
    let needs_pick = true;
    let mut window_builder = WindowBuilder::new()
        .with_title("WebTask")
        .with_inner_size(tao::dpi::LogicalSize::new(440.0, 300.0))
        .with_resizable(false)
        .with_maximizable(false)
        .with_min_inner_size(tao::dpi::LogicalSize::new(360.0, 260.0));

    #[cfg(target_os = "windows")]
    {
        if let Some(icon) = window_icon() {
            window_builder = window_builder.with_window_icon(Some(icon));
        }
    }

    let window = window_builder.build(&event_loop)?;

    let (open_tx, open_rx): (Sender<String>, _) = channel::<String>();
    let open_storage: server::OpenTx = Arc::new(Mutex::new(Some(open_tx)));
    let port = server::start(8765, open_storage.clone()).unwrap_or(8765);
    println!("[WebTask] panel UI: http://127.0.0.1:{port}");
    let shell_url = format!("http://127.0.0.1:{port}/");

    // URL halaman quiz yang perlu diverifikasi setelah selesai load.
    let pending_verify: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let verify_slot = pending_verify.clone();
    let proxy_load = proxy.clone();

    let webview = WebViewBuilder::new()
        .with_url(&shell_url)
        .with_devtools(true)
        .with_additional_browser_args(concat!(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection ",
            // Batasi autentikasi terintegrasi ke host yang tidak pernah dipakai,
            // supaya WebView2 tidak memunculkan dialog "Windows Security" (NTLM)
            // untuk server kampus.
            "--auth-server-allowlist=webtask.invalid ",
            "--auth-negotiate-delegate-allowlist=webtask.invalid"
        ))
        .with_initialization_script(init_script())
        .with_initialization_script(ANTICHEAT_NEUTRALIZE_JS)
        .with_on_page_load_handler(move |event, url| {
            use wry::PageLoadEvent;
            if !matches!(event, PageLoadEvent::Finished) || !is_quiz_page(&url) {
                return;
            }
            *verify_slot.lock().unwrap() = Some(url);
            let _ = proxy_load.send_event(UserEvent::VerifyQuiz);
        })
        .with_ipc_handler({
            let proxy: EventLoopProxy<UserEvent> = proxy.clone();
            move |req| handle_ipc(req.body(), &proxy)
        })
        .build(&window)?;

    #[cfg(target_os = "windows")]
    unsafe {
        windows_header_hook::install(&webview);
    }

    let shell_url_ref = shell_url;
    let mut launched = !needs_pick;

    event_loop.run(move |event, _, control_flow| {
        *control_flow =
            ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(150));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::PickMode(mode)) => {
                if launched {
                    return;
                }
                launched = true;
                let _ = creds::set_mode(mode);

                if mode == creds::Mode::Seb {
                    window.set_maximizable(false);
                    window.set_decorations(false);
                    window.set_resizable(false);
                    window.set_minimizable(false);
                    window.set_closable(false);
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    #[cfg(target_os = "windows")]
                    unsafe {
                        kiosk::install(&window);
                    }
                } else {
                    window.set_fullscreen(None);
                    window.set_decorations(true);
                    window.set_resizable(true);
                    window.set_minimizable(true);
                    window.set_maximizable(true);
                    window.set_closable(true);
                    window.set_inner_size(tao::dpi::LogicalSize::new(1280.0, 860.0));
                    if let Some(monitor) = window.current_monitor() {
                        let screen = monitor.size().to_logical::<f64>(monitor.scale_factor());
                        let size = window.outer_size().to_logical::<f64>(window.scale_factor());
                        let x = (screen.width - size.width) / 2.0;
                        let y = (screen.height - size.height) / 2.0;
                        window.set_outer_position(tao::dpi::LogicalPosition::new(x, y));
                    }
                }
            }
            Event::UserEvent(UserEvent::OpenUrl(url)) => {
                navigate(&webview, &url);
            }
            Event::UserEvent(UserEvent::ShowPanel) => {
                let _ = webview.load_url(&shell_url_ref);
            }
            Event::UserEvent(UserEvent::VerifyQuiz) => {
                start_verify(&webview);
            }
            Event::UserEvent(UserEvent::VerifyResult(line)) => {
                let value: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                let cfg_ok = value
                    .get("res")
                    .and_then(|r| r.get("configkey"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if cfg_ok {
                    println!("[WebTask] VERIFY OK");
                    show_toast(&webview, "WebTask: config cocok", true);
                } else {
                    println!("[WebTask] VERIFY FAIL {line}");
                    show_toast(
                        &webview,
                        "WebTask: config tidak cocok untuk quiz ini. Import ulang file .seb.",
                        false,
                    );
                }
            }
            Event::MainEventsCleared => {
                while let Ok(url) = open_rx.try_recv() {
                    navigate(&webview, &url);
                }
                #[cfg(target_os = "windows")]
                unsafe {
                    if launched && creds::mode() == creds::Mode::Seb {
                        kiosk::enforce();
                    }
                }
            }
            _ => {}
        }
    });
}

fn navigate(webview: &wry::WebView, url: &str) {
    if is_seb_host(url) && header_for(url).is_none() {
        println!("[WebTask] tanpa config untuk {}", short(url));
    }
    let _ = webview.load_url(url);
}

fn start_verify(webview: &wry::WebView) {
    // Suntik panggilan WS. Hasil dikirim balik lewat window.ipc (lihat VERIFY_CALL_JS),
    // sehingga tidak perlu polling dari thread lain.
    let _ = webview.evaluate_script(VERIFY_CALL_JS);
}

fn show_toast(webview: &wry::WebView, text: &str, ok: bool) {
    // Toast memakai palet DESIGN.md: permukaan solid, border 1px, tanda status
    // berbentuk kotak-isi (bukan dot berdenyut). Warna selalu didampingi kata.
    let (bg, fg, border, mark) = if ok {
        ("#E8F3EC", "#1F7A4D", "#BFDCC9", "#1F7A4D")
    } else {
        ("#FBEAE9", "#B3261E", "#E9C3C0", "#B3261E")
    };
    let label = if ok { "Cocok" } else { "Tidak cocok" };
    let js = format!(
        r#"(function(){{
  var id='__WebTask_toast';
  var d = document.getElementById(id);
  if(d) d.remove();
  d = document.createElement('div'); d.id=id;
  d.setAttribute('role','status');
  d.style.cssText='position:fixed;z-index:2147483647;left:20px;bottom:20px;max-width:360px;display:flex;gap:10px;align-items:flex-start;padding:11px 14px;border-radius:10px;border:1px solid {border};background:{bg};color:{fg};font:600 13px/1.45 "Segoe UI",system-ui,sans-serif';
  var m=document.createElement('span');
  m.setAttribute('aria-hidden','true');
  m.style.cssText='flex:none;width:8px;height:8px;margin-top:5px;border-radius:2px;background:{mark}';
  var t=document.createElement('span');
  var b=document.createElement('b'); b.textContent={label:?}+' Ã¢â‚¬â€ ';
  t.appendChild(b);
  t.appendChild(document.createTextNode({text:?}));
  d.appendChild(m); d.appendChild(t);
  document.body.appendChild(d);
  var hide=function(){{d.style.opacity='0';d.style.transition='opacity .3s';}};
  setTimeout(hide, 5600);
  setTimeout(function(){{d.remove();}}, 6200);
}})();"#
    );
    let _ = webview.evaluate_script(&js);
}

fn handle_ipc(body: &str, proxy: &EventLoopProxy<UserEvent>) {
    let msg: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return,
    };
    match msg.get("type").and_then(Value::as_str) {
        Some("open") => {
            if let Some(url) = msg.get("url").and_then(Value::as_str) {
                let _ = proxy.send_event(UserEvent::OpenUrl(url.to_string()));
            }
        }
        Some("panel") => {
            let _ = proxy.send_event(UserEvent::ShowPanel);
        }
        Some("verify") => {
            if let Some(line) = msg.get("line").and_then(Value::as_str) {
                let _ = proxy.send_event(UserEvent::VerifyResult(line.to_string()));
            }
        }
        Some("save_login") => {
            // Halaman login MyKlass menyerahkan kredensial saat submit; disimpan
            // terenkripsi supaya kunjungan berikutnya otomatis terisi.
            let user = msg.get("username").and_then(Value::as_str).unwrap_or("");
            let pass = msg.get("password").and_then(Value::as_str).unwrap_or("");
            if !user.is_empty() && !pass.is_empty() {
                let _ = creds::save(user, pass);
            }
        }
        Some("pick_mode") => {
            let mode = match msg.get("mode").and_then(Value::as_str) {
                Some("web") => creds::Mode::Web,
                Some("seb") => creds::Mode::Seb,
                _ => return,
            };
            let _ = proxy.send_event(UserEvent::PickMode(mode));
        }
        _ => {}
    }
}

fn short(url: &str) -> String {
    url.split("umy.ac.id")
        .last()
        .unwrap_or(url)
        .chars()
        .take(80)
        .collect()
}

#[cfg(target_os = "windows")]
mod windows_header_hook {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2WebResourceRequestedEventArgs, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
    };
    use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
    use windows::core::{PWSTR, HSTRING};
    use wry::WebViewExtWindows;

    use crate::app::header_for;
    use crate::creds;

    pub unsafe fn install(webview: &wry::WebView) {
        let core = webview.webview();
        let handler = WebResourceRequestedEventHandler::create(Box::new(|_sender, args| {
            let args: Option<&ICoreWebView2WebResourceRequestedEventArgs> = args.as_ref();
            if let Some(args) = args {
                // Mode web biasa tidak mengirim header SEB sama sekali.
                if creds::mode() != creds::Mode::Seb {
                    return Ok(());
                }
                let req = args.Request()?;
                let mut pw = PWSTR::null();
                req.Uri(&mut pw)?;
                let url = take_pwstr(pw);
                if let Some((value, _)) = header_for(&url) {
                    let headers = req.Headers()?;
                    let v = HSTRING::from(value.as_str());
                    headers.SetHeader(&HSTRING::from("X-SafeExamBrowser-ConfigKeyHash"), &v)?;
                    headers.SetHeader(&HSTRING::from("X-SafeExamBrowser-RequestHash"), &v)?;
                }
            }
            Ok(())
        }));
        // Filter dokumen saja; cukup untuk header per halaman.
        for pattern in ["https://myklass-*/*", "https://*.umy.ac.id/*"] {
            let _ = core.AddWebResourceRequestedFilter(
                &HSTRING::from(pattern),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
            );
        }
        let _ = core.add_WebResourceRequested(&handler, std::ptr::null_mut());
    }
}

/// Penjaga mode kiosk di Windows.
///
/// `tao` tidak mencegah `SetFullscreen`/`Restore` dari luar (Win+Ã¢â€ â€˜, klik taskbar,
/// atau shell), dan borderless fullscreen masih bisa kehilangan status topmost
/// saat app lain diaktifkan lewat Alt+Tab. Karena itu jendela diperiksa berkala
/// dari event loop: kalau ada yang mengubah posisinya, ia dikembalikan.
#[cfg(target_os = "windows")]
mod kiosk {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongPtrW, IsZoomed, SetWindowLongPtrW, SetWindowPos, GWL_STYLE,
        HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_SYSMENU, WS_THICKFRAME,
    };

    /// Tinggi tambahan di bawah client area untuk menutup taskbar. Borderless
    /// fullscreen kadang masih menyisakan taskbar karena area kerja dihitung
    /// sebelum window masuk mode fullscreen.
    const TASKBAR_COVER: i32 = 96;

    static HWND_CELL: AtomicIsize = AtomicIsize::new(0);

    pub unsafe fn install(window: &tao::window::Window) {
        use tao::platform::windows::WindowExtWindows;

        let raw = window.hwnd();
        let hwnd = HWND(raw as *mut _);
        HWND_CELL.store(raw, Ordering::SeqCst);

        // Buang tombol minimize/maximize/close dari style jendela. Title bar
        // sendiri sudah hilang lewat `with_decorations(false)`.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let stripped = style
            & !(WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0 | WS_THICKFRAME.0);
        SetWindowLongPtrW(hwnd, GWL_STYLE, stripped as isize);

        window.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
        // JANGAN pakai HWND_TOPMOST: jendela topmost merebut depan terus dan
        // membuat Alt+Tab hanya menampilkan preview tanpa bisa berpindah.
        // Cukup tempatkan di lapisan normal paling atas.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        cover_taskbar(hwnd);
    }

    /// Dipanggil berkala dari event loop. Sengaja TIDAK merebut fokus atau
    /// menaikkan jendela ke depan: itu membuat Alt+Tab hanya menampilkan preview
    /// karena WebTask langsung menarik dirinya kembali. Yang dijaga hanya region
    /// jendela supaya taskbar tetap tertutup.
    pub unsafe fn enforce() {
        let raw = HWND_CELL.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        let hwnd = HWND(raw as *mut _);

        // Kalau ada yang keluar dari maximized (mis. shell restore), pastikan
        // tetap berukuran penuh. Tidak menyentuh fokus sama sekali.
        if !IsZoomed(hwnd).as_bool() {
            cover_taskbar(hwnd);
        }
    }

    /// Perbesar region jendela melewati tepi bawah layar supaya taskbar tertutup.
    unsafe fn cover_taskbar(hwnd: HWND) {
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let rgn = CreateRectRgn(0, 0, rect.right, rect.bottom + TASKBAR_COVER);
        if !rgn.is_invalid() {
            SetWindowRgn(hwnd, Some(rgn), true);
            let _ = DeleteObject(HGDIOBJ(rgn.0));
        }
    }
}
