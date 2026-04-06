// simpbro JS polyfill - bridges native __dom_* functions to standard Web APIs

class Element {
    constructor(nodeId) {
        this.__id = nodeId;
    }
    getAttribute(name) { return __dom_get_attr(this.__id, name); }
    setAttribute(name, value) { __dom_set_attr(this.__id, name, String(value)); }
    hasAttribute(name) { return __dom_get_attr(this.__id, name) !== null; }
    removeAttribute(name) { __dom_remove_attr(this.__id, name); }

    appendChild(child) { __dom_append_child(this.__id, child.__id); return child; }
    removeChild(child) { __dom_remove_child(this.__id, child.__id); return child; }
    insertBefore(newChild, refChild) {
        __dom_insert_before(this.__id, newChild.__id, refChild ? refChild.__id : -1);
        return newChild;
    }
    replaceChild(newChild, oldChild) {
        this.insertBefore(newChild, oldChild);
        this.removeChild(oldChild);
        return oldChild;
    }
    cloneNode(deep) { return new Element(__dom_clone_node(this.__id, !!deep)); }
    contains(other) {
        let n = other;
        while (n) { if (n.__id === this.__id) return true; n = n.parentNode; }
        return false;
    }

    get id() { return this.getAttribute('id') || ''; }
    set id(v) { this.setAttribute('id', v); }
    get className() { return this.getAttribute('class') || ''; }
    set className(v) { this.setAttribute('class', v); }
    get tagName() { return __dom_get_tag(this.__id).toUpperCase(); }
    get nodeName() { return this.tagName; }
    get nodeType() { return __dom_get_node_type(this.__id); }
    get nodeValue() { return this.nodeType === 3 ? this.textContent : null; }

    get textContent() { return __dom_get_text_content(this.__id); }
    set textContent(v) { __dom_set_text_content(this.__id, String(v)); }
    get innerHTML() { return __dom_get_inner_html(this.__id); }
    set innerHTML(v) { __dom_set_inner_html(this.__id, String(v)); }
    get outerHTML() { return __dom_get_outer_html(this.__id); }

    get children() { return __dom_get_children(this.__id).map(id => new Element(id)); }
    get childNodes() { return __dom_get_child_nodes(this.__id).map(id => new Element(id)); }
    get firstChild() {
        const kids = __dom_get_child_nodes(this.__id);
        return kids.length > 0 ? new Element(kids[0]) : null;
    }
    get lastChild() {
        const kids = __dom_get_child_nodes(this.__id);
        return kids.length > 0 ? new Element(kids[kids.length - 1]) : null;
    }
    get firstElementChild() {
        const kids = __dom_get_children(this.__id);
        return kids.length > 0 ? new Element(kids[0]) : null;
    }
    get parentNode() {
        const pid = __dom_get_parent(this.__id);
        return pid >= 0 ? new Element(pid) : null;
    }
    get parentElement() { return this.parentNode; }
    get nextSibling() {
        const pid = __dom_get_parent(this.__id);
        if (pid < 0) return null;
        const sibs = __dom_get_child_nodes(pid);
        const idx = sibs.indexOf(this.__id);
        return idx >= 0 && idx < sibs.length - 1 ? new Element(sibs[idx + 1]) : null;
    }
    get previousSibling() {
        const pid = __dom_get_parent(this.__id);
        if (pid < 0) return null;
        const sibs = __dom_get_child_nodes(pid);
        const idx = sibs.indexOf(this.__id);
        return idx > 0 ? new Element(sibs[idx - 1]) : null;
    }
    get ownerDocument() { return document; }

    querySelector(sel) {
        const id = __dom_query_selector(this.__id, sel);
        return id >= 0 ? new Element(id) : null;
    }
    querySelectorAll(sel) {
        return __dom_query_selector_all(this.__id, sel).map(id => new Element(id));
    }
    getElementsByTagName(tag) {
        return __dom_get_by_tag(this.__id, tag).map(id => new Element(id));
    }
    getElementsByClassName(cls) {
        return __dom_get_by_class(this.__id, cls).map(id => new Element(id));
    }
    matches(sel) { return __dom_matches(this.__id, sel); }

