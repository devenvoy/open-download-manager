const test = require('node:test');
const assert = require('node:assert/strict');
const mediaPolicy = require('./media-policy.js');

test('mediaPolicy correctly identifies full video MIME catalog across arbitrary websites', () => {
  const sampleMimes = [
    'video/mp4', 'video/webm', 'video/x-msvideo', 'video/avi', 'video/msvideo',
    'video/mpeg', 'video/mpg', 'video/x-mpeg', 'video/mp2t', 'video/quicktime',
    'video/x-quicktime', 'video/x-ms-wmv', 'video/x-ms-asf', 'video/x-ms-wvx',
    'video/asf', 'video/wmv', 'video/x-flv', 'video/flv', 'video/x-f4v', 'video/f4v',
    'video/x-m4v', 'video/m4v', 'video/divx', 'video/x-divx', 'video/3gpp',
    'video/3gpp2', 'video/ogg', 'video/x-matroska', 'video/mkv'
  ];

  for (const m of sampleMimes) {
    assert.equal(mediaPolicy.isVideoMime(m), true, `Expected ${m} to be recognized as video`);
    assert.equal(mediaPolicy.isMediaMime(m), true, `Expected ${m} to be recognized as media`);
  }

  assert.equal(mediaPolicy.isVideoMime('application/pdf'), false);
  assert.equal(mediaPolicy.isVideoMime('text/html'), false);
});

test('mediaPolicy recognizes HLS and DASH streaming MIME types', () => {
  assert.equal(mediaPolicy.isHlsMime('application/vnd.apple.mpegurl'), true);
  assert.equal(mediaPolicy.isHlsMime('application/x-mpegurl'), true);
  assert.equal(mediaPolicy.isHlsMime('audio/mpegurl'), true);
  assert.equal(mediaPolicy.isHlsMime('video/mp4'), false);

  assert.equal(mediaPolicy.isDashMime('application/dash+xml'), true);
  assert.equal(mediaPolicy.isDashMime('video/mp4'), false);
});

test('mediaPolicy recognizes file extensions and player routes', () => {
  assert.equal(mediaPolicy.isMediaExtension('mp4'), true);
  assert.equal(mediaPolicy.isMediaExtension('.avi'), true);
  assert.equal(mediaPolicy.isMediaExtension('m3u8'), true);
  assert.equal(mediaPolicy.isMediaExtension('mpd'), true);
  assert.equal(mediaPolicy.isMediaExtension('pdf'), false);

  assert.equal(mediaPolicy.isYouTubeHost('www.youtube.com'), true);
  assert.equal(mediaPolicy.isYouTubeHost('youtu.be'), true);
  assert.equal(mediaPolicy.isYouTubeHost('vimeo.com'), false);

  assert.equal(mediaPolicy.isGoogleVideoHost('r1---sn-4g5ednss.googlevideo.com'), true);
  assert.equal(mediaPolicy.isPlayerInfoRoute('/videoplayback'), true);
  assert.equal(mediaPolicy.isPlayerInfoRoute('/youtubei/v1/player'), true);
});

test('mediaPolicy redacts signed query parameters and basic auth credentials', () => {
  const signedUrl = 'https://video.twimg.com/amplify_video/123/pl.m3u8?token=secret123&expire=1700000000&sig=abc999&tag=1';
  const redacted = mediaPolicy.redactSignedUrl(signedUrl);
  assert.match(redacted, /token=\*\*\*\*\*\*/);
  assert.match(redacted, /expire=\*\*\*\*\*\*/);
  assert.match(redacted, /sig=\*\*\*\*\*\*/);
  assert.match(redacted, /tag=1/);
  assert.doesNotMatch(redacted, /secret123/);

  const credUrl = 'https://user:password123@media.example.com/stream.mp4';
  const redactedCred = mediaPolicy.redactSignedUrl(credUrl);
  assert.match(redactedCred, /user:\*\*\*\*\*\*@media\.example\.com/);
  assert.doesNotMatch(redactedCred, /password123/);
});

test('createMediaCandidate formats roles and classifies unsupported streams', () => {
  const validCandidate = mediaPolicy.createMediaCandidate({
    mediaUrl: 'https://vimeo.com/stream.mp4?token=secret',
    mimeType: 'video/mp4',
    codecs: 'avc1.64001f, mp4a.40.2',
    height: 1080
  });

  assert.equal(validCandidate.role, 'video-audio');
  assert.equal(validCandidate.unsupported, false);
  assert.equal(validCandidate.isSelectable, true);
  assert.match(validCandidate.sanitizedUrl, /token=\*\*\*\*\*\*/);

  const dashCandidate = mediaPolicy.createMediaCandidate({
    mediaUrl: 'https://cdn.example.com/manifest.mpd',
    mimeType: 'application/dash+xml'
  });

  assert.equal(dashCandidate.unsupported, true);
  assert.equal(dashCandidate.isSelectable, false);
  assert.match(dashCandidate.reason, /DASH streams/i);
});
