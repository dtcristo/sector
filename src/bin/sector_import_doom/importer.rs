use geo::{Area, Contains, Covers, InteriorPoint};
use geo::{LineString, Point, Polygon};
use geo_polygonize_core::Polygonizer;
use geo_types::Geometry;
use sector::map::{MapSector, MapVertex, MapWall, SectorMap, SectorMapError};
use sector::player::{PLAYER_EYE_HEIGHT_METERS, PLAYER_HEIGHT_METERS, PLAYER_RADIUS_METERS};
use thiserror::Error;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

const DOOM_PLAYER_RADIUS_UNITS: f32 = 16.0;
const DOOM_PLAYER_EYE_HEIGHT_UNITS: f32 = 41.0;
const XY_SCALE: f32 = DOOM_PLAYER_RADIUS_UNITS / PLAYER_RADIUS_METERS;
const Z_SCALE: f32 = DOOM_PLAYER_EYE_HEIGHT_UNITS / PLAYER_EYE_HEIGHT_METERS;
const MIN_HEADROOM_METERS: f32 = PLAYER_HEIGHT_METERS;
const OUTDOOR_CEILING_METERS: f32 = 12.0;
const MIN_PORTAL_OVERLAP_METERS: f32 = 0.05;
const DEFAULT_WALL_COLOR: [u8; 3] = [96, 96, 104];
const IMPASSABLE_FLAG: i16 = 0x0001;
const DOOR_TEXTURE_HINTS: [&str; 3] = ["BIGDOOR", "EXITDOOR", "DOOR"];

type IntPoint = (i32, i32);
type EdgeKey = (IntPoint, IntPoint);

#[derive(Debug)]
pub struct ConvertedMap {
    pub map: SectorMap,
    pub doom_sector_count: usize,
    pub generated_sector_count: usize,
    pub sky_sector_count: usize,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("failed to read WAD {path}: {source}")]
    ReadWad {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("WAD {path} is malformed: {message}")]
    InvalidWad { path: String, message: String },
    #[error("WAD {path} does not contain map marker {map_name}")]
    MissingMap { path: String, map_name: String },
    #[error("map {map_name} is missing lump {lump}")]
    MissingMapLump {
        map_name: String,
        lump: &'static str,
    },
    #[error("texture {texture_name} references missing patch index {patch_index}")]
    MissingPatchReference {
        texture_name: String,
        patch_index: usize,
    },
    #[error("polygonization failed for sector {sector_index}: {message}")]
    Polygonization {
        sector_index: usize,
        message: String,
    },
    #[error("triangulation failed for sector {sector_index}: {message}")]
    Triangulation {
        sector_index: usize,
        message: String,
    },
    #[error("could not find a Doom player 1 start in map {map_name}")]
    MissingPlayerStart { map_name: String },
    #[error("could not place Doom spawn inside any generated sector for map {map_name}")]
    SpawnOutsideGeneratedCells { map_name: String },
    #[error(transparent)]
    Map(#[from] SectorMapError),
}

#[derive(Debug, Clone)]
struct DirectoryEntry {
    offset: usize,
    size: usize,
    name: String,
}

#[derive(Debug, Clone, Copy)]
struct SideDef {
    upper: [u8; 8],
    lower: [u8; 8],
    middle: [u8; 8],
    sector: usize,
}

impl SideDef {
    fn upper_name(self) -> String {
        decode_name(&self.upper)
    }

    fn lower_name(self) -> String {
        decode_name(&self.lower)
    }

    fn middle_name(self) -> String {
        decode_name(&self.middle)
    }
}

#[derive(Debug, Clone, Copy)]
struct LineDef {
    start: usize,
    end: usize,
    flags: i16,
    right: i16,
    left: i16,
}

impl LineDef {
    fn is_blocking(self) -> bool {
        self.flags & IMPASSABLE_FLAG != 0
    }
}

#[derive(Debug, Clone)]
struct DoomSector {
    floor: i16,
    ceil: i16,
    floor_flat: String,
    ceil_flat: String,
}

#[derive(Debug, Clone, Copy)]
struct Thing {
    x: i16,
    y: i16,
    angle: i16,
    thing_type: i16,
}

#[derive(Debug, Clone, Copy)]
struct EdgeSurface {
    color: [u8; 3],
    upper: Option<[u8; 3]>,
    lower: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Copy)]
struct BoundarySegment {
    start: IntPoint,
    end: IntPoint,
    surface: EdgeSurface,
    walkable: bool,
}

#[derive(Debug, Clone)]
struct Cell {
    source_sector: usize,
    vertices: Vec<IntPoint>,
}

#[derive(Debug, Clone, Copy)]
struct SectorProfile {
    floor: f32,
    ceil: f32,
    floor_color: [u8; 3],
    ceil_color: [u8; 3],
    wall_default: [u8; 3],
    no_ceiling: bool,
}

#[derive(Debug, Clone)]
struct TextureDef {
    width: usize,
    height: usize,
    patches: Vec<TexturePatch>,
}

#[derive(Debug, Clone)]
struct TexturePatch {
    origin_x: i16,
    origin_y: i16,
    patch_name: String,
}

struct Wad {
    path: String,
    data: Vec<u8>,
    directory: Vec<DirectoryEntry>,
    first_lump_by_name: HashMap<String, usize>,
}

