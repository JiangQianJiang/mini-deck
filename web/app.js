'use strict';
const storageKey = 'mini-deck-session-v2';
let token = '', mock = false, epoch = 0, loadSequence = 0, streamController = null;
let currentButtons = [], draft = null, recording = false, recordCleanup = null, saving = false;
const status = document.querySelector('#status');
const pair = document.querySelector('#pair');
const panel = document.querySelector('#panel');
const reconnect = document.querySelector('#reconnect');
const editor = document.querySelector('#editor');
const editorError = document.querySelector('#editor-error');
const mainKey = document.querySelector('#main-key');
const modifierInputs = MiniDeckProtocol.modifiers.map(name => {
  const label = document.createElement('label'), input = document.createElement('input');
  input.type = 'checkbox'; input.value = name; label.append(input, name);
  document.querySelector('#modifiers').append(label); return input;
});
for (const key of MiniDeckProtocol.mainKeys) {
  const option = document.createElement('option'); option.value = key; option.textContent = key; mainKey.append(option);
}
function show(message) { status.textContent = message; }
function forget() { try { localStorage.removeItem(storageKey); } catch { /* 存储禁用时仍允许临时使用。 */ } }
function stopStream() { streamController?.abort(); streamController = null; }
function stopRecording() {
  recording = false; recordCleanup?.(); recordCleanup = null;
  document.querySelector('#stop-record').hidden = true;
  document.querySelector('#record').disabled = saving;
  updatePreview();
}
function reset() {
  epoch++; loadSequence++; stopStream(); stopRecording(); editor.close(); draft = null;
  token = ''; currentButtons = []; forget(); pair.hidden = false; panel.hidden = true; reconnect.hidden = true;
  document.querySelector('#grid').replaceChildren(); document.querySelector('#devices').replaceChildren();
}
async function api(path, method = 'GET', body) {
  const requestEpoch = epoch, requestToken = token;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetch(path, { method, signal: controller.signal, headers: {
      ...(requestToken ? {Authorization: `Bearer ${requestToken}`} : {}),
      ...(body ? {'Content-Type': 'application/json'} : {})
    }, body: body ? JSON.stringify(body) : undefined });
    if (requestEpoch !== epoch) throw new Error('会话已变化，请重新操作');
    if (!response.ok) {
      if (response.status === 401) reset();
      const data = await response.json().catch(() => ({}));
      const error = new Error(data.error || `请求失败（${response.status}）`); error.status = response.status; throw error;
    }
    const data = response.status === 204 ? null : await response.json();
    if (requestEpoch !== epoch) throw new Error('会话已变化，请重新操作');
    return data;
  } catch (error) {
    if (error.name === 'AbortError') throw new Error('请求超时，执行状态未知；请检查电脑，避免重复触发');
    if (error instanceof TypeError) throw new Error('连接失败，请检查电脑和局域网');
    throw error;
  } finally { clearTimeout(timer); }
}
async function load() {
  const sequence = ++loadSequence, requestEpoch = epoch;
  const data = await api('/api/buttons');
  if (sequence !== loadSequence || requestEpoch !== epoch) return;
  const nextButtons = data.buttons.sort((a,b) => a.position - b.position);
  const changed = JSON.stringify(nextButtons) !== JSON.stringify(currentButtons);
  currentButtons = nextButtons;
  if (draft && currentButtons.find(b => b.id === draft.id)?.revision !== draft.revision) {
    editorError.textContent = '服务端配置已变化，草稿已保留。请核对最新配置后再确认保存。';
    document.querySelector('#rebase').hidden = false;
  }
  if (changed) render();
}
function render() {
  const grid = document.querySelector('#grid'); grid.replaceChildren();
  for (const item of currentButtons) {
    const button = document.createElement('button');
    const name = document.createElement('strong'); name.textContent = item.name;
    const shortcut = document.createElement('small'); shortcut.textContent = item.shortcut;
    button.append(name, shortcut);
    button.addEventListener('click', async () => {
      if (document.querySelector('#edit').checked) { openEditor(item); return; }
      button.disabled = true;
      try {
        await api(`/api/buttons/${item.id}/press`, 'POST', {revision: item.revision});
        show(`${mock ? '模拟完成' : '已发送'}：${item.name}`);
      } catch (error) {
        if (error.status === 409) {
          try { await load(); show('按钮已被其他设备修改，未执行按键。请核对更新后的按钮，再次点击确认。'); }
          catch { show('按钮版本冲突，未执行按键；刷新失败，请重新连接后确认。'); }
        } else show(error.message);
      } finally { button.disabled = false; }
    }); grid.append(button);
  }
}
function shortcutValue() { return [...modifierInputs.filter(i => i.checked).map(i => i.value), mainKey.value].join('+'); }
function updatePreview(mods) { document.querySelector('#preview').textContent = mods ? [...mods, '等待主键…'].join('+') : shortcutValue(); }
function openEditor(item) {
  stopRecording(); draft = {...item}; editorError.textContent = ''; document.querySelector('#rebase').hidden = true;
  document.querySelector('#button-name').value = item.name;
  const parts = item.shortcut.split('+').map(s => s.trim().toUpperCase());
  for (const input of modifierInputs) input.checked = parts.includes(input.value.toUpperCase());
  mainKey.value = MiniDeckProtocol.normalizeKey(parts.at(-1));
  updatePreview(); editor.showModal(); document.querySelector('#button-name').focus();
}
for (const input of [...modifierInputs, mainKey]) input.addEventListener('change', () => { stopRecording(); updatePreview(); });
document.querySelector('#record').addEventListener('click', () => {
  stopRecording(); recording = true; editorError.textContent = '请按组合键；使用“取消录制”退出录制。';
  document.querySelector('#stop-record').hidden = false; document.querySelector('#record').disabled = true;
  document.querySelector('#stop-record').focus();
  const keydown = event => {
    if (!recording) return;
    if (event.isComposing || event.keyCode === 229 || ['Process', 'Dead'].includes(event.key)) return;
    if (event.cancelable) event.preventDefault(); event.stopImmediatePropagation();
    const result = MiniDeckProtocol.recorded(event);
    if (result.ignore) return;
    if (result.error) { editorError.textContent = result.error; return; }
    updatePreview(result.mods);
    if (!result.key) return;
    for (const input of modifierInputs) input.checked = result.mods.includes(input.value);
    mainKey.value = result.key; stopRecording(); editorError.textContent = '已录入，请核对预览后保存。';
    document.querySelector('#record').focus();
  };
  const keyup = event => {
    if (event.cancelable) event.preventDefault(); event.stopImmediatePropagation();
    const result = MiniDeckProtocol.recorded(event); if (!result.ignore) updatePreview(result.mods || []);
  };
  window.addEventListener('keydown', keydown, true); window.addEventListener('keyup', keyup, true);
  recordCleanup = () => { window.removeEventListener('keydown', keydown, true); window.removeEventListener('keyup', keyup, true); };
});
document.querySelector('#stop-record').addEventListener('click', () => { stopRecording(); editorError.textContent = '已取消录制'; document.querySelector('#record').focus(); });
window.addEventListener('blur', () => { if (recording) { stopRecording(); editorError.textContent = '页面失焦，录制已停止'; } });
editor.addEventListener('cancel', event => { if (recording || saving) event.preventDefault(); });
editor.addEventListener('close', () => {
  stopRecording();
  const index = currentButtons.findIndex(button => button.id === draft?.id);
  document.querySelector('#grid').children[index]?.focus(); draft = null;
});
document.querySelector('#cancel-editor').addEventListener('click', () => { if (!saving) editor.close(); });
document.querySelector('#rebase').addEventListener('click', async () => {
  try {
    await load(); if (!draft) return;
    const latest = currentButtons.find(b => b.id === draft.id);
    if (!confirm(`当前配置：${latest.name} / ${latest.shortcut}。保留你的草稿，并基于此版本再次保存？`)) return;
    draft.revision = latest.revision; document.querySelector('#rebase').hidden = true;
    editorError.textContent = '已采用最新版本，请核对草稿后点击保存。';
  } catch (error) { editorError.textContent = error.message; }
});
document.querySelector('#editor-form').addEventListener('submit', async event => {
  event.preventDefault(); if (!draft || saving || recording) return;
  const submitted = {...draft}, requestEpoch = epoch;
  const name = document.querySelector('#button-name').value.trim();
  if (!name || [...name].length > 40) { editorError.textContent = '名称须为 1–40 字'; return; }
  saving = true; document.querySelector('#save-button').disabled = true; document.querySelector('#record').disabled = true;
  try {
    await api(`/api/buttons/${submitted.id}`, 'PUT', {name, shortcut: shortcutValue(), revision: submitted.revision});
    if (requestEpoch !== epoch) return;
    editor.close(); await load(); show('配置已保存');
  } catch (error) {
    if (requestEpoch !== epoch) return;
    editorError.textContent = error.message;
    if (error.status === 409) {
      document.querySelector('#rebase').hidden = false;
      try { await load(); } catch { editorError.textContent = '配置冲突，草稿已保留；同步失败，请稍后重试。'; }
    }
  } finally { saving = false; document.querySelector('#save-button').disabled = false; document.querySelector('#record').disabled = false; }
});
function delay(ms, signal) {
  return new Promise(resolve => {
    const done = () => { clearTimeout(timer); signal.removeEventListener('abort', done); resolve(); };
    const timer = setTimeout(done, ms); signal.addEventListener('abort', done, {once: true});
    if (signal.aborted) done();
  });
}
function startStream() {
  stopStream();
  if (!token || document.hidden) return;
  const controller = new AbortController(), streamEpoch = epoch, streamToken = token;
  streamController = controller;
  (async () => {
    for (let attempt = 0; attempt <= 5 && !controller.signal.aborted; attempt++) {
      let reader, refreshError;
      const attemptController = new AbortController();
      const abortAttempt = () => attemptController.abort();
      controller.signal.addEventListener('abort', abortAttempt, {once: true});
      const handshake = setTimeout(abortAttempt, 8000);
      try {
        const response = await fetch('/api/events', {headers: {Authorization: `Bearer ${streamToken}`}, signal: attemptController.signal});
        clearTimeout(handshake);
        if (controller.signal.aborted || epoch !== streamEpoch) return;
        if (response.status === 401) { reset(); show('认证已失效，请重新配对'); return; }
        if (!response.ok || !response.headers.get('content-type')?.startsWith('text/event-stream')) throw new Error('同步连接失败');
        // 每次重连都先读取完整快照。通知合并为一个待执行刷新，避免慢客户端积压。
        reader = response.body.getReader();
        await load();
        let dirty = false, refreshing = false;
        const refresh = async () => {
          dirty = true; if (refreshing) return; refreshing = true;
          try {
            while (dirty && !controller.signal.aborted && epoch === streamEpoch) { dirty = false; await load(); }
          } catch (error) { refreshError = error; await reader.cancel().catch(() => {}); }
          finally { refreshing = false; }
        };
        const parse = MiniDeckProtocol.eventParser(event => { if (event.type === 'config') void refresh(); });
        const decoder = new TextDecoder();
        while (!controller.signal.aborted) {
          const {value, done} = await reader.read();
          if (done) break;
          parse(decoder.decode(value, {stream: true}));
        }
        if (!controller.signal.aborted) throw refreshError || new Error('同步连接已断开');
      } catch (error) {
        if (controller.signal.aborted || epoch !== streamEpoch) return;
        if (attempt === 5) { show('自动同步已停止。请点击“重新连接”，确认配置后操作。'); reconnect.hidden = false; return; }
        show('同步连接中断，正在重连…');
        await delay(1000 * 2 ** attempt, controller.signal);
      } finally {
        clearTimeout(handshake); controller.signal.removeEventListener('abort', abortAttempt); attemptController.abort();
        if (reader) { await reader.cancel().catch(() => {}); reader.releaseLock(); }
      }
    }
  })().catch(error => { if (!controller.signal.aborted) show(error.message); });
}
async function connect() {
  reconnect.disabled = true; const requestEpoch = epoch;
  try {
    mock = (await api('/api/health')).mock; await load();
    if (requestEpoch !== epoch) return;
    pair.hidden = true; panel.hidden = false; reconnect.hidden = true; startStream();
    show(mock ? '已连接（模拟模式，不执行真实快捷键）' : '已连接');
  } catch (error) { show(error.message); reconnect.hidden = !token; throw error; }
  finally { reconnect.disabled = false; }
}
document.addEventListener('visibilitychange', () => {
  if (document.hidden) { stopStream(); if (recording) stopRecording(); }
  else if (token) connect().catch(() => {});
});
window.addEventListener('pagehide', () => { stopStream(); stopRecording(); });
window.addEventListener('pageshow', event => { if (event.persisted && token) connect().catch(() => {}); });
pair.addEventListener('submit', async event => {
  event.preventDefault(); const button = pair.querySelector('button'); button.disabled = true;
  try {
    const remember = document.querySelector('#remember').checked;
    const result = await api('/api/pair', 'POST', {
      code: document.querySelector('#code').value.trim().toLowerCase(),
      remember, name: document.querySelector('#device-name').value.trim()
    });
    token = result.token; forget();
    let stored = true;
    if (remember) { try { localStorage.setItem(storageKey, token); } catch { stored = false; } }
    document.querySelector('#code').value = ''; await connect();
    if (!stored) show('已连接，但浏览器禁止保存凭据；刷新后需重新配对');
  } catch (error) { show(error.message); } finally { button.disabled = false; }
});
reconnect.addEventListener('click', () => { connect().catch(() => {}); });
document.querySelector('#logout').addEventListener('click', async () => {
  try { await api('/api/logout', 'POST'); reset(); show('已注销并撤销当前设备'); } catch (error) { show(error.message); }
});
async function devices() {
  const items = await api('/api/devices');
  const list = document.querySelector('#devices'); list.replaceChildren();
  for (const item of items) {
    const row = document.createElement('div'); row.className = 'device';
    const name = document.createElement('span');
    name.textContent = `${item.name}${item.current ? '（当前）' : ''} · ${item.remembered ? '记住设备' : '临时会话'} · 到期 ${new Date(item.expires_at * 1000).toLocaleDateString()}`;
    const button = document.createElement('button'); button.textContent = '撤销';
    button.addEventListener('click', async () => {
      if (!confirm(`撤销设备“${item.name}”？`)) return;
      button.disabled = true;
      try {
        await api(`/api/devices/${item.id}`, 'DELETE');
        if (item.current) reset(); else await devices();
        show('设备已撤销');
      } catch (error) { show(error.message); } finally { button.disabled = false; }
    }); row.append(name, button); list.append(row);
  }
}
document.querySelector('#refresh-devices').addEventListener('click', () => { devices().catch(error => show(error.message)); });
document.querySelector('#device-manager').addEventListener('toggle', event => {
  if (event.target.open) devices().catch(error => show(error.message));
});
try { const saved = localStorage.getItem(storageKey); if (/^[a-f0-9]{64}$/.test(saved || '')) token = saved; } catch { /* 允许浏览器禁用存储。 */ }
if (token) { show('正在恢复已记住的设备…'); connect().catch(() => {}); }
