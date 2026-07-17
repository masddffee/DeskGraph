use std::io::{Cursor, Read};
use std::time::Instant;

use deskgraph_domain::is_valid_xlsx_cell_reference;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader as XmlReader;
use zip::{CompressionMethod, ZipArchive};

use crate::{
    ABSOLUTE_MAX_DECOMPRESSED_BYTES, CancellationSignal, ChunkProvenance, ControlledSource,
    ExtractedChunk, ExtractionError, ExtractionLimits, ExtractionOutput, ExtractionRequest,
    ExtractorProvider, MediaKind, UNTRUSTED_TEXT, check_control, read_bounded_source,
    validate_limits,
};

const MAX_ARCHIVE_ENTRIES: usize = 4_096;
const MAX_SELECTED_PARTS: usize = 1_024;
const MAX_COMPRESSION_RATIO: u128 = 200;
const MAX_XML_DEPTH: usize = 128;
const MAX_XML_ATTRIBUTES: usize = 128;
const MAX_XML_EVENTS: usize = 1_000_000;
const MAX_XML_TEXT_NODE_BYTES: usize = 256 * 1024;
const MAX_STRUCTURAL_UNITS: u32 = 100_000;
const MAX_SHARED_STRINGS: usize = 65_536;
const WORDPROCESSINGML_TRANSITIONAL: &[u8] =
    b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const WORDPROCESSINGML_STRICT: &[u8] = b"http://purl.oclc.org/ooxml/wordprocessingml/main";
const DRAWINGML_TRANSITIONAL: &[u8] = b"http://schemas.openxmlformats.org/drawingml/2006/main";
const DRAWINGML_STRICT: &[u8] = b"http://purl.oclc.org/ooxml/drawingml/main";
const SPREADSHEETML_TRANSITIONAL: &[u8] =
    b"http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SPREADSHEETML_STRICT: &[u8] = b"http://purl.oclc.org/ooxml/spreadsheetml/main";
const EOCD_MIN_BYTES: usize = 22;
const EOCD_MAX_COMMENT_BYTES: usize = u16::MAX as usize;

#[derive(Clone, Copy, Debug, Default)]
pub struct OoxmlTextExtractor;