impl Wad {
    fn open(path: &Path) -> Result<Self, ImportError> {
        let data = std::fs::read(path).map_err(|source| ImportError::ReadWad {
            path: path.display().to_string(),
            source,
        })?;
        if data.len() < 12 {
            return Err(ImportError::InvalidWad {
                path: path.display().to_string(),
                message: "header is truncated".into(),
            });
        }

        let lump_count = read_i32(&data, 4)? as usize;
        let directory_offset = read_i32(&data, 8)? as usize;
        let directory_bytes =
            lump_count
                .checked_mul(16)
                .ok_or_else(|| ImportError::InvalidWad {
                    path: path.display().to_string(),
                    message: "directory size overflowed".into(),
                })?;
        if directory_offset + directory_bytes > data.len() {
            return Err(ImportError::InvalidWad {
                path: path.display().to_string(),
                message: "directory extends past end of file".into(),
            });
        }

        let mut directory = Vec::with_capacity(lump_count);
        let mut first_lump_by_name = HashMap::new();
        for index in 0..lump_count {
            let offset = directory_offset + index * 16;
            let lump_offset = read_i32(&data, offset)? as usize;
            let lump_size = read_i32(&data, offset + 4)? as usize;
            let raw_name = &data[offset + 8..offset + 16];
            let name = decode_name(raw_name);
            directory.push(DirectoryEntry {
                offset: lump_offset,
                size: lump_size,
                name: name.clone(),
            });
            first_lump_by_name.entry(name).or_insert(index);
        }

        Ok(Self {
            path: path.display().to_string(),
            data,
            directory,
            first_lump_by_name,
        })
    }

    fn read_lump(&self, name: &str) -> Result<&[u8], ImportError> {
        let Some(index) = self.first_lump_by_name.get(name) else {
            return Err(ImportError::InvalidWad {
                path: self.path.clone(),
                message: format!("missing lump {name}"),
            });
        };
        self.entry_bytes(&self.directory[*index])
    }

    fn map_entries(&self, map_name: &str) -> Result<HashMap<String, DirectoryEntry>, ImportError> {
        let Some(marker_index) = self
            .directory
            .iter()
            .position(|entry| entry.name.eq_ignore_ascii_case(map_name))
        else {
            return Err(ImportError::MissingMap {
                path: self.path.clone(),
                map_name: map_name.to_string(),
            });
        };

        const REQUIRED: [&str; 5] = ["VERTEXES", "THINGS", "LINEDEFS", "SIDEDEFS", "SECTORS"];
        let mut result = HashMap::new();
        for entry in self.directory.iter().skip(marker_index + 1).take(11) {
            if REQUIRED.contains(&entry.name.as_str()) {
                result.insert(entry.name.clone(), entry.clone());
            }
        }

        for lump in REQUIRED {
            if !result.contains_key(lump) {
                return Err(ImportError::MissingMapLump {
                    map_name: map_name.to_string(),
                    lump,
                });
            }
        }

        Ok(result)
    }

    fn entry_bytes(&self, entry: &DirectoryEntry) -> Result<&[u8], ImportError> {
        let end = entry
            .offset
            .checked_add(entry.size)
            .ok_or_else(|| ImportError::InvalidWad {
                path: self.path.clone(),
                message: format!("lump {} overflows file offsets", entry.name),
            })?;
        if end > self.data.len() {
            return Err(ImportError::InvalidWad {
                path: self.path.clone(),
                message: format!("lump {} extends past end of file", entry.name),
            });
        }
        Ok(&self.data[entry.offset..end])
    }
}

struct TextureAverages<'a> {
    wad: &'a Wad,
    palette: Vec<[u8; 3]>,
    texture_defs: HashMap<String, TextureDef>,
    cache: HashMap<String, Option<[u8; 3]>>,
}

impl<'a> TextureAverages<'a> {
    fn new(wad: &'a Wad) -> Result<Self, ImportError> {
        let playpal = wad.read_lump("PLAYPAL")?;
        if playpal.len() < 256 * 3 {
            return Err(ImportError::InvalidWad {
                path: wad.path.clone(),
                message: "PLAYPAL lump is truncated".into(),
            });
        }
        let palette = playpal[..256 * 3]
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect::<Vec<_>>();

        let pnames = wad.read_lump("PNAMES")?;
        let patch_count = read_i32(pnames, 0)? as usize;
        if pnames.len() < 4 + patch_count * 8 {
            return Err(ImportError::InvalidWad {
                path: wad.path.clone(),
                message: "PNAMES lump is truncated".into(),
            });
        }
        let patch_names = (0..patch_count)
            .map(|index| decode_name(&pnames[4 + index * 8..12 + index * 8]))
            .collect::<Vec<_>>();

        let mut texture_defs = HashMap::new();
        parse_texture_directory(
            &mut texture_defs,
            wad.read_lump("TEXTURE1")?,
            &patch_names,
            &wad.path,
        )?;
        if let Some(texture2_index) = wad.first_lump_by_name.get("TEXTURE2") {
            parse_texture_directory(
                &mut texture_defs,
                wad.entry_bytes(&wad.directory[*texture2_index])?,
                &patch_names,
                &wad.path,
            )?;
        }

        Ok(Self {
            wad,
            palette,
            texture_defs,
            cache: HashMap::new(),
        })
    }

    fn average(&mut self, name: &str) -> Option<[u8; 3]> {
        let normalized = normalize_name(name);
        if normalized.is_empty() || normalized == "-" {
            return None;
        }
        if let Some(cached) = self.cache.get(&normalized) {
            return *cached;
        }

        let average = if let Some(texture_def) = self.texture_defs.get(&normalized).cloned() {
            self.average_texture(&normalized, &texture_def)
        } else if let Some(index) = self.wad.first_lump_by_name.get(&normalized) {
            let entry = &self.wad.directory[*index];
            if entry.size == 64 * 64 {
                self.average_flat(entry).ok()
            } else {
                self.patch_pixels(&normalized)
                    .ok()
                    .flatten()
                    .and_then(|pixels| self.average_visible_indices(&pixels.values))
            }
        } else {
            None
        };

        self.cache.insert(normalized, average);
        average
    }

