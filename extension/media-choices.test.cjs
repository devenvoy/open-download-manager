// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Unit tests verifying media format deduplication, paired audio retention, and extensionless stream handling.

const test=require('node:test'),assert=require('node:assert/strict');
const {mediaChoices}=require('./media-choices.js');
test('choices keep only the matching tweet and deduplicate paired audio',()=>{
  const base='https://video.twimg.com/amplify_video/123/';
  const audioUrl=base+'audio.m3u8';
  const items=[{url:base+'720.m3u8',format:'HLS',quality:'720p',audioUrl,masterUrl:base+'master.m3u8'},
    {url:base+'360.m3u8',format:'HLS',quality:'360p',audioUrl,masterUrl:base+'master.m3u8'},
    {url:audioUrl,format:'HLS'}, {url:base+'init.mp4'},
    {url:'https://video.twimg.com/amplify_video/456/other.mp4'}];
  const choices=mediaChoices(items,{poster:'https://pbs.twimg.com/amplify_video_thumb/123/img/a.jpg'});
  assert.equal(choices.length,3);
  assert.equal(choices.filter(i=>i.format==='MP3').length,1);
  assert.equal(choices.filter(i=>i.audioState==='Video + audio').length,2);
});
test('extensionless webm media keeps its detected format and quality',()=>{
  const choices=mediaChoices([{
    url:'https://cdn.example/stream?signature=hidden',
    mime:'video/webm; codecs="vp09.00.10.08"',
    quality:'720p',
    media:true
  }]);
  assert.equal(choices.length,1);
  assert.equal(choices[0].format,'WEBM');
  assert.equal(choices[0].outputFormat,'webm');
  assert.equal(choices[0].quality,'720p');
});
