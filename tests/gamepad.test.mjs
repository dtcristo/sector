import test from "node:test";
import assert from "node:assert/strict";
import { GamepadReader, installGamepadControls } from "../wasm/gamepad.mjs";

function pad({ index = 0, mapping = "standard", axes = [0, 0, 0, 0], count = 17, pressed = [] } = {}) {
  return { index, mapping, axes, connected: true,
    buttons: Array.from({ length: count }, (_, index) => ({ pressed: pressed.includes(index) })) };
}

test("standard pad preserves sticks and binds A B Y L3 Select and bumpers", () => {
  const reader = new GamepadReader();
  const snapshot = reader.read([pad({ axes: [.4, -.6, -.5, 0], pressed: [0, 1, 3, 4, 5, 8, 10, 12, 14] })]);
  assert.deepEqual(snapshot, { id: 0, buttons: 127, leftX: .4, leftY: .6, rightX: -.5, dpadX: -1, dpadY: 1 });
});

test("browser-standard stickless controllers use the D-pad and shoulders", () => {
  const snapshot = new GamepadReader().read([pad({ axes: [], pressed: [0, 1, 3, 4, 5, 8, 13, 15] })]);
  assert.equal(snapshot.buttons, 119);
  assert.equal(snapshot.dpadX, 1);
  assert.equal(snapshot.dpadY, -1);
  assert.equal(snapshot.leftX, 0);
  assert.equal(snapshot.rightX, 0);
});

test("unmapped eight-button USB SNES profile treats axes as D-pad and maps face buttons", () => {
  const snapshot = new GamepadReader().read([pad({ mapping: "", count: 8, axes: [-1, -1], pressed: [0, 1, 2, 4, 5, 6] })]);
  assert.deepEqual(snapshot, { id: 0, buttons: 119, leftX: 0, leftY: 0, rightX: 0, dpadX: -1, dpadY: 1 });
});

test("released and disconnected pads clear held input and connected pad selection survives sparse slots", () => {
  const reader = new GamepadReader();
  assert.equal(reader.read([null, pad({ index: 1, pressed: [0] })]).buttons, 1);
  assert.equal(reader.read([null, pad({ index: 1 })]).buttons, 0);
  assert.equal(reader.read([null, { ...pad({ index: 1 }), connected: false }]), null);
  assert.equal(reader.index, null);
});

test("another pad can take over when current pad is idle without combining their inputs", () => {
  const reader = new GamepadReader();
  assert.equal(reader.read([pad(), pad({ index: 1, pressed: [0] })]).id, 1);
  assert.equal(reader.read([pad({ pressed: [1] }), pad({ index: 1 })]).id, 0);
});

test("invalid axes cannot produce movement", () => {
  const snapshot = new GamepadReader().read([pad({ axes: [NaN, Infinity, -Infinity] })]);
  assert.equal(snapshot.leftX, 0);
  assert.equal(snapshot.leftY, -0);
  assert.equal(snapshot.rightX, 0);
});

test("browser bridge tolerates unavailable or denied Gamepad API and page hiding", () => {
  globalThis.window = {};
  globalThis.document = { hidden: false };
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: {} });
  installGamepadControls();
  assert.equal(window.sectorGamepadPoll(), 0);
  navigator.getGamepads = () => { throw new Error("denied"); };
  assert.equal(window.sectorGamepadPoll(), 0);
  navigator.getGamepads = () => [pad({ axes: [1, -1], pressed: [0] })];
  assert.equal(window.sectorGamepadPoll(), 129);
  assert.equal(window.sectorGamepadAxis(0), 1);
  document.hidden = true;
  assert.equal(window.sectorGamepadPoll(), 0);
  assert.equal(window.sectorGamepadAxis(0), 0);
  delete globalThis.window;
  delete globalThis.document;
  delete globalThis.navigator;
});
