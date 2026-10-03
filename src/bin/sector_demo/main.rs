//! Rebuild the shipped demo using only the existing map geometry and portals.
#[cfg(test)]
use sector::map::load_map_from_path;
use sector::map::{save_map_to_path, MapSector, MapVertex, MapWall, SectorMap};

const TRIM: [u8; 3] = [72, 72, 76];
const BASE: [u8; 3] = [44, 44, 48];

fn room(vertices: &[(f32, f32)], floor: f32, ceil: f32, color: [u8; 3]) -> MapSector {
    MapSector {
        floor,
        ceil,
        floor_color: [142, 164, 172],
        ceil_color: [236, 242, 240],
        no_ceiling: false,
        sky_color: None,
        vertices: vertices
            .iter()
            .map(|&(x, y)| MapVertex((x * 10000.).round() / 10000., (y * 10000.).round() / 10000.))
            .collect(),
        walls: vertices
            .iter()
            .map(|_| MapWall {
                color,
                portal: None,
                walkable: true,
                upper_color: Some(TRIM),
                lower_color: Some(BASE),
            })
            .collect(),
    }
}

fn rectangle(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    floor: f32,
    ceil: f32,
    color: [u8; 3],
) -> MapSector {
    room(
        &[(x0, y1), (x1, y1), (x1, y0), (x0, y0)],
        floor,
        ceil,
        color,
    )
}

fn add(map: &mut SectorMap, name: &str, sector: MapSector) -> usize {
    let id = map.sectors.len();
    println!("{id}: {name}");
    map.sectors.push(sector);
    id
}

fn rebuild_portals(map: &mut SectorMap) {
    // Split collinear boundaries at every authored corner so door widths and
    // adjacent partitions share exact reversed edges. Preserve convex winding.
    let corners = map
        .sectors
        .iter()
        .flat_map(|s| s.vertices.iter().copied())
        .collect::<Vec<_>>();
    for sector in &mut map.sectors {
        let mut vertices = Vec::new();
        for i in 0..sector.vertices.len() {
            let a = sector.vertices[i];
            let b = sector.vertices[(i + 1) % sector.vertices.len()];
            let dx = b.0 - a.0;
            let dy = b.1 - a.1;
            let length2 = dx * dx + dy * dy;
            let mut points = vec![(0.0, a)];
            for &p in &corners {
                let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2;
                let cross = (p.0 - a.0) * dy - (p.1 - a.1) * dx;
                if t > 0.00001 && t < 0.99999 && cross.abs() < 0.00001 {
                    points.push((t, p));
                }
            }
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            points.dedup_by(|a, b| a.1 == b.1);
            vertices.extend(points.into_iter().map(|(_, p)| p));
        }
        let wall = sector.walls[0];
        sector.walls = vertices
            .iter()
            .map(|_| MapWall {
                portal: None,
                ..wall
            })
            .collect();
        sector.vertices = vertices;
    }
    let mut links = Vec::new();
    for (i, a) in map.sectors.iter().enumerate() {
        for (j, b) in map.sectors.iter().enumerate().skip(i + 1) {
            if a.floor.max(b.floor) >= a.ceil.min(b.ceil) {
                continue;
            }
            for ai in 0..a.vertices.len() {
                for bi in 0..b.vertices.len() {
                    if a.vertices[ai] == b.vertices[(bi + 1) % b.vertices.len()]
                        && a.vertices[(ai + 1) % a.vertices.len()] == b.vertices[bi]
                    {
                        links.push((i, ai, j, bi));
                    }
                }
            }
        }
    }
    for (a, aw, b, bw) in links {
        assert!(
            map.sectors[a].walls[aw].portal.is_none(),
            "ambiguous boundary {a}:{aw} with {b}, existing {:?}",
            map.sectors[a].walls[aw].portal
        );
        assert!(
            map.sectors[b].walls[bw].portal.is_none(),
            "ambiguous boundary {b}:{bw}"
        );
        map.sectors[a].walls[aw].portal = Some(b);
        map.sectors[b].walls[bw].portal = Some(a);
    }
}