impl ExtractorProvider for OoxmlTextExtractor {
    fn provider_id(&self) -> &'static str {
        "deskgraph.ooxml-text"
    }

    fn provider_version(&self) -> &'static str {
        "1"
    }

    fn supports(&self, media_kind: MediaKind) -> bool {
        matches!(
            media_kind,
            MediaKind::Docx | MediaKind::Pptx | MediaKind::Xlsx
        )
    }

    fn extract(
        &self,
        source: &mut dyn ControlledSource,
        request: ExtractionRequest,
        limits: ExtractionLimits,
        cancellation: &dyn CancellationSignal,
    ) -> Result<ExtractionOutput, ExtractionError> {
        validate_limits(limits)?;
        if !self.supports(request.media_kind) {
            return Err(ExtractionError::UnsupportedMediaKind);
        }
        if request.expected_source_bytes > limits.max_source_bytes {
            return Err(ExtractionError::SourceTooLarge);
        }

        let started = Instant::now();
        check_control(started, limits.max_processing_time, cancellation)?;
        let bytes = read_bounded_source(source, request, limits, started, cancellation)?;
        let (mut archive, parts) = open_archive(&bytes, request.media_kind, limits)?;
        let mut chunks = Vec::new();
        let mut output_bytes = 0_u64;
        let mut selected_bytes = 0_usize;
        let mut shared_strings = Vec::new();

        for part in parts {
            check_control(started, limits.max_processing_time, cancellation)?;
            let part_bytes = read_selected_part(
                &mut archive,
                &part,
                limits,
                &mut selected_bytes,
                started,
                cancellation,
            )?;
            match part.kind {
                PartKind::DocxDocument => parse_docx(
                    &part_bytes,
                    &mut chunks,
                    &mut output_bytes,
                    limits,
                    started,
                    cancellation,
                )?,
                PartKind::PptxSlide => parse_pptx_slide(
                    &part_bytes,
                    part.unit_number,
                    &mut chunks,
                    &mut output_bytes,
                    limits,
                    started,
                    cancellation,
                )?,
                PartKind::XlsxSharedStrings => {
                    shared_strings =
                        parse_shared_strings(&part_bytes, limits, started, cancellation)?;
                }
                PartKind::XlsxSheet => parse_xlsx_sheet(
                    &part_bytes,
                    part.unit_number,
                    &shared_strings,
                    &mut chunks,
                    &mut output_bytes,
                    limits,
                    started,
                    cancellation,
                )?,
            }
        }

        Ok(ExtractionOutput {
            provider_id: self.provider_id(),
            provider_version: self.provider_version(),
            media_kind: request.media_kind,
            source_bytes: request.expected_source_bytes,
            output_bytes,
            modified_unix_ns: request.modified_unix_ns,
            chunks,
            image_metadata: None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PartKind {
    DocxDocument,
    XlsxSharedStrings,
    PptxSlide,
    XlsxSheet,
}

#[derive(Clone, Debug)]
struct SelectedPart {
    index: usize,
    kind: PartKind,
    unit_number: u32,
    claimed_size: u64,
}

type OpenedArchive<'a> = (ZipArchive<Cursor<&'a [u8]>>, Vec<SelectedPart>);

fn open_archive(
    bytes: &[u8],
    media_kind: MediaKind,
    limits: ExtractionLimits,
) -> Result<OpenedArchive<'_>, ExtractionError> {
    let declared_entries = declared_entry_count(bytes)?;
    if declared_entries == 0 {
        return Err(ExtractionError::InvalidOoxmlArchive);
    }
    if declared_entries > MAX_ARCHIVE_ENTRIES {
        return Err(ExtractionError::OoxmlEntryLimitExceeded);
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(map_zip_error)?;
    if archive.len() != declared_entries {
        return Err(ExtractionError::UnsafeOoxmlArchive);
    }
    if archive.has_overlapping_files().map_err(map_zip_error)? {
        return Err(ExtractionError::UnsafeOoxmlArchive);
    }

    let archive_claimed_limit = limits
        .max_decompressed_bytes
        .saturating_mul(4)
        .min(ABSOLUTE_MAX_DECOMPRESSED_BYTES);
    let mut claimed_total = 0_u128;
    let mut selected = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(map_zip_error)?;
        let name = file.name();
        if !name.is_ascii()
            || name.contains('\\')
            || file.enclosed_name().is_none()
            || file.is_symlink()
        {
            return Err(ExtractionError::UnsafeOoxmlArchive);
        }
        if file.encrypted() {
            return Err(ExtractionError::EncryptedOoxmlUnsupported);
        }
        if !matches!(
            file.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(ExtractionError::UnsupportedOoxmlCompression);
        }
        claimed_total = claimed_total
            .checked_add(u128::from(file.size()))
            .ok_or(ExtractionError::DecompressionLimitExceeded)?;
        if claimed_total > archive_claimed_limit as u128 {
            return Err(ExtractionError::DecompressionLimitExceeded);
        }
        if file.size() > 0
            && u128::from(file.size())
                > u128::from(file.compressed_size().max(1)) * MAX_COMPRESSION_RATIO
        {
            return Err(ExtractionError::OoxmlCompressionRatioExceeded);
        }
        if !file.is_file() {
            continue;
        }
        if let Some((kind, unit_number)) = selected_part(name, media_kind) {
            selected.push(SelectedPart {
                index,
                kind,
                unit_number,
                claimed_size: file.size(),
            });
        }
    }
    if selected.is_empty() || selected.len() > MAX_SELECTED_PARTS {
        return if selected.is_empty() {
            Err(ExtractionError::MissingOoxmlPart)
        } else {
            Err(ExtractionError::OoxmlEntryLimitExceeded)
        };
    }
    selected.sort_by_key(|part| (part.kind, part.unit_number));
    if !has_required_parts(&selected, media_kind) {
        return Err(ExtractionError::MissingOoxmlPart);
    }
    Ok((archive, selected))
}

fn declared_entry_count(bytes: &[u8]) -> Result<usize, ExtractionError> {
    if bytes.len() < EOCD_MIN_BYTES {
        return Err(ExtractionError::InvalidOoxmlArchive);
    }
    let lower = bytes
        .len()
        .saturating_sub(EOCD_MIN_BYTES + EOCD_MAX_COMMENT_BYTES);
    let upper = bytes.len() - EOCD_MIN_BYTES;
    for offset in (lower..=upper).rev() {
        if bytes.get(offset..offset + 4) != Some(b"PK\x05\x06") {
            continue;
        }
        let comment_length = read_u16(bytes, offset + 20)? as usize;
        if offset + EOCD_MIN_BYTES + comment_length != bytes.len() {
            continue;
        }
        let disk_number = read_u16(bytes, offset + 4)?;
        let directory_disk = read_u16(bytes, offset + 6)?;
        let disk_entries = read_u16(bytes, offset + 8)?;
        let total_entries = read_u16(bytes, offset + 10)?;
        if disk_number != 0 || directory_disk != 0 || disk_entries != total_entries {
            return Err(ExtractionError::InvalidOoxmlArchive);
        }
        if total_entries == u16::MAX {
            return Err(ExtractionError::OoxmlEntryLimitExceeded);
        }
        return Ok(total_entries as usize);
    }
    Err(ExtractionError::InvalidOoxmlArchive)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ExtractionError> {
    let pair: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or(ExtractionError::InvalidOoxmlArchive)?
        .try_into()
        .map_err(|_| ExtractionError::InvalidOoxmlArchive)?;
    Ok(u16::from_le_bytes(pair))
}

fn selected_part(name: &str, media_kind: MediaKind) -> Option<(PartKind, u32)> {
    match media_kind {
        MediaKind::Docx if name == "word/document.xml" => Some((PartKind::DocxDocument, 1)),
        MediaKind::Pptx => numbered_part(name, "ppt/slides/slide", ".xml")
            .map(|number| (PartKind::PptxSlide, number)),
        MediaKind::Xlsx if name == "xl/sharedStrings.xml" => Some((PartKind::XlsxSharedStrings, 0)),
        MediaKind::Xlsx => numbered_part(name, "xl/worksheets/sheet", ".xml")
            .map(|number| (PartKind::XlsxSheet, number)),
        _ => None,
    }
}

fn numbered_part(name: &str, prefix: &str, suffix: &str) -> Option<u32> {
    let number = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    if number.is_empty()
        || number.starts_with('0')
        || !number.as_bytes().iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    number.parse::<u32>().ok().filter(|number| *number > 0)
}

fn has_required_parts(parts: &[SelectedPart], media_kind: MediaKind) -> bool {
    match media_kind {
        MediaKind::Docx => parts.iter().any(|part| part.kind == PartKind::DocxDocument),
        MediaKind::Pptx => parts.iter().any(|part| part.kind == PartKind::PptxSlide),
        MediaKind::Xlsx => parts.iter().any(|part| part.kind == PartKind::XlsxSheet),
        _ => false,
    }
}

fn map_zip_error(error: zip::result::ZipError) -> ExtractionError {
    match error {
        zip::result::ZipError::UnsupportedArchive(message)
            if message == zip::result::ZipError::PASSWORD_REQUIRED =>
        {
            ExtractionError::EncryptedOoxmlUnsupported
        }
        zip::result::ZipError::CompressionMethodNotSupported(_)
        | zip::result::ZipError::UnsupportedArchive(_) => {
            ExtractionError::UnsupportedOoxmlCompression
        }
        _ => ExtractionError::InvalidOoxmlArchive,
    }
}

fn read_selected_part(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    part: &SelectedPart,
    limits: ExtractionLimits,
    selected_bytes: &mut usize,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<Vec<u8>, ExtractionError> {
    let claimed = usize::try_from(part.claimed_size)
        .map_err(|_| ExtractionError::DecompressionLimitExceeded)?;
    let remaining = limits
        .max_decompressed_bytes
        .checked_sub(*selected_bytes)
        .ok_or(ExtractionError::DecompressionLimitExceeded)?;
    if claimed > remaining {
        return Err(ExtractionError::DecompressionLimitExceeded);
    }
    let mut file = archive.by_index(part.index).map_err(map_zip_error)?;
    if file.encrypted() {
        return Err(ExtractionError::EncryptedOoxmlUnsupported);
    }
    let mut output = Vec::with_capacity(claimed.min(64 * 1024));
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| ExtractionError::InvalidOoxmlArchive)?;
        if read == 0 {
            break;
        }
        let next = output
            .len()
            .checked_add(read)
            .ok_or(ExtractionError::DecompressionLimitExceeded)?;
        if next > remaining {
            return Err(ExtractionError::DecompressionLimitExceeded);
        }
        output.extend_from_slice(&buffer[..read]);
    }
    if output.len() != claimed {
        return Err(ExtractionError::InvalidOoxmlArchive);
    }
    *selected_bytes = selected_bytes
        .checked_add(output.len())
        .ok_or(ExtractionError::DecompressionLimitExceeded)?;
    Ok(output)
}

struct XmlGuard {
    depth: usize,
    events: usize,
}

impl XmlGuard {
    fn new() -> Self {
        Self {
            depth: 0,
            events: 0,
        }
    }

    fn observe(&mut self, event: &Event<'_>) -> Result<(), ExtractionError> {
        self.events = self
            .events
            .checked_add(1)
            .ok_or(ExtractionError::OoxmlStructureLimitExceeded)?;
        if self.events > MAX_XML_EVENTS {
            return Err(ExtractionError::OoxmlStructureLimitExceeded);
        }
        match event {
            Event::Start(start) => {
                validate_start(start)?;
                self.depth = self
                    .depth
                    .checked_add(1)
                    .ok_or(ExtractionError::OoxmlStructureLimitExceeded)?;
                if self.depth > MAX_XML_DEPTH {
                    return Err(ExtractionError::OoxmlStructureLimitExceeded);
                }
            }
            Event::Empty(start) => validate_start(start)?,
            Event::End(_) => {
                self.depth = self
                    .depth
                    .checked_sub(1)
                    .ok_or(ExtractionError::InvalidOoxmlXml)?;
            }
            Event::Text(text) if text.len() > MAX_XML_TEXT_NODE_BYTES => {
                return Err(ExtractionError::OoxmlStructureLimitExceeded);
            }
            Event::CData(text) if text.len() > MAX_XML_TEXT_NODE_BYTES => {
                return Err(ExtractionError::OoxmlStructureLimitExceeded);
            }
            Event::PI(_) | Event::DocType(_) => {
                return Err(ExtractionError::InvalidOoxmlXml);
            }
            _ => {}
        }
        Ok(())
    }

    fn finish(self) -> Result<(), ExtractionError> {
        if self.depth == 0 {
            Ok(())
        } else {
            Err(ExtractionError::InvalidOoxmlXml)
        }
    }
}

fn validate_start(start: &BytesStart<'_>) -> Result<(), ExtractionError> {
    if start.name().as_ref().len() > 256 {
        return Err(ExtractionError::OoxmlStructureLimitExceeded);
    }
    let mut count = 0_usize;
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        count = count
            .checked_add(1)
            .ok_or(ExtractionError::OoxmlStructureLimitExceeded)?;
        if count > MAX_XML_ATTRIBUTES
            || attribute.key.as_ref().len() > 256
            || attribute.value.len() > MAX_XML_TEXT_NODE_BYTES
        {
            return Err(ExtractionError::OoxmlStructureLimitExceeded);
        }
    }
    Ok(())
}

