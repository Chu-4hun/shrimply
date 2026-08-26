use std::collections::BTreeMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use shrimply_timeline_value::{
    ExpressionData, ExpressionInput, Interpolation, Time, TimelineExpressionValue,
    TimelineKeyframe, TimelineValueType,
};
use uuid::Uuid;

mod math;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaintDrawing {
    pub strokes: Vec<PaintStroke>,
    pub fills: Vec<PaintFill>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaintDrawingKeyframe {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub time: Time,
    pub value: PaintDrawing,
    #[serde(default = "jump_interpolation")]
    pub interpolation_to_next: Interpolation,
}

const fn jump_interpolation() -> Interpolation {
    Interpolation::Jump
}

impl TimelineKeyframe<PaintDrawing> for PaintDrawingKeyframe {
    fn id(&self) -> Uuid {
        self.id
    }

    fn id_mut(&mut self) -> &mut Uuid {
        &mut self.id
    }

    fn time(&self) -> Time {
        self.time
    }

    fn time_mut(&mut self) -> &mut Time {
        &mut self.time
    }

    fn value(&self) -> &PaintDrawing {
        &self.value
    }

    fn value_mut(&mut self) -> &mut PaintDrawing {
        &mut self.value
    }
}

impl TimelineValueType for PaintDrawing {
    type Keyframe = PaintDrawingKeyframe;

    fn default_value() -> Self {
        Self::default()
    }

    fn keyframe(time: Time, value: Self) -> Self::Keyframe {
        PaintDrawingKeyframe {
            id: Uuid::new_v4(),
            time,
            value,
            interpolation_to_next: Interpolation::Jump,
        }
    }

    fn value_at(keyframes: &[Self::Keyframe], time: Time) -> Self {
        let Some(right_index) = keyframes.iter().position(|keyframe| keyframe.time > time) else {
            return keyframes
                .last()
                .map(|keyframe| keyframe.value.clone())
                .unwrap_or_default();
        };
        if right_index == 0 {
            return keyframes[0].value.clone();
        }
        let left = &keyframes[right_index - 1];
        let right = &keyframes[right_index];
        if left.interpolation_to_next == Interpolation::Jump {
            return left.value.clone();
        }
        let duration = right.time.signed_sub(left.time).as_secs_f64();
        if duration <= f64::EPSILON {
            return right.value.clone();
        }
        let progress = time.signed_sub(left.time).as_secs_f64() / duration;
        let progress = left.interpolation_to_next.value(progress);
        math::interpolate_drawing(&left.value, &right.value, progress.clamp(0.0, 1.0) as f32)
    }
}

impl TimelineExpressionValue for PaintDrawing {
    fn expression_input(&self) -> ExpressionInput {
        ExpressionInput::new(drawing_data(self))
    }

