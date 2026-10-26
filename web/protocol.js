'use strict';
// 流解析与按键映射保持纯函数，供浏览器和自动化测试共用。
const MiniDeckProtocol = (() => {
  function eventParser(emit) {
    let line = '', data = [], type = '', skipLF = false, size = 0;
    function endLine() {
      if (!line) {
        if (data.length) emit({type: type || 'message', data: data.join('\n')});
        data = []; type = ''; size = 0;
      } else if (!line.startsWith(':')) {
        const colon = line.indexOf(':');
        const field = colon < 0 ? line : line.slice(0, colon);
        let value = colon < 0 ? '' : line.slice(colon + 1);
        if (value.startsWith(' ')) value = value.slice(1);
        if (field === 'event') type = value;
        if (field === 'data') data.push(value);
      }
      line = '';
    }
    return chunk => {
      for (const char of chunk) {
        if (skipLF && char === '\n') { skipLF = false; continue; }
        skipLF = false;
        if (++size > 16384) throw new Error('同步事件超过大小限制');
        if (char === '\r' || char === '\n') { endLine(); skipLF = char === '\r'; }
        else line += char;
      }
    };
  }
  const modifiers = ['Ctrl', 'Alt', 'Shift', 'Win'];
  const mainKeys = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789', ...Array.from({length: 12}, (_, i) => `F${i + 1}`), 'Enter', 'Esc', 'Tab', 'Space', 'Left', 'Right', 'Up', 'Down'];
  function recorded(event) {
    if (event.repeat || event.isComposing || event.keyCode === 229 || event.key === 'Process' || event.key === 'Dead') return {ignore: true};
    const mods = modifiers.filter((_, i) => [event.ctrlKey, event.altKey, event.shiftKey, event.metaKey][i]);
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return {mods};
    const names = {Escape: 'Esc', ' ': 'Space', ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down'};
    let key = names[event.key] || event.key;
    if (/^[a-z]$/i.test(key)) key = key.toUpperCase();
    // 数字行加 Shift 后仍对应后端虚拟键 0–9。
    if (/^Digit[0-9]$/.test(event.code)) key = event.code.slice(5);
    return mainKeys.includes(key) ? {mods, key} : {mods, error: '不支持此按键，请选择列表中的主键'};
  }
  return {eventParser, recorded, modifiers, mainKeys};
})();
if (typeof module !== 'undefined') module.exports = MiniDeckProtocol;
