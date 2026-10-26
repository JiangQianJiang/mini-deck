'use strict';
const {test} = require('node:test');
const assert = require('node:assert/strict');
const {eventParser, recorded, mainKeys} = require('../web/protocol.js');

test('SSE 支持任意分片、CR/LF、注释、多行数据及多个事件', () => {
  const source = ': 注释\r\nevent: config\r\ndata: {"消息":"变化"}\r\ndata: next\r\n\r\nevent: config\ndata: {}\n\ndata: end\r\r';
  const expected = [{type: 'config', data: '{"消息":"变化"}\nnext'}, {type: 'config', data: '{}'}, {type: 'message', data: 'end'}];
  for (let cut = 0; cut <= source.length; cut++) {
    const received = [], parse = eventParser(event => received.push(event));
    parse(source.slice(0, cut)); parse(source.slice(cut)); assert.deepEqual(received, expected);
  }
  const received = [], parse = eventParser(event => received.push(event));
  const decoder = new TextDecoder(), bytes = new TextEncoder().encode(source);
  for (const byte of bytes) parse(decoder.decode(new Uint8Array([byte]), {stream: true}));
  assert.deepEqual(received, expected);
});
test('SSE 未完成事件不发布，超大事件有界拒绝', () => {
  const received = [], parse = eventParser(event => received.push(event));
  parse('event: config\ndata: {}\n'); assert.equal(received.length, 0);
  parse('\n'); assert.equal(received.length, 1);
  assert.throws(() => parse('data: ' + 'x'.repeat(17000)), /大小限制/);
});
test('录制范围与后端一致，Esc 和 Meta 可录入', () => {
  assert.deepEqual(recorded({key: 'Escape', ctrlKey: true, metaKey: true}), {mods: ['Ctrl', 'Win'], key: 'Esc'});
  assert.deepEqual(recorded({key: 'Shift', shiftKey: true}), {mods: ['Shift']});
  assert.deepEqual(recorded({key: '!', code: 'Digit1', shiftKey: true}), {mods: ['Shift'], key: '1'});
  for (const key of mainKeys) {
    const browserKey = {Esc: 'Escape', Space: ' ', Left: 'ArrowLeft', Right: 'ArrowRight', Up: 'ArrowUp', Down: 'ArrowDown'}[key] || key;
    assert.equal(recorded({key: browserKey}).key, key);
  }
  assert.ok(recorded({key: 'Unidentified'}).error);
});
test('长按与输入法组合不完成录制', () => {
  for (const event of [{key: 'a', repeat: true}, {key: 'a', isComposing: true}, {keyCode: 229}, {key: 'Process'}, {key: 'Dead'}]) {
    assert.deepEqual(recorded(event), {ignore: true});
  }
});

test('导航键录制保留修饰键，常见缩写可在编辑器恢复', () => {
  const {normalizeKey} = require('../web/protocol.js');
  for (const key of ['PageUp', 'PageDown', 'Home', 'End', 'Insert', 'Delete', 'Backspace']) {
    assert.ok(mainKeys.includes(key));
    assert.deepEqual(recorded({key, ctrlKey: true, shiftKey: true}), {mods: ['Ctrl', 'Shift'], key});
  }
  for (const [alias, key] of [['PgUp', 'PageUp'], ['pgdn', 'PageDown'], ['PgDown', 'PageDown'], ['ins', 'Insert'], ['DEL', 'Delete']]) assert.equal(normalizeKey(alias), key);
});
