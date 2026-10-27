'use strict';
(function () {
  var $ = function (id) { return document.getElementById(id); };
  var state = { bundle: null, filter: null };

  // `server`: POST the document to /convert. `wasm`: convert in this page with the WebAssembly
  // build (the hosted demo); the document never leaves the browser.
  var engineMeta = document.querySelector('meta[name=engine]');
  var engine = engineMeta && engineMeta.content === 'wasm' ? 'wasm' : 'server';
  var wasmReady = null;
  function loadWasm() {
    if (!wasmReady) {
      wasmReady = import('./wasm/ccda2fhir_wasm.js').then(function (m) {
        return m.default().then(function () { return m; });
      });
    }
    return wasmReady;
  }
  function convertLocal(xml) {
    return loadWasm().then(function (m) {
      try { render(JSON.parse(m.convert_ccda(xml, $('profiles').value))); }
      catch (e) { showError(String((e && e.message) || e)); }
    }, function () { showError('Could not load the converter module.'); });
  }
  if (engine === 'wasm') {
    $('token-label').hidden = true;
    $('sub').textContent = 'C-CDA to FHIR R4, running entirely in your browser. Your document is never uploaded.';
  }

  function setBusy(on) { $('go').disabled = on; $('busy').hidden = !on; }
  function showError(text) {
    $('empty').hidden = true; $('result').hidden = true;
    $('error').hidden = false; $('error').textContent = text;
  }
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function label(r) {
    var pick = function (cc) {
      if (!cc) return null;
      if (cc.text) return cc.text;
      var c = (cc.coding || []).find(function (x) { return x.display || x.code; });
      return c ? (c.display || c.code) : null;
    };
    if (r.resourceType === 'Patient' || r.resourceType === 'Practitioner' || r.resourceType === 'RelatedPerson') {
      var n = (r.name || [])[0];
      if (n) return [].concat(n.given || [], n.family ? [n.family] : []).join(' ') || n.text || '';
    }
    if (r.resourceType === 'Organization' || r.resourceType === 'Location') return r.name || '';
    return pick(r.code) || pick(r.vaccineCode) || pick(r.medicationCodeableConcept) || pick(r.type && r.type[0]) ||
      r.title || r.description || (r.name && typeof r.name === 'string' ? r.name : '') || '';
  }

  function render(data) {
    var bundle = data.bundle, entries = bundle.entry || [];
    state.bundle = bundle; state.filter = null;
    $('empty').hidden = true; $('error').hidden = true; $('result').hidden = false;

    var pill = $('status');
    pill.textContent = data.status === 'complete' ? 'Complete' : 'Partial';
    pill.className = 'pill ' + data.status;
    $('counts').textContent = entries.length + ' resources, ' + data.issues.warnings + ' warnings, ' +
      (data.issues.total - data.issues.warnings) + ' notes';

    var counts = {};
    entries.forEach(function (e) { var t = e.resource.resourceType; counts[t] = (counts[t] || 0) + 1; });
    var chips = $('chips'); chips.textContent = '';
    Object.keys(counts).sort().forEach(function (t) {
      var b = el('button', 'chip', t + ' ' + counts[t]);
      b.type = 'button'; b.setAttribute('aria-pressed', 'false');
      b.addEventListener('click', function () {
        state.filter = state.filter === t ? null : t;
        Array.prototype.forEach.call(chips.children, function (c) { c.setAttribute('aria-pressed', 'false'); });
        if (state.filter) b.setAttribute('aria-pressed', 'true');
        renderList();
      });
      chips.appendChild(b);
    });
    renderList();

    var body = $('issues').tBodies[0]; body.textContent = '';
    data.report.forEach(function (i) {
      var tr = el('tr');
      tr.appendChild(el('td', null)).appendChild(el('span', 'sev ' + i.severity, i.severity));
      tr.appendChild(el('td', 'code-cell', i.code));
      tr.appendChild(el('td', null, i.message));
      body.appendChild(tr);
    });
    $('no-issues').hidden = data.report.length > 0;
    $('issues').hidden = data.report.length === 0;
    $('json').textContent = JSON.stringify(bundle, null, 2);
    selectTab('resources');
  }

  function renderList() {
    var list = $('list'); list.textContent = ''; $('detail').hidden = true;
    (state.bundle.entry || []).forEach(function (e) {
      var r = e.resource;
      if (state.filter && r.resourceType !== state.filter) return;
      var li = el('li'), b = el('button');
      b.type = 'button';
      b.appendChild(el('span', 't', r.resourceType));
      b.appendChild(el('span', 'l', label(r) || r.id || ''));
      b.addEventListener('click', function () {
        Array.prototype.forEach.call(list.querySelectorAll('button'), function (x) { x.removeAttribute('aria-current'); });
        b.setAttribute('aria-current', 'true');
        $('detail').textContent = JSON.stringify(r, null, 2); $('detail').hidden = false;
      });
      li.appendChild(b); list.appendChild(li);
    });
  }

  function selectTab(name) {
    Array.prototype.forEach.call(document.querySelectorAll('.tabs button'), function (b) {
      b.setAttribute('aria-selected', String(b.getAttribute('data-tab') === name));
    });
    ['resources', 'report', 'json'].forEach(function (t) { $('tab-' + t).hidden = t !== name; });
  }

  function convert() {
    var xml = $('xml').value.trim();
    if (!xml) { showError('Choose a file or paste a C-CDA document first.'); return; }
    setBusy(true);
    if (engine === 'wasm') { convertLocal(xml).then(function () { setBusy(false); }); return; }
    var headers = { 'Content-Type': 'application/xml' };
    var token = $('token').value.trim();
    if (token) headers.Authorization = 'Bearer ' + token;
    fetch('/convert?report=true&profiles=' + encodeURIComponent($('profiles').value), {
      method: 'POST', headers: headers, body: xml
    }).then(function (res) {
      return res.text().then(function (text) {
        var body = null;
        try { body = JSON.parse(text); } catch (e) { /* not JSON */ }
        if (!res.ok) {
          var msg = body && body.issue && body.issue[0] && body.issue[0].diagnostics;
          if (res.status === 401) msg = (msg || 'Not authorised') + ' Enter a bearer token above.';
          showError('HTTP ' + res.status + (msg ? ': ' + msg : ''));
          return;
        }
        if (!body || !body.bundle) { showError('The server answered with something unexpected.'); return; }
        render(body);
      });
    }).catch(function () {
      showError('Could not reach the server.');
    }).then(function () { setBusy(false); });
  }

  function loadFile(f) {
    if (!f) return;
    var reader = new FileReader();
    reader.onload = function () { $('xml').value = String(reader.result); };
    reader.readAsText(f);
  }

  $('go').addEventListener('click', convert);
  $('clear').addEventListener('click', function () {
    $('xml').value = ''; $('file').value = ''; state.bundle = null;
    $('result').hidden = true; $('error').hidden = true; $('empty').hidden = false;
  });
  $('file').addEventListener('change', function (e) { loadFile(e.target.files[0]); });
  var drop = $('drop');
  ['dragenter', 'dragover'].forEach(function (n) {
    drop.addEventListener(n, function (e) { e.preventDefault(); drop.classList.add('over'); });
  });
  ['dragleave', 'drop'].forEach(function (n) {
    drop.addEventListener(n, function (e) { e.preventDefault(); drop.classList.remove('over'); });
  });
  drop.addEventListener('drop', function (e) { loadFile(e.dataTransfer.files[0]); });
  Array.prototype.forEach.call(document.querySelectorAll('.tabs button'), function (b) {
    b.addEventListener('click', function () { selectTab(b.getAttribute('data-tab')); });
  });
  $('copy').addEventListener('click', function () {
    if (navigator.clipboard) navigator.clipboard.writeText($('json').textContent);
  });
  $('download').addEventListener('click', function () {
    var blob = new Blob([$('json').textContent], { type: 'application/fhir+json' });
    var a = document.createElement('a');
    a.href = URL.createObjectURL(blob); a.download = 'bundle.fhir.json';
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(function () { URL.revokeObjectURL(a.href); }, 1000);
  });
})();
