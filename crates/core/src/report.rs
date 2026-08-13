use crate::matcher::MatchReport;
use crate::scan::ScanErrorEntry;
use retrotools_common::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct ScanReport {
    pub platform: String,
    pub match_report: MatchReport,
    pub scan_errors: Vec<ScanErrorEntry>,
}

impl ScanReport {
    pub fn new(match_report: MatchReport, scan_errors: Vec<ScanErrorEntry>) -> Self {
        Self {
            platform: match_report.platform.clone(),
            match_report,
            scan_errors,
        }
    }

    fn csv_field(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    /// Exports the report as CSV: one row per matched/corrupt/unknown scanned
    /// file, plus one row per missing DAT entry.
    pub fn to_csv(&self) -> String {
        let mut out = String::from("status,file,archive_entry,game,rom,crc32,size\n");

        let mut push_match = |status: &str, m: &crate::matcher::RomMatch| {
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                status,
                Self::csv_field(&m.scanned.source_path.to_string_lossy()),
                Self::csv_field(m.scanned.archive_entry.as_deref().unwrap_or("")),
                Self::csv_field(m.matched_game.as_deref().unwrap_or("")),
                Self::csv_field(m.matched_rom.as_deref().unwrap_or("")),
                Self::csv_field(&m.scanned.hashes.crc32),
                m.scanned.hashes.size,
            ));
        };

        for m in &self.match_report.matched {
            push_match("matched", m);
        }
        for m in &self.match_report.corrupt {
            push_match("corrupt", m);
        }
        for m in &self.match_report.unknown {
            push_match("unknown", m);
        }
        for missing in &self.match_report.missing {
            out.push_str(&format!(
                "missing,,,{},{},{},{}\n",
                Self::csv_field(&missing.game_name),
                Self::csv_field(&missing.rom_name),
                Self::csv_field(missing.expected_crc32.as_deref().unwrap_or("")),
                missing.expected_size,
            ));
        }
        for err in &self.scan_errors {
            out.push_str(&format!(
                "error,{},{},,,,{}\n",
                Self::csv_field(&err.source_path.to_string_lossy()),
                Self::csv_field(err.archive_entry.as_deref().unwrap_or("")),
                Self::csv_field(&err.message),
            ));
        }

        out
    }

    /// Exports a self-contained, minimally styled HTML report suitable for
    /// sharing or archiving.
    pub fn to_html(&self) -> String {
        fn escape(value: &str) -> String {
            value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        }

        let mut rows = String::new();
        let mut section = |title: &str, items: &[crate::matcher::RomMatch]| {
            for m in items {
                rows.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                    title,
                    escape(&m.scanned.source_path.to_string_lossy()),
                    escape(m.matched_game.as_deref().unwrap_or("-")),
                    escape(m.matched_rom.as_deref().unwrap_or("-")),
                    escape(&m.scanned.hashes.crc32),
                ));
            }
        };
        section("Matched", &self.match_report.matched);
        section("Corrupt", &self.match_report.corrupt);
        section("Unknown", &self.match_report.unknown);

        for missing in &self.match_report.missing {
            rows.push_str(&format!(
                "<tr><td>Missing</td><td>-</td><td>{}</td><td>{}</td><td>-</td></tr>\n",
                escape(&missing.game_name),
                escape(&missing.rom_name),
            ));
        }

        format!(
            r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Scan report - {platform}</title>
<style>
body {{ font-family: sans-serif; margin: 2rem; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ccc; padding: 4px 8px; text-align: left; font-size: 0.9rem; }}
th {{ background: #eee; }}
h1 {{ font-size: 1.4rem; }}
.summary {{ margin-bottom: 1rem; }}
</style></head>
<body>
<h1>Scan report — {platform}</h1>
<p class="summary">Matched: {matched} · Corrupt: {corrupt} · Unknown: {unknown} · Missing: {missing} · Completion: {completion:.1}%</p>
<table>
<thead><tr><th>Status</th><th>File</th><th>Game</th><th>ROM</th><th>CRC32</th></tr></thead>
<tbody>
{rows}
</tbody>
</table>
</body></html>
"#,
            platform = escape(&self.platform),
            matched = self.match_report.matched.len(),
            corrupt = self.match_report.corrupt.len(),
            unknown = self.match_report.unknown.len(),
            missing = self.match_report.missing.len(),
            completion = self.match_report.completion_percent(),
            rows = rows,
        )
    }

    /// Renders the same content as [`Self::to_html`] into a PDF, reusing the
    /// HTML-to-PDF layout engine bundled with `printpdf` rather than
    /// hand-laying-out a second, parallel representation of the report.
    pub fn to_pdf(&self) -> AppResult<Vec<u8>> {
        let html = self.to_html();
        let images = std::collections::BTreeMap::new();
        let fonts = std::collections::BTreeMap::new();
        let options = printpdf::GeneratePdfOptions::default();
        let mut warnings = Vec::new();

        let doc = printpdf::PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
            .map_err(|e| AppError::Scan(format!("cannot render PDF report: {e}")))?;

        let save_options = printpdf::PdfSaveOptions::default();
        let mut save_warnings = Vec::new();
        Ok(doc.save(&save_options, &mut save_warnings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{MatchReport, MissingRom};

    #[test]
    fn csv_includes_missing_rows() {
        let mut match_report = MatchReport {
            platform: "Test".into(),
            ..Default::default()
        };
        match_report.missing.push(MissingRom {
            game_name: "Game A".into(),
            rom_name: "game-a.bin".into(),
            expected_crc32: Some("deadbeef".into()),
            expected_size: 4,
        });
        let report = ScanReport::new(match_report, Vec::new());
        let csv = report.to_csv();
        assert!(csv.contains("missing,,,Game A,game-a.bin,deadbeef,4"));
    }

    #[test]
    fn html_contains_summary_counts() {
        let match_report = MatchReport {
            platform: "Test".into(),
            ..Default::default()
        };
        let report = ScanReport::new(match_report, Vec::new());
        let html = report.to_html();
        assert!(html.contains("Scan report"));
        assert!(html.contains("Completion"));
    }

    #[test]
    fn pdf_starts_with_a_valid_header_and_has_content() {
        let mut match_report = MatchReport {
            platform: "Test Platform".into(),
            ..Default::default()
        };
        match_report.missing.push(MissingRom {
            game_name: "Game A".into(),
            rom_name: "game-a.bin".into(),
            expected_crc32: Some("deadbeef".into()),
            expected_size: 4,
        });
        let report = ScanReport::new(match_report, Vec::new());

        let pdf = report.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        // A real rendered document, not just a near-empty stub.
        assert!(pdf.len() > 500);
    }
}
