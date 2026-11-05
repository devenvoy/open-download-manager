use std::path::Path;

use base64::Engine;

use crate::domain::category::DownloadCategory;

/// File formats recognized by extension with bundled vector icons.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FileType {
    Rust,
    Dart,
    Python,
    TypeScript,
    JavaScript,
    Swift,
    Kotlin,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Php,
    Ruby,
    Shell,
    PowerShell,
    Html,
    Css,
    Sass,
    Sql,
    R,
    Lua,
    Zig,
    Vue,
    Svelte,
    Wasm,
    Json,
    Yaml,
    Toml,
    Xml,
    Markdown,
    Text,
    Pdf,
    Word,
    PowerPoint,
    Spreadsheet,
    Csv,
    Svg,
    Image,
    Audio,
    Video,
    Font,
    Epub,
    Zip,
    Tar,
    Dmg,
    Pkg,
    Exe,
    Msi,
    AppImage,
    Deb,
    Rpm,
    Apk,
    Aab,
    Ipa,
    Jar,
    Dll,
    Binary,
    Certificate,
    Lock,
    Diff,
    Gitignore,
}

impl FileType {
    pub const ALL: &'static [Self] = &[
        Self::Rust,
        Self::Dart,
        Self::Python,
        Self::TypeScript,
        Self::JavaScript,
        Self::Swift,
        Self::Kotlin,
        Self::Go,
        Self::Java,
        Self::C,
        Self::Cpp,
        Self::CSharp,
        Self::Php,
        Self::Ruby,
        Self::Shell,
        Self::PowerShell,
        Self::Html,
        Self::Css,
        Self::Sass,
        Self::Sql,
        Self::R,
        Self::Lua,
        Self::Zig,
        Self::Vue,
        Self::Svelte,
        Self::Wasm,
        Self::Json,
        Self::Yaml,
        Self::Toml,
        Self::Xml,
        Self::Markdown,
        Self::Text,
        Self::Pdf,
        Self::Word,
        Self::PowerPoint,
        Self::Spreadsheet,
        Self::Csv,
        Self::Svg,
        Self::Image,
        Self::Audio,
        Self::Video,
        Self::Font,
        Self::Epub,
        Self::Zip,
        Self::Tar,
        Self::Dmg,
        Self::Pkg,
        Self::Exe,
        Self::Msi,
        Self::AppImage,
        Self::Deb,
        Self::Rpm,
        Self::Apk,
        Self::Aab,
        Self::Ipa,
        Self::Jar,
        Self::Dll,
        Self::Binary,
        Self::Certificate,
        Self::Lock,
        Self::Diff,
        Self::Gitignore,
    ];

    /// Canonical extension without a leading dot.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Dart => "dart",
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::JavaScript => "js",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "cs",
            Self::Php => "php",
            Self::Ruby => "rb",
            Self::Shell => "sh",
            Self::PowerShell => "ps1",
            Self::Html => "html",
            Self::Css => "css",
            Self::Sass => "scss",
            Self::Sql => "sql",
            Self::R => "r",
            Self::Lua => "lua",
            Self::Zig => "zig",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Wasm => "wasm",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Xml => "xml",
            Self::Markdown => "md",
            Self::Text => "txt",
            Self::Pdf => "pdf",
            Self::Word => "docx",
            Self::PowerPoint => "pptx",
            Self::Spreadsheet => "xlsx",
            Self::Csv => "csv",
            Self::Svg => "svg",
            Self::Image => "png",
            Self::Audio => "mp3",
            Self::Video => "mp4",
            Self::Font => "woff2",
            Self::Epub => "epub",
            Self::Zip => "zip",
            Self::Tar => "tar",
            Self::Dmg => "dmg",
            Self::Pkg => "pkg",
            Self::Exe => "exe",
            Self::Msi => "msi",
            Self::AppImage => "appimage",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Apk => "apk",
            Self::Aab => "aab",
            Self::Ipa => "ipa",
            Self::Jar => "jar",
            Self::Dll => "dll",
            Self::Binary => "bin",
            Self::Certificate => "pem",
            Self::Lock => "lock",
            Self::Diff => "diff",
            Self::Gitignore => "gitignore",
        }
    }

    /// Map a file format to its corresponding application download category.
    pub const fn category(self) -> DownloadCategory {
        match self {
            Self::Video => DownloadCategory::Video,
            Self::Audio => DownloadCategory::Audio,
            Self::Zip | Self::Tar => DownloadCategory::Archives,
            Self::Pdf
            | Self::Word
            | Self::PowerPoint
            | Self::Spreadsheet
            | Self::Csv
            | Self::Text
            | Self::Markdown
            | Self::Epub
            | Self::Json
            | Self::Yaml
            | Self::Toml
            | Self::Xml => DownloadCategory::Documents,
            Self::Exe
            | Self::Msi
            | Self::AppImage
            | Self::Deb
            | Self::Rpm
            | Self::Apk
            | Self::Aab
            | Self::Ipa
            | Self::Pkg
            | Self::Dmg
            | Self::Jar => DownloadCategory::Programs,
            _ => DownloadCategory::Other,
        }
    }

    /// Recognize a bare extension, including compound extensions.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => Some(Self::Rust),
            "dart" => Some(Self::Dart),
            "py" | "pyw" => Some(Self::Python),
            "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "swift" => Some(Self::Swift),
            "kt" | "kts" => Some(Self::Kotlin),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "php" => Some(Self::Php),
            "rb" => Some(Self::Ruby),
            "sh" | "bash" | "zsh" | "fish" => Some(Self::Shell),
            "ps1" | "psm1" => Some(Self::PowerShell),
            "html" | "htm" => Some(Self::Html),
            "css" => Some(Self::Css),
            "scss" | "sass" => Some(Self::Sass),
            "sql" => Some(Self::Sql),
            "r" => Some(Self::R),
            "lua" => Some(Self::Lua),
            "zig" => Some(Self::Zig),
            "vue" => Some(Self::Vue),
            "svelte" => Some(Self::Svelte),
            "wasm" => Some(Self::Wasm),
            "json" | "jsonc" | "jsonl" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "xml" | "xsd" | "xsl" => Some(Self::Xml),
            "md" | "markdown" | "mdx" => Some(Self::Markdown),
            "txt" | "text" | "log" => Some(Self::Text),
            "pdf" => Some(Self::Pdf),
            "docx" | "doc" => Some(Self::Word),
            "pptx" | "ppt" => Some(Self::PowerPoint),
            "xlsx" | "xls" | "ods" => Some(Self::Spreadsheet),
            "csv" | "tsv" => Some(Self::Csv),
            "svg" => Some(Self::Svg),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "ico" | "avif" => {
                Some(Self::Image)
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" => Some(Self::Audio),
            "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "flv" | "wmv" | "ts" => {
                Some(Self::Video)
            }
            "woff2" | "woff" | "ttf" | "otf" | "eot" => Some(Self::Font),
            "epub" => Some(Self::Epub),
            "zip" | "7z" | "rar" | "iso" => Some(Self::Zip),
            "tar" | "tar.gz" | "tgz" | "tar.xz" | "tar.bz2" | "gz" | "xz" | "bz2" => {
                Some(Self::Tar)
            }
            "dmg" => Some(Self::Dmg),
            "pkg" => Some(Self::Pkg),
            "exe" => Some(Self::Exe),
            "msi" | "msix" => Some(Self::Msi),
            "appimage" => Some(Self::AppImage),
            "deb" => Some(Self::Deb),
            "rpm" => Some(Self::Rpm),
            "apk" => Some(Self::Apk),
            "aab" => Some(Self::Aab),
            "ipa" => Some(Self::Ipa),
            "jar" => Some(Self::Jar),
            "dll" | "so" | "dylib" => Some(Self::Dll),
            "bin" | "dat" => Some(Self::Binary),
            "pem" | "crt" | "cer" | "p12" | "pfx" => Some(Self::Certificate),
            "lock" => Some(Self::Lock),
            "diff" | "patch" => Some(Self::Diff),
            "gitignore" | "gitattributes" | "gitmodules" => Some(Self::Gitignore),
            _ => None,
        }
    }

    /// Recognize a filename or path, preferring compound extensions.
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        for (index, character) in name.char_indices() {
            if character == '.' {
                if let Some(kind) = Self::from_extension(&name[index + 1..]) {
                    return Some(kind);
                }
            }
        }
        let extension = path.extension()?.to_str()?;
        Self::from_extension(extension)
    }

    /// Original SVG bytes, embedded for offline rendering.
    pub const fn svg(self) -> &'static [u8] {
        match self {
            Self::Rust => include_bytes!("../../assets/catalog-icons/rust.svg"),
            Self::Dart => include_bytes!("../../assets/catalog-icons/dart.svg"),
            Self::Python => include_bytes!("../../assets/catalog-icons/python.svg"),
            Self::TypeScript => include_bytes!("../../assets/catalog-icons/typescript.svg"),
            Self::JavaScript => include_bytes!("../../assets/catalog-icons/javascript.svg"),
            Self::Swift => include_bytes!("../../assets/catalog-icons/swift.svg"),
            Self::Kotlin => include_bytes!("../../assets/catalog-icons/kotlin.svg"),
            Self::Go => include_bytes!("../../assets/catalog-icons/go.svg"),
            Self::Java => include_bytes!("../../assets/catalog-icons/java.svg"),
            Self::C => include_bytes!("../../assets/catalog-icons/c.svg"),
            Self::Cpp => include_bytes!("../../assets/catalog-icons/cpp.svg"),
            Self::CSharp => include_bytes!("../../assets/catalog-icons/csharp.svg"),
            Self::Php => include_bytes!("../../assets/catalog-icons/php.svg"),
            Self::Ruby => include_bytes!("../../assets/catalog-icons/ruby.svg"),
            Self::Shell => include_bytes!("../../assets/catalog-icons/bashly.svg"),
            Self::PowerShell => include_bytes!("../../assets/catalog-icons/powershell.svg"),
            Self::Html => include_bytes!("../../assets/catalog-icons/html.svg"),
            Self::Css => include_bytes!("../../assets/catalog-icons/css.svg"),
            Self::Sass => include_bytes!("../../assets/catalog-icons/sass.svg"),
            Self::Sql => include_bytes!("../../assets/catalog-icons/database.svg"),
            Self::R => include_bytes!("../../assets/catalog-icons/r.svg"),
            Self::Lua => include_bytes!("../../assets/catalog-icons/lua.svg"),
            Self::Zig => include_bytes!("../../assets/catalog-icons/zig.svg"),
            Self::Vue => include_bytes!("../../assets/catalog-icons/vue.svg"),
            Self::Svelte => include_bytes!("../../assets/catalog-icons/svelte.svg"),
            Self::Wasm => include_bytes!("../../assets/catalog-icons/webassembly.svg"),
            Self::Json => include_bytes!("../../assets/catalog-icons/json.svg"),
            Self::Yaml => include_bytes!("../../assets/catalog-icons/yaml.svg"),
            Self::Toml => include_bytes!("../../assets/catalog-icons/toml.svg"),
            Self::Xml => include_bytes!("../../assets/catalog-icons/xml.svg"),
            Self::Markdown => include_bytes!("../../assets/catalog-icons/markdown.svg"),
            Self::Text => include_bytes!("../../assets/catalog-icons/document.svg"),
            Self::Pdf => include_bytes!("../../assets/catalog-icons/pdf.svg"),
            Self::Word => include_bytes!("../../assets/catalog-icons/word.svg"),
            Self::PowerPoint => include_bytes!("../../assets/catalog-icons/powerpoint.svg"),
            Self::Spreadsheet => include_bytes!("../../assets/catalog-icons/table.svg"),
            Self::Csv => include_bytes!("../../assets/catalog-icons/table.svg"),
            Self::Svg => include_bytes!("../../assets/catalog-icons/svg.svg"),
            Self::Image => include_bytes!("../../assets/catalog-icons/image.svg"),
            Self::Audio => include_bytes!("../../assets/catalog-icons/audio.svg"),
            Self::Video => include_bytes!("../../assets/catalog-icons/video.svg"),
            Self::Font => include_bytes!("../../assets/catalog-icons/font.svg"),
            Self::Epub => include_bytes!("../../assets/catalog-icons/epub.svg"),
            Self::Zip => include_bytes!("../../assets/catalog-icons/zip.svg"),
            Self::Tar => include_bytes!("../../assets/catalog-icons/zip.svg"),
            Self::Dmg => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Pkg => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Exe => include_bytes!("../../assets/catalog-icons/exe.svg"),
            Self::Msi => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::AppImage => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Deb => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Rpm => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Apk => include_bytes!("../../assets/catalog-icons/android.svg"),
            Self::Aab => include_bytes!("../../assets/catalog-icons/android.svg"),
            Self::Ipa => include_bytes!("../../assets/catalog-icons/installation.svg"),
            Self::Jar => include_bytes!("../../assets/catalog-icons/jar.svg"),
            Self::Dll => include_bytes!("../../assets/catalog-icons/dll.svg"),
            Self::Binary => include_bytes!("../../assets/catalog-icons/document.svg"),
            Self::Certificate => include_bytes!("../../assets/catalog-icons/certificate.svg"),
            Self::Lock => include_bytes!("../../assets/catalog-icons/lock.svg"),
            Self::Diff => include_bytes!("../../assets/catalog-icons/diff.svg"),
            Self::Gitignore => include_bytes!("../../assets/catalog-icons/git.svg"),
        }
    }

    /// Pre-formatted SVG Data URL for direct HTML rendering.
    pub fn svg_data_url(self) -> String {
        let bytes = self.svg();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(bytes);
        format!("data:image/svg+xml;base64,{base64_str}")
    }

    /// Generic document fallback icon.
    pub fn default_svg_data_url() -> String {
        let bytes = include_bytes!("../../assets/catalog-icons/document.svg");
        let base64_str = base64::engine::general_purpose::STANDARD.encode(bytes);
        format!("data:image/svg+xml;base64,{base64_str}")
    }
}
