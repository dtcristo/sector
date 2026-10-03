// One owner per half, fixed at contact start even when a finger crosses the seam.
export class TouchControls {
  constructor() {
    this.reset();
  }

  reset(active = false) {
    this.contacts = new Map();
    this.taps = { left: null, right: null };
    this.active = active;
    this.jump = false;
    this.map = false;
    this.mapGesture = null;
    this.look = 0;
  }

  start(id, x, y, width, time) {
    const side = x < width / 2 ? "left" : "right";
    if (this.contacts.size >= 2) {
      if (this.mapGesture) this.mapGesture.valid = false;
      return false;
    }
    const first = this.contacts.values().next().value;
    const owner = !first || first.side !== side;
    if (this.mapGesture) this.mapGesture.valid = false;
    else if (first && !first.doubleTap && time - first.time <= 150 && !first.moved) {
      this.mapGesture = { remaining: new Set([...this.contacts.keys(), id]), time: first.time, valid: true };
      this.taps = { left: null, right: null };
      this.jump = false;
    }
    const tap = this.taps[side];
    const doubleTap = !this.mapGesture && owner && tap !== null && time - tap.time <= 300 &&
      Math.hypot(x - tap.x, y - tap.y) <= 32;
    this.taps[side] = null;
    this.contacts.set(id, { side, owner, x, y, originX: x, originY: y, time,
      moved: false, crouch: side === "left" && doubleTap, doubleTap });
    this.active = true;
    if (side === "right" && doubleTap) this.jump = true;
    return true;
  }

  move(id, x, y, width) {
    const contact = this.contacts.get(id);
    if (!contact) return;
    if (contact.owner && contact.side === "right") this.look += (x - contact.x) * 320 / width;
    contact.x = x;
    contact.y = y;
    if (Math.hypot(x - contact.originX, y - contact.originY) > 12) {
      contact.moved = true;
      if (this.mapGesture) this.mapGesture.valid = false;
    }
  }

  end(id, time, cancelled = false) {
    const contact = this.contacts.get(id);
    if (!contact) return;
    if (this.mapGesture?.remaining.has(id)) {
      this.mapGesture.remaining.delete(id);
      if (cancelled) this.mapGesture.valid = false;
      if (this.mapGesture.remaining.size === 0) {
        this.map ||= this.mapGesture.valid && time - this.mapGesture.time <= 250;
        this.mapGesture = null;
      }
      this.contacts.delete(id);
      return;
    }
    if (!cancelled && contact.owner && !contact.moved && !contact.doubleTap && time - contact.time <= 250) {
      this.taps[contact.side] = { x: contact.x, y: contact.y, time };
    }
    this.contacts.delete(id);
    if (cancelled) this.taps[contact.side] = null;
  }

  readButtons() {
    let mask = this.active ? 64 : 0;
    for (const contact of this.contacts.values()) {
      if (!contact.owner || contact.side !== "left") continue;
      const dx = contact.x - contact.originX;
      const dy = contact.y - contact.originY;
      if (dy < -16) mask |= 1;
      if (dy > 16) mask |= 2;
      if (dx < -16) mask |= 4;
      if (dx > 16) mask |= 8;
      if (contact.crouch) mask |= 16;
    }
    if (this.jump) mask |= 32;
    if (this.map) mask |= 128;
    this.jump = false;
    this.map = false;
    return mask;
  }

  readLook() {
    const delta = this.look;
    this.look = 0;
    return delta;
  }
}

export function installTouchControls() {
  const controls = new TouchControls();
  window.sectorTouchButtons = () => controls.readButtons();
  window.sectorTouchLook = () => controls.readLook();
  let hintTimer;
  const hint = document.getElementById("touch-hint");
  document.addEventListener("pointerdown", (event) => {
    if (event.pointerType !== "touch" || event.target.tagName !== "CANVAS") return;
    event.preventDefault();
    if (!controls.start(event.pointerId, event.clientX, event.clientY, window.innerWidth, event.timeStamp)) return;
    event.target.setPointerCapture(event.pointerId);
    if (!hintTimer) hintTimer = window.setTimeout(() => { hint.hidden = true; }, 6000);
  }, { capture: true });
  document.addEventListener("pointermove", (event) => {
    if (!controls.contacts.has(event.pointerId)) return;
    event.preventDefault();
    controls.move(event.pointerId, event.clientX, event.clientY, window.innerWidth);
  }, { capture: true });
  const release = (event) => {
    if (!controls.contacts.has(event.pointerId)) return;
    event.preventDefault();
    controls.end(event.pointerId, event.timeStamp, event.type !== "pointerup");
  };
  for (const type of ["pointerup", "pointercancel", "lostpointercapture"]) {
    document.addEventListener(type, release, { capture: true });
  }
  window.addEventListener("blur", () => controls.reset());
  document.addEventListener("visibilitychange", () => { if (document.hidden) controls.reset(); });
  window.addEventListener("resize", () => controls.reset(controls.active));
}
