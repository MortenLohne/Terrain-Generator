use crate::voronoi::Voronoi;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::iter::FromIterator;
use std::mem;

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
struct LakeBuilder {
    water_level: f64,
    area: usize,
    highest_shore_point: usize,
    shores: BinaryHeap<LakeShorePoint>,
}

#[derive(Serialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct Lake {
    water_level: f64,
    area: usize,
    highest_shore_point: usize,
}

fn merge_lakes(
    lake_id: usize,
    other_lake_id: usize,
    lakes: &mut [LakeBuilder],
    lake_associations: &mut [Option<usize>],
) {
    let other_lake = mem::take(&mut lakes[other_lake_id]);

    // Transfer the old lake's shore points to the new lake
    for lake_shore_point in other_lake.shores {
        lakes[lake_id].shores.push(lake_shore_point);
    }

    // Transfer all points over to the new lake
    for old_lake_id in lake_associations.iter_mut().filter_map(|o| o.as_mut()) {
        if *old_lake_id == other_lake_id {
            *old_lake_id = lake_id;
        }
    }

    // Subtract one, to avoid counting the point of merger twice
    lakes[lake_id].area += other_lake.area - 1;
}

fn expand_lake(
    lake_id: usize,
    heights: &[f64],
    voronoi: &Voronoi,
    lakes: &mut [LakeBuilder],
    lake_associations: &mut [Option<usize>],
) {
    let next_shore = lakes[lake_id].shores.pop().unwrap();

    // Duplicate shore points may show up in the queue. Throw them away.
    while lakes[lake_id].shores.peek().cloned() == Some(next_shore) {
        lakes[lake_id].shores.pop();
    }

    if let Some(other_lake_id) = lake_associations[next_shore.id] {
        merge_lakes(lake_id, other_lake_id, lakes, lake_associations);
    }

    lakes[lake_id].water_level = next_shore.height;
    lakes[lake_id].area += 1;
    lakes[lake_id].highest_shore_point = next_shore.id;
    lake_associations[next_shore.id] = Some(lake_id);

    // Check if the lake can expand further from this point
    if voronoi.adjacent[next_shore.id].iter().all(|neighbour| {
        heights[*neighbour] >= next_shore.height || lake_associations[*neighbour] == Some(lake_id)
    }) && !voronoi.is_on_map_border(next_shore.id)
    {
        // Add the new point's neighbours to the lake's shore
        for neighbour in voronoi.adjacent[next_shore.id].iter() {
            if lake_associations[*neighbour] != Some(lake_id) {
                lakes[lake_id].shores.push(LakeShorePoint {
                    id: *neighbour,
                    height: heights[*neighbour],
                });
            }
        }

        expand_lake(lake_id, heights, voronoi, lakes, lake_associations)
    }
}

pub fn generate_lakes(heights: &[f64], voronoi: &Voronoi, sea_level: f64) -> Vec<Option<Lake>> {
    let mut lake_associations = vec![None; heights.len()];

    let mut lake_builders = vec![];

    for (i, height) in heights.iter().enumerate() {
        if *height > sea_level
            && lake_associations[i].is_none()
            && voronoi.adjacent[i]
                .iter()
                .all(|neighbour| heights[*neighbour] > *height)
        {
            let shores =
                BinaryHeap::from_iter(voronoi.adjacent[i].iter().map(|j| LakeShorePoint {
                    id: *j,
                    height: heights[*j],
                }));

            lake_builders.push(LakeBuilder {
                water_level: *height,
                area: 1,
                shores,
                highest_shore_point: i,
            });

            let lake_id = lake_builders.len() - 1;
            lake_associations[i] = Some(lake_id);

            expand_lake(
                lake_id,
                heights,
                voronoi,
                &mut lake_builders,
                &mut lake_associations,
            );
        }
    }

    let lakes = lake_associations
        .into_iter()
        .map(|o| {
            o.map(|lake_id| {
                let lake_builder = lake_builders.get(lake_id).unwrap();
                Lake {
                    water_level: lake_builder.water_level,
                    area: lake_builder.area,
                    highest_shore_point: lake_builder.highest_shore_point,
                }
            })
        })
        .collect();

    lakes
}