fn configured_reader(xml: &[u8]) -> XmlReader<&[u8]> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().enable_all_checks(true);
    reader
        .resolver_mut()
        .set_max_declarations_per_element(MAX_XML_ATTRIBUTES);
    reader
}

fn validate_declaration(declaration: &BytesDecl<'_>) -> Result<(), ExtractionError> {
    let version = declaration
        .version()
        .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
    if version.as_ref() != b"1.0" {
        return Err(ExtractionError::InvalidOoxmlXml);
    }
    if let Some(encoding) = declaration.encoding() {
        let encoding = encoding.map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        if !encoding.eq_ignore_ascii_case(b"UTF-8") && !encoding.eq_ignore_ascii_case(b"UTF8") {
            return Err(ExtractionError::InvalidOoxmlXml);
        }
    }
    Ok(())
}

fn decoded_text(text: &quick_xml::events::BytesText<'_>) -> Result<String, ExtractionError> {
    text.decode()
        .map(|value| value.into_owned())
        .map_err(|_| ExtractionError::InvalidOoxmlXml)
}

fn decoded_cdata(text: &quick_xml::events::BytesCData<'_>) -> Result<String, ExtractionError> {
    text.decode()
        .map(|value| value.into_owned())
        .map_err(|_| ExtractionError::InvalidOoxmlXml)
}

fn decoded_reference(reference: &BytesRef<'_>) -> Result<char, ExtractionError> {
    if let Some(value) = reference
        .resolve_char_ref()
        .map_err(|_| ExtractionError::InvalidOoxmlXml)?
    {
        return if is_xml_10_character(value) {
            Ok(value)
        } else {
            Err(ExtractionError::InvalidOoxmlXml)
        };
    }
    let name = reference
        .decode()
        .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
    match name.as_ref() {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "apos" => Ok('\''),
        "quot" => Ok('"'),
        _ => Err(ExtractionError::InvalidOoxmlXml),
    }
}

