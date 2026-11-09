// Created: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: Integration tests verifying media adaptive stream functionality, edge cases, and error recovery.

use open_download_manager_lib::domain::media::{
    parse_dash, parse_hls, redact_media_url, DashError, HlsByteRange, MediaCandidate,
    MediaCapturePolicy, MediaRole, MediaRoute, ParsedHls,
};

#[test]
fn fixture_matrix_covers_all_registered_video_mime_types() {
    let policy = MediaCapturePolicy::new();

    let supported_types = [
        ("video/mp4", "mp4"),
        ("video/webm", "webm"),
        ("video/x-msvideo", "avi"),
        ("video/avi", "avi"),
        ("video/msvideo", "avi"),
        ("video/mpeg", "mpeg"),
        ("video/mpg", "mpg"),
        ("video/x-mpeg", "mpg"),
        ("video/mp2t", "ts"),
        ("video/quicktime", "mov"),
        ("video/x-quicktime", "mov"),
        ("video/x-ms-wmv", "wmv"),
        ("video/x-ms-asf", "asf"),
        ("video/x-ms-wvx", "wvx"),
        ("video/asf", "asf"),
        ("video/wmv", "wmv"),
        ("video/x-flv", "flv"),
        ("video/flv", "flv"),
        ("video/x-f4v", "f4v"),
        ("video/f4v", "f4v"),
        ("video/x-m4v", "m4v"),
        ("video/m4v", "m4v"),
        ("video/divx", "divx"),
        ("video/x-divx", "divx"),
        ("video/3gpp", "3gp"),
        ("video/3gpp2", "3g2"),
        ("video/ogg", "ogv"),
        ("video/x-matroska", "mkv"),
        ("video/mkv", "mkv"),
    ];

    for (mime, ext) in supported_types {
        assert!(
            policy.is_video_mime(mime),
            "Expected {mime} to be recognized in video MIME catalog"
        );
        let url = format!("https://cdn.example.com/media/clip.{ext}");
        let classification = policy.classify(&url, Some(mime));
        match classification {
            MediaRoute::DirectMedia { .. } => {}
            other => panic!("Expected DirectMedia for {mime} ({ext}), got {other:?}"),
        }
    }

    // Unregistered MIME type should not be recognized as video
    assert!(!policy.is_video_mime("application/pdf"));
    assert!(!policy.is_video_mime("text/html"));
    assert_eq!(
        policy.classify("https://cdn.example.com/doc.pdf", Some("application/pdf")),
        MediaRoute::NonMedia
    );
}

#[test]
fn fixture_matrix_routes_youtube_and_googlevideo_paths() {
    let policy = MediaCapturePolicy::new();

    // Standard watch URL
    let watch_route = policy.classify("https://www.youtube.com/watch?v=dQw4w9WgXcQ", None);
    assert_eq!(
        watch_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "/watch".to_string(),
            is_sabr: false,
            video_id: Some("dQw4w9WgXcQ".to_string()),
        }
    );

    // Shorts URL
    let shorts_route = policy.classify("https://youtube.com/shorts/dQw4w9WgXcQ", None);
    assert_eq!(
        shorts_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "/shorts/dqw4w9wgxcq".to_string(),
            is_sabr: false,
            video_id: Some("dQw4w9WgXcQ".to_string()),
        }
    );

    // Youtu.be shortener
    let youtu_route = policy.classify("https://youtu.be/dQw4w9WgXcQ", None);
    assert_eq!(
        youtu_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "/dqw4w9wgxcq".to_string(),
            is_sabr: false,
            video_id: Some("dQw4w9WgXcQ".to_string()),
        }
    );

    // Google Video playback delivery
    let gv_route = policy.classify(
        "https://rr1---sn-4g5ednss.googlevideo.com/videoplayback?expire=1700000000&id=12345",
        Some("video/mp4"),
    );
    assert_eq!(
        gv_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "videoplayback".to_string(),
            is_sabr: false,
            video_id: None,
        }
    );

    // Google Video SABR delivery
    let sabr_route = policy.classify(
        "https://rr1---sn-4g5ednss.googlevideo.com/videoplayback?sabr=1&id=12345",
        Some("video/mp4"),
    );
    assert_eq!(
        sabr_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "videoplayback".to_string(),
            is_sabr: true,
            video_id: None,
        }
    );

    // Player endpoints
    let player_route = policy.classify("https://www.youtube.com/youtubei/v1/player", None);
    assert_eq!(
        player_route,
        MediaRoute::YouTubeDelivery {
            endpoint: "player".to_string(),
            is_sabr: false,
            video_id: None,
        }
    );
}

