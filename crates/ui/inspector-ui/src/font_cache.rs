use hashbrown::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use reqwest::Url;
use rusqlite::{Connection, OptionalExtension, params};
use skia_safe::{FontMgr, Typeface, font_style::Slant};
use tempfile::TempDir;

const CACHE_DATABASE_NAME: &str = "google-fonts.sqlite3";
const DATABASE_VERSION: i64 = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;
const GOOGLE_FONTS_CSS_ENDPOINT: &str = "https://fonts.googleapis.com/css2";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FontSource {
    Local,
    Google,
}

#[derive(Clone, Debug)]
pub(crate) struct FontFamily {
    pub(crate) name: String,
    pub(crate) source: FontSource,
    pub(crate) revision: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct FontAxis {
    pub(crate) tag: String,
    pub(crate) minimum: f32,
    pub(crate) default: f32,
    pub(crate) maximum: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FontCapabilities {
    pub(crate) axes: Vec<FontAxis>,
}

#[derive(Clone, Debug)]
pub(crate) struct GoogleFamily {
    pub(crate) name: String,
    repository_path: String,
    faces: Vec<GoogleFace>,
}

#[derive(Clone, Debug)]
struct GoogleFace {
    filename: String,
    style: String,
    weight: i32,
    source_url: Option<Url>,
}

struct DownloadedFace {
    metadata: GoogleFace,
    source_url: String,
    data: Vec<u8>,
    axes: Vec<FontAxis>,
}

pub(crate) struct MaterializedFamily {
    name: String,
    directory: TempDir,
    paths: Vec<PathBuf>,
}

struct ActiveFamily {
    _directory: TempDir,
}

static ACTIVE_FAMILIES: OnceLock<Mutex<HashMap<String, ActiveFamily>>> = OnceLock::new();

pub(crate) fn normalize_google_query(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter a Google font family name".to_string());
    }
    let family = if input.contains("://") {
        let url = Url::parse(input).map_err(|error| format!("invalid font URL: {error}"))?;
        if url.scheme() != "https" || url.host_str() != Some("fonts.google.com") {
            return Err("Only HTTPS fonts.google.com specimen URLs are supported".to_string());
        }
        let segments = url
            .path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .unwrap_or_default();
        let Some(specimen) = segments.iter().position(|segment| *segment == "specimen") else {
            return Err("Google Fonts URL must point to /specimen/FAMILY".to_string());
        };
        let Some(family) = segments.get(specimen + 1) else {
            return Err("Google Fonts URL must include a family after /specimen/".to_string());
        };
        family.replace('+', " ")
    } else {
        input.replace('+', " ")
    };
    let family = family.split_whitespace().collect::<Vec<_>>().join(" ");
    if family.is_empty() {
        Err("Enter a Google font family name".to_string())
    } else {
        Ok(family)
    }
}

pub(crate) fn lookup_google_family(input: &str) -> Result<GoogleFamily, String> {
    lookup_css_family(&normalize_google_query(input)?)
}

pub(crate) fn download_google_family(family: &GoogleFamily) -> Result<FontFamily, String> {
    let manager = FontMgr::new();
    let mut filenames = HashSet::new();
    let mut faces = Vec::new();
    let client = http_client()?;
    for face in &family.faces {
        if !filenames.insert(face.filename.clone()) {
            continue;
        }
        let url = face
            .source_url
            .clone()
            .ok_or_else(|| format!("{} has no download URL", face.filename))?;
        let response = client
            .get(url.clone())
            .send()
            .map_err(|error| format!("could not download {}: {error}", face.filename))?;
        if !response.status().is_success() {
            return Err(format!(
                "could not download {}: HTTP {}",
                face.filename,
                response.status()
            ));
        }
        let data = limited_bytes(response)?;
        let typeface = manager
            .new_from_data(&data, None)
            .ok_or_else(|| format!("{} is not a supported font", face.filename))?;
        let mut downloaded =
            downloaded_face(face.filename.clone(), url.to_string(), data, &typeface);
        downloaded.metadata.style.clone_from(&face.style);
        downloaded.metadata.weight = face.weight;
        faces.push(downloaded);
    }
    if faces.is_empty() {
        return Err(format!(
            "Google Fonts returned no files for {}",
            family.name
        ));
    }
    store_family(family, &faces)
}

pub(crate) fn preview_google_family(family: &GoogleFamily) -> Result<Typeface, String> {
    let manager = FontMgr::new();
    let client = http_client()?;
    for face in family.faces.iter().rev() {
        let Some(url) = face.source_url.as_ref() else {
            continue;
        };
        let response = client
            .get(url.clone())
            .send()
            .map_err(|error| format!("could not download {} preview: {error}", family.name))?;
        if !response.status().is_success() {
            continue;
        }
        let data = limited_bytes(response)?;
        if let Some(typeface) = manager.new_from_data(&data, None)
            && typeface.unichar_to_glyph('A' as i32) != 0
        {
            return Ok(typeface);
        }
    }
    Err(format!(
        "Google Fonts returned no usable preview for {}",
        family.name
    ))
}

pub(crate) fn cached_google_families() -> Result<Vec<FontFamily>, String> {
    let connection = connection()?;
    let mut statement = connection
        .prepare("SELECT name, revision FROM families ORDER BY name COLLATE NOCASE")
        .map_err(database_error)?;
    statement
        .query_map([], |row| {
            Ok(FontFamily {
                name: row.get(0)?,
                source: FontSource::Google,
                revision: row.get(1)?,
            })
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)
}

pub(crate) fn cached_family(name: &str) -> Result<Option<FontFamily>, String> {
    connection()?
        .query_row(
            "SELECT name, revision FROM families WHERE name = ?1 COLLATE NOCASE",
            [name],
            |row| {
                Ok(FontFamily {
                    name: row.get(0)?,
                    source: FontSource::Google,
                    revision: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(database_error)
}

pub(crate) fn preview_typeface(name: &str) -> Result<Typeface, String> {
    let connection = connection()?;
    let mut statement = connection
        .prepare(
            "SELECT data FROM font_files WHERE family = ?1 COLLATE NOCASE \
             ORDER BY CASE style WHEN 'normal' THEN 0 ELSE 1 END, ABS(weight - 400), filename",
        )
        .map_err(database_error)?;
    let files = statement
        .query_map([name], |row| row.get::<_, Vec<u8>>(0))
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    let manager = FontMgr::new();
    files
        .iter()
        .filter_map(|data| manager.new_from_data(data, None))
        .find(|typeface| typeface.unichar_to_glyph('A' as i32) != 0)
        .ok_or_else(|| format!("cached font {name} has no usable Latin preview face"))
}

pub(crate) fn cached_capabilities(name: &str) -> Result<FontCapabilities, String> {
    let connection = connection()?;
    let mut statement = connection
        .prepare(
            "SELECT tag, MIN(minimum), AVG(default_value), MAX(maximum) \
             FROM font_axes WHERE family = ?1 COLLATE NOCASE GROUP BY tag ORDER BY tag",
        )
        .map_err(database_error)?;
    let axes = statement
        .query_map([name], |row| {
            Ok(FontAxis {
                tag: row.get(0)?,
                minimum: row.get(1)?,
                default: row.get(2)?,
                maximum: row.get(3)?,
            })
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    Ok(FontCapabilities { axes })
}

pub(crate) fn local_capabilities(name: &str) -> FontCapabilities {
    let mut axes = HashMap::<String, FontAxis>::new();
    let mut style_set = FontMgr::new().match_family(name);
    for index in 0..style_set.count() {
        let Some(typeface) = style_set.new_typeface(index) else {
            continue;
        };
        for axis in typeface.variation_design_parameters().unwrap_or_default() {
            if axis.is_hidden() {
                continue;
            }
            let tag = String::from_utf8_lossy(&axis.tag.to_be_bytes()).into_owned();
            axes.entry(tag.clone()).or_insert(FontAxis {
                tag,
                minimum: axis.min,
                default: axis.def,
                maximum: axis.max,
            });
        }
    }
    let mut axes = axes.into_values().collect::<Vec<_>>();
    axes.sort_by(|left, right| left.tag.cmp(&right.tag));
    FontCapabilities { axes }
}

pub(crate) fn materialize_family(name: &str) -> Result<MaterializedFamily, String> {
    let connection = connection()?;
    let canonical: String = connection
        .query_row(
            "SELECT name FROM families WHERE name = ?1 COLLATE NOCASE",
            [name],
            |row| row.get(0),
        )
        .map_err(database_error)?;
    let mut statement = connection
        .prepare("SELECT filename, data FROM font_files WHERE family = ?1 ORDER BY filename")
        .map_err(database_error)?;
    let files = statement
        .query_map([&canonical], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    let directory = tempfile::Builder::new()
        .prefix("shrimply-fonts-")
        .tempdir()
        .map_err(|error| format!("could not create runtime font directory: {error}"))?;
    let mut paths = Vec::with_capacity(files.len());
    for (filename, data) in files {
        let filename = Path::new(&filename)
            .file_name()
            .ok_or_else(|| "cached font has an invalid filename".to_string())?;
        let path = directory.path().join(filename);
        fs::write(&path, data)
            .map_err(|error| format!("could not materialize {}: {error}", path.display()))?;
        paths.push(path);
    }
    Ok(MaterializedFamily {
        name: canonical,
        directory,
        paths,
    })
}

pub(crate) fn activate_family(family: MaterializedFamily) -> Result<(), String> {
    let active = ACTIVE_FAMILIES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut active = active
        .lock()
        .unwrap_or_else(|_| panic!("active font registry lock died"));
    if active.contains_key(&family.name) {
        return Ok(());
    }
    let paths = family.paths.clone();
    active.insert(
        family.name,
        ActiveFamily {
            _directory: family.directory,
        },
    );
    drop(active);
    for path in paths {
        shrimply_video::text_layout::register_application_font(path)?;
    }
    Ok(())
}

fn store_family(family: &GoogleFamily, faces: &[DownloadedFace]) -> Result<FontFamily, String> {
    let mut connection = connection()?;
    let transaction = connection.transaction().map_err(database_error)?;
    let revision = transaction
        .query_row(
            "SELECT revision FROM families WHERE name = ?1 COLLATE NOCASE",
            [&family.name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(database_error)?
        .unwrap_or(0)
        + 1;
    transaction
        .execute(
            "INSERT INTO families(name, repository_path, revision) VALUES(?1, ?2, ?3) \
             ON CONFLICT(name) DO UPDATE SET repository_path = excluded.repository_path, revision = excluded.revision",
            params![family.name, family.repository_path, revision],
        )
        .map_err(database_error)?;
    transaction
        .execute(
            "DELETE FROM font_files WHERE family = ?1 COLLATE NOCASE",
            [&family.name],
        )
        .map_err(database_error)?;
    transaction
        .execute(
            "DELETE FROM font_axes WHERE family = ?1 COLLATE NOCASE",
            [&family.name],
        )
        .map_err(database_error)?;
    for face in faces {
        transaction
            .execute(
                "INSERT INTO font_files(family, filename, style, weight, source_url, data) \
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    family.name,
                    face.metadata.filename,
                    face.metadata.style,
                    face.metadata.weight,
                    face.source_url,
                    face.data,
                ],
            )
            .map_err(database_error)?;
        for axis in &face.axes {
            transaction
                .execute(
                    "INSERT INTO font_axes(family, filename, tag, minimum, default_value, maximum) \
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        family.name,
                        face.metadata.filename,
                        axis.tag,
                        axis.minimum,
                        axis.default,
                        axis.maximum,
                    ],
                )
                .map_err(database_error)?;
        }
    }
    transaction.commit().map_err(database_error)?;
    Ok(FontFamily {
        name: family.name.clone(),
        source: FontSource::Google,
        revision,
    })
}

fn connection() -> Result<Connection, String> {
    let path = cache_database_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create font cache directory: {error}"))?;
    }
    let connection = Connection::open(&path)
        .map_err(|error| format!("could not open font cache {}: {error}", path.display()))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(database_error)?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS families(
                 name TEXT PRIMARY KEY COLLATE NOCASE,
                 repository_path TEXT NOT NULL,
                 revision INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS font_files(
                 family TEXT NOT NULL REFERENCES families(name) ON DELETE CASCADE,
                 filename TEXT NOT NULL,
                 style TEXT NOT NULL,
                 weight INTEGER NOT NULL,
                 source_url TEXT NOT NULL,
                 data BLOB NOT NULL,
                 PRIMARY KEY(family, filename)
             );
             CREATE TABLE IF NOT EXISTS font_axes(
                 family TEXT NOT NULL,
                 filename TEXT NOT NULL,
                 tag TEXT NOT NULL,
                 minimum REAL NOT NULL,
                 default_value REAL NOT NULL,
                 maximum REAL NOT NULL,
                 PRIMARY KEY(family, filename, tag),
                 FOREIGN KEY(family, filename) REFERENCES font_files(family, filename) ON DELETE CASCADE
             );",
        )
        .map_err(database_error)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(database_error)?;
    if version == 0 {
        connection
            .pragma_update(None, "user_version", DATABASE_VERSION)
            .map_err(database_error)?;
    } else if version != DATABASE_VERSION {
        return Err(format!("unsupported Google font cache version {version}"));
    }
    Ok(connection)
}

fn cache_database_path() -> Result<PathBuf, String> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|root| root.join("shrimply").join(CACHE_DATABASE_NAME))
        .ok_or_else(|| "neither XDG_CACHE_HOME nor HOME is set".to_string())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent("Shrimply Google Fonts")
        .build()
        .map_err(|error| format!("could not create HTTP client: {error}"))
}

fn lookup_css_family(requested: &str) -> Result<GoogleFamily, String> {
    let client = http_client()?;
    let specifications = [
        format!("{requested}:ital,wght@0,100..900;1,100..900"),
        format!("{requested}:wght@100..900"),
        requested.to_string(),
    ];
    for specification in specifications {
        let mut url = Url::parse(GOOGLE_FONTS_CSS_ENDPOINT)
            .map_err(|error| format!("invalid Google Fonts CSS endpoint: {error}"))?;
        url.query_pairs_mut().append_pair("family", &specification);
        let response = client
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 Shrimply Google Fonts",
            )
            .send()
            .map_err(|error| format!("could not query Google Fonts: {error}"))?;
        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            continue;
        }
        if !response.status().is_success() {
            return Err(format!("Google Fonts returned {}", response.status()));
        }
        let css = limited_bytes(response)?;
        let css = std::str::from_utf8(&css)
            .map_err(|error| format!("Google Fonts CSS is not UTF-8: {error}"))?;
        let faces = parse_css_faces(css, requested)?;
        if !faces.is_empty() {
            return Ok(GoogleFamily {
                name: requested.to_string(),
                repository_path: GOOGLE_FONTS_CSS_ENDPOINT.to_string(),
                faces,
            });
        }
    }
    Err(format!("No Google font named {requested}"))
}

fn parse_css_faces(css: &str, requested: &str) -> Result<Vec<GoogleFace>, String> {
    let mut faces = Vec::new();
    let mut urls = HashSet::new();
    for block in css.split("@font-face").skip(1) {
        let Some(block) = block.split('}').next() else {
            continue;
        };
        let family = css_property(block, "font-family")
            .map(|value| value.trim_matches(['\'', '"']))
            .unwrap_or_default();
        if !family.eq_ignore_ascii_case(requested) {
            continue;
        }
        let Some(source) = css_property(block, "src") else {
            continue;
        };
        let Some(raw_url) = source
            .split("url(")
            .nth(1)
            .and_then(|value| value.split(')').next())
            .map(|value| value.trim_matches(['\'', '"']))
        else {
            continue;
        };
        let url = Url::parse(raw_url)
            .map_err(|error| format!("Google Fonts returned an invalid font URL: {error}"))?;
        if url.scheme() != "https" || url.host_str() != Some("fonts.gstatic.com") {
            return Err("Google Fonts returned an unexpected font host".to_string());
        }
        if !urls.insert(url.clone()) {
            continue;
        }
        let extension = Path::new(url.path())
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("woff2");
        faces.push(GoogleFace {
            filename: format!("font-{}.{}", faces.len(), extension),
            style: css_property(block, "font-style")
                .unwrap_or("normal")
                .to_string(),
            weight: css_property(block, "font-weight")
                .and_then(|weight| weight.split_whitespace().next())
                .and_then(|weight| weight.parse().ok())
                .unwrap_or(400),
            source_url: Some(url),
        });
    }
    Ok(faces)
}

fn css_property<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    block.split(';').find_map(|declaration| {
        let (property, value) = declaration.split_once(':')?;
        (property.trim().trim_start_matches('{').trim() == name).then(|| value.trim())
    })
}

fn downloaded_face(
    filename: String,
    source_url: String,
    data: Vec<u8>,
    typeface: &Typeface,
) -> DownloadedFace {
    let style = match typeface.font_style().slant() {
        Slant::Italic => "italic",
        Slant::Oblique => "oblique",
        _ => "normal",
    }
    .to_string();
    let weight = *typeface.font_style().weight();
    let axes = typeface
        .variation_design_parameters()
        .unwrap_or_default()
        .into_iter()
        .filter(|axis| !axis.is_hidden())
        .map(|axis| FontAxis {
            tag: String::from_utf8_lossy(&axis.tag.to_be_bytes()).into_owned(),
            minimum: axis.min,
            default: axis.def,
            maximum: axis.max,
        })
        .collect();
    DownloadedFace {
        metadata: GoogleFace {
            filename,
            style,
            weight,
            source_url: None,
        },
        source_url,
        data,
        axes,
    }
}

fn limited_bytes(response: reqwest::blocking::Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Google Fonts response is too large".to_string());
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("could not read Google Fonts response: {error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("Google Fonts response is too large".to_string());
    }
    Ok(bytes.to_vec())
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Google font cache error: {error}")
}
