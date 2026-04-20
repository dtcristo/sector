function mapNameFromPath(pathname) {
  const trimmed = pathname.trim().replace(/^\/+|\/+$/g, "");
  if (!trimmed || trimmed.toLowerCase() === "index.html") {
    return "default";
  }

  const pieces = trimmed.split("/").filter(Boolean);
  const mapName = pieces[pieces.length - 1].toLowerCase();
  return /^[a-z0-9_-]+$/.test(mapName) ? mapName : "default";
}

function showBootError(error) {
  const target = document.getElementById("boot-error");
  target.hidden = false;
  target.textContent =
    "sector failed to start.\n\n" +
    (error?.stack ?? String(error));
}

const mapName = mapNameFromPath(window.location.pathname);
document.title = mapName === "default" ? "sector" : `sector · ${mapName}`;

const { default: init } = await import("/target/sector.js");
try {
  await init();
} catch (error) {
  console.error(error);
  showBootError(error);
}