    get classList() {
        const self = this;
        return {
            add(...cls) { const c = new Set((self.className || '').split(/\s+/).filter(Boolean)); cls.forEach(x => c.add(x)); self.className = [...c].join(' '); },
            remove(...cls) { const c = new Set((self.className || '').split(/\s+/).filter(Boolean)); cls.forEach(x => c.delete(x)); self.className = [...c].join(' '); },
            contains(c) { return (self.className || '').split(/\s+/).includes(c); },
            toggle(c) { if (this.contains(c)) { this.remove(c); return false; } else { this.add(c); return true; } },
            get length() { return (self.className || '').split(/\s+/).filter(Boolean).length; },
        };
    }

    get style() {
        if (!this.__style) this.__style = new Proxy({}, { set: () => true, get: () => '' });
        return this.__style;
    }
    get dataset() {
        const self = this;
        return new Proxy({}, {
            get(_, key) { return self.getAttribute('data-' + key.replace(/([A-Z])/g, '-$1').toLowerCase()); },
            set(_, key, val) { self.setAttribute('data-' + key.replace(/([A-Z])/g, '-$1').toLowerCase(), val); return true; },
        });
    }

    // Events (stored JS-side)
    addEventListener(event, handler, opts) {
        if (!this.__listeners) this.__listeners = {};
        if (!this.__listeners[event]) this.__listeners[event] = [];
        this.__listeners[event].push(handler);
    }
    removeEventListener(event, handler) {
        if (this.__listeners && this.__listeners[event])
            this.__listeners[event] = this.__listeners[event].filter(h => h !== handler);
    }
    dispatchEvent(event) {
        if (this.__listeners && this.__listeners[event.type])
            this.__listeners[event.type].forEach(h => { try { h(event); } catch(e) { console.error(e); } });
    }

    // Form
    get value() { return this.getAttribute('value') || ''; }
    set value(v) { this.setAttribute('value', v); }
    get type() { return this.getAttribute('type') || ''; }
    get name() { return this.getAttribute('name') || ''; }
    get href() { return this.getAttribute('href') || ''; }
    set href(v) { this.setAttribute('href', v); }
    get src() { return this.getAttribute('src') || ''; }
    set src(v) { this.setAttribute('src', v); }
    get disabled() { return this.hasAttribute('disabled'); }
    set disabled(v) { if (v) this.setAttribute('disabled',''); else this.removeAttribute('disabled'); }

    // Geometry stubs
    getBoundingClientRect() { return { top:0, left:0, bottom:0, right:0, width:0, height:0, x:0, y:0 }; }
    get offsetWidth() { return 0; }
    get offsetHeight() { return 0; }
    get clientWidth() { return 0; }
    get clientHeight() { return 0; }
    get scrollTop() { return 0; }
    set scrollTop(v) {}
    get scrollLeft() { return 0; }
    set scrollLeft(v) {}
    scrollIntoView() {}
    focus() {}
    blur() {}
    click() {}

    toString() { return `[Element ${this.tagName}]`; }
}

// Document (extends Element for node 0)
const document = new Element(__dom_document_id());
document.createElement = function(tag) { return new Element(__dom_create_element(tag)); };
document.createTextNode = function(text) { return new Element(__dom_create_text_node(String(text))); };
document.createDocumentFragment = function() { return new Element(__dom_create_element('__fragment')); };
document.createComment = function() { return new Element(__dom_create_text_node('')); };
document.getElementById = function(id) { const n = __dom_get_element_by_id(id); return n >= 0 ? new Element(n) : null; };
document.getElementsByTagName = function(tag) { return __dom_get_by_tag(0, tag).map(id => new Element(id)); };
document.getElementsByClassName = function(cls) { return __dom_get_by_class(0, cls).map(id => new Element(id)); };
document.createEvent = function(type) { return { type, initEvent(t) { this.type = t; } }; };

