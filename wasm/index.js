const bootError = document.getElementById("boot-error");
const path = window.location.pathname.trim().replace(/^\/+|\/+$/g, "").toLowerCase();
const mapName = path.split("/").pop();
const resolvedMapName =
  !mapName || mapName === "index.html" || !/^[a-z0-9_-]+$/.test(mapName)
    ? "default"
    : mapName;

document.title = resolvedMapName === "default" ? "sector" : `sector · ${resolvedMapName}`;

try {
  await (await import("/target/sector.js")).default();
} catch (error) {
  console.error(error);
  bootError.hidden = false;
  bootError.textContent = `sector failed to start.\n\n${error?.stack ?? String(error)}`;
}
