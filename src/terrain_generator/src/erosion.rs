use crate::voronoi::Voronoi;
use std::collections::BinaryHeap;
use std::iter::FromIterator;
use std::mem;
use wasm_bindgen::__rt::core::cmp::Ordering;
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub fn get_flux(heights: &Vec<f64>, adjacent: &Vec<Vec<usize>>) -> Vec<f64> {
    let mut flux = vec![0.; heights.len()];

    let mut sorted = heights
        .clone()
        .into_iter()
        .enumerate()
        .collect::<Vec<(usize, f64)>>();
    sorted.sort_unstable_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

    // find downhill for each point.
    for &(k, height) in sorted.iter().rev() {
        let mut lowest: Option<usize> = None;
        for &n in adjacent[k].iter() {
            if heights[n] < height {
                lowest = Some(match lowest {
                    Some(low) => {
                        if heights[n] < heights[low] {
                            n
                        } else {
                            low
                        }
                    }
                    None => n,
                });
            }
        }
        if let Some(neighbor) = lowest {
            flux[neighbor] = flux[neighbor] + flux[k] + 1.;
        }
    }
    flux
}

pub fn fill_sinks(heights: Vec<f64>, adjacent: &Vec<Vec<usize>>, sea_level: f64) -> Vec<f64> {
    // Mewo implementation details: https://mewo2.com/notes/terrain/
    // Original paper: https://horizon.documentation.ird.fr/exl-doc/pleins_textes/pleins_textes_7/sous_copyright/010031925.pdf
    let epsilon = 1e-5;

    let mut new_heights: Vec<f64> = heights
        .clone()
        .iter()
        .map(|&height| {
            if height > sea_level {
                f64::INFINITY
            } else {
                height
            }
        })
        .collect();

    let mut sorted: Vec<(usize, f64)> = heights.clone().into_iter().enumerate().collect();
    sorted.sort_unstable_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

    let mut changed = true;
    while changed {
        changed = false;

        for &(i, height) in sorted.iter() {
            if new_heights[i] == height {
                continue;
            }

            let neighbors = &adjacent[i];
            for &neighbor in neighbors.iter() {
                let other = new_heights[neighbor] + epsilon;

                if height >= other {
                    new_heights[i] = height;
                    changed = true;
                    break;
                }

                if new_heights[i] > other && other > height {
                    new_heights[i] = other;
                    changed = true;
                }
            }
        }
    }

    new_heights
}

pub fn plateau(points: &Vec<f64>, mut heights: Vec<f64>) -> Vec<f64> {
    let plateau_start = 0.45; // Magic
    let plateau_cap = (1. - plateau_start) / 4.; // Magic

    let mut peak_index = 0;
    for (j, &height) in heights.iter().enumerate() {
        if height > heights[peak_index] {
            peak_index = j;
        }
    }
    let peak_x = points[peak_index * 2 + 0];
    let peak_y = points[peak_index * 2 + 1];

    let interpolate = |i: f64| {
        plateau_start
            + (1. - (1. - (i - plateau_start) / (1. - plateau_start)).powi(2)) * plateau_cap
    };

    for i in 0..heights.len() {
        let height = heights[i];

        let x = points[i * 2 + 0];
        let y = points[i * 2 + 1];

        let distance_to_peak = ((x - peak_x).hypot(y - peak_y).min(0.5) / 0.5).powi(2);
        heights[i] = (1. - distance_to_peak) * height + distance_to_peak * interpolate(height);
    }

    heights
}

#[derive(Clone, Copy, Default, Serialize, Debug, PartialEq)]
struct LakeShorePoint {
    id: usize,
    height: f64,
}

impl Eq for LakeShorePoint {}

impl PartialOrd for LakeShorePoint {
    /// This ordering is reversed, for use in a PQ
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.height
                .partial_cmp(&other.height)
                .unwrap()
                .reverse()
                .then(self.id.cmp(&other.id)),
        )
    }
}

impl Ord for LakeShorePoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(&other).unwrap()
    }
}

#[derive(Serialize, Default, Debug)]
pub struct Lake {
    water_level: f64,
    area: usize,
    shores: BinaryHeap<LakeShorePoint>,
}

