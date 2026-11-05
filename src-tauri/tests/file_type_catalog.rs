use std::path::Path;

use awesome_downloader_lib::domain::{
    category::DownloadCategory,
    download::sanitize_filename,
    file_type::FileType,
};

#[test]
fn file_types_map_to_correct_download_categories() {
    assert_eq!(FileType::Video.category(), DownloadCategory::Video);
    assert_eq!(FileType::Audio.category(), DownloadCategory::Audio);
    assert_eq!(FileType::Zip.category(), DownloadCategory::Archives);
    assert_eq!(FileType::Tar.category(), DownloadCategory::Archives);
    assert_eq!(FileType::Pdf.category(), DownloadCategory::Documents);
    assert_eq!(FileType::Word.category(), DownloadCategory::Documents);
    assert_eq!(FileType::Spreadsheet.category(), DownloadCategory::Documents);
    assert_eq!(FileType::Exe.category(), DownloadCategory::Programs);
    assert_eq!(FileType::Apk.category(), DownloadCategory::Programs);
    assert_eq!(FileType::Dmg.category(), DownloadCategory::Programs);
    assert_eq!(FileType::Rust.category(), DownloadCategory::Other);
}

#[test]
fn recognizes_common_and_compound_file_extensions() {
    // Media
    assert_eq!(FileType::from_extension("mp4"), Some(FileType::Video));
    assert_eq!(FileType::from_extension("mkv"), Some(FileType::Video));
    assert_eq!(FileType::from_extension("ts"), Some(FileType::Video));
    assert_eq!(FileType::from_extension("mp3"), Some(FileType::Audio));
    assert_eq!(FileType::from_extension("flac"), Some(FileType::Audio));

    // Archives
    assert_eq!(FileType::from_extension("zip"), Some(FileType::Zip));
    assert_eq!(FileType::from_extension("tar.gz"), Some(FileType::Tar));
    assert_eq!(FileType::from_extension("tgz"), Some(FileType::Tar));
    assert_eq!(FileType::from_extension("tar.xz"), Some(FileType::Tar));

    // Documents
    assert_eq!(FileType::from_extension("pdf"), Some(FileType::Pdf));
    assert_eq!(FileType::from_extension("docx"), Some(FileType::Word));
    assert_eq!(FileType::from_extension("xlsx"), Some(FileType::Spreadsheet));
    assert_eq!(FileType::from_extension("md"), Some(FileType::Markdown));

    // Code
    assert_eq!(FileType::from_extension("rs"), Some(FileType::Rust));
    assert_eq!(FileType::from_extension("py"), Some(FileType::Python));
    assert_eq!(FileType::from_extension("tsx"), Some(FileType::TypeScript));
    assert_eq!(FileType::from_extension("js"), Some(FileType::JavaScript));
    assert_eq!(FileType::from_extension("go"), Some(FileType::Go));

    // Installers
    assert_eq!(FileType::from_extension("exe"), Some(FileType::Exe));
    assert_eq!(FileType::from_extension("dmg"), Some(FileType::Dmg));
    assert_eq!(FileType::from_extension("deb"), Some(FileType::Deb));
    assert_eq!(FileType::from_extension("apk"), Some(FileType::Apk));
}

#[test]
fn recognizes_file_type_from_path_with_compound_extensions() {
    assert_eq!(
        FileType::from_path(Path::new("/downloads/archive.tar.gz")),
        Some(FileType::Tar)
    );
    assert_eq!(
        FileType::from_path(Path::new("release-v1.0.tar.xz")),
        Some(FileType::Tar)
    );
    assert_eq!(
        FileType::from_path(Path::new("video-stream.ts")),
        Some(FileType::Video)
    );
    assert_eq!(
        FileType::from_path(Path::new("/var/media/track.flac")),
        Some(FileType::Audio)
    );
    assert_eq!(
        FileType::from_path(Path::new("document.pdf")),
        Some(FileType::Pdf)
    );
}

#[test]
fn category_auto_deduction_from_extension_and_mime() {
    assert_eq!(DownloadCategory::from_extension("mp4"), DownloadCategory::Video);
    assert_eq!(DownloadCategory::from_extension(".mkv"), DownloadCategory::Video);
    assert_eq!(DownloadCategory::from_extension("mp3"), DownloadCategory::Audio);
    assert_eq!(DownloadCategory::from_extension("zip"), DownloadCategory::Archives);
    assert_eq!(DownloadCategory::from_extension("pdf"), DownloadCategory::Documents);
    assert_eq!(DownloadCategory::from_extension("exe"), DownloadCategory::Programs);

    // MIME guess deduction for less common extensions
    assert_eq!(
        DownloadCategory::from_extension("weba"),
        DownloadCategory::Audio
    );
    assert_eq!(
        DownloadCategory::from_extension("m4v"),
        DownloadCategory::Video
    );
}

#[test]
fn svg_bytes_and_data_urls_are_valid_and_non_empty() {
    for file_type in FileType::ALL {
        let svg_bytes = file_type.svg();
        assert!(!svg_bytes.is_empty(), "SVG bytes for {:?} must not be empty", file_type);
        let svg_str = std::str::from_utf8(svg_bytes).expect("SVG must be valid UTF-8");
        assert!(svg_str.contains("<svg"), "SVG for {:?} must contain <svg tag", file_type);

        let data_url = file_type.svg_data_url();
        assert!(
            data_url.starts_with("data:image/svg+xml;base64,"),
            "Data URL for {:?} must be an SVG data URL",
            file_type
        );
    }

    let default_url = FileType::default_svg_data_url();
    assert!(default_url.starts_with("data:image/svg+xml;base64,"));
}

#[test]
fn sanitize_filename_handles_reserved_names_and_separators() {
    assert_eq!(sanitize_filename("con.txt"), "download_con.txt");
    assert_eq!(sanitize_filename("aux"), "download_aux");
    assert_eq!(sanitize_filename("nul.tar.gz"), "download_nul.tar.gz");
    assert_eq!(sanitize_filename("normal-file.zip"), "normal-file.zip");
    assert_eq!(sanitize_filename("../path/traversal.bin"), ".._path_traversal.bin");
}