    fn average_flat(&self, entry: &DirectoryEntry) -> Result<[u8; 3], ImportError> {
        let bytes = self.wad.entry_bytes(entry)?;
        let mut totals = [0_u64; 3];
        for &palette_index in bytes {
            let color = self.palette[palette_index as usize];
            totals[0] += color[0] as u64;
            totals[1] += color[1] as u64;
            totals[2] += color[2] as u64;
        }

        Ok([
            (totals[0] as f64 / bytes.len() as f64).round() as u8,
            (totals[1] as f64 / bytes.len() as f64).round() as u8,
            (totals[2] as f64 / bytes.len() as f64).round() as u8,
        ])
    }

    fn average_texture(&mut self, texture_name: &str, texture_def: &TextureDef) -> Option<[u8; 3]> {
        let mut canvas = vec![-1_i16; texture_def.width * texture_def.height];
        for patch in &texture_def.patches {
            let Some(pixels) = self.patch_pixels(&patch.patch_name).ok().flatten() else {
                continue;
            };
            for patch_y in 0..pixels.height {
                let dest_y = patch.origin_y as isize + patch_y as isize;
                if !(0..texture_def.height as isize).contains(&dest_y) {
                    continue;
                }
                for patch_x in 0..pixels.width {
                    let dest_x = patch.origin_x as isize + patch_x as isize;
                    if !(0..texture_def.width as isize).contains(&dest_x) {
                        continue;
                    }
                    let palette_index = pixels.values[patch_y * pixels.width + patch_x];
                    if palette_index >= 0 {
                        canvas[dest_y as usize * texture_def.width + dest_x as usize] =
                            palette_index;
                    }
                }
            }
        }

        let average = self.average_visible_indices(&canvas);
        if average.is_none() {
            eprintln!("warning: texture {texture_name} had no visible pixels");
        }
        average
    }

    fn patch_pixels(&self, name: &str) -> Result<Option<PatchPixels>, ImportError> {
        let Some(index) = self.wad.first_lump_by_name.get(name) else {
            return Ok(None);
        };
        let entry = &self.wad.directory[*index];
        let lump = self.wad.entry_bytes(entry)?;
        if lump.len() < 8 {
            return Ok(None);
        }

        let width = read_i16(lump, 0)? as isize;
        let height = read_i16(lump, 2)? as isize;
        if width <= 0 || height <= 0 {
            return Ok(None);
        }
        let width = width as usize;
        let height = height as usize;
        if lump.len() < 8 + width * 4 {
            return Ok(None);
        }

        let mut values = vec![-1_i16; width * height];
        for column in 0..width {
            let mut cursor = read_u32(lump, 8 + column * 4)? as usize;
            while cursor < lump.len() {
                let top_delta = lump[cursor];
                if top_delta == 255 {
                    break;
                }
                if cursor + 3 >= lump.len() {
                    break;
                }
                let length = lump[cursor + 1] as usize;
                cursor += 3;
                if cursor + length >= lump.len() {
                    break;
                }
                for offset in 0..length {
                    let y = top_delta as usize + offset;
                    if y < height {
                        values[y * width + column] = lump[cursor + offset] as i16;
                    }
                }
                cursor += length + 1;
            }
        }

        Ok(Some(PatchPixels {
            width,
            height,
            values,
        }))
    }

    fn average_visible_indices(&self, indices: &[i16]) -> Option<[u8; 3]> {
        let mut totals = [0_u64; 3];
        let mut count = 0_u64;
        for &palette_index in indices {
            if palette_index < 0 {
                continue;
            }
            let color = self.palette[palette_index as usize];
            totals[0] += color[0] as u64;
            totals[1] += color[1] as u64;
            totals[2] += color[2] as u64;
            count += 1;
        }

        (count > 0).then_some([
            (totals[0] as f64 / count as f64).round() as u8,
            (totals[1] as f64 / count as f64).round() as u8,
            (totals[2] as f64 / count as f64).round() as u8,
        ])
    }
}

#[derive(Debug)]
struct PatchPixels {
    width: usize,
    height: usize,
    values: Vec<i16>,
}

