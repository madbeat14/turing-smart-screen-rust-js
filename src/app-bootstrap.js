// Template bootstrapper — activates v1 or v2 based on THEME config
var DEFAULT_TEMPLATE = 'v2';

// Signal for the display loop: don't screenshot until the template is rendered
window.__templateReady = false;
function markTemplateReady() {
  window.__templateReady = true;
  window.dispatchEvent(new Event('template-ready'));
}

// Helper to log messages from webview to the Rust log file
function wlog(msg) {
  try { window.__TAURI__.core.invoke('webview_log', { level: 'info', msg: msg }); } catch(e) {}
  console.log(msg);
}

function activateTemplate(templateId) {
  wlog('[TEMPLATE] activateTemplate: ' + templateId);
  document.title = 'Loading: ' + templateId;
  // Hide all inline templates
  var all = document.querySelectorAll('.template');
  for (var i = 0; i < all.length; i++) {
    all[i].classList.remove('active');
  }

  // Remove any previously loaded custom template DOM/CSS/JS
  var existingCustom = document.getElementById('tpl-custom');
  if (existingCustom) existingCustom.remove();
  var existingCustomCss = document.getElementById('custom-template-css');
  if (existingCustomCss) existingCustomCss.remove();
  var existingCustomJs = document.getElementById('custom-template-js');
  if (existingCustomJs) existingCustomJs.remove();

  function loadBuiltinInline() {
    var builtinTpl = document.getElementById('tpl-' + templateId);
    if (!builtinTpl) {
      wlog('[TEMPLATE] No inline DOM for ' + templateId + ', falling back to ' + DEFAULT_TEMPLATE);
      var fb = document.getElementById('tpl-' + DEFAULT_TEMPLATE);
      if (!fb) return;
      builtinTpl = fb;
      templateId = DEFAULT_TEMPLATE;
    }
    wlog('[TEMPLATE] Built-in inline: loading ' + templateId);
    var cssLink = document.getElementById('template-css');
    if (!cssLink) {
      cssLink = document.createElement('link');
      cssLink.id = 'template-css';
      cssLink.rel = 'stylesheet';
      document.head.appendChild(cssLink);
    }
    document.adoptedStyleSheets = [];
    cssLink.disabled = false;
    cssLink.href = 'templates/' + templateId + '/style.css';
    builtinTpl.classList.add('active');

    var existing = document.getElementById('template-js');
    if (existing) existing.remove();
    var script = document.createElement('script');
    script.id = 'template-js';
    script.onload = markTemplateReady;
    script.src = 'templates/' + templateId + '/app.js';
    document.body.appendChild(script);
  }

  function loadCustom() {
    wlog('[TEMPLATE] Custom: loading ' + templateId);

    window.__TAURI__.core.invoke('read_template_files', { name: templateId })
      .then(function(files) {
        wlog('[TEMPLATE] Got HTML: ' + files.html.length + ' bytes, CSS: ' + files.css.length + ' bytes, JS: ' + files.js.length + ' bytes');
        wlog('[TEMPLATE] HTML preview: ' + files.html.substring(0, 200).replace(/\n/g, ' '));

        // Fully remove built-in stylesheet to prevent CSS conflicts
        var cssLink = document.getElementById('template-css');
        if (cssLink) { wlog('[TEMPLATE] Removing old built-in link: ' + cssLink.href); cssLink.remove(); }

        // Remove old built-in script
        var existing = document.getElementById('template-js');
        if (existing) existing.remove();

        // Inject HTML into DOM using a sandboxed parser to strip scripts/handlers.
        // Only allowed element types and safe attributes are copied.
        var container = document.createElement('div');
        container.className = 'template active';
        container.id = 'tpl-custom';
        var parsed = new DOMParser().parseFromString(files.html, 'text/html');
        var allowedTags = ['div','span','canvas','p','h1','h2','h3','h4','strong','em','b','i','br','hr'];
        var safeAttrs = ['id', 'class', 'style', 'width', 'height'];
        function importSafe(src, dest) {
          var children = src.childNodes;
          for (var ci = 0; ci < children.length; ci++) {
            var node = children[ci];
            if (node.nodeType === Node.TEXT_NODE) {
              dest.appendChild(document.createTextNode(node.textContent));
            } else if (node.nodeType === Node.ELEMENT_NODE) {
              var tag = node.tagName.toLowerCase();
              if (allowedTags.indexOf(tag) === -1) continue;
              var el = document.createElement(tag);
              for (var ai = 0; ai < node.attributes.length; ai++) {
                var attr = node.attributes[ai];
                if (safeAttrs.indexOf(attr.name) !== -1) {
                  el.setAttribute(attr.name, attr.value);
                }
              }
              importSafe(node, el);
              dest.appendChild(el);
            }
          }
        }
        importSafe(parsed.body, container);
        wlog('[TEMPLATE] Sanitized container innerHTML length: ' + container.innerHTML.length);
        document.body.insertBefore(container, document.querySelector('script'));

        // Inject CSS and JS via Rust-side WebviewWindow::eval() — bypasses the
        // asset protocol scope and CSP entirely. See templates.rs::inject_custom_template.
        return window.__TAURI__.core.invoke('inject_custom_template', { name: templateId });
      })
      .then(function() {
        wlog('[TEMPLATE] Custom CSS/JS injected via inject_custom_template');
        document.title = 'Ready: ' + templateId;
        markTemplateReady();
      })
      .catch(function(err) {
        wlog('[TEMPLATE] Custom template load FAILED: ' + err);
        document.title = 'FAILED: ' + templateId + ' — ' + err;
        // Fallback to default built-in template
        var fallback = document.getElementById('tpl-' + DEFAULT_TEMPLATE);
        if (fallback) {
          var cssLink = document.getElementById('template-css');
          if (!cssLink) {
            cssLink = document.createElement('link');
            cssLink.id = 'template-css';
            cssLink.rel = 'stylesheet';
            document.head.appendChild(cssLink);
          }
          cssLink.disabled = false;
          cssLink.href = 'templates/' + DEFAULT_TEMPLATE + '/style.css';
          fallback.classList.add('active');
          var script = document.createElement('script');
          script.id = 'template-js';
          script.onload = markTemplateReady;
          script.src = 'templates/' + DEFAULT_TEMPLATE + '/app.js';
          document.body.appendChild(script);
        }
      });
  }

  // Prefer a user-modified template in AppData over the inline built-in DOM.
  // Only fall back to the inline built-in when no user-dir template exists.
  window.__TAURI__.core.invoke('user_template_exists', { name: templateId })
    .then(function(exists) {
      wlog('[TEMPLATE] user_template_exists("' + templateId + '") = ' + exists);
      if (exists) {
        loadCustom();
      } else {
        wlog('[TEMPLATE] No user template found, using built-in inline DOM');
        loadBuiltinInline();
      }
    })
    .catch(function(err) {
      wlog('[TEMPLATE] user_template_exists error: ' + err + ', falling back to built-in');
      loadBuiltinInline();
    });
}

function initTemplate(attempt) {
  if (attempt === undefined) attempt = 0;

  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke('get_config').then(function(cfg) {
      var theme = cfg.config.THEME || DEFAULT_TEMPLATE;
      wlog('[TEMPLATE] initTemplate got THEME=' + theme);
      activateTemplate(theme);
    }).catch(function() {
      activateTemplate(DEFAULT_TEMPLATE);
    });
  } else if (attempt < 50) {
    setTimeout(function() { initTemplate(attempt + 1); }, 100);
  } else {
    activateTemplate(DEFAULT_TEMPLATE);
  }
}

initTemplate();