#[test]
fn fixture_matrix_classifies_adaptive_streams_hls_and_dash() {
    let policy = MediaCapturePolicy::new();

    // HLS manifest
    let hls_route = policy.classify("https://live.example.com/hls/master.m3u8", None);
    assert_eq!(hls_route, MediaRoute::HlsStream { is_master: true });

    let hls_mime_route = policy.classify(
        "https://live.example.com/stream",
        Some("application/vnd.apple.mpegurl"),
    );
    assert_eq!(hls_mime_route, MediaRoute::HlsStream { is_master: false });

    // DASH manifest
    let dash_route = policy.classify("https://live.example.com/dash/manifest.mpd", None);
    assert_eq!(dash_route, MediaRoute::DashManifest);

    let dash_mime_route = policy.classify(
        "https://live.example.com/manifest",
        Some("application/dash+xml"),
    );
    assert_eq!(dash_mime_route, MediaRoute::DashManifest);
}

#[test]
fn hls_plan_resolver_handles_byte_range_segments() {
    let manifest = "\
#EXTM3U
#EXT-X-VERSION:4
#EXT-X-TARGETDURATION:10
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:10.0,
#EXT-X-BYTERANGE:500000@0
main.mp4
#EXTINF:10.0,
#EXT-X-BYTERANGE:500000
main.mp4
#EXTINF:10.0,
#EXT-X-BYTERANGE:300000@1500000
main.mp4
#EXT-X-ENDLIST";

    let parsed = parse_hls(manifest, "https://cdn.example.com/streams/vod.m3u8").unwrap();
    let ParsedHls::Media(media) = parsed else {
        panic!("expected media playlist");
    };

    assert_eq!(media.segments.len(), 3);
    assert_eq!(
        media.segments[0].byte_range,
        Some(HlsByteRange {
            offset: 0,
            length: 500_000
        })
    );
    // When offset is omitted, it continues from previous end (0 + 500,000 = 500,000)
    assert_eq!(
        media.segments[1].byte_range,
        Some(HlsByteRange {
            offset: 500_000,
            length: 500_000
        })
    );
    // Explicit offset
    assert_eq!(
        media.segments[2].byte_range,
        Some(HlsByteRange {
            offset: 1_500_000,
            length: 300_000
        })
    );
}

#[test]
fn hls_plan_resolver_handles_init_map_byte_range() {
    let manifest = "\
#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"
#EXTINF:4.0,
chunk1.m4s
#EXTINF:4.0,
chunk2.m4s
#EXT-X-ENDLIST";

    let parsed = parse_hls(manifest, "https://cdn.example.com/hls/index.m3u8").unwrap();
    let ParsedHls::Media(media) = parsed else {
        panic!("expected media playlist");
    };

    assert_eq!(media.extension, ".mp4");
    assert_eq!(
        media.init_byte_range,
        Some(HlsByteRange {
            offset: 0,
            length: 720
        })
    );
    assert_eq!(media.segments.len(), 3);
    // Init segment is first
    assert_eq!(
        media.segments[0].byte_range,
        Some(HlsByteRange {
            offset: 0,
            length: 720
        })
    );
    assert_eq!(media.segments[0].url, "https://cdn.example.com/hls/init.mp4");
}

#[test]
fn hls_plan_resolver_explicitly_rejects_unsupported_encrypted_and_live_streams() {
    // Encrypted stream
    let encrypted = "\
#EXTM3U
#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://drm.example.com/key\"
#EXTINF:5.0,
seg1.ts
#EXT-X-ENDLIST";
    let err = parse_hls(encrypted, "https://cdn.example.com/enc.m3u8").unwrap_err();
    assert!(err.contains("Encrypted HLS is not supported"));

    // Live stream (no ENDLIST)
    let live = "\
#EXTM3U
#EXT-X-VERSION:3
#EXTINF:5.0,
live_chunk_1001.ts
#EXTINF:5.0,
live_chunk_1002.ts";
    let err = parse_hls(live, "https://cdn.example.com/live.m3u8").unwrap_err();
    assert!(err.contains("Live HLS streams are not supported yet"));

    // Changing initialization maps
    let changing_maps = "\
#EXTM3U
#EXT-X-MAP:URI=\"init1.mp4\"
#EXTINF:4.0,
a.m4s
#EXT-X-MAP:URI=\"init2.mp4\"
#EXTINF:4.0,
b.m4s
#EXT-X-ENDLIST";
    let err = parse_hls(changing_maps, "https://cdn.example.com/changing.m3u8").unwrap_err();
    assert!(err.contains("changing initialization maps are not supported yet"));
}

