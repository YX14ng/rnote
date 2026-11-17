//! Grouping the writing into lines and fragments (about a word each), the unit the
//! recognizer reads. Rnote keeps no times in its strokes, so the times the recognizer asks
//! for are made up from the order of the strokes and their points.

use parry2d_f64::bounding_volume::Aabb;
use parry2d_f64::math::Vector2;
use rnote_compose::penpath::Segment;
use rnote_engine::Engine;
use rnote_engine::store::chrono_comp::StrokeLayer;
use rnote_engine::strokes::Stroke;

/// How far apart two strokes on one line may be and still be one fragment, in median
/// stroke heights.
const FRAGMENT_GAP: f64 = 0.6;
/// How far the vertical center of a stroke may be from its line's, in median stroke heights.
const LINE_BAND: f64 = 0.5;
/// How far the center of a small mark (the dot of an i, an accent) may be from the center of
/// the line it belongs to, in median stroke heights.
const MARK_REACH: f64 = 1.0;
/// Strokes smaller than this both ways are marks, in median stroke heights.
const MARK_SIZE: f64 = 0.5;
/// Strokes taller than this reach above or below their line (an l, the tail of a g), in
/// median stroke heights.
const RAISED: f64 = 1.5;
/// Strokes taller than this (a bracket, a box, a line down the margin) are a line of their
/// own, so they don't pull lines together, in median stroke heights.
const TALL: f64 = 2.5;
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

/// The pen strokes of the document, in the order they were drawn; highlighter strokes
/// aren't writing.
pub(crate) fn ink_strokes(engine: &Engine) -> Vec<InkStroke> {
    let everything = Aabb::new(Vector2::splat(-1e9), Vector2::splat(1e9));
    engine
        .extract_content_in_bounds(everything)
        .strokes
        .iter()
        .filter(|layered| layered.layer != StrokeLayer::Highlighter)
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
    // Points that aren't numbers would poison the bounds and the JSON.
    let strokes: Vec<InkStroke> = strokes
        .iter()
        .map(|stroke| InkStroke {
            points: stroke
                .points
                .iter()
                .copied()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .collect(),
        })
        .collect();
    let strokes = strokes.as_slice();
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
///
/// A stroke belongs to the line whose vertical center is nearest to its own, if near
/// enough or if the stroke goes across that center (an l reaching above the line, the tail
/// of a g below it): the extent of a line never grows, so descenders of one line and
/// ascenders of the next don't join them. Marks (dots, accents) go to the nearest line
/// afterwards, and very tall strokes are lines of their own.
fn lines(boxes: &[(usize, [f64; 4])], height: f64) -> Vec<Vec<usize>> {
    let center = |position: usize| (boxes[position].1[1] + boxes[position].1[3]) / 2.0;
    let size = |position: usize| {
        let [left, top, right, bottom] = boxes[position].1;
        (right - left, bottom - top)
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| center(a).total_cmp(&center(b)));
    // Each line as the sum of its members' centers, and its members.
    let mut lines: Vec<(f64, Vec<usize>)> = Vec::new();
    let mut marks = Vec::new();
    let mut raised = Vec::new();
    // The letters of common height make the lines; the ones reaching above or below them
    // (an l, the tail of a g) would pull a line's center off, so they join afterwards.
    for position in order {
        let (width, tall) = size(position);
        if tall > TALL * height {
            lines.push((center(position), vec![position]));
        } else if tall < MARK_SIZE * height && width < MARK_SIZE * height {
            marks.push(position);
        } else if tall > RAISED * height {
            raised.push(position);
        } else {
            let reach = Reach { band: LINE_BAND * height, span: None };
            join_nearest(&mut lines, position, center(position), reach, &size, height);
        }
    }
    for position in raised {
        let [_, top, _, bottom] = boxes[position].1;
        let reach = Reach { band: LINE_BAND * height, span: Some((top, bottom)) };
        join_nearest(&mut lines, position, center(position), reach, &size, height);
    }
    for position in marks {
        let reach = Reach { band: MARK_REACH * height, span: None };
        join_nearest(&mut lines, position, center(position), reach, &size, height);
    }
    let top =
        |members: &[usize]| members.iter().map(|&p| boxes[p].1[1]).fold(f64::INFINITY, f64::min);
    lines.sort_by(|a, b| top(&a.1).total_cmp(&top(&b.1)));
    lines.into_iter().map(|(_, members)| members).collect()
}

/// Which lines a stroke may join.
struct Reach {
    /// How far the line's center may be from the stroke's.
    band: f64,
    /// The stroke's top and bottom: a line whose center is between them may take it too.
    span: Option<(f64, f64)>,
}

/// Adds the stroke at `position`, centered at `center`, to the line whose center is nearest,
/// if within `reach`, or else to a new line. Tall lines (a single tall stroke) take nothing.
fn join_nearest(
    lines: &mut Vec<(f64, Vec<usize>)>,
    position: usize,
    center: f64,
    reach: Reach,
    size: &impl Fn(usize) -> (f64, f64),
    height: f64,
) {
    let nearest = lines
        .iter_mut()
        .filter(|(_, members)| !(members.len() == 1 && size(members[0]).1 > TALL * height))
        .map(|line| {
            let line_center = line.0 / line.1.len() as f64;
            ((center - line_center).abs(), line)
        })
        .filter(|(distance, line)| {
            let line_center = line.0 / line.1.len() as f64;
            *distance <= reach.band
                || reach.span.is_some_and(|(top, bottom)| (top..=bottom).contains(&line_center))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match nearest {
        Some((_, (sum, members))) => {
            *sum += center;
            members.push(position);
        }
        None => lines.push((center, vec![position])),
    }
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