pub fn import_doom_map(wad_path: &Path, map_name: &str) -> Result<ConvertedMap, ImportError> {
    let normalized_map_name = map_name.to_ascii_uppercase();
    let wad = Wad::open(wad_path)?;
    let mut textures = TextureAverages::new(&wad)?;
    let (vertices, things, linedefs, sidedefs, doom_sectors) =
        load_map_data(&wad, &normalized_map_name)?;

    let sector_polygons = build_sector_polygons(&vertices, &linedefs, &sidedefs)?;
    let boundary_points = build_sector_boundary_points(&vertices, &linedefs, &sidedefs);
    let wall_defaults =
        build_sector_wall_defaults(&linedefs, &sidedefs, &doom_sectors, &mut textures);
    let boundary_segments = build_boundary_segments(
        &vertices,
        &linedefs,
        &sidedefs,
        &wall_defaults,
        &mut textures,
    );
    let neighbors = build_sector_neighbors(&linedefs, &sidedefs);
    let sector_profiles = build_sector_profiles(
        &doom_sectors,
        &neighbors,
        &linedefs,
        &sidedefs,
        &wall_defaults,
        &mut textures,
    );

    let mut cells = Vec::new();
    let empty_boundary_points = HashSet::new();
    for (sector_index, polygons) in &sector_polygons {
        for polygon in polygons {
            cells.extend(polygon_to_cells(
                polygon,
                *sector_index,
                boundary_points
                    .get(sector_index)
                    .unwrap_or(&empty_boundary_points),
            )?);
        }
    }
    sort_cells(&mut cells);

    let mut edge_to_cells: HashMap<EdgeKey, Vec<(usize, usize)>> = HashMap::new();
    for (cell_index, cell) in cells.iter().enumerate() {
        for edge_index in 0..cell.vertices.len() {
            let start = cell.vertices[edge_index];
            let end = cell.vertices[(edge_index + 1) % cell.vertices.len()];
            edge_to_cells
                .entry(unordered_edge(start, end))
                .or_default()
                .push((cell_index, edge_index));
        }
    }

    let mut map_sectors = Vec::with_capacity(cells.len());
    for (cell_index, cell) in cells.iter().enumerate() {
        let profile = sector_profiles[&cell.source_sector];
        let boundary_segments_for_sector = boundary_segments
            .get(&cell.source_sector)
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        let walls = (0..cell.vertices.len())
            .map(|edge_index| {
                let start = cell.vertices[edge_index];
                let end = cell.vertices[(edge_index + 1) % cell.vertices.len()];
                let matches = edge_to_cells
                    .get(&unordered_edge(start, end))
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);

                let portal = if matches.len() == 2 {
                    let (first_cell, _first_edge) = matches[0];
                    let (second_cell, _second_edge) = matches[1];
                    Some(if first_cell == cell_index {
                        second_cell
                    } else {
                        first_cell
                    })
                } else {
                    None
                };

                let surface = find_boundary_segment(boundary_segments_for_sector, start, end)
                    .map(|segment| segment.surface)
                    .unwrap_or(EdgeSurface {
                        color: profile.wall_default,
                        upper: None,
                        lower: None,
                    });

                let walkable = portal
                    .map(|portal_index| {
                        if cells[portal_index].source_sector == cell.source_sector {
                            true
                        } else {
                            find_boundary_segment(boundary_segments_for_sector, start, end)
                                .map(|segment| segment.walkable)
                                .unwrap_or(true)
                        }
                    })
                    .unwrap_or(true);

                MapWall {
                    color: surface.color,
                    portal,
                    walkable,
                    upper_color: portal
                        .filter(|portal_index| {
                            cells[*portal_index].source_sector != cell.source_sector
                        })
                        .and(surface.upper),
                    lower_color: portal
                        .filter(|portal_index| {
                            cells[*portal_index].source_sector != cell.source_sector
                        })
                        .and(surface.lower),
                }
            })
            .collect();

        map_sectors.push(MapSector {
            floor: profile.floor,
            ceil: profile.ceil,
            floor_color: profile.floor_color,
            ceil_color: profile.ceil_color,
            no_ceiling: profile.no_ceiling,
            vertices: cell
                .vertices
                .iter()
                .map(|(x, y)| MapVertex(*x as f32 / XY_SCALE, *y as f32 / XY_SCALE))
                .collect(),
            walls,
        });
    }

    let player_start =
        find_player_start(&things).ok_or_else(|| ImportError::MissingPlayerStart {
            map_name: normalized_map_name.clone(),
        })?;
    let spawn_point = Point::new(player_start.x as f64, player_start.y as f64);
    let initial_sector = cells
        .iter()
        .enumerate()
        .find_map(|(cell_index, cell)| {
            polygon_from_int_ring(&cell.vertices)
                .covers(&spawn_point)
                .then_some(cell_index)
        })
        .ok_or_else(|| ImportError::SpawnOutsideGeneratedCells {
            map_name: normalized_map_name.clone(),
        })?;

    let map = SectorMap {
        initial_sector,
        initial_position: MapVertex(
            player_start.x as f32 / XY_SCALE,
            player_start.y as f32 / XY_SCALE,
        ),
        initial_direction_degrees: player_start.angle as f32 - 90.0,
        sectors: map_sectors,
    };

    sector::map::validate_map(&map)?;

    Ok(ConvertedMap {
        doom_sector_count: doom_sectors.len(),
        generated_sector_count: map.sectors.len(),
        sky_sector_count: doom_sectors
            .iter()
            .filter(|sector| sector.ceil_flat == "F_SKY1")
            .count(),
        map,
    })
}

fn load_map_data(
    wad: &Wad,
    map_name: &str,
) -> Result<
    (
        Vec<IntPoint>,
        Vec<Thing>,
        Vec<LineDef>,
        Vec<SideDef>,
        Vec<DoomSector>,
    ),
    ImportError,
> {
    let entries = wad.map_entries(map_name)?;
    let vertexes = wad.entry_bytes(&entries["VERTEXES"])?;
    let things = wad.entry_bytes(&entries["THINGS"])?;
    let linedefs = wad.entry_bytes(&entries["LINEDEFS"])?;
    let sidedefs = wad.entry_bytes(&entries["SIDEDEFS"])?;
    let sectors = wad.entry_bytes(&entries["SECTORS"])?;

    ensure_multiple(vertexes, 4, "VERTEXES", &wad.path)?;
    ensure_multiple(things, 10, "THINGS", &wad.path)?;
    ensure_multiple(linedefs, 14, "LINEDEFS", &wad.path)?;
    ensure_multiple(sidedefs, 30, "SIDEDEFS", &wad.path)?;
    ensure_multiple(sectors, 26, "SECTORS", &wad.path)?;

    let vertices = vertexes
        .chunks_exact(4)
        .map(|chunk| {
            (
                read_i16(chunk, 0).unwrap() as i32,
                read_i16(chunk, 2).unwrap() as i32,
            )
        })
        .collect::<Vec<_>>();

    let things = things
        .chunks_exact(10)
        .map(|chunk| Thing {
            x: read_i16(chunk, 0).unwrap(),
            y: read_i16(chunk, 2).unwrap(),
            angle: read_i16(chunk, 4).unwrap(),
            thing_type: read_i16(chunk, 6).unwrap(),
        })
        .collect::<Vec<_>>();

    let linedefs = linedefs
        .chunks_exact(14)
        .map(|chunk| LineDef {
            start: read_i16(chunk, 0).unwrap() as usize,
            end: read_i16(chunk, 2).unwrap() as usize,
            flags: read_i16(chunk, 4).unwrap(),
            right: read_i16(chunk, 10).unwrap(),
            left: read_i16(chunk, 12).unwrap(),
        })
        .collect::<Vec<_>>();

    let sidedefs = sidedefs
        .chunks_exact(30)
        .map(|chunk| SideDef {
            upper: chunk[4..12].try_into().unwrap(),
            lower: chunk[12..20].try_into().unwrap(),
            middle: chunk[20..28].try_into().unwrap(),
            sector: read_i16(chunk, 28).unwrap() as usize,
        })
        .collect::<Vec<_>>();

    let sectors = sectors
        .chunks_exact(26)
        .map(|chunk| DoomSector {
            floor: read_i16(chunk, 0).unwrap(),
            ceil: read_i16(chunk, 2).unwrap(),
            floor_flat: decode_name(&chunk[4..12]),
            ceil_flat: decode_name(&chunk[12..20]),
        })
        .collect::<Vec<_>>();

    Ok((vertices, things, linedefs, sidedefs, sectors))
}