fn expand_lake(
    id: usize,
    heights: &[f64],
    voronoi: &Voronoi,
    lakes: &mut [Lake],
    lake_associations: &mut [Option<usize>],
    sea_level: f64,
) {
    let lake_id = lake_associations[id].unwrap();

    if lakes[lake_id].shores.is_empty() {
        println!("No more for lake {}", lake_id);
        return;
    }

    let next_shore = *lakes[lake_id].shores.peek().unwrap();

    log!("Next shore: {:?}", next_shore);

    if let Some(other_lake_id) = lake_associations[next_shore.id] {
        assert_ne!(lake_id, other_lake_id, "Next shore: {:?}", next_shore);
        // if lakes[other_lake_id].shores.peek().unwrap().id == next_shore.id {
        log!(
            "Trying to merge at point {} height {}, shores {:?}",
            next_shore.id,
            next_shore.height,
            lakes[other_lake_id].shores
        );

        log!("Merging lakes {} and {}", lake_id, other_lake_id);

        assert!(
            (next_shore.height - lakes[other_lake_id].water_level).abs() < f64::EPSILON,
            "Tried to merge at height {:.6} into a lake of height {:.6}",
            next_shore.height,
            lakes[other_lake_id].water_level
        );

        // If the new point was already the shore of another lake,
        // merge the lakes.
        // Merge the smaller lake into the larger lake
        let (smaller_lake, smaller_lake_id, larger_lake_id) =
            if lakes[lake_id].area <= lakes[other_lake_id].area {
                (mem::take(&mut lakes[lake_id]), lake_id, other_lake_id)
            } else {
                (mem::take(&mut lakes[other_lake_id]), other_lake_id, lake_id)
            };

        for lake_shore_point in smaller_lake.shores {
            lakes[larger_lake_id].shores.push(lake_shore_point);
        }

        for lake_id in lake_associations.iter_mut().filter_map(|o| o.as_mut()) {
            if *lake_id == smaller_lake_id {
                *lake_id = larger_lake_id;
            }
        }

        lakes[larger_lake_id].water_level = lakes[larger_lake_id]
            .water_level
            .max(smaller_lake.water_level);
        lakes[larger_lake_id].area += smaller_lake.area;

        while lakes[larger_lake_id].shores.peek().cloned() == Some(next_shore) {
            lakes[larger_lake_id].shores.pop();
        }
        expand_lake(id, heights, voronoi, lakes, lake_associations, sea_level)
    } else if voronoi.adjacent[next_shore.id].iter().all(|neighbour| {
        heights[*neighbour] >= next_shore.height || lake_associations[*neighbour] == Some(lake_id)
    }) && !voronoi.is_on_map_border(next_shore.id)
    {
        if let Some(other_lake_id) = lake_associations[next_shore.id] {
            assert!(lakes[other_lake_id].water_level <= lakes[lake_id].water_level);
            assert!(
                lakes[other_lake_id].water_level <= next_shore.height,
                "Found shore with height {:.6}, but inside lake {} with water level {:.6}",
                next_shore.height,
                other_lake_id,
                lakes[other_lake_id].water_level
            );
        }
        log!("Expanding lake {}", lake_id);
        lakes[lake_id].water_level = next_shore.height;
        lakes[lake_id].area += 1;
        lake_associations[next_shore.id] = Some(lake_id);

        log!(
            "Raised lake {} to {:.6}",
            lake_id,
            lakes[lake_id].water_level
        );

        // Add the new point's neighbours to the lake's shore
        for neighbour in voronoi.adjacent[next_shore.id].iter() {
            match lake_associations[*neighbour] {
                None => lakes[lake_id].shores.push(LakeShorePoint {
                    id: *neighbour,
                    height: heights[*neighbour],
                }),
                Some(other_lake_id) if other_lake_id != lake_id => {
                    log!(
                            "Found edge between height {:.6} and height {:.6} between lakes {} and {} with water heights {:.6} and {:.6}",
                            heights[next_shore.id],
                            heights[*neighbour],
                            lake_id,
                            other_lake_id,
                            lakes[lake_id].water_level,
                            lakes[other_lake_id].water_level
                        );
                    lakes[lake_id].shores.push(LakeShorePoint {
                        id: *neighbour,
                        height: heights[*neighbour],
                    });
                }
                Some(_) => (),
            }
        }

        while lakes[lake_id].shores.peek().cloned() == Some(next_shore) {
            lakes[lake_id].shores.pop();
        }
        expand_lake(
            next_shore.id,
            heights,
            voronoi,
            lakes,
            lake_associations,
            sea_level,
        )
    } else {
        log!("Expanding lake {} for the last time", lake_id);
        lakes[lake_id].water_level = next_shore.height;
        lakes[lake_id].area += 1;
        lake_associations[next_shore.id] = Some(lake_id);

        log!(
            "Raised lake {} to {:.6}",
            lake_id,
            lakes[lake_id].water_level
        );

        while lakes[lake_id].shores.peek().cloned() == Some(next_shore) {
            lakes[lake_id].shores.pop();
        }
    }
}

