import test from "node:test";
import assert from "node:assert/strict";
import { TouchControls, installTouchControls } from "../wasm/touch.mjs";

const tap = (controls, id, x, time) => {
  controls.start(id, x, 200, 400, time);
  controls.end(id, time + 40);
};

test("independent movement and look, contact ownership survives crossing the seam", () => {
  const controls = new TouchControls();
  controls.start(1, 100, 200, 400, 0);
  controls.start(2, 300, 200, 400, 200);
  controls.move(1, 240, 160, 400);
  controls.move(2, 340, 200, 400);
  assert.equal(controls.readButtons(), 64 | 1 | 8);
  assert.equal(controls.readLook(), 32);
  assert.equal(controls.readLook(), 0);
  controls.end(1, 500);
  assert.equal(controls.readButtons(), 64);
  controls.move(2, 300, 200, 400);
  assert.equal(controls.readLook(), -32);
});

test("dead zone, backward and left movement, release stops", () => {
  const controls = new TouchControls();
  controls.start(1, 100, 200, 400, 0);
  controls.move(1, 110, 210, 400);
  assert.equal(controls.readButtons(), 64);
  controls.move(1, 60, 240, 400);
  assert.equal(controls.readButtons(), 64 | 2 | 4);
  controls.end(1, 500);
  assert.equal(controls.readButtons(), 64);
});

test("right double tap queues exactly one jump, triple tap does not repeat", () => {
  const controls = new TouchControls();
  tap(controls, 1, 300, 0);
  controls.start(2, 310, 200, 400, 150);
  assert.equal(controls.readButtons(), 64 | 32);
  assert.equal(controls.readButtons(), 64);
  controls.end(2, 190);
  tap(controls, 3, 300, 220);
  assert.equal(controls.readButtons(), 64);
});

test("left double tap holds crouch while moving and looking, release stands", () => {
  const controls = new TouchControls();
  tap(controls, 1, 100, 0);
  controls.start(2, 105, 200, 400, 150);
  controls.start(3, 300, 200, 400, 160);
  controls.move(2, 105, 150, 400);
  controls.move(3, 330, 200, 400);
  assert.equal(controls.readButtons(), 64 | 1 | 16);
  assert.equal(controls.readLook(), 24);
  controls.end(2, 600);
  assert.equal(controls.readButtons(), 64);
});

test("drags, long holds, distant taps and expired taps cannot trigger double taps", () => {
  for (const scenario of ["drag", "hold", "distant", "expired"]) {
    const controls = new TouchControls();
    controls.start(1, 300, 200, 400, 0);
    if (scenario === "drag") controls.move(1, 320, 200, 400);
    controls.end(1, scenario === "hold" ? 260 : 40);
    controls.start(2, scenario === "distant" ? 350 : 300, 200, 400,
      scenario === "expired" ? 400 : 280);
    assert.equal(controls.readButtons(), 64, scenario);
  }
});

test("extra fingers cannot steal movement, cancellation and reset clear gestures", () => {
  const controls = new TouchControls();
  assert.equal(controls.start(1, 100, 200, 400, 0), true);
  assert.equal(controls.start(2, 120, 200, 400, 0), true);
  assert.equal(controls.start(4, 125, 200, 400, 0), false);
  controls.move(1, 100, 150, 400);
  controls.end(1, 40, true);
  controls.end(2, 50, true);
  controls.start(3, 100, 200, 400, 80);
  assert.equal(controls.readButtons(), 64);
  controls.reset();
  assert.equal(controls.readButtons(), 0);
  assert.equal(controls.readLook(), 0);
  assert.equal(controls.contacts.size, 0);
});

test("look sensitivity scales with viewport width in either orientation", () => {
  for (const width of [390, 844]) {
    const controls = new TouchControls();
    controls.start(1, width * 0.7, 200, width, 0);
    controls.move(1, width * 0.8, 200, width);
    assert.ok(Math.abs(controls.readLook() - 32) < 0.0001);
  }
});

test("browser listeners ignore mouse, capture touch, and clear state on lifecycle events", () => {
  const documentEvents = new Map();
  const windowEvents = new Map();
  globalThis.document = {
    hidden: false,
    getElementById: () => ({ hidden: false }),
    addEventListener: (type, listener) => documentEvents.set(type, listener),
  };
  globalThis.window = {
    innerWidth: 400,
    setTimeout: () => 1,
    addEventListener: (type, listener) => windowEvents.set(type, listener),
  };
  installTouchControls();
  let captures = 0;
  let prevented = 0;
  const event = { pointerId: 1, pointerType: "mouse", clientX: 100, clientY: 200,
    timeStamp: 0, target: { tagName: "CANVAS", setPointerCapture: () => captures++ },
    preventDefault: () => prevented++ };
  documentEvents.get("pointerdown")(event);
  assert.equal(window.sectorTouchButtons(), 0);
  event.pointerType = "touch";
  documentEvents.get("pointerdown")(event);
  assert.equal(captures, 1);
  assert.equal(prevented, 1);
  documentEvents.get("pointermove")({ ...event, clientY: 160 });
  assert.equal(window.sectorTouchButtons(), 64 | 1);
  documentEvents.get("pointercancel")({ ...event, type: "pointercancel", timeStamp: 50 });
  assert.equal(window.sectorTouchButtons(), 64);
  windowEvents.get("resize")();
  assert.equal(window.sectorTouchButtons(), 64);
  windowEvents.get("blur")();
  assert.equal(window.sectorTouchButtons(), 0);
  documentEvents.get("pointerdown")(event);
  document.hidden = true;
  documentEvents.get("visibilitychange")();
  assert.equal(window.sectorTouchButtons(), 0);
  delete globalThis.document;
  delete globalThis.window;
});


test("two-finger tap cycles map once on either half without queuing a jump", () => {
  for (const x of [120, 300]) {
    const controls = new TouchControls();
    controls.start(1, 100, 200, 400, 0);
    controls.start(2, x, 200, 400, 30);
    controls.end(1, 80);
    controls.end(2, 90);
    assert.equal(controls.readButtons(), 64 | 128);
    assert.equal(controls.readButtons(), 64);
    tap(controls, 3, x, 150);
    assert.equal(controls.readButtons(), 64);
  }
});

test("moving, held, cancelled or three-finger contacts cannot open the map", () => {
  for (const scenario of ["moving", "held", "cancelled", "third"]) {
    const controls = new TouchControls();
    controls.start(1, 100, 200, 400, 0);
    controls.start(2, 300, 200, 400, 20);
    if (scenario === "moving") controls.move(1, 100, 160, 400);
    if (scenario === "third") controls.start(3, 320, 200, 400, 30);
    controls.end(1, 80, scenario === "cancelled");
    controls.end(2, scenario === "held" ? 500 : 90);
    assert.equal(controls.readButtons(), 64, scenario);
  }
});
