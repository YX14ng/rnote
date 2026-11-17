//! Grouping the writing into lines and fragments (about a word each), the unit the
//! recognizer reads. Rnote keeps no times in its strokes, so the times the recognizer asks
//! for are made up from the order of the strokes and their points.

use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::penpath::Segment;
use rnote_engine::Engine;
use rnote_engine::strokes::Stroke;

/// How far apart two strokes on one line may be and still be one fragment, in median
/// stroke heights.
const FRAGMENT_GAP: f64 = 0.6;
/// How far above or below a line a stroke may be and still belong to it (the dot of an i,
/// an accent), in median stroke heights.
const LINE_GAP: f64 = 0.4;
/// The made-up time between two points of a stroke, in milliseconds.
const POINT_MS: i64 = 10;
/// The made-up pause between two strokes, in milliseconds.
const STROKE_MS: i64 = 100;
/// The smallest stroke height counted, so dots don't make the median zero.
const MIN_HEIGHT: f64 = 1.0;

/// A stroke written with the pen, as the points it goes through.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InkStroke {
    pub points: Vec<(f64, f64)>,
}

/// Strokes the recognizer reads together.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fragment {
    /// The SHA-256 of its points, rounded to a hundredth: the same writing, the same hash.
    pub hash: String,
    /// Its bounds: left, top, right, bottom.
    pub rect: [f64; 4],
    /// The line it is on, counting from the top.
    pub line: usize,
    /// Its strokes in writing order, each point with a made-up time in milliseconds.
    pub strokes: Vec<Vec<(f64, f64, i64)>>,
}

/// The pen strokes of the document, in the order they were drawn.
pub(crate) fn ink_strokes(engine: &Engine) -> Vec<InkStroke> {
    let everything = Aabb::new(Vector2::splat(-1e9), Vector2::splat(1e9));
    engine
        .extract_content_in_bounds(everything)
        .strokes
        .iter()
        .filter_map(|layered| match layered.stroke.as_ref() {
            Stroke::BrushStroke(brush) => {
                let path = &brush.path;
                let mut points = vec![(path.start.pos.x, path.start.pos.y)];
                points.extend(path.segments.iter().map(|segment| {
                    let end = match segment {
                        Segment::LineTo { end }
                        | Segment::QuadBezTo { end, .. }
                        | Segment::CubBezTo { end, .. } => end,
                    };
                    (end.pos.x, end.pos.y)
                }));
                Some(InkStroke { points })
            }
            _ => None,
        })
        .collect()
}

/// The fragments of `strokes`, line by line from the top and left to right in each line.
pub(crate) fn fragments(strokes: &[InkStroke]) -> Vec<Fragment> {
    let boxes: Vec<(usize, [f64; 4])> = strokes
        .iter()
        .enumerate()
        .filter_map(|(index, stroke)| bounds(&stroke.points).map(|rect| (index, rect)))
        .collect();
    if boxes.is_empty() {
        return Vec::new();
    }
    let height = median_height(&boxes);
    let mut found = Vec::new();
    for (line, members) in lines(&boxes, height).into_iter().enumerate() {
        for group in split_fragments(&members, &boxes, height) {
            found.push(fragment(strokes, &group, line));
        }
    }
    found
}

fn bounds(points: &[(f64, f64)]) -> Option<[f64; 4]> {
    let (&(x, y), rest) = points.split_first()?;
    Some(rest.iter().fold([x, y, x, y], |[x0, y0, x1, y1], &(x, y)| {
        [x0.min(x), y0.min(y), x1.max(x), y1.max(y)]
    }))
}

/// The median height of the strokes (the upper one of the two middle ones).
fn median_height(boxes: &[(usize, [f64; 4])]) -> f64 {
    let mut heights: Vec<f64> = boxes.iter().map(|(_, r)| (r[3] - r[1]).max(MIN_HEIGHT)).collect();
    heights.sort_by(f64::total_cmp);
    heights[heights.len() / 2]
}

/// The strokes of each line, as positions in `boxes`, lines from the top.
fn lines(boxes: &[(usize, [f64; 4])], height: f64) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| boxes[a].1[1].total_cmp(&boxes[b].1[1]));
    // Each line with its vertical extent so far.
    let mut lines: Vec<(f64, f64, Vec<usize>)> = Vec::new();
    for position in order {
        let [_, top, _, bottom] = boxes[position].1;
        let near = lines.iter_mut().rev().find(|(line_top, line_bottom, _)| {
            let gap = (top - *line_bottom).max(*line_top - bottom).max(0.0);
            gap <= LINE_GAP * height
        });
        match near {
            Some((line_top, line_bottom, members)) => {
                *line_top = line_top.min(top);
                *line_bottom = line_bottom.max(bottom);
                members.push(position);
            }
            None => lines.push((top, bottom, vec![position])),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines.into_iter().map(|(_, _, members)| members).collect()
}

/// The strokes of a line split where the gap is wide, each group in writing order.
fn split_fragments(members: &[usize], boxes: &[(usize, [f64; 4])], height: f64) -> Vec<Vec<usize>> {
    let mut by_x = members.to_vec();
    by_x.sort_by(|&a, &b| boxes[a].1[0].total_cmp(&boxes[b].1[0]));
    let mut groups: Vec<(f64, Vec<usize>)> = Vec::new();
    for position in by_x {
        let [left, _, right, _] = boxes[position].1;
        match groups.last_mut() {
            Some((group_right, group)) if left - *group_right <= FRAGMENT_GAP * height => {
                *group_right = group_right.max(right);
                group.push(position);
            }
            _ => groups.push((right, vec![position])),
        }
    }
    groups
        .into_iter()
        .map(|(_, mut group)| {
            // Writing order: the order the strokes were drawn in.
            group.sort_by_key(|&position| boxes[position].0);
            group.into_iter().map(|position| boxes[position].0).collect()
        })
        .collect()
}

fn fragment(strokes: &[InkStroke], group: &[usize], line: usize) -> Fragment {
    let mut timed = Vec::with_capacity(group.len());
    let mut key = String::new();
    let mut time = 0;
    let mut rect: Option<[f64; 4]> = None;
    for (n, &index) in group.iter().enumerate() {
        if n > 0 {
            time += STROKE_MS - POINT_MS;
            key.push('|');
        }
        let points = &strokes[index].points;
        let mut stroke = Vec::with_capacity(points.len());
        for (i, &(x, y)) in points.iter().enumerate() {
            if i > 0 || n > 0 {
                time += POINT_MS;
            }
            stroke.push((x, y, time));
            key.push_str(&format!("{x:.2},{y:.2};"));
        }
        if let Some(bounds) = bounds(points) {
            rect = Some(match rect {
                None => bounds,
                Some(r) => [
                    r[0].min(bounds[0]),
                    r[1].min(bounds[1]),
                    r[2].max(bounds[2]),
                    r[3].max(bounds[3]),
                ],
            });
        }
        timed.push(stroke);
    }
    Fragment {
        hash: rnote_sync::sha256_hex(key.as_bytes()),
        rect: rect.unwrap_or_default(),
        line,
        strokes: timed,
    }
}
