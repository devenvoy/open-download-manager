use std::collections::HashMap;

use open_download_manager_lib::domain::media::{
    calculate_stream_progress, choices_from_info, parse_hls, scoped_headers,
    site_choices_from_info, youtube_error, youtube_url, ParsedHls, StreamSegment,
};

#[test]
fn hls_parser_resolves_variants_and_media_urls() {
    let master_text = "\
#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=100000,RESOLUTION=640x360
low/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=1920x1080
high/index.m3u8";
    let parsed = parse_hls(master_text, "https://example.com/master.m3u8").unwrap();
    let ParsedHls::Master(master) = parsed else {
        panic!("expected master playlist");
    };
    assert_eq!(master.variants.len(), 2);
    assert_eq!(master.variants[0].resolution.as_deref(), Some("1920x1080"));
    assert_eq!(master.variants[0].url, "https://example.com/high/index.m3u8");

    let media_text = "\
#EXTM3U
#EXT-X-MAP:URI=\"init.mp4\"
#EXTINF:4.0,
a.m4s
#EXTINF:5.0,
b.m4s
#EXT-X-ENDLIST";
    let parsed = parse_hls(media_text, "https://example.com/v/index.m3u8").unwrap();
    let ParsedHls::Media(media) = parsed else {
        panic!("expected media playlist");
    };
    assert_eq!(media.duration, 9.0);
    assert_eq!(media.extension, ".mp4");
    assert_eq!(
        media.urls,
        vec![
            "https://example.com/v/init.mp4",
            "https://example.com/v/a.m4s",
            "https://example.com/v/b.m4s"
        ]
    );
}

#[test]
fn hls_rejects_unsupported_formats_and_pairs_audio_group() {
    assert!(parse_hls(
        "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\n#EXTINF:3,\na.ts\n#EXT-X-ENDLIST",
        "https://example.com/a"
    )
    .is_err());
    assert!(parse_hls(
        "#EXTM3U\n#EXTINF:3,\na.ts",
        "https://example.com/a"
    )
    .is_err());
    assert!(parse_hls(
        "#EXTM3U\n#EXT-X-BYTERANGE:invalid@offset\na.ts\n#EXT-X-ENDLIST",
        "https://example.com/a"
    )
    .is_err());

    let paired_text = "\
#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"sound\",URI=\"audio.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=3,AUDIO=\"sound\"
v.m3u8";
    let parsed = parse_hls(paired_text, "https://example.com/a").unwrap();
    let ParsedHls::Master(master) = parsed else {
        panic!("expected master playlist");
    };
    assert_eq!(
        master.variants[0].audio_url.as_deref(),
        Some("https://example.com/audio.m3u8")
    );

    let mut headers = HashMap::new();
    headers.insert("cookie".to_owned(), "private".to_owned());
    headers.insert("authorization".to_owned(), "Bearer private".to_owned());
    headers.insert("referer".to_owned(), "https://page.test".to_owned());
    let scoped = scoped_headers(&headers, "https://a.test/a", "https://b.test/b");
    assert!(!scoped.contains_key("cookie"));
    assert!(!scoped.contains_key("authorization"));
    assert_eq!(scoped.get("referer").map(String::as_str), Some("https://page.test"));
}

#[test]
fn stream_progress_estimates_accurately() {
    let single = vec![StreamSegment {
        done: true,
        downloaded: 10,
    }];
    assert_eq!(calculate_stream_progress(&single).estimated_bytes, None);

    let multiple = vec![
        StreamSegment {
            done: true,
            downloaded: 10,
        },
        StreamSegment {
            done: true,
            downloaded: 30,
        },
        StreamSegment {
            done: false,
            downloaded: 2,
        },
    ];
    let progress = calculate_stream_progress(&multiple);
    assert_eq!(progress.completed, 2);
    assert_eq!(progress.total, 3);
    assert_eq!(progress.estimated_bytes, Some(60));
}

