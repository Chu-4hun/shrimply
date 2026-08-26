use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use glam::Vec3;
use shrimply_math_color::Color;

const DEGENERATE_TRIANGLE_EPSILON_SQUARED: f32 = 1.0e-20;

#[derive(Clone, Debug)]
pub struct ObjMesh {
    pub positions: Vec<[f32; 4]>,
    pub normals: Vec<[f32; 4]>,
    pub tangents: Vec<[f32; 4]>,
    pub tex_coords_0: Vec<[f32; 4]>,
    pub tex_coords_1: Vec<[f32; 4]>,
    pub colors: Vec<Color>,
    pub materials: Vec<MeshMaterial>,
    pub texture_atlas: TextureAtlas,
    pub face_normals: Vec<[f32; 4]>,
    pub triangles: Vec<[u32; 4]>,
    pub source_center: Vec3,
    pub source_radius: f32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MeshMaterial {
    pub base_color_factor: Color,
    pub metallic_roughness_normal_alpha: [f32; 4],
    pub flags: [u32; 4],
    pub base_color_texture: TextureMapping,
    pub metallic_roughness_texture: TextureMapping,
    pub normal_texture: TextureMapping,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct TextureMapping {
    pub atlas: [f32; 4],
    pub transform: [f32; 4],
    pub settings: [u32; 4],
    pub rotation_filter: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct TextureAtlas {
    pub pixels: Vec<Color<u8>>,
    pub width: u32,
    pub height: u32,
}

impl Default for TextureAtlas {
    fn default() -> Self {
        Self {
            pixels: vec![Color::WHITE],
            width: 1,
            height: 1,
        }
    }
}

impl Default for MeshMaterial {
    fn default() -> Self {
        Self {
            base_color_factor: Color::WHITE,
            metallic_roughness_normal_alpha: [1.0, 1.0, 1.0, 0.0],
            flags: [0; 4],
            base_color_texture: TextureMapping::default(),
            metallic_roughness_texture: TextureMapping::default(),
            normal_texture: TextureMapping::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjError {
    line: Option<usize>,
    message: String,
}

impl ObjError {
    fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            message: message.into(),
        }
    }

    fn file(message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: message.into(),
        }
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for ObjError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(formatter, "OBJ line {line}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for ObjError {}

#[derive(Clone, Copy)]
struct FaceVertex {
    position: usize,
    normal: Option<usize>,
}

#[derive(Clone, Copy)]
struct Triangle {
    vertices: [FaceVertex; 3],
    area_normal: Vec3,
}

pub fn load_obj(path: impl AsRef<Path>) -> Result<ObjMesh, ObjError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|error| ObjError::file(format!("failed to open {}: {error}", path.display())))?;
    parse_obj(BufReader::new(file))
}

pub fn parse_obj(reader: impl BufRead) -> Result<ObjMesh, ObjError> {
    let mut positions = Vec::<Vec3>::new();
    let mut supplied_normals = Vec::<Vec3>::new();
    let mut triangles = Vec::<Triangle>::new();

    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.map_err(|error| ObjError::at(line_number, error.to_string()))?;
        let mut fields = line.split_whitespace();
        let Some(kind) = fields.next() else { continue };
        if kind.starts_with('#') {
            continue;
        }
        match kind {
            "v" => {
                positions
                    .try_reserve(1)
                    .map_err(|_| ObjError::at(line_number, "unable to allocate OBJ vertices"))?;
                positions.push(parse_position(fields, line_number)?);
            }
            "vn" => {
                supplied_normals
                    .try_reserve(1)
                    .map_err(|_| ObjError::at(line_number, "unable to allocate OBJ normals"))?;
                supplied_normals.push(parse_normal(fields, line_number)?);
            }
            "f" => parse_face(
                fields,
                line_number,
                &positions,
                &supplied_normals,
                &mut triangles,
            )?,
            "vt" | "s" | "usemtl" | "mtllib" | "o" | "g" => {}
            _ => {}
        }
    }

    if triangles.is_empty() {
        return Err(ObjError::file("OBJ contains no nondegenerate triangles"));
    }
    let (source_center, source_radius) = bounds(&positions, &triangles)?;
    let mut smooth_normals = Vec::new();
    smooth_normals
        .try_reserve_exact(positions.len())
        .map_err(|_| ObjError::file("unable to allocate generated OBJ normals"))?;
    smooth_normals.resize(positions.len(), Vec3::ZERO);
    for triangle in &triangles {
        for vertex in triangle.vertices {
            smooth_normals[vertex.position] += triangle.area_normal;
        }
    }

    let corner_count = triangles
        .len()
        .checked_mul(3)
        .ok_or_else(|| ObjError::file("OBJ vertex count overflow"))?;
    u32::try_from(corner_count)
        .map_err(|_| ObjError::file("OBJ vertex count exceeds GPU limits"))?;
    u32::try_from(triangles.len())
        .map_err(|_| ObjError::file("OBJ triangle count exceeds GPU limits"))?;
    let mut output_positions = Vec::new();
    let mut output_normals = Vec::new();
    let mut output_face_normals = Vec::new();
    let mut output_triangles = Vec::new();
    output_positions
        .try_reserve_exact(corner_count)
        .map_err(|_| ObjError::file("unable to allocate OBJ vertex data"))?;
    output_normals
        .try_reserve_exact(corner_count)
        .map_err(|_| ObjError::file("unable to allocate OBJ normal data"))?;
    output_face_normals
        .try_reserve_exact(triangles.len())
        .map_err(|_| ObjError::file("unable to allocate OBJ face-normal data"))?;
    output_triangles
        .try_reserve_exact(triangles.len())
        .map_err(|_| ObjError::file("unable to allocate OBJ index data"))?;

    for triangle in triangles {
        let first = u32::try_from(output_positions.len())
            .map_err(|_| ObjError::file("OBJ vertex count exceeds GPU limits"))?;
        let face_normal = triangle.area_normal.normalize();
        for vertex in triangle.vertices {
            let position = (positions[vertex.position] - source_center) / source_radius;
            let normal = vertex
                .normal
                .map(|index| supplied_normals[index])
                .unwrap_or(smooth_normals[vertex.position])
                .try_normalize()
                .unwrap_or(face_normal);
            output_positions.push([position.x, position.y, position.z, 1.0]);
            output_normals.push([normal.x, normal.y, normal.z, 0.0]);
        }
        output_face_normals.push([face_normal.x, face_normal.y, face_normal.z, 0.0]);
        output_triangles.push([first, first + 1, first + 2, 0]);
    }

    Ok(ObjMesh {
        positions: output_positions,
        normals: output_normals,
        tangents: vec![[0.0; 4]; corner_count],
        tex_coords_0: vec![[0.0; 4]; corner_count],
        tex_coords_1: vec![[0.0; 4]; corner_count],
        colors: vec![Color::WHITE; corner_count],
        materials: vec![MeshMaterial::default(); output_triangles.len()],
        texture_atlas: TextureAtlas::default(),
        face_normals: output_face_normals,
        triangles: output_triangles,
        source_center,
        source_radius,
    })
}

fn parse_position<'a>(
    mut fields: impl Iterator<Item = &'a str>,
    line: usize,
) -> Result<Vec3, ObjError> {
    let x = parse_f32(fields.next(), line, "vertex x")?;
    let y = parse_f32(fields.next(), line, "vertex y")?;
    let z = parse_f32(fields.next(), line, "vertex z")?;
    let w = fields
        .next()
        .filter(|value| !value.starts_with('#'))
        .map(|value| parse_f32(Some(value), line, "vertex w"))
        .transpose()?
        .unwrap_or(1.0);
    if w == 0.0 {
        return Err(ObjError::at(line, "vertex homogeneous w must not be zero"));
    }
    let position = Vec3::new(x / w, y / w, z / w);
    if !position.is_finite() {
        return Err(ObjError::at(line, "vertex coordinates must be finite"));
    }
    Ok(position)
}

fn parse_normal<'a>(
    mut fields: impl Iterator<Item = &'a str>,
    line: usize,
) -> Result<Vec3, ObjError> {
    let normal = Vec3::new(
        parse_f32(fields.next(), line, "normal x")?,
        parse_f32(fields.next(), line, "normal y")?,
        parse_f32(fields.next(), line, "normal z")?,
    );
    if !normal.is_finite() {
        return Err(ObjError::at(line, "normal coordinates must be finite"));
    }
    normal
        .try_normalize()
        .ok_or_else(|| ObjError::at(line, "normal must not be zero"))
}

fn parse_f32(value: Option<&str>, line: usize, label: &str) -> Result<f32, ObjError> {
    let value = value.ok_or_else(|| ObjError::at(line, format!("missing {label}")))?;
    let parsed = value
        .parse::<f32>()
        .map_err(|_| ObjError::at(line, format!("invalid {label} `{value}`")))?;
    if !parsed.is_finite() {
        return Err(ObjError::at(line, format!("{label} must be finite")));
    }
    Ok(parsed)
}

fn parse_face<'a>(
    fields: impl Iterator<Item = &'a str>,
    line: usize,
    positions: &[Vec3],
    normals: &[Vec3],
    triangles: &mut Vec<Triangle>,
) -> Result<(), ObjError> {
    let mut face = Vec::new();
    for field in fields.take_while(|field| !field.starts_with('#')) {
        face.try_reserve(1)
            .map_err(|_| ObjError::at(line, "unable to allocate OBJ face"))?;
        face.push(parse_face_vertex(
            field,
            line,
            positions.len(),
            normals.len(),
        )?);
    }
    if face.len() < 3 {
        return Err(ObjError::at(
            line,
            "face must contain at least three vertices",
        ));
    }
    triangles
        .try_reserve(face.len() - 2)
        .map_err(|_| ObjError::at(line, "unable to allocate OBJ triangles"))?;
    for index in 1..face.len() - 1 {
        let vertices = [face[0], face[index], face[index + 1]];
        let edge_a = positions[vertices[1].position] - positions[vertices[0].position];
        let edge_b = positions[vertices[2].position] - positions[vertices[0].position];
        let area_normal = edge_a.cross(edge_b);
        if area_normal.length_squared() > DEGENERATE_TRIANGLE_EPSILON_SQUARED {
            triangles.push(Triangle {
                vertices,
                area_normal,
            });
        }
    }
    Ok(())
}

