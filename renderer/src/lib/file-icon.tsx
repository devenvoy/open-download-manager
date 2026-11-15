import { useMemo, useState } from "react";

const EXTENSION_MAP: Record<string, string> = {
  // Video
  mp4: "video",
  mkv: "video",
  webm: "video",
  avi: "video",
  mov: "video",
  m4v: "video",
  flv: "video",
  wmv: "video",
  mpg: "video",
  mpeg: "video",
  "3gp": "video",

  // Audio
  mp3: "audio",
  wav: "audio",
  flac: "audio",
  aac: "audio",
  ogg: "audio",
  m4a: "audio",
  opus: "audio",
  wma: "audio",

  // Archives
  zip: "zip",
  rar: "zip",
  "7z": "zip",
  tar: "zip",
  gz: "zip",
  xz: "zip",
  bz2: "zip",
  iso: "zip",

  // Documents
  pdf: "pdf",
  doc: "word",
  docx: "word",
  xls: "table",
  xlsx: "table",
  csv: "table",
  tsv: "table",
  ppt: "powerpoint",
  pptx: "powerpoint",
  txt: "document",
  text: "document",
  log: "document",
  md: "markdown",
  markdown: "markdown",
  epub: "epub",

  // Images
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  webp: "image",
  bmp: "image",
  svg: "svg",
  ico: "image",
  avif: "image",

  // Executables / Installers
  exe: "exe",
  msi: "installation",
  dmg: "installation",
  pkg: "installation",
  appimage: "installation",
  deb: "installation",
  rpm: "installation",
  apk: "android",
  aab: "android",
  ipa: "installation",
  jar: "jar",
  dll: "dll",

  // Code & Data
  rs: "rust",
  py: "python",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  ts: "typescript",
  tsx: "typescript",
  go: "go",
  java: "java",
  c: "c",
  h: "c",
  cpp: "cpp",
  hpp: "cpp",
  cs: "csharp",
  php: "php",
  rb: "ruby",
  sh: "bashly",
  bash: "bashly",
  zsh: "bashly",
  html: "html",
  htm: "html",
  css: "css",
  scss: "sass",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  xml: "xml",
  sql: "database",
};

export function extractExtension(filename: string | null | undefined): string {
  if (!filename) return "";
  const match = String(filename).match(/\.([a-z0-9]{1,12})$/i);
  return match && match[1] ? match[1].toLowerCase() : "";
}

export function getFileIconUrl(
  filename: string | null | undefined,
  fileIcons?: Record<string, string>,
): string {
  const ext = extractExtension(filename);
  if (!ext) return "/catalog-icons/document.svg";

  // Check if native snapshot has it (with leading dot)
  if (fileIcons) {
    const dotExt = `.${ext}`;
    if (fileIcons[dotExt]) return fileIcons[dotExt];
    if (fileIcons[ext]) return fileIcons[ext];
  }

  const iconName = EXTENSION_MAP[ext] || "document";
  return `/catalog-icons/${iconName}.svg`;
}

export interface FileTypeIconProps {
  filename?: string | null;
  iconUrl?: string;
  className?: string;
  size?: number;
}

export function FileTypeIcon({
  filename,
  iconUrl,
  className = "file-type-icon native-file-icon",
  size = 16,
}: FileTypeIconProps) {
  const [error, setError] = useState(false);
  const src = useMemo(() => {
    if (error) return "/catalog-icons/document.svg";
    return iconUrl || getFileIconUrl(filename);
  }, [error, iconUrl, filename]);

  return (
    <img
      src={src}
      className={className}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      onError={() => {
        if (!error) setError(true);
      }}
    />
  );
}
