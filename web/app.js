'use strict';
// 会话只驻留当前页面内存，刷新后重新配对，避免长期存储认证凭据。
let token = '';
let mock = false;
const status = document.querySelector('#status');
const pair = document.querySelector('#pair');
const panel = document.querySelector('#panel');
function show(message) { status.textContent = message; }
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
function reset() { token = ''; pair.hidden = false; panel.hidden = true; document.querySelector('#grid').replaceChildren(); }
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
        } else { await api(`/api/buttons/${item.id}/press`, 'POST'); show(`${mock ? "模拟完成" : "已发送"}：${item.name}`); }
      } catch (error) { show(error.message); } finally { button.disabled = false; }
    }); grid.append(button);
  }
}
pair.addEventListener('submit', async event => {
  event.preventDefault(); const button = pair.querySelector('button'); button.disabled = true;
  try {
    const result = await api('/api/pair', 'POST', {code: document.querySelector('#code').value.trim().toLowerCase()});
    token = result.token; mock = (await api('/api/health')).mock; document.querySelector('#code').value = ''; await load(); pair.hidden = true; panel.hidden = false; show(mock ? '已连接（模拟模式，不执行真实快捷键）' : '已连接');
  } catch (error) { show(error.message); } finally { button.disabled = false; }
});
document.querySelector('#logout').addEventListener('click', async () => {
  try { await api('/api/logout', 'POST'); reset(); show('已注销'); } catch (error) { show(error.message); }
});
