use crate::Rect;

pub fn to_skia_matrix(matrix: glam::Mat3) -> skia_safe::Matrix {
    let values = matrix.to_cols_array();
    skia_safe::Matrix::new_all(
        values[0], values[3], values[6], values[1], values[4], values[7], values[2], values[5],
        values[8],
    )
}

impl From<Rect> for skia_safe::Rect {
    fn from(rect: Rect) -> Self {
        Self::from_ltrb(rect.min.x, rect.min.y, rect.max.x, rect.max.y)
    }
}
