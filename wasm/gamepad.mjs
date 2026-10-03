// Browser-standard buttons, plus the common unconverted eight-button SNES USB layout.
export class GamepadReader {
  constructor() { this.index = null; }

  decode(pad) {
    const pressed = (index) => Boolean(pad.buttons[index]?.pressed);
    const axis = (index) => {
      const value = pad.axes[index] ?? 0;
      return Number.isFinite(value) ? Math.max(-1, Math.min(1, value)) : 0;
    };
    const snes = pad.mapping !== "standard" && pad.buttons.length === 8 && pad.axes.length <= 2;
    const buttons = (pressed(snes ? 2 : 0) ? 1 : 0) |
      (pressed(1) ? 2 : 0) | (pressed(snes ? 0 : 3) ? 4 : 0) |
      (!snes && pressed(10) ? 8 : 0) | (pressed(snes ? 6 : 8) ? 16 : 0) |
      (pressed(4) ? 32 : 0) | (pressed(5) ? 64 : 0);
    return {
      id: pad.index, buttons,
      leftX: snes ? 0 : axis(0), leftY: snes ? 0 : -axis(1),
      rightX: snes ? 0 : axis(2),
      dpadX: snes ? axis(0) : Number(pressed(15)) - Number(pressed(14)),
      dpadY: snes ? -axis(1) : Number(pressed(12)) - Number(pressed(13)),
    };
  }

  read(pads) {
    let retained = null;
    let candidate = null;
    for (const pad of pads) {
      if (!pad?.connected) continue;
      const snapshot = this.decode(pad);
      const active = snapshot.buttons !== 0 || Math.hypot(snapshot.leftX, snapshot.leftY) > 0.2 ||
        Math.abs(snapshot.rightX) > 0.2 || snapshot.dpadX !== 0 || snapshot.dpadY !== 0;
      if (snapshot.id === this.index) retained = { snapshot, active };
      if (!candidate || active && !candidate.active) candidate = { snapshot, active };
    }
    const selected = retained?.active ? retained : candidate?.active ? candidate : retained ?? candidate;
    this.index = selected?.snapshot.id ?? null;
    return selected?.snapshot ?? null;
  }
}

export function installGamepadControls() {
  const reader = new GamepadReader();
  let snapshot = null;
  window.sectorGamepadPoll = () => {
    // Gamepad API availability varies by browser and Permissions Policy.
    try { snapshot = document.hidden ? null : reader.read(navigator.getGamepads?.() ?? []); }
    catch { snapshot = null; }
    return snapshot ? snapshot.buttons | 128 : 0;
  };
  window.sectorGamepadId = () => snapshot?.id ?? 0;
  const axes = ["leftX", "leftY", "rightX", "dpadX", "dpadY"];
  window.sectorGamepadAxis = (index) => snapshot?.[axes[index]] ?? 0;
}
