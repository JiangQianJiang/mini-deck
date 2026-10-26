'use strict';
const storageKey = 'mini-deck-session-v2';
let token = '';
let mock = false;
const status = document.querySelector('#status');
const pair = document.querySelector('#pair');
const panel = document.querySelector('#panel');
const reconnect = document.querySelector('#reconnect');
function show(message) { status.textContent = message; }
function forget() { try { localStorage.removeItem(storageKey); } catch { /* 存储被禁用时仍允许临时使用。 */ } }
function reset() {
  token = ''; forget(); pair.hidden = false; panel.hidden = true; reconnect.hidden = true;
  document.querySelector('#grid').replaceChildren(); document.querySelector('#devices').replaceChildren();
}
async function api(path, method = 'GET', body) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetch(path, { method, signal: controller.signal, headers: {
      ...(token ? {Authorization: `Bearer ${token}`} : {}),
      ...(body ? {'Content-Type': 'application/json'} : {})
    }, body: body ? JSON.stringify(body) : undefined });
    if (!response.ok) {
      if (response.status === 401) reset();
      const data = await response.json().catch(() => ({}));
      throw new Error(data.error || `请求失败（${response.status}）`);
    }
    return response.status === 204 ? null : response.json();
  } catch (error) {
    if (error.name === 'AbortError') throw new Error('请求超时，执行状态未知；请检查电脑，避免重复触发');
    if (error instanceof TypeError) throw new Error('连接失败，请检查电脑和局域网');
    throw error;
  } finally { clearTimeout(timer); }
}
async function load() {
  const data = await api('/api/buttons');
  const grid = document.querySelector('#grid'); grid.replaceChildren();
  for (const item of data.buttons.sort((a,b) => a.position - b.position)) {
    const button = document.createElement('button');
    const name = document.createElement('strong'); name.textContent = item.name;
    const shortcut = document.createElement('small'); shortcut.textContent = item.shortcut;
    button.append(name, shortcut);
    button.addEventListener('click', async () => {
      button.disabled = true;
      try {
        if (document.querySelector('#edit').checked) {
          const name = prompt('按钮名称（最多 40 字）', item.name); if (name === null) return;
          const shortcut = prompt('快捷键，例如 Ctrl+Shift+Esc', item.shortcut); if (shortcut === null) return;
          await api(`/api/buttons/${item.id}`, 'PUT', {name, shortcut}); await load(); show('配置已保存');
        } else {
          await api(`/api/buttons/${item.id}/press`, 'POST');
          show(`${mock ? '模拟完成' : '已发送'}：${item.name}`);
        }
      } catch (error) { show(error.message); } finally { button.disabled = false; }
    }); grid.append(button);
  }
}
async function connect() {
  reconnect.disabled = true;
  try {
    mock = (await api('/api/health')).mock;
    await load(); pair.hidden = true; panel.hidden = false; reconnect.hidden = true;
    show(mock ? '已连接（模拟模式，不执行真实快捷键）' : '已连接');
  } catch (error) {
    show(error.message); reconnect.hidden = !token;
    throw error;
  } finally { reconnect.disabled = false; }
}
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
