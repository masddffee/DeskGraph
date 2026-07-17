use std::time::Instant;

use lopdf::{DecompressError, Document, LoadOptions};

use crate::{
    CancellationSignal, ChunkProvenance, ControlledSource, ExtractedChunk, ExtractionError,
    ExtractionLimits, ExtractionOutput, ExtractionRequest, ExtractorProvider, MediaKind,
    UNTRUSTED_TEXT, check_control, read_bounded_source, validate_limits,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct PdfTextExtractor;

impl ExtractorProvider for PdfTextExtractor {
    fn provider_id(&self) -> &'static str {
        "deskgraph.pdf-text"
    }

    fn provider_version(&self) -> &'static str {
        "1"
    }

    fn supports(&self, media_kind: MediaKind) -> bool {
        media_kind == MediaKind::Pdf
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
        let document = Document::load_mem_with_options(
            &bytes,
            LoadOptions {
                strict: true,
                max_decompressed_size: Some(limits.max_decompressed_bytes),
                ..Default::default()
            },
        )
        .map_err(map_pdf_error)?;
        check_control(started, limits.max_processing_time, cancellation)?;

        if document.is_encrypted() || document.was_encrypted() {
            return Err(ExtractionError::EncryptedPdfUnsupported);
        }
        let page_numbers = document.get_pages().keys().copied().collect::<Vec<_>>();
        if page_numbers.is_empty() {
            return Err(ExtractionError::InvalidPdf);
        }
        if page_numbers.len()
            > usize::try_from(limits.max_pdf_pages)
                .map_err(|_| ExtractionError::PageLimitExceeded)?
        {
            return Err(ExtractionError::PageLimitExceeded);
        }

        let mut chunks = Vec::new();
        let mut output_bytes = 0_u64;
        for page_number in page_numbers {
            check_control(started, limits.max_processing_time, cancellation)?;
            let page_text = document
                .extract_text_with_limit(&[page_number], limits.max_decompressed_bytes)
                .map_err(map_pdf_error)?;
            check_control(started, limits.max_processing_time, cancellation)?;
            append_page_chunks(
                &page_text,
                page_number,
                &mut chunks,
                &mut output_bytes,
                limits,
                started,
                cancellation,
            )?;
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

fn map_pdf_error(error: lopdf::Error) -> ExtractionError {
    match error {
        lopdf::Error::Decompress(DecompressError::MemoryLimitExceeded { .. }) => {
            ExtractionError::DecompressionLimitExceeded
        }
        lopdf::Error::Decryption(_)
        | lopdf::Error::InvalidPassword
        | lopdf::Error::UnsupportedSecurityHandler(_) => ExtractionError::EncryptedPdfUnsupported,
        _ => ExtractionError::InvalidPdf,
    }
}

fn append_page_chunks(
    content: &str,
    page_number: u32,
    chunks: &mut Vec<ExtractedChunk>,
    output_bytes: &mut u64,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<(), ExtractionError> {
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
            provenance: ChunkProvenance::PdfPage {
                page_number,
                fragment_index,
            },
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
    use std::io::Cursor;
    use std::time::Duration;

    use lopdf::content::{Content, Operation};
    use lopdf::{
        Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, StringFormat,
        dictionary,
    };

    use super::*;
    use crate::NoCancellation;

    const MULTILINGUAL_CMAP: &str = r#"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /DeskGraph-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
14 beginbfchar
<0001> <0044>
<0002> <0065>
<0003> <0073>
<0004> <006B>
<0005> <0047>
<0006> <0072>
<0007> <0061>
<0008> <0070>
<0009> <0068>
<000A> <0020>
<000B> <672C>
<000C> <6A5F>
<000D> <60C5>
<000E> <5883>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;

    fn limits() -> ExtractionLimits {
        ExtractionLimits {
            max_source_bytes: 1024 * 1024,
            max_output_bytes: 1024 * 1024,
            max_chunks: 128,
            max_chunk_bytes: 64 * 1024,
            chunk_overlap_bytes: 0,
            max_decompressed_bytes: 1024 * 1024,
            max_pdf_pages: 16,
            max_image_source_bytes: 1024 * 1024,
            max_image_probe_bytes: 1024 * 1024,
            max_image_dimension: 100_000,
            max_image_pixels: 500_000_000,
            max_processing_time: Duration::from_secs(2),
        }
    }

    fn request(bytes: &[u8]) -> ExtractionRequest {
        ExtractionRequest {
            media_kind: MediaKind::Pdf,
            expected_source_bytes: bytes.len() as u64,
            modified_unix_ns: Some(11),
        }
    }

    fn save(mut document: Document) -> Vec<u8> {
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("fixture PDF should save");
        bytes
    }

    fn multilingual_document(page_count: usize) -> Document {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let cmap_id = document.add_object(Stream::new(
            dictionary! {},
            MULTILINGUAL_CMAP.as_bytes().to_vec(),
        ));
        let font_id = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "DeskGraphFixture",
            "Encoding" => "Identity-H",
            "ToUnicode" => Object::Reference(cmap_id),
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let encoded_text = vec![
            0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00, 0x07,
            0x00, 0x08, 0x00, 0x09, 0x00, 0x0A, 0x00, 0x0B, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x0E,
        ];
        let mut page_ids = Vec::new();
        for _ in 0..page_count {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new(
                        "Tj",
                        vec![Object::String(
                            encoded_text.clone(),
                            StringFormat::Hexadecimal,
                        )],
                    ),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = document.add_object(Stream::new(
                dictionary! {},
                content.encode().expect("content should encode"),
            ));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Resources" => resources_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            page_ids.push(Object::Reference(page_id));
        }
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids,
                "Count" => page_count as i64,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        document
    }

    fn ascii_document(texts: &[&str]) -> Document {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let font_id = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let mut page_ids = Vec::new();
        for text in texts {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = document.add_object(Stream::new(
                dictionary! {},
                content.encode().expect("content should encode"),
            ));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Resources" => resources_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            page_ids.push(Object::Reference(page_id));
        }
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids,
                "Count" => texts.len() as i64,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        document
    }

    fn extract(
        bytes: &[u8],
        limits: ExtractionLimits,
    ) -> Result<ExtractionOutput, ExtractionError> {
        PdfTextExtractor.extract(
            &mut Cursor::new(bytes),
            request(bytes),
            limits,
            &NoCancellation,
        )
    }

    #[test]
    fn extracts_traditional_chinese_and_english_with_page_provenance() {
        let bytes = save(multilingual_document(1));
        let output = extract(&bytes, limits()).expect("valid PDF should extract");

        assert_eq!(output.provider_id, "deskgraph.pdf-text");
        assert!(
            output
                .chunks
                .iter()
                .any(|chunk| chunk.text.contains("DeskGraph"))
        );
        assert!(
            output
                .chunks
                .iter()
                .any(|chunk| chunk.text.contains("本機情境"))
        );
        assert!(output.chunks.iter().all(|chunk| {
            matches!(
                chunk.provenance,
                ChunkProvenance::PdfPage { page_number: 1, .. }
            ) && chunk.trust_class == UNTRUSTED_TEXT
        }));
    }

    #[test]
    fn corrupt_pdf_is_isolated_with_a_fixed_error() {
        let bytes = b"%PDF-1.7\nnot-a-valid-document";

        assert_eq!(
            extract(bytes, limits()).expect_err("corrupt PDF must fail"),
            ExtractionError::InvalidPdf
        );
    }

    #[test]
    fn empty_password_encrypted_pdf_is_rejected_after_parser_authentication() {
        let mut document = ascii_document(&["encrypted text"]);
        document.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal("fixture-id-1"),
                Object::string_literal("fixture-id-2"),
            ]),
        );
        let state = EncryptionState::try_from(EncryptionVersion::V2 {
            document: &document,
            owner_password: "",
            user_password: "",
            key_length: 128,
            permissions: Permissions::all(),
        })
        .expect("encryption state should build");
        document
            .encrypt(&state)
            .expect("fixture PDF should encrypt");
        let bytes = save(document);

        assert_eq!(
            extract(&bytes, limits()).expect_err("encrypted PDF must fail closed"),
            ExtractionError::EncryptedPdfUnsupported
        );
    }

    #[test]
    fn javascript_launch_uri_and_attachment_payloads_remain_inert() {
        let mut document = ascii_document(&["visible page text"]);
        let javascript_id = document.add_object(dictionary! {
            "S" => "JavaScript",
            "JS" => Object::string_literal("javascript-secret-must-not-appear"),
        });
        let launch_id = document.add_object(dictionary! {
            "S" => "Launch",
            "F" => Object::string_literal("launch-secret-must-not-appear"),
        });
        let uri_id = document.add_object(dictionary! {
            "S" => "URI",
            "URI" => Object::string_literal("https://invalid.example/uri-secret"),
        });
        let attachment_stream_id = document.add_object(Stream::new(
            dictionary! { "Type" => "EmbeddedFile" },
            b"attachment-secret-must-not-appear".to_vec(),
        ));
        let file_spec_id = document.add_object(dictionary! {
            "Type" => "Filespec",
            "F" => Object::string_literal("payload.txt"),
            "EF" => dictionary! { "F" => attachment_stream_id },
        });
        let embedded_names_id = document.add_object(dictionary! {
            "Names" => vec![Object::string_literal("payload.txt"), Object::Reference(file_spec_id)],
        });
        let names_id = document.add_object(dictionary! {
            "EmbeddedFiles" => embedded_names_id,
        });
        let root_id = document
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .expect("catalog should exist");
        let catalog = document
            .get_object_mut(root_id)
            .and_then(Object::as_dict_mut)
            .expect("catalog should be mutable");
        catalog.set("OpenAction", javascript_id);
        catalog.set(
            "AA",
            dictionary! {
                "WC" => launch_id,
                "WS" => uri_id,
            },
        );
        catalog.set("Names", names_id);
        let bytes = save(document);

        let output = extract(&bytes, limits()).expect("inert objects must not block page text");
        let combined = output
            .chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>();
        assert!(combined.contains("visible page text"));
        assert!(!combined.contains("secret"));
        assert!(!combined.contains("invalid.example"));
    }

    #[test]
    fn compressed_page_content_respects_decompression_limit() {
        let mut document = ascii_document(&["placeholder"]);
        let page_id = *document
            .get_pages()
            .values()
            .next()
            .expect("page should exist");
        let content_id = document
            .get_object(page_id)
            .and_then(Object::as_dict)
            .and_then(|page| page.get(b"Contents"))
            .and_then(Object::as_reference)
            .expect("content stream should exist");
        let stream = document
            .get_object_mut(content_id)
            .and_then(Object::as_stream_mut)
            .expect("content should be a stream");
        stream.set_content(vec![b' '; 8 * 1024]);
        stream.compress().expect("fixture stream should compress");
        let bytes = save(document);
        let mut bounded = limits();
        bounded.max_decompressed_bytes = 512;

        assert_eq!(
            extract(&bytes, bounded).expect_err("decompression bomb must fail"),
            ExtractionError::DecompressionLimitExceeded
        );
    }

    #[test]
    fn page_and_output_limits_publish_no_partial_result() {
        let bytes = save(ascii_document(&["first page", "second page"]));
        let mut page_limited = limits();
        page_limited.max_pdf_pages = 1;
        assert_eq!(
            extract(&bytes, page_limited).expect_err("page cap must fail"),
            ExtractionError::PageLimitExceeded
        );

        let mut output_limited = limits();
        output_limited.max_output_bytes = 4;
        assert_eq!(
            extract(&bytes, output_limited).expect_err("output cap must fail"),
            ExtractionError::OutputTooLarge
        );
    }

    struct CancelAfterChecks {
        checks: Cell<usize>,
        cancel_at: usize,
    }

    impl CancellationSignal for CancelAfterChecks {
        fn is_cancelled(&self) -> bool {
            let current = self.checks.get();
            self.checks.set(current + 1);
            current >= self.cancel_at
        }
    }

    #[test]
    fn cancellation_is_observed_between_bounded_pages() {
        let bytes = save(ascii_document(&["first page", "second page"]));
        let signal = CancelAfterChecks {
            checks: Cell::new(0),
            cancel_at: 8,
        };
        let result =
            PdfTextExtractor.extract(&mut Cursor::new(&bytes), request(&bytes), limits(), &signal);

        assert_eq!(
            result.expect_err("cancellation must discard partial PDF output"),
            ExtractionError::Cancelled
        );
    }
}
