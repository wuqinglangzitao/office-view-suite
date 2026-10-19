use std::{
    collections::HashMap,
    io::{Read, Seek},
};

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use zip::read::ZipArchive;

use crate::XlsxError;

#[derive(Clone, Debug, PartialEq)]
pub struct CellImage {
    pub id: String,
    pub image_name: String,
    pub image_data: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Default)]
struct ImageRef {
    id: String,
    rel_id: String,
    width_px: u32,
    height_px: u32,
}

pub fn read<R: Read + Seek>(reader: R) -> Result<Vec<CellImage>, XlsxError> {
    let mut archive = ZipArchive::new(reader)?;
    let cell_xml = match read_entry(&mut archive, "xl/cellimages.xml")? {
        Some(value) => value,
        None => return Ok(Vec::new()),
    };
    let rels_xml = match read_entry(&mut archive, "xl/_rels/cellimages.xml.rels")? {
        Some(value) => value,
        None => return Ok(Vec::new()),
    };
    let refs = parse_cell_image_refs(&cell_xml)?;
    let relationships = parse_relationships(&rels_xml)?;
    let mut result = Vec::new();
    for image_ref in refs {
        let Some(target) = relationships.get(&image_ref.rel_id) else {
            continue;
        };
        let path = normalize_target(target);
        let Some(image_data) = read_entry(&mut archive, &path)? else {
            continue;
        };
        result.push(CellImage {
            id: image_ref.id,
            image_name: path.rsplit('/').next().unwrap_or("image").to_string(),
            image_data,
            width_px: image_ref.width_px.max(1),
            height_px: image_ref.height_px.max(1),
        });
    }
    Ok(result)
}

fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>, XlsxError> {
    let mut file = match archive.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn attr(element: &BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        let key = attribute.key.as_ref();
        (key == local_name || key.rsplit(|byte| *byte == b':').next() == Some(local_name))
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn parse_cell_image_refs(xml: &[u8]) -> Result<Vec<ImageRef>, XlsxError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut result = Vec::new();
    let mut current: Option<ImageRef> = None;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) if element.local_name().as_ref() == b"cellImage" => {
                current = Some(ImageRef::default());
            }
            Event::Start(element) | Event::Empty(element) => {
                if let Some(image_ref) = current.as_mut() {
                    match element.local_name().as_ref() {
                        b"cNvPr" => image_ref.id = attr(&element, b"name").unwrap_or_default(),
                        b"blip" => image_ref.rel_id = attr(&element, b"embed").unwrap_or_default(),
                        b"ext" => {
                            image_ref.width_px = attr(&element, b"cx")
                                .and_then(|value| value.parse::<u64>().ok())
                                .map(|value| (value / 9_525).max(1) as u32)
                                .unwrap_or(1);
                            image_ref.height_px = attr(&element, b"cy")
                                .and_then(|value| value.parse::<u64>().ok())
                                .map(|value| (value / 9_525).max(1) as u32)
                                .unwrap_or(1);
                        }
                        _ => {}
                    }
                }
            }
            Event::End(element) if element.local_name().as_ref() == b"cellImage" => {
                if let Some(image_ref) = current.take() {
                    if !image_ref.id.is_empty() && !image_ref.rel_id.is_empty() {
                        result.push(image_ref);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(result)
}

fn parse_relationships(xml: &[u8]) -> Result<HashMap<String, String>, XlsxError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut result = HashMap::new();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"Relationship" =>
            {
                if let (Some(id), Some(target)) = (attr(&element, b"Id"), attr(&element, b"Target"))
                {
                    result.insert(id, target);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(result)
}

fn normalize_target(target: &str) -> String {
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wps_cell_image_ids_relationships_and_dimensions() {
        let refs = parse_cell_image_refs(br#"<etc:cellImages xmlns:etc="urn:e" xmlns:xdr="urn:x" xmlns:a="urn:a" xmlns:r="urn:r"><etc:cellImage><xdr:pic><xdr:nvPicPr><xdr:cNvPr name="ID_ABC"/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="rId7"/></xdr:blipFill><xdr:spPr><a:xfrm><a:ext cx="914400" cy="457200"/></a:xfrm></xdr:spPr></xdr:pic></etc:cellImage></etc:cellImages>"#).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(
            (refs[0].id.as_str(), refs[0].rel_id.as_str()),
            ("ID_ABC", "rId7")
        );
        assert_eq!((refs[0].width_px, refs[0].height_px), (96, 48));

        let rels = parse_relationships(br#"<Relationships><Relationship Id="rId7" Target="media/image7.png"/></Relationships>"#).unwrap();
        assert_eq!(
            rels.get("rId7").map(String::as_str),
            Some("media/image7.png")
        );
    }
}
