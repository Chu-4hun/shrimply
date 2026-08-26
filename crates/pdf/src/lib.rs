use cairo::{Context, Format, ImageSurface};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageSize {
    pub width: u32,
    pub height: u32,
}

pub struct RenderedPage {
    pub size: PageSize,
    pub rgba: Vec<u8>,
}

pub fn page_sizes(bytes: Vec<u8>) -> Result<Vec<PageSize>, String> {
    let document = document(bytes)?;
    let page_count = usize::try_from(document.n_pages())
        .map_err(|_| "PDF reported a negative page count".to_string())?;
    if page_count == 0 {
        return Err("PDF contains no pages".to_string());
    }
    (0..page_count)
        .map(|index| {
            let page = document
                .page(i32::try_from(index).expect("Poppler page index must fit i32"))
                .ok_or_else(|| format!("PDF page {} does not exist", index + 1))?;
            page_size(page.size())
        })
        .collect()
}

pub fn render_page(bytes: Vec<u8>, page_index: u32) -> Result<RenderedPage, String> {
    let document = document(bytes)?;
    let page = document
        .page(i32::try_from(page_index).map_err(|_| "PDF page index is too large")?)
        .ok_or_else(|| format!("PDF page {} does not exist", page_index.saturating_add(1)))?;
    let size = page_size(page.size())?;
    let width = i32::try_from(size.width).map_err(|_| "PDF page width is too large")?;
    let height = i32::try_from(size.height).map_err(|_| "PDF page height is too large")?;
    let surface = ImageSurface::create(Format::ARgb32, width, height)
        .map_err(|error| format!("create PDF page surface: {error}"))?;
    let context = Context::new(&surface)
        .map_err(|error| format!("create PDF page drawing context: {error}"))?;
    context.set_source_rgb(1.0, 1.0, 1.0);
    context
        .paint()
        .map_err(|error| format!("clear PDF page surface: {error}"))?;
    page.render(&context);
    context
        .status()
        .map_err(|error| format!("render PDF page: {error}"))?;
    drop(context);

    let stride = usize::try_from(surface.stride()).map_err(|_| "invalid PDF surface stride")?;
    let data = surface
        .take_data()
        .map_err(|error| format!("read rendered PDF page: {error}"))?;
    let row_bytes = usize::try_from(size.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or("PDF page row size overflow")?;
    let capacity = row_bytes
        .checked_mul(usize::try_from(size.height).expect("page height must fit usize"))
        .ok_or("PDF page pixel size overflow")?;
    let mut rgba = Vec::with_capacity(capacity);
    for row in data.chunks(stride).take(size.height as usize) {
        for pixel in row[..row_bytes].chunks_exact(4) {
            #[cfg(target_endian = "little")]
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            #[cfg(target_endian = "big")]
            rgba.extend_from_slice(&[pixel[1], pixel[2], pixel[3], pixel[0]]);
        }
    }
    Ok(RenderedPage { size, rgba })
}

fn document(bytes: Vec<u8>) -> Result<poppler::Document, String> {
    poppler::Document::from_bytes(&glib::Bytes::from_owned(bytes), None)
        .map_err(|error| format!("could not open PDF: {error}"))
}

fn page_size((width, height): (f64, f64)) -> Result<PageSize, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("PDF page has invalid dimensions".to_string());
    }
    if width.ceil() > f64::from(i32::MAX) || height.ceil() > f64::from(i32::MAX) {
        return Err("PDF page dimensions are too large".to_string());
    }
    Ok(PageSize {
        width: width.ceil() as u32,
        height: height.ceil() as u32,
    })
}