    fn expression_output(&self, output: ExpressionData) -> Option<Self> {
        drawing_from_data(output, self)
    }
}

fn drawing_data(drawing: &PaintDrawing) -> ExpressionData {
    ExpressionData::Object(BTreeMap::from([
        (
            "strokes".into(),
            ExpressionData::Array(
                drawing
                    .strokes
                    .iter()
                    .map(|stroke| {
                        ExpressionData::Object(BTreeMap::from([
                            (
                                "points".into(),
                                ExpressionData::Array(
                                    stroke
                                        .points
                                        .iter()
                                        .map(|point| {
                                            ExpressionData::Object(BTreeMap::from([
                                                ("position".into(), vec2_data(point.position)),
                                                (
                                                    "pressure".into(),
                                                    point.pressure.map_or(
                                                        ExpressionData::Unit,
                                                        ExpressionData::Number,
                                                    ),
                                                ),
                                            ]))
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "width_scale".into(),
                                ExpressionData::Number(stroke.width_scale),
                            ),
                            (
                                "color_index".into(),
                                ExpressionData::Integer(stroke.color_index as i64),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        (
            "fills".into(),
            ExpressionData::Array(
                drawing
                    .fills
                    .iter()
                    .map(|fill| {
                        ExpressionData::Object(BTreeMap::from([
                            ("seed".into(), vec2_data(fill.seed)),
                            (
                                "loops".into(),
                                ExpressionData::Array(
                                    fill.loops
                                        .iter()
                                        .map(|boundary| {
                                            ExpressionData::Array(
                                                boundary.iter().copied().map(vec2_data).collect(),
                                            )
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "color_index".into(),
                                ExpressionData::Integer(fill.color_index as i64),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn drawing_from_data(output: ExpressionData, base: &PaintDrawing) -> Option<PaintDrawing> {
    let ExpressionData::Object(mut output) = output else {
        return None;
    };
    let ExpressionData::Array(strokes) = output.remove("strokes")? else {
        return None;
    };
    let ExpressionData::Array(fills) = output.remove("fills")? else {
        return None;
    };
    if strokes.len() != base.strokes.len() || fills.len() != base.fills.len() {
        return None;
    }
    let mut drawing = base.clone();
    for (output, stroke) in strokes.into_iter().zip(&mut drawing.strokes) {
        let ExpressionData::Object(mut output) = output else {
            return None;
        };
        let ExpressionData::Array(points) = output.remove("points")? else {
            return None;
        };
        if points.len() != stroke.points.len() {
            return None;
        }
        stroke.width_scale = number(output.remove("width_scale")?)?.max(0.0);
        stroke.color_index = index(output.remove("color_index")?)?;
        for (output, point) in points.into_iter().zip(&mut stroke.points) {
            let ExpressionData::Object(mut output) = output else {
                return None;
            };
            point.position = vec2_from_data(output.remove("position")?)?;
            point.pressure = match output.remove("pressure")? {
                ExpressionData::Unit => None,
                value => Some(number(value)?.clamp(0.0, 1.0)),
            };
        }
    }
    for (output, fill) in fills.into_iter().zip(&mut drawing.fills) {
        let ExpressionData::Object(mut output) = output else {
            return None;
        };
        fill.seed = vec2_from_data(output.remove("seed")?)?;
        fill.color_index = index(output.remove("color_index")?)?;
        let ExpressionData::Array(loops) = output.remove("loops")? else {
            return None;
        };
        if loops.len() != fill.loops.len() {
            return None;
        }
        for (output, boundary) in loops.into_iter().zip(&mut fill.loops) {
            let ExpressionData::Array(points) = output else {
                return None;
            };
            if points.len() != boundary.len() {
                return None;
            }
            for (output, point) in points.into_iter().zip(boundary) {
                *point = vec2_from_data(output)?;
            }
        }
    }
    Some(drawing)
}

fn vec2_data(value: Vec2) -> ExpressionData {
    ExpressionData::Array(vec![
        ExpressionData::Number(value.x),
        ExpressionData::Number(value.y),
    ])
}

fn vec2_from_data(value: ExpressionData) -> Option<Vec2> {
    let ExpressionData::Array(values) = value else {
        return None;
    };
    let [x, y] = values.as_slice() else {
        return None;
    };
    Some(Vec2::new(number(x.clone())?, number(y.clone())?)).filter(|value| value.is_finite())
}

fn number(value: ExpressionData) -> Option<f32> {
    match value {
        ExpressionData::Number(value) => value.is_finite().then_some(value),
        ExpressionData::Integer(value) => Some(value as f32),
        _ => None,
    }
}

fn index(value: ExpressionData) -> Option<usize> {
    match value {
        ExpressionData::Integer(value) => usize::try_from(value).ok(),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintStroke {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    #[serde(default = "Uuid::new_v4")]
    pub correspondence_id: Uuid,
    #[serde(default = "default_stroke_width_scale")]
    pub width_scale: f32,
    #[serde(default)]
    pub color_index: usize,
    pub points: Vec<PaintPoint>,
}

impl PaintStroke {
    pub fn new(points: Vec<PaintPoint>, width_scale: f32, color_index: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            correspondence_id: Uuid::new_v4(),
            width_scale,
            color_index,
            points,
        }
    }
}

fn default_stroke_width_scale() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintPoint {
    pub position: Vec2,
    pub pressure: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintFill {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    #[serde(default = "Uuid::new_v4")]
    pub correspondence_id: Uuid,
    pub seed: Vec2,
    #[serde(default)]
    pub color_index: usize,
    #[serde(default)]
    pub loops: Vec<Vec<Vec2>>,
}

impl PaintFill {
    pub fn new(seed: Vec2, loops: Vec<Vec<Vec2>>, color_index: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            correspondence_id: Uuid::new_v4(),
            seed,
            color_index,
            loops,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> PaintPoint {
        PaintPoint {
            position: Vec2::new(x, y),
            pressure: Some(0.5),
        }
    }

    fn stroke(points: &[(f32, f32)]) -> PaintStroke {
        PaintStroke::new(points.iter().map(|&(x, y)| point(x, y)).collect(), 1.0, 0)
    }

    fn midpoint(from: PaintDrawing, to: PaintDrawing) -> PaintDrawing {
        let mut left = PaintDrawing::keyframe(Time::ZERO, from);
        left.interpolation_to_next = Interpolation::Linear;
        PaintDrawing::value_at(
            &[left, PaintDrawing::keyframe(Time::from_fraction(2, 1), to)],
            Time::from_fraction(1, 1),
        )
    }

    #[test]
    fn jump_in_graph_holds_the_left_drawing() {
        let from = PaintDrawing {
            strokes: vec![stroke(&[(0.0, 0.0), (2.0, 0.0)])],
            fills: Vec::new(),
        };
        let drawing = PaintDrawing::value_at(
            &[
                PaintDrawing::keyframe(Time::ZERO, from.clone()),
                PaintDrawing::keyframe(
                    Time::from_fraction(2, 1),
                    PaintDrawing {
                        strokes: vec![stroke(&[(0.0, 2.0), (2.0, 2.0)])],
                        fills: Vec::new(),
                    },
                ),
            ],
            Time::from_fraction(1, 1),
        );

        assert_eq!(drawing, from);
    }

    #[test]
    fn morphs_positions_and_pressure_at_intermediate_time() {
        let drawing = midpoint(
            PaintDrawing {
                strokes: vec![stroke(&[(0.0, 0.0), (2.0, 0.0)])],
                fills: Vec::new(),
            },
            PaintDrawing {
                strokes: vec![PaintStroke::new(
                    vec![
                        PaintPoint {
                            position: Vec2::new(2.0, 2.0),
                            pressure: Some(1.0),
                        },
                        PaintPoint {
                            position: Vec2::new(3.0, 2.0),
                            pressure: Some(1.0),
                        },
                        PaintPoint {
                            position: Vec2::new(4.0, 2.0),
                            pressure: Some(1.0),
                        },
                    ],
                    1.0,
                    0,
                )],
                fills: Vec::new(),
            },
        );

        assert_eq!(drawing.strokes[0].points.len(), 3);
        assert!(drawing.strokes[0].points[0].position.y > 0.0);
        assert!(drawing.strokes[0].points[0].position.y < 2.0);
        assert_eq!(drawing.strokes[0].points[0].pressure, Some(0.75));
    }

    #[test]
    fn splits_lines_when_no_safe_merge_can_equalize_counts() {
        let drawing = midpoint(
            PaintDrawing {
                strokes: vec![stroke(&[(0.0, 0.0), (10.0, 0.0)])],
                fills: Vec::new(),
            },
            PaintDrawing {
                strokes: vec![
                    stroke(&[(0.0, 10.0), (4.0, 10.0)]),
                    stroke(&[(6.0, 10.0), (10.0, 10.0)]),
                ],
                fills: Vec::new(),
            },
        );

        assert_eq!(drawing.strokes.len(), 2);
        assert!(drawing.strokes.iter().all(|stroke| {
            stroke.points.len() >= 2 && stroke.points.iter().all(|point| point.position.is_finite())
        }));
    }

    #[test]
    fn creates_a_continuous_result_when_one_cel_is_empty() {
        let drawing = midpoint(
            PaintDrawing::default(),
            PaintDrawing {
                strokes: vec![stroke(&[(0.0, 0.0), (10.0, 0.0)])],
                fills: Vec::new(),
            },
        );

        assert_eq!(drawing.strokes.len(), 1);
        assert!(drawing.strokes[0].width_scale > 0.0);
        assert!(drawing.strokes[0].width_scale < 1.0);
    }

    #[test]
    fn near_closed_open_strokes_keep_their_extent() {
        let drawing = midpoint(
            PaintDrawing {
                strokes: vec![stroke(&[
                    (-0.1, 1.4),
                    (0.7, 1.1),
                    (1.1, 0.4),
                    (0.9, -0.7),
                    (0.1, -1.2),
                    (-0.8, -0.9),
                    (-1.1, -0.1),
                    (-0.8, 0.8),
                    (0.6, 1.0),
                ])],
                fills: Vec::new(),
            },
            PaintDrawing {
                strokes: vec![stroke(&[
                    (-0.7, 1.0),
                    (0.1, 1.4),
                    (0.9, 1.0),
                    (1.1, 0.1),
                    (0.6, -0.9),
                    (-0.3, -1.2),
                    (-1.0, -0.6),
                    (-1.1, 0.4),
                    (0.2, 1.2),
                ])],
                fills: Vec::new(),
            },
        );
        let points = &drawing.strokes[0].points;
        let minimum = points
            .iter()
            .map(|point| point.position)
            .reduce(Vec2::min)
            .unwrap();
        let maximum = points
            .iter()
            .map(|point| point.position)
            .reduce(Vec2::max)
            .unwrap();

        assert!((maximum.x - minimum.x) > 1.5);
        assert!((maximum.y - minimum.y) > 1.5);
    }
}
