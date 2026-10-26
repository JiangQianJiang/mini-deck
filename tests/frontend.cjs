'use strict';
// 直接运行页面脚本，以可控的 DOM/网络验证异步时序；不替代真实浏览器兼容性测试。
const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
class Element extends EventTarget {
  constructor() { super(); this.children = []; this.value = ''; this.hidden = false; this.checked = false; this.open = false; }
  append(...items) { this.children.push(...items); }
  replaceChildren(...items) { this.children = items; }
  querySelector() { return this.button ||= new Element(); }
  showModal() { this.open = true; }
  close() { if (this.open) { this.open = false; this.dispatchEvent(new Event('close')); } }
  focus() {}
}
function page(fetch) {
  const nodes = new Map(), doc = new EventTarget(), win = new EventTarget();
  doc.hidden = false;
  doc.querySelector = id => { if (!nodes.has(id)) nodes.set(id, new Element()); return nodes.get(id); };
  doc.createElement = () => new Element();
  const context = vm.createContext({document: doc, window: win, fetch, AbortController, TextDecoder, setTimeout, clearTimeout, console,
    localStorage: {getItem() { return null; }, removeItem() {}, setItem() {}}, confirm: () => true});
  for (const file of ['protocol.js', 'app.js']) vm.runInContext(fs.readFileSync(`web/${file}`, 'utf8'), context);
  const run = code => vm.runInContext(code, context);
  run("token = 'a'.repeat(64)");
  return {run, nodes, doc, win};
}
const config = revision => ({buttons: [{id: 0, name: `版本${revision}`, shortcut: 'Ctrl+C', position: 0, revision}]});
const json = value => new Response(JSON.stringify(value), {headers: {'content-type': 'application/json'}});
const tick = () => new Promise(resolve => setImmediate(resolve));
async function until(predicate) { for (let i = 0; i < 100; i++) { if (predicate()) return; await tick(); } throw new Error('异步操作未完成'); }
function event(key, options = {}) { const value = new Event('keydown', {cancelable: true}); Object.assign(value, {key, ...options}); return value; }

test('较早快照与已注销会话的响应不能覆盖新状态', async () => {
  const pending = [], app = page(() => new Promise(resolve => pending.push(resolve)));
  const older = app.run('load()'), newer = app.run('load()');
  pending[1](json(config(2))); await newer;
  pending[0](json(config(1))); await older;
  assert.equal(app.run('currentButtons[0].revision'), 2);
  const late = app.run('load()'); app.run('reset()'); pending[2](json(config(3)));
  await assert.rejects(late, /会话已变化/); assert.equal(app.run('currentButtons.length'), 0);
});

test('SSE 沿用 Bearer，重连刷新快照，重复连接取消旧流，401 清理会话', async () => {
  let opens = 0, snapshots = 0, rejectAuth = false;
  const requests = [], signals = [];
  const app = page(async (url, options) => {
    requests.push({url, options});
    if (url === '/api/buttons') return json(config(++snapshots));
    opens++; signals.push(options.signal);
    if (rejectAuth) return new Response('{}', {status: 401});
    return new Response(new ReadableStream({start(controller) {
      controller.enqueue(new TextEncoder().encode('event: config\ndata: {}\n\n'));
      if (opens === 1) controller.close();
      else options.signal.addEventListener('abort', () => controller.close(), {once: true});
    }}), {headers: {'content-type': 'text/event-stream'}});
  });
  app.run('delay = async () => {}; startStream()');
  await until(() => opens === 2 && snapshots >= 2);
  assert.ok(requests.every(r => !r.url.includes('aaaa')));
  assert.ok(requests.every(r => r.options.headers.Authorization === `Bearer ${'a'.repeat(64)}`));
  app.run('startStream()'); await until(() => opens === 3);
  assert.equal(signals[1].aborted, true);
  rejectAuth = true; app.run('startStream()');
  await until(() => app.run("token === ''"));
  assert.equal(app.run('panel.hidden'), true);
  app.run('stopStream()');
});