fn build_sector_polygons(
    vertices: &[IntPoint],
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> Result<BTreeMap<usize, Vec<Polygon<f64>>>, ImportError> {
    let mut sector_lines: BTreeMap<usize, Vec<LineString<f64>>> = BTreeMap::new();
    for linedef in linedefs {
        let start = vertices[linedef.start];
        let end = vertices[linedef.end];
        let line = LineString::from(vec![
            (start.0 as f64, start.1 as f64),
            (end.0 as f64, end.1 as f64),
        ]);
        if linedef.right >= 0 {
            sector_lines
                .entry(sidedefs[linedef.right as usize].sector)
                .or_default()
                .push(line.clone());
        }
        if linedef.left >= 0 {
            sector_lines
                .entry(sidedefs[linedef.left as usize].sector)
                .or_default()
                .push(line.clone());
        }
    }

    let mut sector_polygons = BTreeMap::new();
    for (sector_index, lines) in sector_lines {
        let mut polygonizer = Polygonizer::new();
        for line in lines {
            polygonizer.add_geometry(Geometry::LineString(line));
        }
        let raw_polygons = polygonizer
            .polygonize()
            .map_err(|error| ImportError::Polygonization {
                sector_index,
                message: error.to_string(),
            })?
            .polygons
            .into_iter()
            .map(|polygon| polygon.to_polygon_2d())
            .filter(|polygon| polygon.unsigned_area() >= 1.0)
            .collect::<Vec<_>>();
        let mut filtered = filter_sector_polygons(raw_polygons);
        filtered.sort_by(|left, right| {
            right
                .unsigned_area()
                .total_cmp(&left.unsigned_area())
                .then_with(|| polygon_probe(left).y().total_cmp(&polygon_probe(right).y()))
                .then_with(|| polygon_probe(left).x().total_cmp(&polygon_probe(right).x()))
        });
        sector_polygons.insert(sector_index, filtered);
    }

    Ok(sector_polygons)
}

fn filter_sector_polygons(raw_polygons: Vec<Polygon<f64>>) -> Vec<Polygon<f64>> {
    let hole_polygons = raw_polygons
        .iter()
        .flat_map(|polygon| {
            polygon
                .interiors()
                .iter()
                .cloned()
                .map(|ring| Polygon::new(ring, vec![]))
        })
        .collect::<Vec<_>>();

    let mut kept = Vec::new();
    for (index, polygon) in raw_polygons.iter().enumerate() {
        let point = polygon_probe(polygon);
        if hole_polygons.iter().any(|hole| hole.contains(&point)) {
            continue;
        }
        if raw_polygons
            .iter()
            .enumerate()
            .any(|(other_index, other)| other_index != index && other.contains(&point))
        {
            continue;
        }
        kept.push(polygon.clone());
    }
    kept
}

fn build_sector_boundary_points(
    vertices: &[IntPoint],
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> HashMap<usize, HashSet<IntPoint>> {
    let mut boundary_points = HashMap::<usize, HashSet<IntPoint>>::new();
    for linedef in linedefs {
        let start = vertices[linedef.start];
        let end = vertices[linedef.end];
        if linedef.right >= 0 {
            boundary_points
                .entry(sidedefs[linedef.right as usize].sector)
                .or_default()
                .extend([start, end]);
        }
        if linedef.left >= 0 {
            boundary_points
                .entry(sidedefs[linedef.left as usize].sector)
                .or_default()
                .extend([start, end]);
        }
    }
    boundary_points
}

fn build_sector_wall_defaults(
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
    sectors: &[DoomSector],
    textures: &mut TextureAverages<'_>,
) -> HashMap<usize, [u8; 3]> {
    let mut sector_colors: HashMap<usize, Vec<[u8; 3]>> = HashMap::new();

    for linedef in linedefs {
        for side_index in [linedef.right, linedef.left] {
            if side_index < 0 {
                continue;
            }
            let side = sidedefs[side_index as usize];
            for texture_name in [side.middle_name(), side.upper_name(), side.lower_name()] {
                if let Some(color) = textures.average(&texture_name) {
                    sector_colors.entry(side.sector).or_default().push(color);
                }
            }
        }
    }

    (0..sectors.len())
        .map(|sector_index| {
            let average = sector_colors
                .get(&sector_index)
                .map(|colors| average_colors(colors))
                .unwrap_or(DEFAULT_WALL_COLOR);
            (sector_index, average)
        })
        .collect()
}

fn build_boundary_segments(
    vertices: &[IntPoint],
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
    wall_defaults: &HashMap<usize, [u8; 3]>,
    textures: &mut TextureAverages<'_>,
) -> HashMap<usize, Vec<BoundarySegment>> {
    let mut segments = HashMap::<usize, Vec<BoundarySegment>>::new();

    let mut surface_for_side = |side_index: usize| {
        let side = sidedefs[side_index];
        let middle = textures.average(&side.middle_name());
        let upper = textures.average(&side.upper_name());
        let lower = textures.average(&side.lower_name());
        let color = middle
            .or(upper)
            .or(lower)
            .unwrap_or(wall_defaults[&side.sector]);
        EdgeSurface {
            color,
            upper,
            lower,
        }
    };

    for linedef in linedefs {
        let start = vertices[linedef.start];
        let end = vertices[linedef.end];
        if linedef.right >= 0 {
            let sector_index = sidedefs[linedef.right as usize].sector;
            segments
                .entry(sector_index)
                .or_default()
                .push(BoundarySegment {
                    start,
                    end,
                    surface: surface_for_side(linedef.right as usize),
                    walkable: !linedef.is_blocking(),
                });
        }
        if linedef.left >= 0 {
            let sector_index = sidedefs[linedef.left as usize].sector;
            segments
                .entry(sector_index)
                .or_default()
                .push(BoundarySegment {
                    start,
                    end,
                    surface: surface_for_side(linedef.left as usize),
                    walkable: !linedef.is_blocking(),
                });
        }
    }

    segments
}

fn build_sector_neighbors(
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> HashMap<usize, HashSet<usize>> {
    let mut neighbors = HashMap::<usize, HashSet<usize>>::new();
    for linedef in linedefs {
        if linedef.right < 0 || linedef.left < 0 {
            continue;
        }
        let right_sector = sidedefs[linedef.right as usize].sector;
        let left_sector = sidedefs[linedef.left as usize].sector;
        if right_sector == left_sector {
            continue;
        }
        neighbors
            .entry(right_sector)
            .or_default()
            .insert(left_sector);
        neighbors
            .entry(left_sector)
            .or_default()
            .insert(right_sector);
    }
    neighbors
}

fn build_sector_profiles(
    sectors: &[DoomSector],
    neighbors: &HashMap<usize, HashSet<usize>>,
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
    wall_defaults: &HashMap<usize, [u8; 3]>,
    textures: &mut TextureAverages<'_>,
) -> HashMap<usize, SectorProfile> {
    let mut heights = (0..sectors.len())
        .map(|sector_index| {
            (
                sector_index,
                scaled_floor_and_ceil(sector_index, sectors, neighbors, linedefs, sidedefs),
            )
        })
        .collect::<HashMap<_, _>>();
    ensure_vertical_overlap(&mut heights, neighbors);

    (0..sectors.len())
        .map(|sector_index| {
            let source = &sectors[sector_index];
            let (floor, ceil) = heights[&sector_index];
            let wall_default = wall_defaults[&sector_index];
            (
                sector_index,
                SectorProfile {
                    floor,
                    ceil,
                    floor_color: textures.average(&source.floor_flat).unwrap_or(wall_default),
                    ceil_color: textures.average(&source.ceil_flat).unwrap_or(wall_default),
                    wall_default,
                    no_ceiling: source.ceil_flat == "F_SKY1",
                },
            )
        })
        .collect()
}

fn scaled_floor_and_ceil(
    sector_index: usize,
    sectors: &[DoomSector],
    neighbors: &HashMap<usize, HashSet<usize>>,
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> (f32, f32) {
    let source = &sectors[sector_index];
    let floor = source.floor as f32 / Z_SCALE;
    let mut ceil = source.ceil as f32 / Z_SCALE;

    if source.ceil_flat == "F_SKY1" {
        ceil = ceil.max(OUTDOOR_CEILING_METERS);
    }

    if source.ceil == source.floor || sector_has_door_textures(sector_index, linedefs, sidedefs) {
        if let Some(neighbor_ceil) = neighbors.get(&sector_index).and_then(|sector_neighbors| {
            sector_neighbors
                .iter()
                .map(|neighbor| sectors[*neighbor].ceil as f32 / Z_SCALE)
                .max_by(f32::total_cmp)
        }) {
            ceil = ceil.max(neighbor_ceil);
        }
    }

    if ceil - floor < MIN_HEADROOM_METERS {
        ceil = floor + MIN_HEADROOM_METERS;
    }

    (floor, ceil)
}

fn ensure_vertical_overlap(
    heights: &mut HashMap<usize, (f32, f32)>,
    neighbors: &HashMap<usize, HashSet<usize>>,
) {
    let mut seen_pairs = HashSet::new();
    for (&sector, sector_neighbors) in neighbors {
        for &neighbor in sector_neighbors {
            let pair = if sector <= neighbor {
                (sector, neighbor)
            } else {
                (neighbor, sector)
            };
            if !seen_pairs.insert(pair) {
                continue;
            }

            let (sector_floor, sector_ceil) = heights[&sector];
            let (neighbor_floor, neighbor_ceil) = heights[&neighbor];
            let overlap = sector_ceil.min(neighbor_ceil) - sector_floor.max(neighbor_floor);
            if overlap > 0.0 {
                continue;
            }

            if sector_ceil <= neighbor_floor {
                heights.insert(
                    sector,
                    (sector_floor, neighbor_floor + MIN_PORTAL_OVERLAP_METERS),
                );
            } else if neighbor_ceil <= sector_floor {
                heights.insert(
                    neighbor,
                    (neighbor_floor, sector_floor + MIN_PORTAL_OVERLAP_METERS),
                );
            }
        }
    }
}

fn sector_has_door_textures(
    sector_index: usize,
    linedefs: &[LineDef],
    sidedefs: &[SideDef],
) -> bool {
    linedefs.iter().any(|linedef| {
        [linedef.right, linedef.left]
            .into_iter()
            .filter(|side_index| *side_index >= 0)
            .any(|side_index| {
                let side = sidedefs[side_index as usize];
                side.sector == sector_index
                    && [side.upper_name(), side.lower_name(), side.middle_name()]
                        .into_iter()
                        .any(|texture_name| {
                            texture_name == "DOORTRAK"
                                || texture_name == "DOORSTOP"
                                || DOOR_TEXTURE_HINTS
                                    .iter()
                                    .any(|prefix| texture_name.starts_with(prefix))
                        })
            })
    })
}

fn polygon_to_cells(
    polygon: &Polygon<f64>,
    source_sector: usize,
    boundary_points: &HashSet<IntPoint>,
) -> Result<Vec<Cell>, ImportError> {
    if polygon.interiors().is_empty() {
        let mut ring = expand_ring(
            &ensure_clockwise(rounded_ring(polygon.exterior())),
            boundary_points,
        );
        if is_convex_polygon(&ring) {
            return Ok(vec![Cell {
                source_sector,
                vertices: std::mem::take(&mut ring),
            }]);
        }
    }

    let mut point_rings = Vec::<Vec<IntPoint>>::new();
    point_rings.push(expand_ring(
        &ensure_counter_clockwise(rounded_ring(polygon.exterior())),
        boundary_points,
    ));
    for interior in polygon.interiors() {
        point_rings.push(expand_ring(
            &ensure_clockwise(rounded_ring(interior)),
            boundary_points,
        ));
    }

    let mut flat_points = Vec::<f64>::new();
    let mut hole_indices = Vec::<usize>::new();
    for (ring_index, ring) in point_rings.iter().enumerate() {
        if ring_index > 0 {
            hole_indices.push(flat_points.len() / 2);
        }
        for &(x, y) in ring {
            flat_points.push(x as f64);
            flat_points.push(y as f64);
        }
    }

    let indices = earcutr::earcut(&flat_points, &hole_indices, 2).map_err(|error| {
        ImportError::Triangulation {
            sector_index: source_sector,
            message: error.to_string(),
        }
    })?;

    let mut cells = Vec::new();
    for triangle in indices.chunks_exact(3) {
        let mut vertices = triangle
            .iter()
            .map(|index| {
                (
                    flat_points[index * 2].round() as i32,
                    flat_points[index * 2 + 1].round() as i32,
                )
            })
            .collect::<Vec<_>>();
        if polygon_signed_area(&vertices) > 0.0 {
            vertices.reverse();
        }
        cells.push(Cell {
            source_sector,
            vertices,
        });
    }

    Ok(cells)
}

fn find_player_start(things: &[Thing]) -> Option<Thing> {
    things.iter().copied().find(|thing| thing.thing_type == 1)
}

fn find_boundary_segment(
    segments: &[BoundarySegment],
    start: IntPoint,
    end: IntPoint,
) -> Option<&BoundarySegment> {
    segments
        .iter()
        .filter(|segment| {
            point_on_segment(start, segment.start, segment.end)
                && point_on_segment(end, segment.start, segment.end)
        })
        .min_by_key(|segment| segment_length_sq(segment.start, segment.end))
}

fn polygon_probe(polygon: &Polygon<f64>) -> Point<f64> {
    polygon
        .interior_point()
        .or_else(|| {
            polygon
                .exterior()
                .0
                .first()
                .map(|coord| Point::new(coord.x, coord.y))
        })
        .unwrap_or_else(|| Point::new(0.0, 0.0))
}

fn polygon_from_int_ring(vertices: &[IntPoint]) -> Polygon<f64> {
    Polygon::new(
        LineString::from(
            vertices
                .iter()
                .map(|(x, y)| (*x as f64, *y as f64))
                .collect::<Vec<_>>(),
        ),
        vec![],
    )
}

fn rounded_ring(line: &LineString<f64>) -> Vec<IntPoint> {
    let mut vertices = line
        .0
        .iter()
        .map(|coord| (coord.x.round() as i32, coord.y.round() as i32))
        .collect::<Vec<_>>();
    if vertices.len() > 1 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    dedupe_ring(&vertices)
}

fn sort_cells(cells: &mut [Cell]) {
    cells.sort_by(|left, right| {
        left.source_sector
            .cmp(&right.source_sector)
            .then_with(|| {
                right
                    .vertices
                    .iter()
                    .map(|(_, y)| *y)
                    .max()
                    .cmp(&left.vertices.iter().map(|(_, y)| *y).max())
            })
            .then_with(|| {
                left.vertices
                    .iter()
                    .map(|(x, _)| *x)
                    .min()
                    .cmp(&right.vertices.iter().map(|(x, _)| *x).min())
            })
            .then_with(|| left.vertices.cmp(&right.vertices))
    });
}

fn average_colors(colors: &[[u8; 3]]) -> [u8; 3] {
    let totals = colors.iter().fold([0_u64; 3], |mut totals, color| {
        totals[0] += color[0] as u64;
        totals[1] += color[1] as u64;
        totals[2] += color[2] as u64;
        totals
    });
    [
        (totals[0] as f64 / colors.len() as f64).round() as u8,
        (totals[1] as f64 / colors.len() as f64).round() as u8,
        (totals[2] as f64 / colors.len() as f64).round() as u8,
    ]
}

fn dedupe_ring(vertices: &[IntPoint]) -> Vec<IntPoint> {
    let mut ring = Vec::with_capacity(vertices.len());
    for &vertex in vertices {
        if ring.last().copied() != Some(vertex) {
            ring.push(vertex);
        }
    }
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    ring
}

fn ensure_clockwise(mut vertices: Vec<IntPoint>) -> Vec<IntPoint> {
    if polygon_signed_area(&vertices) > 0.0 {
        vertices.reverse();
    }
    vertices
}

fn ensure_counter_clockwise(mut vertices: Vec<IntPoint>) -> Vec<IntPoint> {
    if polygon_signed_area(&vertices) < 0.0 {
        vertices.reverse();
    }
    vertices
}

fn expand_ring(ring: &[IntPoint], boundary_points: &HashSet<IntPoint>) -> Vec<IntPoint> {
    let mut expanded = Vec::with_capacity(ring.len());
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        expanded.push(start);
        let mut intermediate = boundary_points
            .iter()
            .copied()
            .filter(|point| {
                *point != start && *point != end && point_on_segment(*point, start, end)
            })
            .collect::<Vec<_>>();
        intermediate.sort_by_key(|point| {
            let dx = point.0 as i64 - start.0 as i64;
            let dy = point.1 as i64 - start.1 as i64;
            dx * dx + dy * dy
        });
        expanded.extend(intermediate);
    }
    dedupe_ring(&expanded)
}

fn point_on_segment(point: IntPoint, start: IntPoint, end: IntPoint) -> bool {
    if point == start || point == end {
        return true;
    }
    let segment_x = end.0 as i64 - start.0 as i64;
    let segment_y = end.1 as i64 - start.1 as i64;
    let point_x = point.0 as i64 - start.0 as i64;
    let point_y = point.1 as i64 - start.1 as i64;
    if segment_x * point_y != segment_y * point_x {
        return false;
    }
    let dot = point_x * segment_x + point_y * segment_y;
    dot > 0 && dot < segment_x * segment_x + segment_y * segment_y
}

fn polygon_signed_area(vertices: &[IntPoint]) -> f64 {
    let mut area = 0.0;
    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        area += current.0 as f64 * next.1 as f64 - next.0 as f64 * current.1 as f64;
    }
    area * 0.5
}

fn is_convex_polygon(vertices: &[IntPoint]) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let mut turn_sign = 0_i32;
    for index in 0..vertices.len() {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        let cross = (current.0 - previous.0) as i64 * (next.1 - current.1) as i64
            - (current.1 - previous.1) as i64 * (next.0 - current.0) as i64;
        if cross == 0 {
            continue;
        }
        let sign = if cross < 0 { -1 } else { 1 };
        if turn_sign == 0 {
            turn_sign = sign;
        } else if sign != turn_sign {
            return false;
        }
    }
    turn_sign != 0
}

fn unordered_edge(a: IntPoint, b: IntPoint) -> EdgeKey {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn segment_length_sq(start: IntPoint, end: IntPoint) -> i64 {
    let dx = end.0 as i64 - start.0 as i64;
    let dy = end.1 as i64 - start.1 as i64;
    dx * dx + dy * dy
}

fn normalize_name(name: &str) -> String {
    name.trim_matches('\0').trim().to_ascii_uppercase()
}

fn decode_name(raw: &[u8]) -> String {
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).to_ascii_uppercase()
}

fn parse_texture_directory(
    texture_defs: &mut HashMap<String, TextureDef>,
    data: &[u8],
    patch_names: &[String],
    wad_path: &str,
) -> Result<(), ImportError> {
    let texture_count = read_i32(data, 0)? as usize;
    if data.len() < 4 + texture_count * 4 {
        return Err(ImportError::InvalidWad {
            path: wad_path.to_string(),
            message: "TEXTURE lump is truncated".into(),
        });
    }

    for texture_index in 0..texture_count {
        let offset = read_i32(data, 4 + texture_index * 4)? as usize;
        if offset + 22 > data.len() {
            return Err(ImportError::InvalidWad {
                path: wad_path.to_string(),
                message: "TEXTURE entry extends past end of lump".into(),
            });
        }
        let name = decode_name(&data[offset..offset + 8]);
        let width = read_i16(data, offset + 12)? as usize;
        let height = read_i16(data, offset + 14)? as usize;
        let patch_count = read_i16(data, offset + 20)? as usize;
        if offset + 22 + patch_count * 10 > data.len() {
            return Err(ImportError::InvalidWad {
                path: wad_path.to_string(),
                message: format!("texture {name} has truncated patch records"),
            });
        }

        let mut patches = Vec::with_capacity(patch_count);
        for patch_index in 0..patch_count {
            let patch_offset = offset + 22 + patch_index * 10;
            let patch_name_index = read_i16(data, patch_offset + 4)? as usize;
            let patch_name = patch_names.get(patch_name_index).ok_or_else(|| {
                ImportError::MissingPatchReference {
                    texture_name: name.clone(),
                    patch_index: patch_name_index,
                }
            })?;
            patches.push(TexturePatch {
                origin_x: read_i16(data, patch_offset)?,
                origin_y: read_i16(data, patch_offset + 2)?,
                patch_name: patch_name.clone(),
            });
        }

        texture_defs.insert(
            name,
            TextureDef {
                width,
                height,
                patches,
            },
        );
    }

    Ok(())
}

fn ensure_multiple(
    bytes: &[u8],
    chunk_size: usize,
    lump_name: &str,
    wad_path: &str,
) -> Result<(), ImportError> {
    if !bytes.len().is_multiple_of(chunk_size) {
        return Err(ImportError::InvalidWad {
            path: wad_path.to_string(),
            message: format!("lump {lump_name} has invalid length {}", bytes.len()),
        });
    }
    Ok(())
}

fn read_i16(bytes: &[u8], offset: usize) -> Result<i16, ImportError> {
    let chunk = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| ImportError::InvalidWad {
            path: "<memory>".into(),
            message: "unexpected end of lump".into(),
        })?;
    Ok(i16::from_le_bytes([chunk[0], chunk[1]]))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, ImportError> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ImportError::InvalidWad {
            path: "<memory>".into(),
            message: "unexpected end of lump".into(),
        })?;
    Ok(i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ImportError> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ImportError::InvalidWad {
            path: "<memory>".into(),
            message: "unexpected end of lump".into(),
        })?;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}