fn parse_face_vertex(
    field: &str,
    line: usize,
    position_count: usize,
    normal_count: usize,
) -> Result<FaceVertex, ObjError> {
    let mut indices = field.split('/');
    let position = parse_index(
        indices.next().unwrap_or_default(),
        position_count,
        line,
        "vertex",
    )?;
    if let Some(texture) = indices.next().filter(|index| !index.is_empty()) {
        let texture = texture
            .parse::<i64>()
            .map_err(|_| ObjError::at(line, format!("invalid texture index `{texture}`")))?;
        if texture == 0 {
            return Err(ObjError::at(line, "OBJ index zero is invalid"));
        }
    }
    let normal = indices
        .next()
        .filter(|index| !index.is_empty())
        .map(|index| parse_index(index, normal_count, line, "normal"))
        .transpose()?;
    if indices.next().is_some() {
        return Err(ObjError::at(line, format!("invalid face vertex `{field}`")));
    }
    Ok(FaceVertex { position, normal })
}

fn parse_index(value: &str, count: usize, line: usize, label: &str) -> Result<usize, ObjError> {
    let index = value
        .parse::<i64>()
        .map_err(|_| ObjError::at(line, format!("invalid {label} index `{value}`")))?;
    if index == 0 {
        return Err(ObjError::at(line, "OBJ index zero is invalid"));
    }
    let resolved = if index > 0 {
        usize::try_from(index - 1).ok()
    } else {
        let magnitude = usize::try_from(index.unsigned_abs()).ok();
        magnitude.and_then(|magnitude| count.checked_sub(magnitude))
    };
    resolved
        .filter(|index| *index < count)
        .ok_or_else(|| ObjError::at(line, format!("{label} index `{value}` is out of range")))
}

fn bounds(positions: &[Vec3], triangles: &[Triangle]) -> Result<(Vec3, f32), ObjError> {
    let mut referenced = triangles
        .iter()
        .flat_map(|triangle| triangle.vertices)
        .map(|vertex| positions[vertex.position]);
    let first = referenced
        .next()
        .ok_or_else(|| ObjError::file("OBJ contains no vertices"))?;
    let (minimum, maximum) = referenced.fold((first, first), |(minimum, maximum), position| {
        (minimum.min(position), maximum.max(position))
    });
    let center = (minimum + maximum) * 0.5;
    let radius = triangles
        .iter()
        .flat_map(|triangle| triangle.vertices)
        .map(|vertex| positions[vertex.position].distance(center))
        .fold(0.0_f32, f32::max);
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ObjError::file("OBJ has a zero-radius bounding sphere"));
    }
    Ok((center, radius))
}