test('自动重连有次数上限，不无限重试', async () => {
  let opens = 0;
  const app = page(async () => { opens++; return new Response('{}', {status: 503}); });
  app.run('delay = async () => {}; startStream()');
  await until(() => opens === 6 && app.nodes.get('#reconnect').hidden === false);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(opens, 6); assert.match(app.nodes.get('#status').textContent, /自动同步已停止/);
  app.run('stopStream()');
});

test('录制须主动开启，Esc 可录入，失焦/关闭清理监听，IME 不被截获', async () => {
  const app = page(async () => json(config(0)));
  app.run("openEditor({id:0,name:'复制',shortcut:'Ctrl+C',revision:0})");
  const idle = event('a'); app.win.dispatchEvent(idle); assert.equal(idle.defaultPrevented, false);
  app.nodes.get('#record').dispatchEvent(new Event('click'));
  const composing = event('a', {isComposing: true}); app.win.dispatchEvent(composing);
  assert.equal(composing.defaultPrevented, false); assert.equal(app.run('recording'), true);
  const escape = event('Escape', {metaKey: true}); app.win.dispatchEvent(escape);
  assert.equal(escape.defaultPrevented, true); assert.equal(app.run('shortcutValue()'), 'Win+Esc'); assert.equal(app.run('recording'), false);
  app.nodes.get('#record').dispatchEvent(new Event('click')); app.win.dispatchEvent(new Event('blur'));
  assert.equal(app.run('recording'), false);
  app.nodes.get('#record').dispatchEvent(new Event('click')); app.run('editor.close()');
  const closed = event('c'); app.win.dispatchEvent(closed); assert.equal(closed.defaultPrevented, false);
});

test('编辑冲突保留名称与快捷键草稿，不自动重提；点击冲突只刷新不重发按键', async () => {
  let saves = 0, presses = 0;
  const app = page(async (url, options) => {
    if (options.method === 'PUT') { saves++; return new Response('{"error":"版本冲突"}', {status: 409}); }
    if (url.endsWith('/press')) { presses++; return new Response('{"error":"版本冲突"}', {status: 409}); }
    return json(config(1));
  });
  app.run("openEditor({id:0,name:'复制',shortcut:'Ctrl+C',revision:0})");
  app.nodes.get('#button-name').value = '我的草稿'; app.nodes.get('#main-key').value = 'V';
  app.nodes.get('#editor-form').dispatchEvent(new Event('submit', {cancelable: true}));
  await until(() => saves === 1 && app.run('saving === false'));
  assert.equal(app.nodes.get('#button-name').value, '我的草稿'); assert.equal(app.run('shortcutValue()'), 'Ctrl+V');
  assert.equal(app.run('draft.revision'), 0); assert.equal(app.nodes.get('#rebase').hidden, false);
  app.run('editor.close()');
  const button = app.nodes.get('#grid').children[0]; button.dispatchEvent(new Event('click'));
  await until(() => presses === 1 && /未执行按键/.test(app.nodes.get('#status').textContent));
  assert.equal(presses, 1); assert.equal(saves, 1);
});

test('页面隐藏取消流，重新可见重新取完整配置，并停止录制', async () => {
  let opens = 0, snapshots = 0;
  const signals = [];
  const app = page(async (url, options) => {
    if (url === '/api/health') return json({mock: true});
    if (url === '/api/buttons') return json(config(++snapshots));
    opens++; signals.push(options.signal);
    return new Response(new ReadableStream({start(controller) {
      options.signal.addEventListener('abort', () => controller.close(), {once: true});
    }}), {headers: {'content-type': 'text/event-stream'}});
  });
  app.run('startStream()'); await until(() => opens === 1 && snapshots === 1);
  app.run("openEditor({id:0,name:'复制',shortcut:'Ctrl+C',revision:0})");
  app.nodes.get('#record').dispatchEvent(new Event('click'));
  app.doc.hidden = true; app.doc.dispatchEvent(new Event('visibilitychange'));
  assert.equal(signals[0].aborted, true); assert.equal(app.run('recording'), false);
  app.doc.hidden = false; app.doc.dispatchEvent(new Event('visibilitychange'));
  await until(() => opens === 2 && snapshots >= 2);
  assert.ok(app.run('currentButtons[0].revision') >= 2);
  app.run('stopStream(); editor.close()');
});