fn build_demo() -> SectorMap {
    let stone = [214, 230, 226];
    let teal = [104, 222, 218];
    let violet = [214, 148, 240];
    let amber = [246, 192, 100];
    let mut map = SectorMap {
        initial_sector: 0,
        initial_position: MapVertex(-1.5, -1.),
        initial_direction_degrees: 0.,
        sectors: Vec::new(),
    };
    let a = (-1.2, 6.);
    let b = (1.2, 6.);
    let c = (-2., 10.);
    let d = (0.4, 10.);
    add(
        &mut map,
        "angled foyer",
        room(
            &[
                (-3., 6.),
                a,
                b,
                (3., 6.),
                (5., 4.),
                (4., -2.),
                (-4., -2.),
                (-5., 1.),
            ],
            0.,
            3.2,
            stone,
        ),
    );
    add(
        &mut map,
        "entrance throat",
        room(&[b, a, c, d], 0., 3.2, stone),
    );
    add(
        &mut map,
        "transition hall",
        room(&[(-3., 14.), (0., 14.), d, c], 0., 3.2, stone),
    );
    let south = add(
        &mut map,
        "atrium entrance",
        rectangle(-8., 14., 8., 18., 0., 13., stone),
    );
    let west_lane = add(
        &mut map,
        "atrium west aisle",
        rectangle(-6., 18., -4., 25., 0., 13., stone),
    );
    let east = add(
        &mut map,
        "atrium east aisle",
        rectangle(4., 18., 8., 26., 0., 13., stone),
    );
    let north = add(
        &mut map,
        "atrium north aisle",
        rectangle(-8., 26., 8., 30., 0., 13., stone),
    );
    add(
        &mut map,
        "pavilion entrance",
        rectangle(-1.2, 18., 1.2, 18.2, 0., 3.2, teal),
    );
    add(
        &mut map,
        "pavilion south",
        rectangle(-3.8, 18.2, 3.8, 20., 0., 3.2, teal),
    );
    add(
        &mut map,
        "pavilion west",
        rectangle(-3.8, 20., -2., 24., 0., 3.2, teal),
    );
    add(
        &mut map,
        "pavilion east",
        rectangle(2., 20., 3.8, 24., 0., 3.2, teal),
    );
    add(
        &mut map,
        "pavilion north",
        rectangle(-3.8, 24., 3.8, 25.8, 0., 3.2, teal),
    );
    add(
        &mut map,
        "pavilion exit",
        rectangle(-1.2, 25.8, 1.2, 26., 0., 3.2, teal),
    );
    add(
        &mut map,
        "inner chamber entrance",
        rectangle(-0.6, 20., 0.6, 20.2, 0., 2.4, violet),
    );
    let core = add(
        &mut map,
        "inner chamber",
        rectangle(-1.8, 20.2, 1.8, 23.8, 0., 2.4, violet),
    );
    map.sectors[core].floor_color = [180, 122, 206];
    add(
        &mut map,
        "inner chamber exit",
        rectangle(-0.6, 23.8, 0.6, 24., 0., 2.4, violet),
    );
    add(
        &mut map,
        "hidden crouch shortcut",
        rectangle(3.8, 22., 4., 23., 0., 1.35, [120, 182, 248]),
    );
    for step in 0..18 {
        let y0 = 18. + step as f32 * 0.4;
        let floor = step as f32 * 0.2;
        add(
            &mut map,
            &format!("pavilion stair {step}"),
            rectangle(-8., y0, -6., y0 + 0.4, floor, floor + 3.2, violet),
        );
    }
    let loft = add(
        &mut map,
        "pavilion upper landing",
        rectangle(-8., 25.2, -4., 26., 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper chamber doorway",
        rectangle(-4., 25.2, -3.8, 25.8, 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper pavilion south",
        rectangle(-3.8, 18.2, 3.8, 20., 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper pavilion west",
        rectangle(-3.8, 20., -2., 24., 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper pavilion east",
        rectangle(2., 20., 3.8, 24., 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper pavilion north",
        rectangle(-3.8, 24., 3.8, 25.8, 3.6, 6.8, violet),
    );
    add(
        &mut map,
        "upper inner doorway",
        rectangle(-0.6, 20., 0.6, 20.2, 3.6, 6., amber),
    );
    add(
        &mut map,
        "upper inner chamber",
        rectangle(-1.8, 20.2, 1.8, 23.8, 3.6, 6., amber),
    );
    add(
        &mut map,
        "upper inner exit",
        rectangle(-0.6, 23.8, 0.6, 24., 3.6, 6., amber),
    );
    // Leave wall thickness where the ground aisle ends below the upper landing.
    add(
        &mut map,
        "west aisle bend",
        rectangle(-5.95, 25., -4., 25.95, 0., 3.2, stone),
    );
    let approach = add(
        &mut map,
        "stair approach",
        rectangle(8., 15., 16., 18., 0., 3.2, amber),
    );
    add(
        &mut map,
        "tower foot",
        rectangle(16., 15., 19., 26., 0., 3.2, amber),
    );
    let court = add(
        &mut map,
        "sky court",
        room(
            &[
                (10., 20.),
                (14., 20.),
                (15., 19.),
                (14., 18.),
                (10., 18.),
                (9., 19.),
            ],
            0.,
            12.,
            teal,
        ),
    );
    map.sectors[court].no_ceiling = true;
    map.sectors[court].sky_color = Some([170, 214, 244]);
    map.sectors[court].floor_color = [132, 206, 120];
    let point = |r: f32, step: usize| {
        let angle = (step % 24) as f32 * std::f32::consts::TAU / 24.;
        let round = |n: f32| (n * 10000.).round() / 10000.;
        (round(14. + r * angle.cos()), round(26. + r * angle.sin()))
    };
    for step in 0..48 {
        let floor = step as f32 * 0.2;
        let vertices = [
            point(2., step),
            point(2., step + 1),
            point(5., step + 1),
            point(5., step),
        ];
        let id = add(
            &mut map,
            &format!("spiral tread {step}"),
            room(&vertices, floor, floor + 3.2, amber),
        );
        map.sectors[id].floor_color = [208, 160, 92];
    }
    // A landing branches outward at the start of the second turn. Its polygon
    // follows the tower chord, then joins a straight corridor outside the ring.
    let p = point(5., 24);
    let q = point(5., 25);
    add(
        &mut map,
        "middle tower spur",
        room(&[p, q, (20.4, q.1), (20.4, p.1)], 4.8, 8., amber),
    );
    add(
        &mut map,
        "middle tower passage",
        rectangle(20.4, 26., 23.4, 31., 4.8, 8., amber),
    );
    add(
        &mut map,
        "middle east passage",
        rectangle(8., 31., 23.4, 34., 4.8, 8., amber),
    );
    let gallery = add(
        &mut map,
        "atrium viewing gallery",
        rectangle(-8., 30., 8., 34., 4.8, 13., teal),
    );
    add(
        &mut map,
        "tower crown",
        rectangle(16., 26., 19., 34., 9.6, 13., amber),
    );
    add(
        &mut map,
        "highest east passage",
        rectangle(8.05, 31., 16., 34., 9.6, 13., amber),
    );
    let high = add(
        &mut map,
        "high overlook",
        room(
            &[
                (8., 30.95),
                (8.05, 31.),
                (9., 31.),
                (9., 18.05),
                (8., 18.05),
            ],
            9.6,
            13.,
            violet,
        ),
    );
    for step in 0..24 {
        let right = -8. - step as f32 * 0.6;
        let left = -8. - (step + 1) as f32 * 0.6;
        let floor = (23 - step) as f32 * 0.2;
        add(
            &mut map,
            &format!("return tread {step}"),
            rectangle(left, 31., right, 34., floor, floor + 3.2, teal),
        );
    }
    add(
        &mut map,
        "return bend",
        rectangle(-25.4, 31., -22.4, 34., 0., 3.2, teal),
    );
    let return_hall = add(
        &mut map,
        "return hall",
        rectangle(-25.4, 14., -22.4, 31., 0., 3.2, teal),
    );
    add(
        &mut map,
        "return cross hall",
        rectangle(-25.4, c.1, -8., 14., 0., 3.2, stone),
    );
    add(
        &mut map,
        "return to entrance",
        room(&[(-8., 14.), (-3., 14.), c, (-8., c.1)], 0., 3.2, stone),
    );
    // A concave star court is one room assembled from an octagonal centre and
    // eight convex petals. Alternate roofs and shallow dais floors vary its volume.
    let flower_point = |r: f32, n: usize| {
        let angle = (n % 8) as f32 * std::f32::consts::TAU / 8.;
        let round = |v: f32| (v * 10000.).round() / 10000.;
        (round(-34. + r * angle.cos()), round(21.5 + r * angle.sin()))
    };
    let center_vertices = (0..8)
        .rev()
        .map(|i| flower_point(3., i))
        .collect::<Vec<_>>();
    let flower_center = add(
        &mut map,
        "star court centre",
        room(&center_vertices, 0., 9., teal),
    );
    map.sectors[flower_center].no_ceiling = true;
    map.sectors[flower_center].sky_color = Some([170, 214, 244]);
    for i in 0..8 {
        let outer = |n: usize| flower_point(if n % 2 == 0 { 7. } else { 4.3 }, n);
        let floor = if i % 3 == 0 { 0.4 } else { 0. };
        add(
            &mut map,
            &format!("star court petal {i}"),
            room(
                &[
                    flower_point(3., i),
                    flower_point(3., i + 1),
                    outer(i + 1),
                    outer(i),
                ],
                floor,
                if i % 2 == 0 { 9. } else { 3.6 },
                [teal, violet, amber, [176, 238, 120]][i % 4],
            ),
        );
    }
    let u = flower_point(7., 0);
    let v = flower_point(4.3, 1);
    add(
        &mut map,
        "star court entrance",
        room(&[u, v, (-25.4, v.1), (-25.4, u.1)], 0., 3.6, amber),
    );
    rebuild_portals(&mut map);
    for (i, s) in map.sectors.iter_mut().enumerate() {
        for wall in &mut s.walls {
            wall.upper_color = wall.portal.map(|_| TRIM);
            wall.lower_color = wall.portal.map(|_| BASE);
            if (i == north && wall.portal == Some(gallery))
                || (i == gallery && wall.portal == Some(north))
                || (i == high && [Some(east), Some(north), Some(gallery)].contains(&wall.portal))
                || ([east, north, gallery].contains(&i) && wall.portal == Some(high))
                || (i == loft && wall.portal == Some(north))
                || (i == north && wall.portal == Some(loft))
                || (i == west_lane && wall.portal.is_some_and(|p| (17..35).contains(&p)))
                || ((17..35).contains(&i) && wall.portal == Some(west_lane))
            {
                wall.walkable = false;
            }
        }
    }
    assert!(map.sectors[south].walls.iter().any(|w| w.portal == Some(2)));
    assert!(map.sectors[approach]
        .walls
        .iter()
        .any(|w| w.portal == Some(south)));
    assert!(map.sectors[return_hall]
        .walls
        .iter()
        .any(|w| w.portal == Some(map.sectors.len() - 1)));
    // A shared affine skew preserves convexity, coincidence and winding while
    // giving every rectilinear hall oblique walls. The foyer, tower and petals
    // already use several other wall directions.
    let skew = |p: MapVertex| MapVertex(p.0 + 0.22 * p.1, p.1 + 0.08 * p.0);
    for s in &mut map.sectors {
        for p in &mut s.vertices {
            *p = skew(*p);
        }
    }
    map.initial_position = skew(map.initial_position);
    let door = skew(MapVertex(0., 6.));
    map.initial_direction_degrees = (-(door.0 - map.initial_position.0))
        .atan2(door.1 - map.initial_position.1)
        .to_degrees();
    map
}

fn main() {
    save_map_to_path(&build_demo(), "assets/maps/default.map.pb").expect("demo must validate");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_demo_matches_generator() {
        let mut generated = build_demo();
        let shipped = load_map_from_path("assets/maps/default.map.pb").unwrap();
        assert!(
            (generated.initial_direction_degrees - shipped.initial_direction_degrees).abs()
                < 0.0001
        );
        // Trigonometric facing may differ by one ULP across platform math libraries.
        generated.initial_direction_degrees = shipped.initial_direction_degrees;
        assert_eq!(
            ron::to_string(&generated).unwrap(),
            ron::to_string(&shipped).unwrap()
        );
    }
}