#[test]
fn youtube_canonical_urls_and_error_handling() {
    assert_eq!(
        youtube_url("https://youtu.be/wLdZWyLOJCI"),
        Some("https://www.youtube.com/watch?v=wLdZWyLOJCI".to_owned())
    );
    assert_eq!(
        youtube_url("https://youtube.com.evil.test/watch?v=wLdZWyLOJCI"),
        None
    );
    assert_eq!(youtube_url("file:///secret"), None);
    assert_eq!(
        youtube_url("https://user:secret@youtube.com/watch?v=wLdZWyLOJCI"),
        None
    );

    let (msg, code, err_type) = youtube_error(
        "Deprecated Feature: Python\nHTTP Error 429: Too Many Requests\nSign in to confirm you are not a bot",
    );
    assert_eq!(code, 429);
    assert_eq!(err_type, "YOUTUBE_RATE_LIMIT");
    assert!(!msg.contains("Python"));

    let (_, code, err_type) = youtube_error("Sign in to confirm you are not a bot");
    assert_eq!(code, 422);
    assert_eq!(err_type, "YOUTUBE_VERIFICATION");

    let (msg, code, _) =
        youtube_error("WARNING: old runtime\nERROR: failed https://secret.test/?token=secret");
    assert_eq!(code, 400);
    assert_eq!(msg, "ERROR: failed [URL]");
}

#[test]
fn youtube_formats_pair_audio_and_provide_mp3() {
    let info = serde_json::json!({
        "title": "Test Video",
        "duration": 120.0,
        "formats": [
            {
                "format_id": "140",
                "ext": "m4a",
                "vcodec": "none",
                "acodec": "mp4a",
                "abr": 128.0,
                "url": "https://cdn/audio",
                "protocol": "https",
                "filesize": 20
            },
            {
                "format_id": "137",
                "ext": "mp4",
                "vcodec": "avc1",
                "acodec": "none",
                "height": 1080,
                "url": "https://cdn/video",
                "protocol": "https",
                "filesize": 100
            },
            {
                "format_id": "270",
                "ext": "mp4",
                "vcodec": "avc1",
                "acodec": "none",
                "height": 1080,
                "url": "https://cdn/hls",
                "protocol": "m3u8"
            },
            {
                "format_id": "secret",
                "ext": "mp4",
                "vcodec": "avc1",
                "acodec": "none",
                "height": 2160,
                "url": "https://cdn/drm",
                "protocol": "https",
                "has_drm": true
            },
            {
                "format_id": "sb0",
                "ext": "mhtml",
                "vcodec": "images",
                "url": "https://cdn/images",
                "protocol": "mhtml"
            }
        ]
    });

    let choices = choices_from_info(&info, "https://www.youtube.com/watch?v=wLdZWyLOJCI").unwrap();
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].youtube_format, "137+140");
    assert_eq!(choices[0].estimated_size, 120);
    assert_eq!(choices[0].quality, "1080p");
    assert_eq!(choices[1].format, "MP3");
    assert_eq!(choices[1].youtube_format, "audio:140");
}

#[test]
fn generic_site_formats_pair_supported_containers_and_provide_audio() {
    let info = serde_json::json!({
        "title": "Public video",
        "duration": 30.0,
        "formats": [
            {
                "format_id": "audio-m4a",
                "ext": "m4a",
                "vcodec": "none",
                "acodec": "mp4a.40.2",
                "abr": 128.0,
                "url": "https://cdn.example/audio.m4a",
                "protocol": "https",
                "filesize": 20
            },
            {
                "format_id": "video-mp4",
                "ext": "mp4",
                "vcodec": "avc1",
                "acodec": "none",
                "height": 720,
                "url": "https://cdn.example/video.mp4",
                "protocol": "https",
                "filesize": 100
            },
            {
                "format_id": "video-webm",
                "ext": "webm",
                "vcodec": "vp9",
                "acodec": "none",
                "height": 1080,
                "url": "https://cdn.example/video.webm",
                "protocol": "https",
                "filesize": 200
            },
            {
                "format_id": "audio-webm",
                "ext": "webm",
                "vcodec": "none",
                "acodec": "opus",
                "abr": 160.0,
                "url": "https://cdn.example/audio.webm",
                "protocol": "https",
                "filesize": 30
            }
        ]
    });

    let choices = site_choices_from_info(&info, "https://video.example/watch/1").unwrap();
    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0].format, "WEBM");
    assert_eq!(choices[0].youtube_format, "video-webm+audio-webm");
    assert_eq!(choices[0].output_format, "webm");
    assert_eq!(choices[1].format, "MP4");
    assert_eq!(choices[1].youtube_format, "video-mp4+audio-m4a");
    assert_eq!(choices[1].estimated_size, 120);
    assert_eq!(choices[2].format, "MP3");
    assert_eq!(choices[2].youtube_format, "audio:audio-webm");
    assert_eq!(choices[2].provider, "extractor");
    assert!(choices.iter().all(|choice| choice.url == "https://video.example/watch/1"));
}