Object.defineProperty(document, 'body', {
    get() { const id = __dom_body(); return id >= 0 ? new Element(id) : null; }
});
Object.defineProperty(document, 'head', {
    get() { const id = __dom_head(); return id >= 0 ? new Element(id) : null; }
});
Object.defineProperty(document, 'documentElement', {
    get() {
        const kids = __dom_get_children(0);
        for (const kid of kids) { if (__dom_get_tag(kid) === 'html') return new Element(kid); }
        return null;
    }
});
Object.defineProperty(document, 'readyState', { get() { return 'complete'; } });
Object.defineProperty(document, 'cookie', { get() { return ''; }, set(v) {} });
Object.defineProperty(document, 'title', {
    get() {
        const t = document.querySelector('title');
        return t ? t.textContent : '';
    },
    set(v) {
        let t = document.querySelector('title');
        if (!t) { t = document.createElement('title'); document.head?.appendChild(t); }
        t.textContent = v;
    }
});

// Window
const window = globalThis;
window.document = document;
window.self = window;
window.top = window;
window.parent = window;
window.frames = window;

// Location
window.location = { href: __page_url || '' };
try {
    const _u = new URL(window.location.href);
    Object.assign(window.location, {
        hostname: _u.hostname, pathname: _u.pathname, search: _u.search,
        hash: _u.hash, origin: _u.origin, protocol: _u.protocol, host: _u.host, port: _u.port,
    });
} catch(e) {}
window.location.replace = function(url) { window.location.href = url; };
window.location.assign = function(url) { window.location.href = url; };
window.location.reload = function() {};
window.location.toString = function() { return this.href; };

// Navigator
window.navigator = {
    userAgent: 'simpbro/0.1', language: 'en', languages: ['en'],
    platform: 'simpbro', cookieEnabled: false, onLine: true,
    sendBeacon: function() { return true; },
};

// Console
const console = {
    log(...args) { __console_log(args.map(String).join(' ')); },
    warn(...args) { __console_log('[WARN] ' + args.map(String).join(' ')); },
    error(...args) { __console_log('[ERROR] ' + args.map(String).join(' ')); },
    info(...args) { __console_log(args.map(String).join(' ')); },
    debug() {}, trace() {}, dir() {}, table() {}, group() {}, groupEnd() {},
    time() {}, timeEnd() {}, count() {}, countReset() {},
    assert(cond, ...args) { if (!cond) console.error('Assertion failed:', ...args); },
};
window.console = console;

// Timers
let __timer_id = 0;
const __timers = {};
window.setTimeout = function(fn, ms) {
    if (typeof fn === 'string') { const code = fn; fn = () => eval(code); }
    const id = ++__timer_id;
    __timers[id] = { fn, ms: ms || 0, interval: false };
    __register_timer(id, ms || 0, false);
    return id;
};
window.setInterval = function(fn, ms) {
    if (typeof fn === 'string') { const code = fn; fn = () => eval(code); }
    const id = ++__timer_id;
    __timers[id] = { fn, ms: ms || 0, interval: true };
    __register_timer(id, ms || 0, true);
    return id;
};
window.clearTimeout = function(id) { delete __timers[id]; };
window.clearInterval = function(id) { delete __timers[id]; };
window.__fire_timer = function(id) {
    const t = __timers[id];
    if (!t) return;
    if (!t.interval) delete __timers[id];
    try { t.fn(); } catch(e) { console.error(e); }
};

// Fetch
window.fetch = function(url, options) {
    options = options || {};
    return new Promise((resolve, reject) => {
        try {
            const result = __fetch_sync(
                String(url), options.method || 'GET',
                JSON.stringify(options.headers || {}), options.body || ''
            );
            resolve({
                ok: result.status >= 200 && result.status < 300,
                status: result.status, statusText: '',
                url: result.url || String(url),
                headers: { get(n) { return (result.headers || {})[n] || null; } },
                text() { return Promise.resolve(result.body); },
                json() { return Promise.resolve(JSON.parse(result.body)); },
                blob() { return Promise.resolve(new Blob()); },
                arrayBuffer() { return Promise.resolve(new ArrayBuffer(0)); },
                clone() { return this; },
            });
        } catch(e) { reject(e); }
    });
};

