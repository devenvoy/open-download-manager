(function(root) {
  const VIDEO_MIME_TYPES = new Set([
    'video/mp4',
    'video/webm',
    'video/x-msvideo',
    'video/avi',
    'video/msvideo',
    'video/mpeg',
    'video/mpg',
    'video/x-mpeg',
    'video/mp2t',
    'video/quicktime',
    'video/x-quicktime',
    'video/x-ms-wmv',
    'video/x-ms-asf',
    'video/x-ms-wvx',
    'video/asf',
    'video/wmv',
    'video/x-flv',
    'video/flv',
    'video/x-f4v',
    'video/f4v',
    'video/x-m4v',
    'video/m4v',
    'video/divx',
    'video/x-divx',
    'video/3gpp',
    'video/3gpp2',
    'video/ogg',
    'video/x-matroska',
    'video/mkv'
  ]);

  const AUDIO_MIME_TYPES = new Set([
    'audio/mp4',
    'audio/mp3',
    'audio/mpeg',
    'audio/webm',
    'audio/aac',
    'audio/ogg',
    'audio/opus',
    'audio/wav',
    'audio/x-wav',
    'audio/flac',
    'audio/x-flac',
    'audio/x-m4a',
    'audio/m4a'
  ]);

  const HLS_MIME_TYPES = new Set([
    'application/vnd.apple.mpegurl',
    'application/x-mpegurl',
    'audio/mpegurl',
    'audio/x-mpegurl'
  ]);

  const DASH_MIME_TYPES = new Set([
    'application/dash+xml'
  ]);

  const VIDEO_EXTENSIONS = new Set([
    'mp4', 'webm', 'avi', 'mpg', 'mpeg', 'mov', 'wmv', 'asf', 'wvx',
    'flv', 'f4v', 'm4v', 'divx', 'mkv', '3gp', '3g2', 'ogv', 'ts'
  ]);

  const AUDIO_EXTENSIONS = new Set([
    'mp3', 'm4a', 'aac', 'ogg', 'opus', 'wav', 'flac', 'weba'
  ]);

  const SENSITIVE_PARAMS = new Set([
    'sig', 'signature', 'token', 'access_token', 'auth_token', 'auth',
    'key', 'key_id', 'key-pair-id', 'hmac', 'expire', 'expires', 'exp',
    'session', 'session_id', 'sessionid', 'secret', 'password', 'pass', 'sparams'
  ]);

  function cleanMime(mime) {
    return String(mime || '').split(';')[0].trim().toLowerCase();
  }

  function isVideoMime(mime) {
    const m = cleanMime(mime);
    return VIDEO_MIME_TYPES.has(m) || m.startsWith('video/');
  }

  function isAudioMime(mime) {
    const m = cleanMime(mime);
    return AUDIO_MIME_TYPES.has(m) || m.startsWith('audio/');
  }

  function isHlsMime(mime) {
    const m = cleanMime(mime);
    return HLS_MIME_TYPES.has(m) || /mpegurl/i.test(m);
  }

  function isDashMime(mime) {
    const m = cleanMime(mime);
    return DASH_MIME_TYPES.has(m) || /dash\+xml/i.test(m);
  }

  function isMediaMime(mime) {
    return isVideoMime(mime) || isAudioMime(mime) || isHlsMime(mime) || isDashMime(mime);
  }

  function isMediaExtension(ext) {
    const clean = String(ext || '').trim().replace(/^\./, '').toLowerCase();
    return VIDEO_EXTENSIONS.has(clean) || AUDIO_EXTENSIONS.has(clean) || clean === 'm3u8' || clean === 'mpd';
  }

  function isYouTubeHost(hostname) {
    const host = String(hostname || '').toLowerCase();
    return host === 'youtube.com' || host.endsWith('.youtube.com') || host === 'youtu.be' || host.endsWith('.youtu.be');
  }

  function isGoogleVideoHost(hostname) {
    const host = String(hostname || '').toLowerCase();
    return host === 'googlevideo.com' || host.endsWith('.googlevideo.com');
  }

  function isPlayerInfoRoute(pathname) {
    const path = String(pathname || '').toLowerCase();
    return path === '/videoplayback' || path.includes('/player') || path.includes('get_video_info');
  }

  function redactSignedUrl(rawUrl) {
    try {
      const u = new URL(rawUrl);
      if (u.password) {
        u.password = '******';
      }
      for (const [key] of Array.from(u.searchParams.entries())) {
        if (SENSITIVE_PARAMS.has(key.toLowerCase())) {
          u.searchParams.set(key, '******');
        }
      }
      return u.href;
    } catch {
      return String(rawUrl || '');
    }
  }

  function createMediaCandidate(opts = {}) {
    const mediaUrl = String(opts.mediaUrl || opts.url || '');
    const mime = cleanMime(opts.mimeType || opts.mime);
    const codecs = String(opts.codecs || '');
    const width = Number(opts.width) || undefined;
    const height = Number(opts.height) || undefined;
    const bitrate = Number(opts.bitrate) || 0;

    let role = opts.role;
    if (!role) {
      if (isHlsMime(mime) || /\.m3u8(?:[?#]|$)/i.test(mediaUrl)) {
        role = opts.audioUrl ? 'video-audio' : 'adaptive-master';
      } else if (opts.audioUrl) {
        role = 'video-audio';
      } else if (isAudioMime(mime)) {
        role = 'audio-only';
      } else if (/mp4a|ac-3|ec-3|opus/i.test(codecs)) {
        role = 'video-audio';
      } else if (codecs) {
        role = 'video-only';
      } else {
        role = isVideoMime(mime) ? 'video-audio' : 'unknown';
      }
    }

    const unsupported = !!opts.unsupported || /\.mpd(?:[?#]|$)/i.test(mediaUrl) || isDashMime(mime);
    const reason = opts.reason || (unsupported ? 'DASH streams requiring dynamic manifests or DRM are not supported yet.' : '');

    return {
      pageUrl: opts.pageUrl ? redactSignedUrl(opts.pageUrl) : undefined,
      mediaUrl,
      sanitizedUrl: redactSignedUrl(mediaUrl),
      mimeType: mime || undefined,
      codecs: codecs || undefined,
      width,
      height,
      bitrate,
      role,
      unsupported,
      reason,
      isSelectable: !unsupported
    };
  }

  const mediaPolicy = {
    VIDEO_MIME_TYPES,
    AUDIO_MIME_TYPES,
    HLS_MIME_TYPES,
    DASH_MIME_TYPES,
    VIDEO_EXTENSIONS,
    AUDIO_EXTENSIONS,
    isVideoMime,
    isAudioMime,
    isHlsMime,
    isDashMime,
    isMediaMime,
    isMediaExtension,
    isYouTubeHost,
    isGoogleVideoHost,
    isPlayerInfoRoute,
    redactSignedUrl,
    createMediaCandidate
  };

  root.mediaPolicy = mediaPolicy;
  if (typeof module !== 'undefined') {
    module.exports = mediaPolicy;
  }
})(globalThis);
