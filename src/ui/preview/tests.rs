// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    MEDIA_PLUGIN_INSTALL_COMMAND, PDF_MAX_ZOOM, PDF_MIN_ZOOM, column_decl_for,
    database_count_badge, declares_blob, declares_numeric_affinity, format_database_cell,
    format_file_size, format_media_time, is_numeric_cell, media_error_feedback,
    pdf_zoom_after_scroll, preview_width_for_empty_space,
};

#[test]
fn formats_preview_file_sizes() {
    assert_eq!(format_file_size(999), "999 B");
    assert_eq!(format_file_size(1_200), "1.2 kB");
    assert_eq!(format_file_size(2_500_000), "2.5 MB");
}

#[test]
fn media_errors_explain_missing_runtime_plugins() {
    let (title, detail, command) =
        media_error_feedback("Your GStreamer installation is missing a plug-in.");
    assert_eq!(title, "Additional media support required");
    assert!(detail.contains("GStreamer plugins"));
    assert_eq!(command, Some(MEDIA_PLUGIN_INSTALL_COMMAND));
    assert_eq!(
        command,
        Some("sudo pacman -S --needed gst-plugins-good gst-libav")
    );

    let (title, detail, command) = media_error_feedback("The media data is corrupt");
    assert_eq!(title, "Preview unavailable");
    assert!(detail.contains("The media data is corrupt"));
    assert_eq!(command, None);
}

#[test]
fn initial_preview_uses_most_of_the_unoccupied_width() {
    assert_eq!(preview_width_for_empty_space(2_000, 500), 1_350);
    assert_eq!(preview_width_for_empty_space(700, 650), 280);
}

#[test]
fn pdf_scroll_zoom_stays_within_its_supported_range() {
    assert!(pdf_zoom_after_scroll(1.0, -1.0) > 1.0);
    assert!(pdf_zoom_after_scroll(2.0, 1.0) < 2.0);
    assert_eq!(pdf_zoom_after_scroll(PDF_MIN_ZOOM, 100.0), PDF_MIN_ZOOM);
    assert_eq!(pdf_zoom_after_scroll(PDF_MAX_ZOOM, -100.0), PDF_MAX_ZOOM);
}

#[test]
fn media_time_formats_minutes_and_seconds() {
    assert_eq!(format_media_time(0, 0), "0:00/0:00");
    assert_eq!(format_media_time(1_500_000, 65_000_000), "0:01/1:05");
    assert_eq!(format_media_time(125_000_000, 125_000_000), "2:05/2:05");
}

#[test]
fn media_time_clamps_negative_timestamps_to_zero() {
    assert_eq!(format_media_time(-500_000, 10_000_000), "0:00/0:10");
}

#[test]
fn parse_csv_rows_handles_basic_table() {
    let csv = "id,name,role\n1,Alice,admin\n2,Bob,member";
    let (headers, rows) = super::parse_csv_rows(csv);
    assert_eq!(headers, vec!["id", "name", "role"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["1", "Alice", "admin"]);
    assert_eq!(rows[1], vec!["2", "Bob", "member"]);
}

#[test]
fn parse_csv_rows_handles_quotes_and_commas() {
    let csv = "id,description\n1,\"Item, with comma\"\n2,\"Item with \"\"quotes\"\"\"";
    let (headers, rows) = super::parse_csv_rows(csv);
    assert_eq!(headers, vec!["id", "description"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["1", "Item, with comma"]);
    assert_eq!(rows[1], vec!["2", "Item with \"quotes\""]);
}

#[test]
fn parse_csv_rows_handles_multiline_cells() {
    let csv = "id,notes\n1,\"Line 1\nLine 2\"\n2,Single line";
    let (headers, rows) = super::parse_csv_rows(csv);
    assert_eq!(headers, vec!["id", "notes"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["1", "Line 1\nLine 2"]);
    assert_eq!(rows[1], vec!["2", "Single line"]);
}

#[test]
fn parse_csv_rows_handles_empty_input() {
    let (headers, rows) = super::parse_csv_rows("");
    assert!(headers.is_empty());
    assert!(rows.is_empty());
}

#[test]
fn numeric_cells_cover_plain_and_scientific_notation() {
    for cell in ["42", "-3.5", "+0.25", "1e6", "007"] {
        assert!(is_numeric_cell(cell), "{cell} should align numerically");
    }
    for cell in ["", "12px", "1,000", "2024-01-02", "NULL", "  "] {
        assert!(!is_numeric_cell(cell), "{cell} should align as text");
    }
}

#[test]
fn database_cells_flatten_newlines_and_truncate() {
    assert_eq!(format_database_cell("Line 1\nLine 2"), "Line 1 ⏎ Line 2");
    assert_eq!(format_database_cell("a\rb\nc"), "ab ⏎ c");
    let long = "x".repeat(250);
    let display = format_database_cell(&long);
    assert_eq!(display.chars().count(), 201);
    assert!(display.ends_with('…'));
    assert_eq!(format_database_cell("short"), "short");
}

#[test]
fn database_count_badge_combines_rows_and_columns() {
    assert_eq!(database_count_badge(Some(1), 3), "1 row · 3 cols");
    assert_eq!(database_count_badge(Some(42), 1), "42 rows · 1 col");
    assert_eq!(database_count_badge(Some(0), 2), "0 rows · 2 cols");
    assert_eq!(database_count_badge(None, 4), "4 cols");
    assert_eq!(database_count_badge(Some(7), 0), "7 rows");
    assert_eq!(database_count_badge(None, 0), "");
}

#[test]
fn declared_types_drive_numeric_and_blob_cells() {
    for decl in [
        "INTEGER",
        "INT",
        "BIGINT",
        "REAL",
        "DOUBLE PRECISION",
        "FLOAT",
        "DECIMAL(10,2)",
        "NUMERIC",
        "BOOLEAN",
    ] {
        assert!(
            declares_numeric_affinity(decl),
            "{decl} should align numerically"
        );
        assert!(!declares_blob(decl));
    }
    for decl in ["TEXT", "VARCHAR(10)", "CHAR(1)", "CLOB", "BLOB", "", "  "] {
        assert!(
            !declares_numeric_affinity(decl),
            "{decl} should not force alignment"
        );
    }
    assert!(declares_blob("BLOB"));
    assert!(!declares_blob("TEXT"));
    assert!(!declares_blob(""));
}

#[test]
fn column_decls_prefer_position_then_fall_back_to_name() {
    let columns = vec![
        crate::services::DatabaseColumn {
            name: "id".to_owned(),
            decl_type: "INTEGER".to_owned(),
        },
        crate::services::DatabaseColumn {
            name: "name".to_owned(),
            decl_type: "TEXT".to_owned(),
        },
    ];
    let headers = vec!["id".to_owned(), "name".to_owned()];
    assert_eq!(column_decl_for(&columns, &headers, 0), "INTEGER");
    assert_eq!(column_decl_for(&columns, &headers, 5), "");

    let reordered = vec!["name".to_owned(), "id".to_owned()];
    assert_eq!(column_decl_for(&columns, &reordered, 0), "TEXT");
    assert_eq!(column_decl_for(&[], &headers, 0), "");
}