fn is_xml_10_character(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&value)
        || ('\u{E000}'..='\u{FFFD}').contains(&value)
        || ('\u{10000}'..='\u{10FFFF}').contains(&value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OoxmlNamespace {
    Wordprocessing,
    Drawing,
    Spreadsheet,
    Other,
}

fn namespace_kind(namespace: ResolveResult<'_>) -> Result<OoxmlNamespace, ExtractionError> {
    match namespace {
        ResolveResult::Bound(namespace)
            if matches!(
                namespace.as_ref(),
                WORDPROCESSINGML_TRANSITIONAL | WORDPROCESSINGML_STRICT
            ) =>
        {
            Ok(OoxmlNamespace::Wordprocessing)
        }
        ResolveResult::Bound(namespace)
            if matches!(
                namespace.as_ref(),
                DRAWINGML_TRANSITIONAL | DRAWINGML_STRICT
            ) =>
        {
            Ok(OoxmlNamespace::Drawing)
        }
        ResolveResult::Bound(namespace)
            if matches!(
                namespace.as_ref(),
                SPREADSHEETML_TRANSITIONAL | SPREADSHEETML_STRICT
            ) =>
        {
            Ok(OoxmlNamespace::Spreadsheet)
        }
        ResolveResult::Bound(_) | ResolveResult::Unbound => Ok(OoxmlNamespace::Other),
        ResolveResult::Unknown(_) => Err(ExtractionError::InvalidOoxmlXml),
    }
}

fn is_element(
    namespace: OoxmlNamespace,
    start: &BytesStart<'_>,
    expected_namespace: OoxmlNamespace,
    expected_local_name: &[u8],
) -> bool {
    namespace == expected_namespace && start.local_name().as_ref() == expected_local_name
}

fn parse_docx(
    xml: &[u8],
    chunks: &mut Vec<ExtractedChunk>,
    output_bytes: &mut u64,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<(), ExtractionError> {
    let mut reader = configured_reader(xml);
    let mut guard = XmlGuard::new();
    let mut paragraph_number = 0_u32;
    let mut paragraph = None::<String>;
    let mut in_text = false;
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        let namespace = namespace_kind(namespace)?;
        guard.observe(&event)?;
        match event {
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Wordprocessing, b"p") =>
            {
                if paragraph.is_some() {
                    return Err(ExtractionError::InvalidOoxmlXml);
                }
                paragraph_number = paragraph_number
                    .checked_add(1)
                    .ok_or(ExtractionError::OoxmlStructureLimitExceeded)?;
                if paragraph_number > MAX_STRUCTURAL_UNITS {
                    return Err(ExtractionError::OoxmlStructureLimitExceeded);
                }
                paragraph = Some(String::new());
            }
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Wordprocessing, b"t") =>
            {
                in_text = true
            }
            Event::Empty(start)
                if is_element(namespace, &start, OoxmlNamespace::Wordprocessing, b"tab")
                    && paragraph.is_some() =>
            {
                paragraph.as_mut().expect("checked").push('\t');
            }
            Event::Empty(start)
                if is_element(namespace, &start, OoxmlNamespace::Wordprocessing, b"br")
                    && paragraph.is_some() =>
            {
                paragraph.as_mut().expect("checked").push('\n');
            }
            Event::Text(text) if in_text && paragraph.is_some() => {
                paragraph
                    .as_mut()
                    .expect("checked")
                    .push_str(&decoded_text(&text)?);
            }
            Event::CData(text) if in_text && paragraph.is_some() => {
                paragraph
                    .as_mut()
                    .expect("checked")
                    .push_str(&decoded_cdata(&text)?);
            }
            Event::GeneralRef(reference) if in_text && paragraph.is_some() => {
                paragraph
                    .as_mut()
                    .expect("checked")
                    .push(decoded_reference(&reference)?);
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Wordprocessing
                    && end.local_name().as_ref() == b"t" =>
            {
                in_text = false
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Wordprocessing
                    && end.local_name().as_ref() == b"p" =>
            {
                let content = paragraph.take().ok_or(ExtractionError::InvalidOoxmlXml)?;
                append_structural_chunks(
                    &content,
                    chunks,
                    output_bytes,
                    limits,
                    started,
                    cancellation,
                    |fragment_index| ChunkProvenance::DocxParagraph {
                        paragraph_number,
                        fragment_index,
                    },
                )?;
            }
            Event::Decl(declaration) => validate_declaration(&declaration)?,
            Event::Eof => break,
            _ => {}
        }
    }
    if paragraph.is_some() || in_text {
        return Err(ExtractionError::InvalidOoxmlXml);
    }
    guard.finish()
}

#[allow(clippy::too_many_arguments)]
fn parse_pptx_slide(
    xml: &[u8],
    slide_number: u32,
    chunks: &mut Vec<ExtractedChunk>,
    output_bytes: &mut u64,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<(), ExtractionError> {
    let mut reader = configured_reader(xml);
    let mut guard = XmlGuard::new();
    let mut content = String::new();
    let mut in_text = false;
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        let namespace = namespace_kind(namespace)?;
        guard.observe(&event)?;
        match event {
            Event::Start(start) if is_element(namespace, &start, OoxmlNamespace::Drawing, b"t") => {
                in_text = true
            }
            Event::Text(text) if in_text => content.push_str(&decoded_text(&text)?),
            Event::CData(text) if in_text => content.push_str(&decoded_cdata(&text)?),
            Event::GeneralRef(reference) if in_text => {
                content.push(decoded_reference(&reference)?);
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Drawing && end.local_name().as_ref() == b"t" =>
            {
                in_text = false
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Drawing
                    && end.local_name().as_ref() == b"p"
                    && !content.ends_with('\n') =>
            {
                content.push('\n');
            }
            Event::Decl(declaration) => validate_declaration(&declaration)?,
            Event::Eof => break,
            _ => {}
        }
    }
    if in_text {
        return Err(ExtractionError::InvalidOoxmlXml);
    }
    guard.finish()?;
    if content.ends_with('\n') {
        content.pop();
    }
    append_structural_chunks(
        &content,
        chunks,
        output_bytes,
        limits,
        started,
        cancellation,
        |fragment_index| ChunkProvenance::PptxSlide {
            slide_number,
            fragment_index,
        },
    )
}

fn parse_shared_strings(
    xml: &[u8],
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<Vec<String>, ExtractionError> {
    let mut reader = configured_reader(xml);
    let mut guard = XmlGuard::new();
    let mut strings = Vec::new();
    let mut current = None::<String>;
    let mut in_text = false;
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        let namespace = namespace_kind(namespace)?;
        guard.observe(&event)?;
        match event {
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"si") =>
            {
                if current.is_some() || strings.len() >= MAX_SHARED_STRINGS {
                    return Err(ExtractionError::OoxmlStructureLimitExceeded);
                }
                current = Some(String::new());
            }
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"t")
                    && current.is_some() =>
            {
                in_text = true;
            }
            Event::Text(text) if in_text && current.is_some() => {
                current
                    .as_mut()
                    .expect("checked")
                    .push_str(&decoded_text(&text)?);
            }
            Event::CData(text) if in_text && current.is_some() => {
                current
                    .as_mut()
                    .expect("checked")
                    .push_str(&decoded_cdata(&text)?);
            }
            Event::GeneralRef(reference) if in_text && current.is_some() => {
                current
                    .as_mut()
                    .expect("checked")
                    .push(decoded_reference(&reference)?);
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Spreadsheet
                    && end.local_name().as_ref() == b"t" =>
            {
                in_text = false
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Spreadsheet
                    && end.local_name().as_ref() == b"si" =>
            {
                strings.push(current.take().ok_or(ExtractionError::InvalidOoxmlXml)?);
            }
            Event::Decl(declaration) => validate_declaration(&declaration)?,
            Event::Eof => break,
            _ => {}
        }
    }
    if current.is_some() || in_text {
        return Err(ExtractionError::InvalidOoxmlXml);
    }
    guard.finish()?;
    Ok(strings)
}

struct CellState {
    reference: String,
    cell_type: Option<String>,
    has_formula: bool,
    raw_value: String,
    inline_text: String,
    in_value: bool,
    in_text: bool,
}

#[allow(clippy::too_many_arguments)]
fn parse_xlsx_sheet(
    xml: &[u8],
    sheet_number: u32,
    shared_strings: &[String],
    chunks: &mut Vec<ExtractedChunk>,
    output_bytes: &mut u64,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<(), ExtractionError> {
    let mut reader = configured_reader(xml);
    let mut guard = XmlGuard::new();
    let mut cell = None::<CellState>;
    let mut cell_count = 0_u32;
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        let namespace = namespace_kind(namespace)?;
        guard.observe(&event)?;
        match event {
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"c") =>
            {
                if cell.is_some() {
                    return Err(ExtractionError::InvalidOoxmlXml);
                }
                let reference = attribute_value(&start, b"r")?
                    .filter(|value| is_valid_xlsx_cell_reference(value))
                    .ok_or(ExtractionError::InvalidOoxmlXml)?;
                let cell_type = attribute_value(&start, b"t")?;
                cell = Some(CellState {
                    reference,
                    cell_type,
                    has_formula: false,
                    raw_value: String::new(),
                    inline_text: String::new(),
                    in_value: false,
                    in_text: false,
                });
            }
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"f")
                    && cell.is_some() =>
            {
                cell.as_mut().expect("checked").has_formula = true;
            }
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"v")
                    && cell.is_some() =>
            {
                cell.as_mut().expect("checked").in_value = true;
            }
            Event::Start(start)
                if is_element(namespace, &start, OoxmlNamespace::Spreadsheet, b"t")
                    && cell.is_some() =>
            {
                cell.as_mut().expect("checked").in_text = true;
            }
            Event::Text(text) if cell.as_ref().is_some_and(|cell| cell.in_value) => {
                cell.as_mut()
                    .expect("checked")
                    .raw_value
                    .push_str(&decoded_text(&text)?);
            }
            Event::Text(text) if cell.as_ref().is_some_and(|cell| cell.in_text) => {
                cell.as_mut()
                    .expect("checked")
                    .inline_text
                    .push_str(&decoded_text(&text)?);
            }
            Event::CData(text) if cell.as_ref().is_some_and(|cell| cell.in_value) => {
                cell.as_mut()
                    .expect("checked")
                    .raw_value
                    .push_str(&decoded_cdata(&text)?);
            }
            Event::CData(text) if cell.as_ref().is_some_and(|cell| cell.in_text) => {
                cell.as_mut()
                    .expect("checked")
                    .inline_text
                    .push_str(&decoded_cdata(&text)?);
            }
            Event::GeneralRef(reference) if cell.as_ref().is_some_and(|cell| cell.in_value) => {
                cell.as_mut()
                    .expect("checked")
                    .raw_value
                    .push(decoded_reference(&reference)?);
            }
            Event::GeneralRef(reference) if cell.as_ref().is_some_and(|cell| cell.in_text) => {
                cell.as_mut()
                    .expect("checked")
                    .inline_text
                    .push(decoded_reference(&reference)?);
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Spreadsheet
                    && end.local_name().as_ref() == b"v"
                    && cell.is_some() =>
            {
                cell.as_mut().expect("checked").in_value = false;
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Spreadsheet
                    && end.local_name().as_ref() == b"t"
                    && cell.is_some() =>
            {
                cell.as_mut().expect("checked").in_text = false;
            }
            Event::End(end)
                if namespace == OoxmlNamespace::Spreadsheet
                    && end.local_name().as_ref() == b"c" =>
            {
                let completed = cell.take().ok_or(ExtractionError::InvalidOoxmlXml)?;
                cell_count = cell_count
                    .checked_add(1)
                    .ok_or(ExtractionError::OoxmlStructureLimitExceeded)?;
                if cell_count > MAX_STRUCTURAL_UNITS {
                    return Err(ExtractionError::OoxmlStructureLimitExceeded);
                }
                if let Some(value) = cell_value(&completed, shared_strings)? {
                    let reference = completed.reference.clone();
                    append_structural_chunks(
                        value,
                        chunks,
                        output_bytes,
                        limits,
                        started,
                        cancellation,
                        |fragment_index| ChunkProvenance::XlsxCell {
                            sheet_number,
                            cell_reference: reference.clone(),
                            fragment_index,
                        },
                    )?;
                }
            }
            Event::Decl(declaration) => validate_declaration(&declaration)?,
            Event::Eof => break,
            _ => {}
        }
    }
    if cell.is_some() {
        return Err(ExtractionError::InvalidOoxmlXml);
    }
    guard.finish()
}

fn attribute_value(
    start: &BytesStart<'_>,
    local_key: &[u8],
) -> Result<Option<String>, ExtractionError> {
    let mut value = None;
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        let key = attribute.key.local_name();
        if key.as_ref() != local_key {
            continue;
        }
        if value.is_some() {
            return Err(ExtractionError::InvalidOoxmlXml);
        }
        let decoded = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, start.decoder())
            .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
        if decoded.len() > MAX_XML_TEXT_NODE_BYTES {
            return Err(ExtractionError::OoxmlStructureLimitExceeded);
        }
        value = Some(decoded.into_owned());
    }
    Ok(value)
}