// XMLHttpRequest
window.XMLHttpRequest = class {
    constructor() { this.readyState = 0; this.status = 0; this.responseText = ''; this._h = {}; }
    open(m, u) { this._m = m; this._u = u; this.readyState = 1; }
    setRequestHeader(n, v) { this._h[n] = v; }
    send(body) {
        try {
            const r = __fetch_sync(this._u, this._m || 'GET', JSON.stringify(this._h), body || '');
            this.status = r.status; this.responseText = r.body; this.response = r.body; this.readyState = 4;
            if (this.onload) this.onload();
            if (this.onreadystatechange) this.onreadystatechange();
        } catch(e) { if (this.onerror) this.onerror(e); }
    }
    getResponseHeader() { return null; }
    getAllResponseHeaders() { return ''; }
    abort() {}
};

// Storage stub
const _store = {};
window.localStorage = window.sessionStorage = {
    getItem(k) { return _store[k] ?? null; },
    setItem(k, v) { _store[k] = String(v); },
    removeItem(k) { delete _store[k]; },
    clear() { for (const k in _store) delete _store[k]; },
    get length() { return Object.keys(_store).length; },
};

// Browser API stubs
window.getComputedStyle = () => new Proxy({}, { get: () => '' });
window.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} });
window.requestAnimationFrame = fn => setTimeout(fn, 16);
window.cancelAnimationFrame = id => clearTimeout(id);
window.requestIdleCallback = fn => setTimeout(fn, 0);
window.cancelIdleCallback = id => clearTimeout(id);
window.addEventListener = function() {};
window.removeEventListener = function() {};
window.dispatchEvent = function() { return true; };
window.postMessage = function() {};
window.innerWidth = 1024; window.innerHeight = 768;
window.outerWidth = 1024; window.outerHeight = 768;
window.scrollX = 0; window.scrollY = 0;
window.pageXOffset = 0; window.pageYOffset = 0;
window.scrollTo = function() {};
window.scroll = function() {};
window.devicePixelRatio = 1;
window.screen = { width: 1024, height: 768, availWidth: 1024, availHeight: 768, colorDepth: 24 };
window.history = { length: 1, pushState() {}, replaceState() {}, go() {}, back() {}, forward() {}, state: null };
window.performance = { now() { return Date.now(); }, timing: {}, mark() {}, measure() {}, getEntriesByName() { return []; }, getEntriesByType() { return []; } };
window.crypto = { getRandomValues(a) { for (let i=0;i<a.length;i++) a[i]=Math.floor(Math.random()*256); return a; }, randomUUID() { return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g,c=>{const r=Math.random()*16|0;return(c==='x'?r:r&0x3|0x8).toString(16)}); } };
window.IntersectionObserver = class { observe(){} unobserve(){} disconnect(){} };
window.ResizeObserver = class { observe(){} unobserve(){} disconnect(){} };
window.MutationObserver = class { observe(){} disconnect(){} takeRecords(){ return []; } };
window.CustomEvent = class { constructor(t, o) { this.type = t; this.detail = o?.detail; this.bubbles = false; this.cancelable = false; } preventDefault(){} stopPropagation(){} };
window.Event = class { constructor(t) { this.type = t; } preventDefault(){} stopPropagation(){} };
window.Blob = class { constructor(p,o) { this.size = 0; this.type = o?.type || ''; } };
window.File = class extends Blob { constructor(p,n,o) { super(p,o); this.name = n; } };
window.FileReader = class { readAsText(){} readAsDataURL(){} };
window.FormData = class { constructor() { this._d = []; } append(k,v) { this._d.push([k,v]); } get(k) { const e = this._d.find(x=>x[0]===k); return e?e[1]:null; } };
window.Headers = class { constructor(i) { this._h = i || {}; } get(n) { return this._h[n]||null; } set(n,v) { this._h[n]=v; } };
window.AbortController = class { constructor() { this.signal = { aborted: false, addEventListener(){}, removeEventListener(){} }; } abort() { this.signal.aborted = true; } };
window.btoa = function(s) { return s; };
window.atob = function(s) { return s; };
window.queueMicrotask = function(fn) { Promise.resolve().then(fn); };
window.structuredClone = function(obj) { return JSON.parse(JSON.stringify(obj)); };

// DOMParser
window.DOMParser = class {
    parseFromString(html, type) {
        // Minimal: return a document-like object
        const div = document.createElement('div');
        div.innerHTML = html;
        return { documentElement: div, body: div, querySelector: s => div.querySelector(s), querySelectorAll: s => div.querySelectorAll(s) };
    }
};