#[test]
fn dash_plan_resolver_parses_static_vod_and_rejects_drm_and_live() {
    // 1. Valid static VOD DASH manifest
    let static_vod_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT30S">
  <Period>
    <AdaptationSet contentType="video" mimeType="video/mp4">
      <Representation id="v1080" bandwidth="4500000" width="1920" height="1080" codecs="avc1.640028">
        <BaseURL>video_1080p.mp4</BaseURL>
      </Representation>
      <Representation id="v720" bandwidth="2500000" width="1280" height="720" codecs="avc1.4d401f">
        <BaseURL>video_720p.mp4</BaseURL>
      </Representation>
    </AdaptationSet>
    <AdaptationSet contentType="audio" mimeType="audio/mp4">
      <Representation id="a1" bandwidth="128000" codecs="mp4a.40.2">
        <BaseURL>audio_128k.m4a</BaseURL>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    let plan = parse_dash(static_vod_xml, "https://cdn.example.com/dash/manifest.mpd").unwrap();
    assert!(!plan.is_dynamic);
    assert!(!plan.has_drm);

    let video = plan.video_track.expect("should find video track");
    assert_eq!(video.id, "v1080");
    assert_eq!(video.width, Some(1920));
    assert_eq!(video.height, Some(1080));
    assert_eq!(video.bandwidth, 4_500_000);
    assert_eq!(video.segment_urls, vec!["https://cdn.example.com/dash/video_1080p.mp4"]);

    let audio = plan.audio_track.expect("should find audio track");
    assert_eq!(audio.id, "a1");
    assert_eq!(audio.bandwidth, 128_000);
    assert_eq!(audio.segment_urls, vec!["https://cdn.example.com/dash/audio_128k.m4a"]);

    // 2. Encrypted DRM rejection
    let drm_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static">
  <Period>
    <AdaptationSet contentType="video">
      <ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"/>
      <Representation id="v1" bandwidth="2000000">
        <BaseURL>video.mp4</BaseURL>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;
    let drm_err = parse_dash(drm_xml, "https://cdn.example.com/dash/drm.mpd").unwrap_err();
    match drm_err {
        DashError::EncryptedDrm(msg) => {
            assert!(msg.contains("Encrypted DRM DASH streams are not supported"));
        }
        other => panic!("Expected EncryptedDrm, got {other:?}"),
    }

    // 3. Live dynamic stream rejection
    let live_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="dynamic" minimumUpdatePeriod="PT5S">
  <Period>
    <AdaptationSet contentType="video">
      <Representation id="v1" bandwidth="2000000">
        <BaseURL>live.mp4</BaseURL>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;
    let live_err = parse_dash(live_xml, "https://cdn.example.com/dash/live.mpd").unwrap_err();
    match live_err {
        DashError::LiveStream(msg) => {
            assert!(msg.contains("Live DASH streams are not supported"));
        }
        other => panic!("Expected LiveStream, got {other:?}"),
    }
}

#[test]
fn security_redacts_signed_url_tokens_and_credentials_in_diagnostics() {
    // 1. Signed URL query parameters
    let raw_signed_url = "https://rr1---sn-4g5ednss.googlevideo.com/videoplayback?expire=1700000000&id=12345&ip=1.2.3.4&signature=secret_sig_value123&token=abcde_auth_token_999&sparams=expire,id,ip";
    let redacted = redact_media_url(raw_signed_url);
    assert!(!redacted.contains("secret_sig_value123"));
    assert!(!redacted.contains("abcde_auth_token_999"));
    assert!(!redacted.contains("1700000000"));
    assert!(redacted.contains("signature=******"));
    assert!(redacted.contains("token=******"));
    assert!(redacted.contains("expire=******"));
    assert!(redacted.contains("id=12345")); // Non-sensitive ID is preserved

    // 2. Credentials in URL authority
    let cred_url = "https://alice:supersecret123@media.private-server.net/stream.m3u8?token=topsecret";
    let redacted_cred = redact_media_url(cred_url);
    assert!(!redacted_cred.contains("supersecret123"));
    assert!(!redacted_cred.contains("topsecret"));
    assert!(redacted_cred.contains("alice:******@media.private-server.net"));

    // 3. MediaCandidate presentation URL
    let candidate = MediaCandidate {
        page_url: Some("https://example.com/watch".to_string()),
        media_url: raw_signed_url.to_string(),
        mime_type: Some("video/mp4".to_string()),
        codecs: Some("avc1.64001f".to_string()),
        width: Some(1920),
        height: Some(1080),
        bitrate: Some(4_500_000),
        role: MediaRole::VideoWithAudio,
        unsupported: false,
        unsupported_reason: None,
    };

    let display_url = candidate.sanitized_display_url();
    assert!(!display_url.contains("secret_sig_value123"));
    assert!(display_url.contains("signature=******"));
    assert_eq!(candidate.format_label(), "1080p (Video + audio)");
    assert!(candidate.is_selectable());
}