fn cell_value<'a>(
    cell: &'a CellState,
    shared_strings: &'a [String],
) -> Result<Option<&'a str>, ExtractionError> {
    if cell.has_formula {
        return Ok(None);
    }
    match cell.cell_type.as_deref() {
        Some("s") => {
            let index = cell
                .raw_value
                .parse::<usize>()
                .map_err(|_| ExtractionError::InvalidOoxmlXml)?;
            shared_strings
                .get(index)
                .map(String::as_str)
                .map(Some)
                .ok_or(ExtractionError::InvalidOoxmlXml)
        }
        Some("inlineStr") => Ok(Some(cell.inline_text.as_str())),
        Some("str" | "n" | "b" | "e" | "d") | None => Ok(Some(cell.raw_value.as_str())),
        Some(_) => Err(ExtractionError::InvalidOoxmlXml),
    }
}

#[allow(clippy::too_many_arguments)]
fn append_structural_chunks<F>(
    content: &str,
    chunks: &mut Vec<ExtractedChunk>,
    output_bytes: &mut u64,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
    mut provenance: F,
) -> Result<(), ExtractionError>
where
    F: FnMut(u32) -> ChunkProvenance,
{
    let mut start = 0_usize;
    let mut fragment_index = 0_u32;
    while start < content.len() {
        check_control(started, limits.max_processing_time, cancellation)?;
        if chunks.len() >= limits.max_chunks {
            return Err(ExtractionError::ChunkLimitExceeded);
        }
        let mut end = start
            .saturating_add(limits.max_chunk_bytes)
            .min(content.len());
        while end > start && !content.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err(ExtractionError::InvalidLimits);
        }
        let chunk_bytes =
            u64::try_from(end - start).map_err(|_| ExtractionError::OutputTooLarge)?;
        *output_bytes = output_bytes
            .checked_add(chunk_bytes)
            .ok_or(ExtractionError::OutputTooLarge)?;
        if *output_bytes > limits.max_output_bytes {
            return Err(ExtractionError::OutputTooLarge);
        }
        chunks.push(ExtractedChunk {
            ordinal: u32::try_from(chunks.len())
                .map_err(|_| ExtractionError::ChunkLimitExceeded)?,
            text: content[start..end].to_string(),
            provenance: provenance(fragment_index),
            trust_class: UNTRUSTED_TEXT,
        });
        fragment_index = fragment_index
            .checked_add(1)
            .ok_or(ExtractionError::ChunkLimitExceeded)?;
        if end == content.len() {
            break;
        }
        let mut next = end.saturating_sub(limits.chunk_overlap_bytes);
        while next > start && !content.is_char_boundary(next) {
            next -= 1;
        }
        start = if next > start { next } else { end };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::{Cursor, Write};
    use std::time::Duration;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::*;
    use crate::{AtomicCancellation, NoCancellation};

    fn limits() -> ExtractionLimits {
        ExtractionLimits {
            max_source_bytes: 1024 * 1024,
            max_output_bytes: 1024 * 1024,
            max_chunks: 256,
            max_chunk_bytes: 64 * 1024,
            chunk_overlap_bytes: 0,
            max_decompressed_bytes: 1024 * 1024,
            max_pdf_pages: 64,
            max_image_source_bytes: 1024 * 1024,
            max_image_probe_bytes: 1024 * 1024,
            max_image_dimension: 100_000,
            max_image_pixels: 500_000_000,
            max_processing_time: Duration::from_secs(2),
        }
    }

    fn request(media_kind: MediaKind, bytes: &[u8]) -> ExtractionRequest {
        ExtractionRequest {
            media_kind,
            expected_source_bytes: bytes.len() as u64,
            modified_unix_ns: Some(17),
        }
    }

    fn archive(parts: &[(&str, &[u8])], compression: CompressionMethod) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(compression);
        for (name, contents) in parts {
            writer
                .start_file(*name, options)
                .expect("fixture entry should start");
            writer
                .write_all(contents)
                .expect("fixture entry should write");
        }
        writer
            .finish()
            .expect("fixture archive should finish")
            .into_inner()
    }

    fn replace_archive_name(bytes: &mut [u8], from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len(), "fixture names must be the same size");
        let offsets = bytes
            .windows(from.len())
            .enumerate()
            .filter_map(|(offset, candidate)| (candidate == from).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(
            offsets.len(),
            2,
            "local and central names should both exist"
        );
        for offset in offsets {
            bytes[offset..offset + to.len()].copy_from_slice(to);
        }
    }

    fn header_offsets(bytes: &[u8], signature: &[u8; 4]) -> Vec<usize> {
        bytes
            .windows(signature.len())
            .enumerate()
            .filter_map(|(offset, candidate)| (candidate == signature).then_some(offset))
            .collect()
    }

    fn fixture_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixture u16"))
    }

    fn fixture_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixture u32"))
    }

    fn set_fixture_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn set_fixture_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn extract(bytes: &[u8], media_kind: MediaKind) -> Result<ExtractionOutput, ExtractionError> {
        OoxmlTextExtractor.extract(
            &mut Cursor::new(bytes),
            request(media_kind, bytes),
            limits(),
            &NoCancellation,
        )
    }

    #[test]
    fn docx_extracts_mixed_language_paragraphs_with_structural_provenance() {
        let document = r#"<?xml version="1.0" encoding="UTF-8"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body>
            <w:p><w:r><w:t>DeskGraph &amp; local</w:t></w:r><a:p><a:t>ignored drawing text</a:t></a:p><w:tab/><w:r><w:t>本機情境</w:t></w:r></w:p>
            <w:p><w:r><w:t>第二段</w:t></w:r></w:p>
            </w:body></w:document>"#.as_bytes();
        let bytes = archive(
            &[("word/document.xml", document)],
            CompressionMethod::Deflated,
        );
        let output = extract(&bytes, MediaKind::Docx).expect("DOCX should extract");

        assert_eq!(output.provider_id, "deskgraph.ooxml-text");
        assert_eq!(output.chunks.len(), 2);
        assert_eq!(output.chunks[0].text, "DeskGraph & local\t本機情境");
        assert_eq!(output.chunks[1].text, "第二段");
        assert_eq!(
            output.chunks[0].provenance,
            ChunkProvenance::DocxParagraph {
                paragraph_number: 1,
                fragment_index: 0,
            }
        );
    }

    #[test]
    fn pptx_orders_numeric_slide_parts_and_keeps_slide_provenance() {
        let slide_one = r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:p><a:r><a:t>第一張</a:t></a:r></a:p></p:sld>"#.as_bytes();
        let slide_two = r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:p><a:r><a:t>Second slide</a:t></a:r></a:p></p:sld>"#.as_bytes();
        let bytes = archive(
            &[
                ("ppt/slides/slide2.xml", slide_two),
                ("ppt/slides/slide1.xml", slide_one),
            ],
            CompressionMethod::Stored,
        );
        let output = extract(&bytes, MediaKind::Pptx).expect("PPTX should extract");

        assert_eq!(
            output
                .chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一張", "Second slide"]
        );
        assert_eq!(
            output.chunks[1].provenance,
            ChunkProvenance::PptxSlide {
                slide_number: 2,
                fragment_index: 0,
            }
        );
    }

    #[test]
    fn xlsx_extracts_shared_inline_and_numeric_cells_but_ignores_formulas() {
        let shared = r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>專案 Alpha</t></si><si><r><t>Rich</t></r><r><t> text</t></r></si></sst>"#.as_bytes();
        let sheet = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1">
            <c r="A1" t="s"><v>0</v></c>
            <c r="B1" t="inlineStr"><is><t>本機</t></is></c>
            <c r="C1"><v>42</v></c>
            <c r="D1" t="str"><f>REMOTE("secret")</f><v>must-not-publish</v></c>
            </row></sheetData></worksheet>"#.as_bytes();
        let bytes = archive(
            &[
                ("xl/worksheets/sheet1.xml", sheet),
                ("xl/sharedStrings.xml", shared),
                ("xl/externalLinks/externalLink1.xml", b"external secret"),
            ],
            CompressionMethod::Deflated,
        );
        let output = extract(&bytes, MediaKind::Xlsx).expect("XLSX should extract");

        assert_eq!(
            output
                .chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["專案 Alpha", "本機", "42"]
        );
        assert!(output.chunks.iter().all(|chunk| {
            matches!(
                chunk.provenance,
                ChunkProvenance::XlsxCell {
                    sheet_number: 1,
                    ..
                }
            )
        }));
    }

    #[test]
    fn corrupt_archive_and_xml_are_fixed_per_file_errors() {
        assert_eq!(
            extract(b"not a zip", MediaKind::Docx).expect_err("corrupt ZIP must fail"),
            ExtractionError::InvalidOoxmlArchive
        );
        let invalid_xml = archive(
            &[("word/document.xml", b"<w:document><w:p></w:document>")],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&invalid_xml, MediaKind::Docx).expect_err("corrupt XML must fail"),
            ExtractionError::InvalidOoxmlXml
        );
        let xml_11 = archive(
            &[(
                "word/document.xml",
                br#"<?xml version="1.1"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#,
            )],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&xml_11, MediaKind::Docx).expect_err("XML 1.1 must fail closed"),
            ExtractionError::InvalidOoxmlXml
        );
    }

    #[test]
    fn unsafe_duplicate_and_unselected_active_parts_fail_or_stay_inert() {
        let unsafe_archive = archive(
            &[
                ("../word/document.xml", b"outside"),
                ("word/document.xml", b"<w:document/>"),
            ],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&unsafe_archive, MediaKind::Docx).expect_err("traversal must fail"),
            ExtractionError::UnsafeOoxmlArchive
        );

        let mut duplicate = archive(
            &[
                ("word/document.xml", b"<w:document/>"),
                ("word/document.xm1", b"<w:document><w:p/></w:document>"),
            ],
            CompressionMethod::Stored,
        );
        replace_archive_name(&mut duplicate, b"word/document.xm1", b"word/document.xml");
        assert_eq!(
            extract(&duplicate, MediaKind::Docx).expect_err("duplicate names must fail"),
            ExtractionError::UnsafeOoxmlArchive
        );

        let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>safe text</w:t></w:r></w:p></w:body></w:document>"#.as_bytes();
        let inert = archive(
            &[
                ("word/document.xml", document),
                ("word/vbaProject.bin", b"macro secret"),
                ("word/embeddings/oleObject1.bin", b"embedded secret"),
                (
                    "word/_rels/document.xml.rels",
                    b"https://example.invalid/secret",
                ),
            ],
            CompressionMethod::Stored,
        );
        let output = extract(&inert, MediaKind::Docx).expect("unselected parts stay inert");
        assert_eq!(output.chunks[0].text, "safe text");
    }

    #[test]
    fn encrypted_unsupported_and_overlapping_archives_fail_closed() {
        let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:t>safe</w:t></w:p></w:body></w:document>"#.as_bytes();

        let mut encrypted = archive(
            &[("word/document.xml", document)],
            CompressionMethod::Stored,
        );
        let local = header_offsets(&encrypted, b"PK\x03\x04");
        let central = header_offsets(&encrypted, b"PK\x01\x02");
        assert_eq!((local.len(), central.len()), (1, 1));
        let local_flags = fixture_u16(&encrypted, local[0] + 6) | 1;
        let central_flags = fixture_u16(&encrypted, central[0] + 8) | 1;
        set_fixture_u16(&mut encrypted, local[0] + 6, local_flags);
        set_fixture_u16(&mut encrypted, central[0] + 8, central_flags);
        assert_eq!(
            extract(&encrypted, MediaKind::Docx).expect_err("encrypted OOXML must fail"),
            ExtractionError::EncryptedOoxmlUnsupported
        );

        let mut unsupported = archive(
            &[("word/document.xml", document)],
            CompressionMethod::Stored,
        );
        let local = header_offsets(&unsupported, b"PK\x03\x04");
        let central = header_offsets(&unsupported, b"PK\x01\x02");
        set_fixture_u16(&mut unsupported, local[0] + 8, 12);
        set_fixture_u16(&mut unsupported, central[0] + 10, 12);
        assert_eq!(
            extract(&unsupported, MediaKind::Docx).expect_err("unsupported compression must fail"),
            ExtractionError::UnsupportedOoxmlCompression
        );

        let mut overlapping = archive(
            &[
                ("word/document.xml", document),
                ("word/styles.xml", b"<styles/>"),
            ],
            CompressionMethod::Stored,
        );
        let central = header_offsets(&overlapping, b"PK\x01\x02");
        assert_eq!(central.len(), 2);
        let first_local_offset = fixture_u32(&overlapping, central[0] + 42);
        set_fixture_u32(&mut overlapping, central[1] + 42, first_local_offset);
        assert_eq!(
            extract(&overlapping, MediaKind::Docx)
                .expect_err("overlapping archive entries must fail"),
            ExtractionError::UnsafeOoxmlArchive
        );
    }

    #[test]
    fn required_parts_shared_strings_output_and_chunk_count_are_bounded() {
        let missing = archive(
            &[("word/styles.xml", b"<styles/>")],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&missing, MediaKind::Docx).expect_err("document part is required"),
            ExtractionError::MissingOoxmlPart
        );

        let invalid_shared_reference = archive(
            &[(
                "xl/worksheets/sheet1.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row><c r="A1" t="s"><v>7</v></c></row></sheetData></worksheet>"#.as_bytes(),
            )],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&invalid_shared_reference, MediaKind::Xlsx)
                .expect_err("missing shared string index must fail"),
            ExtractionError::InvalidOoxmlXml
        );

        let document = archive(
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:t>abcdefgh</w:t></w:p></w:body></w:document>"#.as_bytes(),
            )],
            CompressionMethod::Stored,
        );
        let mut output_limited = limits();
        output_limited.max_output_bytes = 4;
        output_limited.max_chunk_bytes = 4;
        assert_eq!(
            OoxmlTextExtractor
                .extract(
                    &mut Cursor::new(&document),
                    request(MediaKind::Docx, &document),
                    output_limited,
                    &NoCancellation,
                )
                .expect_err("output limit must stop publication"),
            ExtractionError::OutputTooLarge
        );

        let mut chunk_limited = limits();
        chunk_limited.max_chunks = 1;
        chunk_limited.max_chunk_bytes = 4;
        assert_eq!(
            OoxmlTextExtractor
                .extract(
                    &mut Cursor::new(&document),
                    request(MediaKind::Docx, &document),
                    chunk_limited,
                    &NoCancellation,
                )
                .expect_err("chunk limit must stop publication"),
            ExtractionError::ChunkLimitExceeded
        );
    }

    #[test]
    fn dtd_custom_entities_and_deep_xml_fail_closed() {
        let dtd = archive(
            &[(
                "word/document.xml",
                r#"<!DOCTYPE x [<!ENTITY secret "boom">]><w:document><w:p><w:t>&secret;</w:t></w:p></w:document>"#.as_bytes(),
            )],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&dtd, MediaKind::Docx).expect_err("DTD must fail"),
            ExtractionError::InvalidOoxmlXml
        );

        let mut deep = String::from(
            "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:p><w:t>",
        );
        for _ in 0..MAX_XML_DEPTH {
            deep.push_str("<x>");
        }
        deep.push_str("text");
        for _ in 0..MAX_XML_DEPTH {
            deep.push_str("</x>");
        }
        deep.push_str("</w:t></w:p></w:document>");
        let deep_archive = archive(
            &[("word/document.xml", deep.as_bytes())],
            CompressionMethod::Stored,
        );
        assert_eq!(
            extract(&deep_archive, MediaKind::Docx).expect_err("deep XML must fail"),
            ExtractionError::OoxmlStructureLimitExceeded
        );
    }

    #[test]
    fn compression_ratio_and_selected_decompression_are_bounded() {
        let repeated = "A".repeat(64 * 1024);
        let document = format!("<w:document><w:p><w:t>{repeated}</w:t></w:p></w:document>");
        let bomb = archive(
            &[("word/document.xml", document.as_bytes())],
            CompressionMethod::Deflated,
        );
        assert_eq!(
            extract(&bomb, MediaKind::Docx).expect_err("ratio bomb must fail"),
            ExtractionError::OoxmlCompressionRatioExceeded
        );

        let normal = archive(
            &[(
                "word/document.xml",
                b"<w:document><w:p><w:t>bounded</w:t></w:p></w:document>",
            )],
            CompressionMethod::Stored,
        );
        let mut tiny = limits();
        tiny.max_decompressed_bytes = 8;
        let error = OoxmlTextExtractor
            .extract(
                &mut Cursor::new(&normal),
                request(MediaKind::Docx, &normal),
                tiny,
                &NoCancellation,
            )
            .expect_err("selected bytes must be bounded");
        assert_eq!(error, ExtractionError::DecompressionLimitExceeded);
    }

    struct StepCancellation {
        remaining: Cell<usize>,
    }

    impl CancellationSignal for StepCancellation {
        fn is_cancelled(&self) -> bool {
            let remaining = self.remaining.get();
            self.remaining.set(remaining.saturating_sub(1));
            remaining == 0
        }
    }

    #[test]
    fn cancellation_is_checked_during_archive_and_xml_units() {
        let document = format!(
            "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">{}</w:document>",
            "<w:p><w:t>unit</w:t></w:p>".repeat(1_000)
        );
        let bytes = archive(
            &[("word/document.xml", document.as_bytes())],
            CompressionMethod::Stored,
        );
        let cancellation = StepCancellation {
            remaining: Cell::new(20),
        };
        let error = OoxmlTextExtractor
            .extract(
                &mut Cursor::new(&bytes),
                request(MediaKind::Docx, &bytes),
                limits(),
                &cancellation,
            )
            .expect_err("cooperative cancellation must stop extraction");
        assert_eq!(error, ExtractionError::Cancelled);

        let cancelled = AtomicCancellation::new();
        cancelled.cancel();
        let error = OoxmlTextExtractor
            .extract(
                &mut Cursor::new(&bytes),
                request(MediaKind::Docx, &bytes),
                limits(),
                &cancelled,
            )
            .expect_err("pre-cancelled extraction must not read");
        assert_eq!(error, ExtractionError::Cancelled);
    }
}