pub fn fill_lakes(heights: &[f64], voronoi: &Voronoi, sea_level: f64) -> Vec<Option<usize>> {
    let mut lake_affiliation = vec![None; heights.len()];

    let mut lakes = vec![];

    for (i, height) in heights.iter().enumerate() {
        if *height > sea_level
            && lake_affiliation[i].is_none()
            && voronoi.adjacent[i]
                .iter()
                .all(|neighbour| heights[*neighbour] > *height)
        {
            let shores =
                BinaryHeap::from_iter(voronoi.adjacent[i].iter().map(|j| LakeShorePoint {
                    id: *j,
                    height: heights[*j],
                }));
            let lowest_shore = shores.peek().unwrap().height;

            lakes.push(Lake {
                water_level: lowest_shore,
                area: 1,
                shores,
            });

            lake_affiliation[i] = Some(lakes.len() - 1);

            expand_lake(
                i,
                heights,
                voronoi,
                &mut lakes,
                &mut lake_affiliation,
                sea_level,
            );
        }
    }

    /*
    for (i, height) in heights.iter().enumerate() {
        if *height < sea_level || voronoi.is_on_map_border(i) {
            stack.push(i);
        }
    }

    while let Some(i) = stack.pop() {
        if !visited[i] {
            visited[i] = true;
            lakes[i] = false;
            if heights[i] > sea_level {
                log!(
                    "Made surface tile {} with height {} into non-lake with sea level {}",
                    i,
                    heights[i],
                    sea_level
                )
            } else {
                log!(
                    "Made sea tile {} with height {} into non-lake with sea level {}",
                    i,
                    heights[i],
                    sea_level
                )
            }
            stack.extend(
                voronoi.adjacent[i]
                    .iter()
                    .filter(|j| heights[**j] > heights[i]),
            )
        }
    }

    for i in 0..lakes.len() {
        if lakes[i]
            && voronoi.adjacent[i]
                .iter()
                .all(|neighbour| !lakes[*neighbour])
        {
            lakes[i] = false;
            log!("Removed microlake");
        }
    }
    */

    // assert!(lakes.iter().any(|b| *b));

    lake_affiliation
}

pub fn erode(heights: Vec<f64>, adjacent: &Vec<Vec<usize>>, sea_level: f64) -> Vec<f64> {
    // let heights = smooth_coasts(heights, adjacent, sea_level);
    let heights = smooth(heights, adjacent);
    // let heights = fill_sinks(heights, adjacent, sea_level);

    let flux = get_flux(&heights, adjacent);
    // let n = heights.len() as f64;

    let erosion_rate = 0.015;
    // let erosion_rate = 0.0125;
    // let flux_exponent = 2500 as i32;

    // let erosion = |(i, height): (usize, f64)| {
    //     let underwater_discount = if height < sea_level
    //         { 1e4_f64.powf(height - sea_level) } else { 1. };
    //     let point_flux = 1. - (1. - flux[i] / n).powi(flux_exponent);
    //     height - point_flux * point_flux * erosion_rate * underwater_discount
    // };

    // let erosion = |(i, height): (usize, f64)| {
    //     let mut height_discount = height;
    //     // let near_coast_discount = (1. - (1. - (height - sea_level).abs() * 50.)).min(1.).max(0.3);
    //     if height < sea_level { height_discount = height_discount.powi(2) };
    //     let point_flux = (flux[i] + 1.).ln();
    //     height - (point_flux * erosion_rate * height_discount)
    // };
    let adjacent = adjacent
        .iter()
        .map(|arr| arr.iter().map(|n| heights[*n]).collect::<Vec<f64>>())
        .collect::<Vec<Vec<f64>>>();

    let erosion = |(i, height): (usize, f64)| {
        let point_flux = (flux[i] + 1.).ln();

        let erosion = point_flux * erosion_rate * height;

        if height >= sea_level {
            let low = adjacent[i]
                .iter()
                .cloned()
                .fold(0. / 0., f64::min)
                .min(height);

            let eroded = height - erosion;
            let alpha = 0.125;

            low.max(eroded) * (1. - alpha) + eroded * alpha
        } else {
            height - erosion * 0.25
        }
    };

    let heights = heights
        .into_iter()
        .enumerate()
        .map(erosion)
        .collect::<Vec<f64>>();

    heights
}

pub fn smooth(mut heights: Vec<f64>, adjacent: &Vec<Vec<usize>>) -> Vec<f64> {
    let alpha = 1.;
    let alpha = 0.66;

    for (i, height) in heights
        .clone()
        .into_iter()
        .enumerate()
        .collect::<Vec<(usize, f64)>>()
    {
        let sum = adjacent[i].iter().map(|n| heights[*n]).sum::<f64>() + height;

        let mean = sum / (adjacent[i].len() + 1) as f64;

        heights[i] = height * (1. - alpha) + mean * alpha;

        for n in adjacent[i].iter() {
            heights[*n] = heights[*n] * (1. - alpha) + mean * alpha;
        }
    }

    heights
}

pub fn smooth_coasts(
    mut heights: Vec<f64>,
    adjacent: &Vec<Vec<usize>>,
    sea_level: f64,
) -> Vec<f64> {
    let alpha = 0.25;
    let mut sorted = heights
        .clone()
        .into_iter()
        .enumerate()
        .collect::<Vec<(usize, f64)>>();

    sorted.sort_unstable_by(|(_, a), (_, b)| {
        (a - sea_level)
            .abs()
            .partial_cmp(&(b - sea_level).abs())
            .unwrap()
    });

    for &(i, height) in sorted.iter() {
        if (height - sea_level).abs() > 0.015 {
            break;
        }

        let sum = adjacent[i].iter().map(|n| heights[*n]).sum::<f64>() + height;

        let mean = sum / (adjacent[i].len() + 1) as f64;

        heights[i] = height * (1. - alpha) + mean * alpha;

        for n in adjacent[i].iter() {
            heights[*n] = heights[*n] * (1. - alpha) + mean * alpha;
        }
    }

    heights
}
